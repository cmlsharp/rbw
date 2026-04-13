use crate::{browser, form, domain::EntryExt as _, generator};

use super::{Effect, EffectOutcome, EffectResult, Mode, State};

/// High-level event routed by screen before it reaches a reducer.
pub(crate) enum Action {
    System(SystemAction),
    Browser(browser::Action),
    Form(form::Action),
    Generator(generator::Action),
    Delete(browser::delete::Action),
}

/// App-level events that are not owned by one screen.
pub(crate) enum SystemAction {
    Quit,
    Effect(EffectResult),
}

/// One reducer step result.
pub(crate) struct Transition {
    mode: Option<Mode>,
    notification: Option<NotificationSpec>,
    pub effect: Option<Effect>,
    pub output: Option<String>,
}

pub(crate) struct NotificationSpec {
    message: String,
    is_error: bool,
}

impl Transition {
    pub(crate) fn none() -> Self {
        Self {
            mode: None,
            notification: None,
            effect: None,
            output: None,
        }
    }

    pub(crate) fn effect(effect: Effect) -> Self {
        Self {
            mode: None,
            notification: None,
            effect: Some(effect),
            output: None,
        }
    }

    pub(crate) fn output(output: String) -> Self {
        Self {
            mode: None,
            notification: None,
            effect: None,
            output: Some(output),
        }
    }

    pub(crate) fn mode(mode: Mode) -> Self {
        Self::none().with_mode(mode)
    }

    pub(crate) fn notify(message: impl Into<String>) -> Self {
        Self::none().with_notification(message, false)
    }

    pub(crate) fn notify_error(message: impl Into<String>) -> Self {
        Self::none().with_notification(message, true)
    }

    pub(crate) fn with_mode(mut self, mode: Mode) -> Self {
        self.mode = Some(mode);
        self
    }

    pub(crate) fn with_notification(mut self, message: impl Into<String>, is_error: bool) -> Self {
        self.notification = Some(NotificationSpec {
            message: message.into(),
            is_error,
        });
        self
    }
}

/// Applies one action to the current app state and returns requested effects.
pub(super) fn reduce(state: &mut State, action: Action) -> Transition {
    let mut transition = match action {
        Action::System(action) => reduce_system(state, action),
        Action::Browser(action) => browser::reduce_browser(
            &mut state.browser,
            &state.context,
            action,
        ),
        Action::Form(action) => {
            debug_assert!(
                matches!(state.mode, Mode::Form(_)),
                "Form action dispatched outside Form mode"
            );
            let Mode::Form(form_state) = &mut state.mode else {
                return Transition::none();
            };
            form::reduce_form(form_state, &state.generator_settings, action)
        }
        Action::Generator(action) => {
            debug_assert!(
                matches!(state.mode, Mode::Generator(_)),
                "Generator action dispatched outside Generator mode"
            );
            let Mode::Generator(generator) = &mut state.mode else {
                return Transition::none();
            };
            generator::reduce_generator(generator, &mut state.generator_settings, action)
        }
        Action::Delete(action) => {
            debug_assert!(
                matches!(state.mode, Mode::DeleteConfirm(_)),
                "Delete action dispatched outside DeleteConfirm mode"
            );
            browser::delete::reduce(
                match &state.mode {
                    Mode::DeleteConfirm(entry) => Some(entry),
                    _ => None,
                },
                action,
            )
        }
    };

    if let Some(mode) = transition.mode.take() {
        state.mode = mode;
    }
    if let Some(notification) = transition.notification.take() {
        if notification.is_error {
            state.notify_error(notification.message);
        } else {
            state.notify(notification.message);
        }
    }
    transition
}

pub(super) fn canceled_json() -> String {
    r#"{"status":"cancel"}"#.to_string()
}

fn reduce_system(state: &mut State, action: SystemAction) -> Transition {
    match action {
        SystemAction::Quit => Transition::output(canceled_json()),
        SystemAction::Effect(Err(message)) => Transition::notify_error(message),
        SystemAction::Effect(Ok(outcome)) => apply_effect_outcome(state, outcome),
    }
}

fn apply_effect_outcome(state: &mut State, outcome: EffectOutcome) -> Transition {
    match outcome {
        EffectOutcome::Synced(entries) => {
            state.browser.replace_entries(entries, &state.context);
            Transition::notify("Synced vault")
        }
        EffectOutcome::Copied(label) => Transition::notify(format!("Copied {label}")),
        EffectOutcome::SelectionReady(json) => Transition::output(json),
        EffectOutcome::GeneratedPassword { password } => {
            let return_to_form = match &mut state.mode {
                Mode::Generator(generator) => generator.return_to_form.take(),
                _ => None,
            };
            match return_to_form {
                Some(mut form_state) => {
                    form_state.apply_generated_password(password);
                    Transition::mode(Mode::Form(form_state))
                }
                None => Transition::mode(Mode::Normal).with_notification("Generated password", false),
            }
        }
        EffectOutcome::Created { draft, entries } => {
            state.browser.replace_entries(entries, &state.context);
            if let Some(index) = state.browser.find_visible(|entry| {
                entry.name == draft.name && entry.username() == draft.username
            }) {
                state.browser.selected = index;
            }
            Transition::mode(Mode::Normal).with_notification(format!("Created {}", draft.name), false)
        }
        EffectOutcome::Edited { name, entries } => {
            state.browser.replace_entries(entries, &state.context);
            Transition::mode(Mode::Normal).with_notification(format!("Edited {name}"), false)
        }
        EffectOutcome::Deleted { entry_name, entries } => {
            state.browser.replace_entries(entries, &state.context);
            Transition::mode(Mode::Normal).with_notification(format!("Deleted {entry_name}"), false)
        }
    }
}
