use crate::{
    app::{Binding, StaticLabel},
    bind,
};

/// Actions available from the generator modal.
#[derive(Clone, Copy)]
pub enum Action {
    Cancel,
    Down,
    Up,
    DecLength,
    IncLength,
    Confirm,
    Toggle,
    Backspace,
    InsertDigit(char),
    AcceptLength,
}

impl StaticLabel for Action {
    fn label(&self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::Down => "down",
            Self::Up => "up",
            Self::DecLength => "decrease",
            Self::IncLength => "increase",
            Self::Confirm => "confirm",
            Self::Toggle => "toggle",
            Self::Backspace => "delete",
            Self::InsertDigit(_) => "digit",
            Self::AcceptLength => "accept",
        }
    }
}

const DEFAULT_BINDINGS: &[Binding<Action>] = &[
    bind!(ctrl + 'c' => Action::Cancel),
    bind!(esc => Action::Cancel),
    bind!('q' => Action::Cancel, hint),
    bind!(down => Action::Down, repeatable),
    bind!('j' => Action::Down, repeatable),
    bind!(tab => Action::Down, repeatable),
    bind!(up => Action::Up, repeatable),
    bind!('k' => Action::Up, repeatable),
    bind!(shift + tab => Action::Up, repeatable),
    bind!(left => Action::DecLength, repeatable),
    bind!('h' => Action::DecLength, repeatable),
    bind!(right => Action::IncLength, repeatable),
    bind!('l' => Action::IncLength, repeatable),
    bind!(enter => Action::Confirm),
    bind!(ctrl + 's' => Action::Confirm),
    bind!(' ' => Action::Toggle, hint),
];

const LENGTH_BINDINGS: &[Binding<Action>] = &[
    bind!(ctrl + 'c' => Action::Cancel),
    bind!(esc => Action::Cancel),
    bind!('q' => Action::Cancel, hint),
    bind!(down => Action::Down, repeatable),
    bind!('j' => Action::Down, repeatable),
    bind!(tab => Action::Down, repeatable),
    bind!(up => Action::Up, repeatable),
    bind!('k' => Action::Up, repeatable),
    bind!(shift + tab => Action::Up, repeatable),
    bind!(left => Action::DecLength, repeatable),
    bind!('h' => Action::DecLength, repeatable, hint),
    bind!(right => Action::IncLength, repeatable),
    bind!('l' => Action::IncLength, repeatable),
    bind!(enter => Action::Confirm),
    bind!(ctrl + 's' => Action::Confirm),
    bind!(' ' => Action::Toggle, hint),
];

const EDIT_LENGTH_BINDINGS: &[Binding<Action>] = &[
    bind!(ctrl + 'c' => Action::Cancel),
    bind!(esc => Action::AcceptLength),
    bind!('q' => Action::Cancel, hint),
    bind!(enter => Action::AcceptLength),
    bind!(backspace => Action::Backspace, hint),
];

pub(super) fn bindings(state: &super::State) -> &'static [Binding<Action>] {
    if state.editing_length {
        EDIT_LENGTH_BINDINGS
    } else if state.selected_index == 1 {
        LENGTH_BINDINGS
    } else {
        DEFAULT_BINDINGS
    }
}
