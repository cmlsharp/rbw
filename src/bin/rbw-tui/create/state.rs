use crate::domain::EntryDraft;

/// Identifies one editable field in the create-entry form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Username,
    Password,
    Uri(usize),
    Folder,
    Notes,
}

/// Create-entry modal state.
#[derive(Debug, Clone)]
pub struct State {
    pub draft: EntryDraft,
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
    pub fn new(mut draft: EntryDraft) -> Self {
        if draft.uris.is_empty() {
            draft.uris.push(String::new());
        }
        Self {
            draft,
            field: Field::Name,
            show_password: false,
            replace_on_input: true,
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
