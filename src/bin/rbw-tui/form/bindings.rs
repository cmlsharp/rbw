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
    DeleteWordBack,
    Delete,
    Insert(char),
    Left,
    Right,
    Home,
    End,
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
            Self::Backspace | Self::DeleteWordBack | Self::Delete => {
                "backspace"
            }
            Self::Insert(_) => "insert",
            Self::Left | Self::Right | Self::Home | Self::End => "move",
        }
    }
}

/// Returns the create-entry modal bindings.
const CREATE_BINDINGS: &[Binding<Action>] = &[
    bind!(ctrl + 'c' => Action::Cancel),
    bind!(esc => Action::Cancel),
    bind!(ctrl + 'v' => Action::TogglePassword, hint),
    bind!(tab => Action::NextField, repeatable),
    bind!(up => Action::NextField, repeatable),
    bind!(shift + tab => Action::PrevField, repeatable),
    bind!(down => Action::PrevField, repeatable),
    bind!(ctrl + 'n' => Action::NextField, repeatable),
    bind!(ctrl + 'p' => Action::PrevField, repeatable),
    bind!(ctrl + 'g' => Action::GeneratePassword, hint),
    bind!(ctrl + 'a' => Action::Home),
    bind!(ctrl + 'e' => Action::End),
    bind!(ctrl + 'o' => Action::AddUri),
    bind!(ctrl + 'x' => Action::RemoveUri),
    bind!(enter => Action::Save, hint),
    bind!(ctrl + 's' => Action::Save),
];

pub(super) fn bindings() -> &'static [Binding<Action>] {
    CREATE_BINDINGS
}
