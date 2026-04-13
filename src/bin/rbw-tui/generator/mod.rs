mod bindings;
mod render;
mod state;

use crossterm::event::{KeyCode, KeyEvent};

pub(crate) use self::{
    bindings::Action,
    render::{cursor_position, render_modal},
    state::{Settings, State},
};

use crate::app::{self, Effect, Mode, Transition, lookup_action_with_fallback};

pub(crate) fn map_generator_key(state: &State, key: KeyEvent) -> Vec<app::Action> {
    lookup_action_with_fallback(bindings::bindings(state), key, |key| {
        if state.selected_index == 1 {
            match key.code {
                KeyCode::Char(ch) if ch.is_ascii_digit() => Some(Action::InsertDigit(ch)),
                _ => None,
            }
        } else {
            None
        }
    })
    .map(|action| vec![app::Action::Generator(action)])
    .unwrap_or_default()
}

pub(crate) fn reduce_generator(
    state: &mut State,
    generator_settings: &mut Settings,
    action: Action,
) -> Transition {
    match action {
        Action::Cancel => {
            if state.editing_length {
                state.commit_length();
                return Transition::none();
            }
            if let Some(form_state) = state.return_to_form.clone() {
                Transition::mode(Mode::Form(form_state))
            } else {
                Transition::mode(Mode::Normal)
            }
        }
        Action::Down => {
            state.move_down();
            Transition::none()
        }
        Action::Up => {
            state.move_up();
            Transition::none()
        }
        Action::DecLength => {
            if state.selected_index == 1 {
                state.adjust_length(-1);
            } else if !state.editing_length {
                state.activate();
            }
            Transition::none()
        }
        Action::IncLength => {
            if state.selected_index == 1 {
                state.adjust_length(1);
            } else if !state.editing_length {
                state.activate();
            }
            Transition::none()
        }
        Action::Confirm => {
            let settings = state.settings.clone();
            *generator_settings = settings.clone();
            Transition::effect(Effect::GeneratePassword { settings })
        }
        Action::Toggle => {
            state.activate();
            Transition::none()
        }
        Action::Backspace => {
            state.length_buffer.backspace();
            if state.length_buffer.is_empty() {
                state.length_buffer.set("0");
            }
            Transition::none()
        }
        Action::InsertDigit(ch) => {
            if !state.editing_length {
                state.editing_length = true;
                state.length_buffer.set(&ch.to_string());
            } else if state.length_buffer.as_str() == "0" {
                state.length_buffer.set(&ch.to_string());
            } else {
                state.length_buffer.insert(ch);
            }
            Transition::none()
        }
        Action::AcceptLength => {
            state.commit_length();
            Transition::none()
        }
    }
}
