use crate::domain::{Entry, EntryDraft, EntryExt as _};

/// Identifies one editable field in the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Username,
    Password,
    Uri(usize),
    Folder,
    Notes,
}

/// Whether the form is creating a new entry or editing an existing one.
#[derive(Debug, Clone)]
pub enum Purpose {
    Create,
    Edit { entry_id: String },
}

/// Entry form modal state (shared by create and edit).
#[derive(Debug, Clone)]
pub struct State {
    pub draft: EntryDraft,
    pub purpose: Purpose,
    pub field: Field,
    pub show_password: bool,
    pub replace_on_input: bool,
}

impl Field {
    /// Returns the display label used for this form field.
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Username => "Username",
            Self::Password => "Password",
            Self::Uri(_) => "URI",
            Self::Folder => "Folder",
            Self::Notes => "Notes",
        }
    }
}

impl State {
    /// Starts a fresh create-entry session.
    pub fn new_create(mut draft: EntryDraft) -> Self {
        if draft.uris.is_empty() {
            draft.uris.push(String::new());
        }
        Self {
            draft,
            purpose: Purpose::Create,
            field: Field::Name,
            show_password: false,
            replace_on_input: true,
        }
    }

    /// Starts an edit session from an existing decrypted entry.
    pub fn new_edit(entry: &Entry) -> Self {
        let uris = entry.uri_strings().into_iter().map(str::to_string).collect::<Vec<_>>();
        let draft = EntryDraft {
            name: entry.name.clone(),
            username: entry.username().to_string(),
            password: entry.password().to_string(),
            totp: entry.totp().unwrap_or_default().to_string(),
            uris: if uris.is_empty() { vec![String::new()] } else { uris },
            folder: entry.folder_str().to_string(),
            notes: entry.notes_str().to_string(),
            org_id: None,
        };
        Self {
            draft,
            purpose: Purpose::Edit { entry_id: entry.id.clone() },
            field: Field::Name,
            show_password: false,
            replace_on_input: true,
        }
    }

    /// Title for the modal popup.
    pub fn title(&self) -> &'static str {
        match &self.purpose {
            Purpose::Create => "Create Entry",
            Purpose::Edit { .. } => "Edit Entry",
        }
    }

    /// Returns the value of the currently focused field.
    pub fn field_value(&self, field: Field) -> &str {
        match field {
            Field::Name => &self.draft.name,
            Field::Username => &self.draft.username,
            Field::Password => &self.draft.password,
            Field::Uri(i) => self.draft.uris.get(i).map(|s| s.as_str()).unwrap_or(""),
            Field::Folder => &self.draft.folder,
            Field::Notes => &self.draft.notes,
        }
    }

    /// Sets the value of the given field.
    pub fn set_field_value(&mut self, field: Field, value: String) {
        match field {
            Field::Name => self.draft.name = value,
            Field::Username => self.draft.username = value,
            Field::Password => self.draft.password = value,
            Field::Uri(i) => {
                while self.draft.uris.len() <= i {
                    self.draft.uris.push(String::new());
                }
                self.draft.uris[i] = value;
            }
            Field::Folder => self.draft.folder = value,
            Field::Notes => self.draft.notes = value,
        }
    }

    /// All fields in display order, including one row per URI.
    pub fn fields(&self) -> Vec<Field> {
        let mut fields = vec![Field::Name, Field::Username, Field::Password];
        let count = self.draft.uris.len().max(1);
        for i in 0..count {
            fields.push(Field::Uri(i));
        }
        fields.push(Field::Folder);
        fields.push(Field::Notes);
        fields
    }

    /// Moves to the next field in tab order, wrapping at the end.
    pub fn next_field(&mut self) {
        let fields = self.fields();
        let pos = fields.iter().position(|f| *f == self.field).unwrap_or(0);
        self.field = fields[(pos + 1) % fields.len()];
        self.replace_on_input = true;
    }

    /// Moves to the previous field in tab order, wrapping at the start.
    pub fn previous_field(&mut self) {
        let fields = self.fields();
        let pos = fields.iter().position(|f| *f == self.field).unwrap_or(0);
        self.field = fields[(pos + fields.len() - 1) % fields.len()];
        self.replace_on_input = true;
    }

    /// Toggles whether the password field is visually revealed.
    pub fn toggle_password_visibility(&mut self) {
        self.show_password = !self.show_password;
    }

    /// Replaces the password field with a generated password and focuses it.
    pub fn apply_generated_password(&mut self, password: String) {
        self.draft.password = password;
        self.field = Field::Password;
        self.replace_on_input = true;
    }

    /// Adds a new URI row and focuses it.
    pub fn add_uri(&mut self) {
        self.draft.uris.push(String::new());
        self.field = Field::Uri(self.draft.uris.len() - 1);
        self.replace_on_input = true;
    }

    /// Removes the currently focused URI row (if there's more than one).
    pub fn remove_current_uri(&mut self) {
        if let Field::Uri(i) = self.field {
            if self.draft.uris.len() > 1 {
                self.draft.uris.remove(i);
                let new_count = self.draft.uris.len();
                self.field = Field::Uri(i.min(new_count - 1));
                self.replace_on_input = true;
            }
        }
    }

    /// Applies a backspace to the currently focused field.
    pub fn backspace(&mut self) {
        let mut value = self.field_value(self.field).to_string();
        if self.replace_on_input {
            value.clear();
        } else {
            value.pop();
        }
        self.set_field_value(self.field, value);
        self.replace_on_input = false;
    }

    /// Inserts one printable character into the focused field.
    pub fn insert_char(&mut self, ch: char) {
        let mut value = if self.replace_on_input {
            String::new()
        } else {
            self.field_value(self.field).to_string()
        };
        value.push(ch);
        self.set_field_value(self.field, value);
        self.replace_on_input = false;
    }
}
