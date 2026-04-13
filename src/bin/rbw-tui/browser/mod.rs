mod bindings;
pub(crate) mod delete;
mod render;
mod state;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(crate) use self::{
    bindings::Action,
    render::{render, search_cursor},
    state::{PendingPrefix, State, site_targets},
};

use crate::{
    app::{self, Context, Effect, Mode, SystemAction, Transition},
    form,
    domain::draft_from_seed,
};

use self::bindings::bindings;

pub(crate) fn map_browser_key(state: &app::State, key: KeyEvent) -> Vec<app::Action> {
    if let Some(binding) = bindings(&state.context, &state.browser).find(|b| b.matches(key)) {
        let action = binding.action();
        return vec![if matches!(action, Action::Cancel) {
            app::Action::System(SystemAction::Quit)
        } else {
            app::Action::Browser(*action)
        }];
    }

    if key.kind != crossterm::event::KeyEventKind::Press {
        return Vec::new();
    }

    if state.browser.pending.is_some() {
        vec![app::Action::Browser(Action::ClearPrefix)]
    } else {
        Vec::new()
    }
}

pub(crate) fn map_search_key(key: KeyEvent) -> Vec<app::Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Esc | KeyCode::Enter, _) => vec![app::Action::Browser(Action::FinishSearch)],
        (KeyCode::Backspace, KeyModifiers::CONTROL) => {
            vec![app::Action::Browser(Action::SearchClear)]
        }
        (KeyCode::Backspace, _) => vec![app::Action::Browser(Action::SearchBackspace)],
        (KeyCode::Char(ch), modifiers)
            if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT =>
        {
            vec![app::Action::Browser(Action::SearchInput(ch))]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn reduce_browser(
    state: &mut State,
    context: &Context,
    action: Action,
) -> Transition {
    match action {
        Action::Cancel => Transition::none(),
        Action::ClearPrefix => {
            state.clear_prefixes();
            Transition::none()
        }
        Action::Search => Transition::mode(Mode::Search),
        Action::SearchClearAndFocus => {
            state.search.clear();
            state.refresh_visible(context);
            Transition::mode(Mode::Search)
        }
        Action::SearchClear => {
            state.search.clear();
            state.refresh_visible(context);
            Transition::none()
        }
        Action::SearchBackspace => {
            state.search.pop();
            state.refresh_visible(context);
            Transition::none()
        }
        Action::SearchInput(ch) => {
            state.search.push(ch);
            state.refresh_visible(context);
            Transition::none()
        }
        Action::FinishSearch => Transition::mode(Mode::Normal),
        Action::MoveDown => {
            state.reset_selection_context();
            state.move_selection_down();
            Transition::none()
        }
        Action::MoveUp => {
            state.reset_selection_context();
            state.move_selection_up();
            Transition::none()
        }
        Action::PageDown => {
            state.reset_selection_context();
            state.page_down();
            Transition::none()
        }
        Action::PageUp => {
            state.reset_selection_context();
            state.page_up();
            Transition::none()
        }
        Action::StartTopPrefix => {
            state.pending = Some(PendingPrefix::Top);
            Transition::none()
        }
        Action::Top => {
            state.clear_prefixes();
            state.select_top();
            state.clear_revealed_password();
            Transition::none()
        }
        Action::Bottom => {
            state.reset_selection_context();
            state.select_bottom();
            Transition::none()
        }
        Action::RevealPassword => {
            state.clear_prefixes();
            state.reveal_password = !state.reveal_password;
            Transition::none()
        }
        Action::ToggleFilter => {
            state.clear_prefixes();
            state.scope = state.scope.toggle(!context.url.is_empty());
            state.refresh_visible(context);
            Transition::none()
        }
        Action::Sync => {
            state.clear_prefixes();
            Transition::effect(Effect::SyncVault)
        }
        Action::Copy(target) => {
            state.clear_prefixes();
            let Some(entry) = state.selected_entry().cloned() else {
                return Transition::none();
            };
            Transition::effect(Effect::copy_target(&entry, target))
        }
        Action::YankPrefix => {
            state.pending = Some(PendingPrefix::Yank);
            Transition::none()
        }
        Action::Select => {
            state.clear_prefixes();
            let Some(entry) = state.selected_entry().cloned() else {
                return Transition::none();
            };
            Transition::effect(Effect::ResolveSelection(entry))
        }
        Action::Create => {
            state.clear_prefixes();
            let draft = draft_from_seed(&context.url, &context.username);
            Transition::mode(Mode::Form(form::State::new_create(draft)))
        }
        Action::Edit => {
            state.clear_prefixes();
            state
                .selected_entry()
                .cloned()
                .map(|entry| Transition::mode(Mode::Form(form::State::new_edit(&entry))))
                .unwrap_or_else(Transition::none)
        }
        Action::Delete => {
            state.clear_prefixes();
            state
                .selected_entry()
                .cloned()
                .map(|entry| Transition::mode(Mode::DeleteConfirm(entry)))
                .unwrap_or_else(Transition::none)
        }
    }
}
