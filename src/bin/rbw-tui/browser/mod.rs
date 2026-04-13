mod bindings;
pub mod delete;
mod render;
mod state;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub use self::{
    bindings::Action,
    render::{render, search_cursor},
    state::{PendingPrefix, State, site_targets},
};

use crate::{
    app::{self, Context, Effect, Mode, SystemAction, Transition, lookup_action_with_fallback},
    form,
    domain::draft_from_seed,
};

use self::bindings::bindings;

pub fn map_browser_key(state: &app::State, key: KeyEvent) -> Vec<app::Action> {
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

pub fn map_search_key(key: KeyEvent) -> Vec<app::Action> {
    lookup_action_with_fallback(bindings::search_bindings(), key, |key| match key.code {
        KeyCode::Backspace => Some(Action::SearchBackspace),
        KeyCode::Home => Some(Action::SearchHome),
        KeyCode::End => Some(Action::SearchEnd),
        KeyCode::Char(ch) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            Some(Action::SearchInput(ch))
        }
        _ => None,
    })
    .map(|action| vec![app::Action::Browser(action)])
    .unwrap_or_default()
}

pub fn reduce_browser(
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
            state.search.backspace();
            state.refresh_visible(context);
            Transition::none()
        }
        Action::SearchDeleteWordBack => {
            state.search.delete_word_back();
            state.refresh_visible(context);
            Transition::none()
        }
        Action::SearchInput(ch) => {
            state.search.insert(ch);
            state.refresh_visible(context);
            Transition::none()
        }
        Action::SearchLeft => {
            state.search.move_left();
            Transition::none()
        }
        Action::SearchRight => {
            state.search.move_right();
            Transition::none()
        }
        Action::SearchHome => {
            state.search.move_to_start();
            Transition::none()
        }
        Action::SearchEnd => {
            state.search.move_to_end();
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
                .cloned().map_or_else(Transition::none, |entry| Transition::mode(Mode::Form(form::State::new_edit(&entry))))
        }
        Action::Delete => {
            state.clear_prefixes();
            state
                .selected_entry()
                .cloned().map_or_else(Transition::none, |entry| Transition::mode(Mode::DeleteConfirm(entry)))
        }
    }
}
