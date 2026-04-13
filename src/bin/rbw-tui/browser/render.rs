use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
};

use crate::app::{Context, Mode, hint_line};
use crate::domain::EntryExt;

use super::{State, bindings::bindings};

fn entry_summary_lines(state: &State) -> Text<'static> {
    let entry = state.selected_entry();
    if let Some(entry) = entry {
        let password_line = if state.reveal_password {
                entry.password().to_string()
            } else {
                "<hidden>".to_string()
            };
        let username = entry.username();
        let folder = entry.folder_str();
        let notes = entry.notes_str();
        let uris = entry.uri_strings();
        Text::from(vec![
            Line::from(entry.name.clone()).style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from(""),
            Line::from(format!(
                "Username: {}",
                if username.is_empty() { "-" } else { username }
            )),
            Line::from(format!(
                "Folder: {}",
                if folder.is_empty() { "-" } else { folder }
            )),
            Line::from(format!("Password: {password_line}")),
            Line::from(""),
            Line::from("URIs:"),
            Line::from(if uris.is_empty() {
                "-".to_string()
            } else {
                uris.join("\n")
            }),
            Line::from(""),
            Line::from("Notes:"),
            Line::from(if notes.is_empty() {
                "-".to_string()
            } else {
                notes.to_string()
            }),
        ])
    } else {
        Text::from("No matching entries")
    }
}

/// Computes the number of visible table rows for the entry list viewport.
pub(crate) fn viewport_rows(area: Rect) -> usize {
    area.height.saturating_sub(3) as usize
}

/// Renders the main split view with entries, details, and footer hints.
pub(crate) fn render(
    frame: &mut ratatui::Frame<'_>,
    state: &State,
    context: &Context,
    mode: &Mode,
) -> usize {
    let palette = &context.palette;
    let show_search = matches!(mode, Mode::Search) || !state.search.is_empty();
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if show_search { 3 } else { 0 }),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    if show_search {
        let search_title = match mode {
            Mode::Search => "Search (editing)",
            _ => "Search",
        };
        let search = Paragraph::new(state.search.as_str())
            .block(
                Block::default()
                    .title(search_title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(palette.border)),
            )
            .style(Style::default().fg(palette.text));
        frame.render_widget(search, root[0]);
    }

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(root[1]);
    let viewport_rows = viewport_rows(body[0]).max(1);

    let (mut name_max, mut user_max, mut folder_max) = (0usize, 0usize, 0usize);
    for &i in &state.visible {
        let entry = &state.entries[i];
        name_max = name_max.max(entry.name.chars().count());
        user_max = user_max.max(entry.username().chars().count());
        folder_max = folder_max.max(entry.folder_str().chars().count());
    }
    let name_width = name_max.clamp(4, 32) as u16;
    let user_width = user_max.clamp(8, 36) as u16;
    let folder_width = folder_max.clamp(6, 20) as u16;

    let rows: Vec<Row> = state
        .visible
        .iter()
        .enumerate()
        .skip(state.scroll_offset)
        .take(viewport_rows)
        .map(|(idx, &entry_idx)| {
            let entry = &state.entries[entry_idx];
            let style = if idx == state.selected {
                Style::default()
                    .fg(palette.selected_fg)
                    .bg(palette.selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette.text)
            };
            let username = entry.username();
            let folder = entry.folder_str();
            Row::new(vec![
                entry.name.clone(),
                if username.is_empty() {
                    "-".to_string()
                } else {
                    username.to_string()
                },
                if folder.is_empty() {
                    "-".to_string()
                } else {
                    folder.to_string()
                },
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(name_width),
            Constraint::Length(user_width),
            Constraint::Length(folder_width),
        ],
    )
    .header(
        Row::new(vec!["Name", "Username", "Folder"]).style(
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .column_spacing(2)
    .block(
        Block::default()
            .title("Entries")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border)),
    );
    frame.render_widget(table, body[0]);

    let details = Paragraph::new(entry_summary_lines(state))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title("Details")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border)),
        );
    frame.render_widget(details, body[1]);

    let footer = Paragraph::new(Text::from(vec![hint_line(
        bindings(context, state),
        palette,
    )]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border)),
    );
    frame.render_widget(footer, root[2]);

    viewport_rows
}

/// Returns the cursor position for browser search mode.
pub(crate) fn search_cursor(area: Rect, state: &State) -> (u16, u16) {
    (area.x + 1 + state.search.chars().count() as u16, area.y + 1)
}
