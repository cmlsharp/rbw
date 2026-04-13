mod bindings;
mod render;
pub(crate) mod state;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(crate) use self::{
    bindings::Action,
    render::{cursor_position, render_modal},
    state::State,
};

use crate::{
    app::{self, Effect, Mode, Transition, lookup_action_with_fallback},
    generator,
};

use self::bindings::bindings;

pub(crate) fn map_form_key(key: KeyEvent) -> Vec<app::Action> {
    lookup_action_with_fallback(&bindings(), key, |key| match (key.code, key.modifiers) {
        (KeyCode::Backspace, KeyModifiers::CONTROL) => Some(Action::DeleteWordBack),
        (KeyCode::Backspace, _) => Some(Action::Backspace),
        (KeyCode::Delete, _) => Some(Action::Delete),
        (KeyCode::Left, _) => Some(Action::Left),
        (KeyCode::Right, _) => Some(Action::Right),
        (KeyCode::Home, _) => Some(Action::Home),
        (KeyCode::End, _) => Some(Action::End),
        (KeyCode::Char(ch), m) if m.is_empty() || m == KeyModifiers::SHIFT => {
            Some(Action::Insert(ch))
        }
        _ => None,
    })
    .map(|action| vec![app::Action::Form(action)])
    .unwrap_or_default()
}

pub(crate) fn reduce_form(
    state: &mut State,
    generator_settings: &generator::Settings,
    action: Action,
) -> Transition {
    match action {
        Action::Cancel => Transition::mode(Mode::Normal),
        Action::TogglePassword => {
            state.toggle_password_visibility();
            Transition::none()
        }
        Action::NextField => {
            state.next_field();
            Transition::none()
        }
        Action::PrevField => {
            state.previous_field();
            Transition::none()
        }
        Action::GeneratePassword => {
            let mut generator = generator::State::from_settings(generator_settings.clone());
            generator.return_to_form = Some(state.clone());
            Transition::mode(Mode::Generator(generator))
        }
        Action::AddUri => {
            state.add_uri();
            Transition::none()
        }
        Action::RemoveUri => {
            state.remove_current_uri();
            Transition::none()
        }
        Action::Save => {
            state.sync_draft();
            if state.draft.name.trim().is_empty() {
                Transition::notify_error("Name is required")
            } else {
                state.draft.clean_uris();
                match &state.purpose {
                    state::Purpose::Create => {
                        Transition::effect(Effect::CreateEntry(state.draft.clone()))
                    }
                    state::Purpose::Edit { entry_id } => {
                        Transition::effect(Effect::EditEntry {
                            entry_id: entry_id.clone(),
                            draft: state.draft.clone(),
                        })
                    }
                }
            }
        }
        Action::Backspace => {
            state.backspace();
            Transition::none()
        }
        Action::DeleteWordBack => {
            state.delete_word_back();
            Transition::none()
        }
        Action::Delete => {
            state.delete();
            Transition::none()
        }
        Action::Insert(ch) => {
            state.insert_char(ch);
            Transition::none()
        }
        Action::Left => {
            state.move_left();
            Transition::none()
        }
        Action::Right => {
            state.move_right();
            Transition::none()
        }
        Action::Home => {
            state.move_to_start();
            Transition::none()
        }
        Action::End => {
            state.move_to_end();
            Transition::none()
        }
    }
}
