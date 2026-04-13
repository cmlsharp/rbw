use crate::{
    app::Context,
    domain::{Entry, EntryExt as _, Scope},
    text_input::TextInput,
};

/// Browser/listing state for the main screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingPrefix {
    Top,
    Yank,
}

/// Browser/listing state for the main screen.
#[derive(Debug)]
pub struct State {
    pub scope: Scope,
    pub search: TextInput,
    pub pending: Option<PendingPrefix>,
    /// The full entry list. Only accessible within the browser module;
    /// external code must use `replace_entries` to ensure `visible` stays in sync.
    pub(super) entries: Vec<Entry>,
    /// Indices into `entries` for the current filtered/sorted view.
    pub(super) visible: Vec<usize>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub viewport_rows: usize,
    pub reveal_password: bool,
}

impl State {
    /// Starts a fresh browser state from the initial scope and entry list.
    pub fn new(scope: Scope, entries: Vec<Entry>) -> Self {
        Self {
            scope,
            search: TextInput::new(),
            pending: None,
            entries,
            visible: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            viewport_rows: 1,
            reveal_password: false,
        }
    }

    /// Returns the currently selected visible entry.
    pub fn selected_entry(&self) -> Option<&Entry> {
        self.visible
            .get(self.selected)
            .map(|&i| &self.entries[i])
    }

    /// Replaces the full entry list and rebuilds the visible indices.
    /// This is the only way to mutate `entries` from outside this module.
    pub fn replace_entries(&mut self, entries: Vec<Entry>, context: &Context) {
        self.entries = entries;
        self.refresh_visible(context);
    }

    /// Finds the first visible-list position matching a predicate.
    pub fn find_visible(&self, predicate: impl Fn(&Entry) -> bool) -> Option<usize> {
        self.visible
            .iter()
            .position(|&i| predicate(&self.entries[i]))
    }

    /// Clears the currently revealed password, if any.
    pub fn clear_revealed_password(&mut self) {
        self.reveal_password = false;
    }

    /// Resets transient command prefixes such as `g` and `y`.
    pub fn clear_prefixes(&mut self) {
        self.pending = None;
    }

    /// Clears prefix state plus any password reveal that depends on selection context.
    pub fn reset_selection_context(&mut self) {
        self.clear_prefixes();
        self.clear_revealed_password();
    }

    /// Moves the selection one row down if possible.
    pub fn move_selection_down(&mut self) {
        if self.selected + 1 < self.visible.len() {
            self.selected += 1;
        }
    }

    /// Moves the selection one row up if possible.
    pub fn move_selection_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Moves the selection by one viewport page toward the end of the list.
    pub fn page_down(&mut self) {
        if !self.visible.is_empty() {
            let step = self.viewport_rows.max(1);
            self.selected = (self.selected + step).min(self.visible.len() - 1);
        }
    }

    /// Moves the selection by one viewport page toward the start of the list.
    pub fn page_up(&mut self) {
        let step = self.viewport_rows.max(1);
        self.selected = self.selected.saturating_sub(step);
    }

    /// Jumps to the first row in the visible list.
    pub fn select_top(&mut self) {
        self.selected = 0;
    }

    /// Jumps to the last row in the visible list.
    pub fn select_bottom(&mut self) {
        if !self.visible.is_empty() {
            self.selected = self.visible.len() - 1;
        }
    }

    /// Recomputes the visible entry list using the current filter and search state.
    pub fn refresh_visible(&mut self, context: &Context) {
        self.visible = filter_entry_indices(
            &self.entries,
            self.scope,
            &context.url,
            &context.username,
            self.search.as_str(),
        );
        if self.visible.is_empty() {
            self.selected = 0;
            self.scroll_offset = 0;
        } else if self.selected >= self.visible.len() {
            self.selected = self.visible.len() - 1;
        }
        self.ensure_selected_visible();
        self.clear_prefixes();
        self.clear_revealed_password();
    }

    /// Ensures the selected row remains visible in the current viewport.
    pub fn ensure_selected_visible(&mut self) {
        let rows = self.viewport_rows.max(1);
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + rows {
            self.scroll_offset = self.selected + 1 - rows;
        }
        let max_offset = self.visible.len().saturating_sub(rows);
        if self.scroll_offset > max_offset {
            self.scroll_offset = max_offset;
        }
    }
}

/// Filters and orders entries, returning indices into the original slice.
fn filter_entry_indices(
    entries: &[Entry],
    scope: Scope,
    url: &str,
    username: &str,
    search: &str,
) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..entries.len()).collect();

    if scope == Scope::Site && !url.is_empty() {
        let targets = site_targets(url);
        indices.retain(|&i| entry_matches(&entries[i], &targets));
    }
    if !username.is_empty() {
        let needle = username.to_lowercase();
        let mut exact = Vec::new();
        let mut partial = Vec::new();
        let mut rest = Vec::new();
        for i in indices {
            if entries[i].username() == username {
                exact.push(i);
            } else if entries[i].username().to_lowercase().contains(&needle) {
                partial.push(i);
            } else {
                rest.push(i);
            }
        }
        indices = exact;
        indices.extend(partial);
        indices.extend(rest);
    }
    if search.is_empty() {
        return indices;
    }

    let needle = search.to_lowercase();
    indices
        .into_iter()
        .filter(|&i| search_matches(&entries[i], &needle))
        .collect()
}

fn entry_matches(entry: &Entry, targets: &[String]) -> bool {
    if targets.is_empty() {
        return true;
    }

    let lower_name = entry.name.to_lowercase();
    let uris = entry.uri_strings();
    uris.iter().any(|uri| {
        let host = host_from_url(uri);
        let lower_uri = uri.to_lowercase();
        targets.iter().any(|target| {
            host == *target || host.ends_with(&format!(".{target}")) || lower_uri.contains(target)
        })
    }) || targets.iter().any(|target| lower_name.contains(target))
}

fn search_matches(entry: &Entry, needle: &str) -> bool {
    contains_case_insensitive(&entry.name, needle)
        || contains_case_insensitive(entry.username(), needle)
        || contains_case_insensitive(entry.folder_str(), needle)
        || contains_case_insensitive(entry.notes_str(), needle)
        || entry
            .uri_strings()
            .iter()
            .any(|uri| contains_case_insensitive(uri, needle))
}

fn contains_case_insensitive(value: &str, needle: &str) -> bool {
    value.to_lowercase().contains(needle)
}

/// Extracts the normalized host portion from a URL-like string.
pub(crate) fn host_from_url(url: &str) -> String {
    url.split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .split('@')
        .next_back()
        .unwrap_or(url)
        .split(':')
        .next()
        .unwrap_or(url)
        .trim_matches('.')
        .to_lowercase()
}

/// Builds site-matching targets from the current page URL.
pub(crate) fn site_targets(url: &str) -> Vec<String> {
    let host = host_from_url(url);
    if host.is_empty() {
        return Vec::new();
    }

    let mut out = vec![host.clone()];

    if let Some(domain) = psl::domain(host.as_bytes()) {
        if let Ok(domain) = std::str::from_utf8(domain.as_bytes()) {
            out.push(domain.to_string());
            if let Some(label) = domain.split('.').next() {
                out.push(label.to_string());
            }
        }
    } else {
        let parts: Vec<&str> = host.split('.').collect();
        if parts.len() >= 2 {
            out.push(format!(
                "{}.{}",
                parts[parts.len() - 2],
                parts[parts.len() - 1]
            ));
            out.push(parts[parts.len() - 2].to_string());
        } else if let Some(first) = parts.first() {
            out.push((*first).to_string());
        }
    }

    out.retain(|value| !value.is_empty());
    out.dedup();
    out
}
