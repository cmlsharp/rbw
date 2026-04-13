use crate::{
    app::{Binding, StaticLabel},
    bind,
};

/// Actions available from the create-entry modal.
#[derive(Clone, Copy)]
pub enum Action {
    Cancel,
    TogglePassword,
    NextField,
    PrevField,
    GeneratePassword,
    AddUri,
    RemoveUri,
    Save,
    Backspace,
    Insert(char),
}

impl StaticLabel for Action {
    fn label(&self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::TogglePassword => "show/hide password",
            Self::NextField | Self::PrevField => "move",
            Self::GeneratePassword => "generate password",
            Self::AddUri => "add URI",
            Self::RemoveUri => "remove URI",
            Self::Save => "save",
            Self::Backspace => "backspace",
            Self::Insert(_) => "insert",
        }
    }
}

/// Returns the create-entry modal bindings.
const CREATE_BINDINGS: &[Binding<Action>] = &[
    bind!(ctrl + 'c' => Action::Cancel),
    bind!(esc => Action::Cancel),
    bind!(ctrl + 'v' => Action::TogglePassword, hint),
    bind!(tab => Action::NextField, repeatable, hint),
    bind!(shift + tab => Action::PrevField, repeatable),
    bind!(ctrl + 'g' => Action::GeneratePassword, hint),
    bind!(ctrl + 'a' => Action::AddUri, hint),
    bind!(ctrl + 'd' => Action::RemoveUri, hint),
    bind!(enter => Action::Save, hint),
    bind!(ctrl + 's' => Action::Save),
];

pub(super) fn bindings() -> &'static [Binding<Action>] {
    CREATE_BINDINGS
}
