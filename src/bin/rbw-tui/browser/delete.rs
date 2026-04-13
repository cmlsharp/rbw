use crate::{
    app::{self, Binding, Effect, Mode, StaticLabel, Transition, lookup_action},
    bind,
    config::Palette,
    domain::{Entry, EntryExt as _},
    app::hint_line,
};

use ratatui::{
    text::{Line, Text},
    widgets::Paragraph,
};

use crossterm::event::KeyEvent;

/// Actions available from the delete-confirm modal.
#[derive(Clone, Copy)]
pub enum Action {
    Cancel,
    Confirm,
}

impl StaticLabel for Action {
    fn label(&self) -> &'static str {
        match self {
            Self::Cancel => "no",
            Self::Confirm => "yes",
        }
    }
}

const BINDINGS: &[Binding<Action>] = &[
    bind!('y' => Action::Confirm, hint),
    bind!(ctrl + 'c' => Action::Cancel),
    bind!(esc => Action::Cancel),
    bind!('q' => Action::Cancel),
    bind!('n' => Action::Cancel, hint),
];

pub fn key_map(key: KeyEvent) -> Vec<app::Action> {
    lookup_action(BINDINGS, key)
        .map(|action| vec![app::Action::Delete(action)])
        .unwrap_or_default()
}

pub fn reduce(entry: Option<&Entry>, action: Action) -> Transition {
    match action {
        Action::Cancel => Transition::mode(Mode::Normal),
        Action::Confirm => {
            let Some(entry) = entry else {
                return Transition::none();
            };
            Transition::effect(Effect::delete_entry(entry))
        }
    }
}

pub fn render_confirm(
    frame: &mut ratatui::Frame<'_>,
    palette: &Palette,
    entry: &crate::domain::Entry,
) {
    let inner = crate::app::render_popup_shell(frame, 56, 8, "Delete Entry", palette.danger);
    let username = entry.username();
    let text = Text::from(vec![
        Line::from(format!(
            "Delete '{}'{}?",
            entry.name,
            if username.is_empty() {
                String::new()
            } else {
                format!(" ({username})")
            }
        )),
        Line::from(""),
        Line::from("This cannot be undone."),
        Line::from(""),
        hint_line(BINDINGS, palette),
    ]);
    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);
}
