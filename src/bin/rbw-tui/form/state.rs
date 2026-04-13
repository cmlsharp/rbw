use crate::domain::{Entry, EntryDraft, EntryExt as _};
use crate::text_input::TextInput;

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
    /// Active editing buffer for the focused field.
    input: TextInput,
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
        let input = TextInput::from_str(&draft.name);
        Self {
            draft,
            purpose: Purpose::Create,
            field: Field::Name,
            show_password: false,
            replace_on_input: true,
            input,
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
        let input = TextInput::from_str(&draft.name);
        Self {
            draft,
            purpose: Purpose::Edit { entry_id: entry.id.clone() },
            field: Field::Name,
            show_password: false,
            replace_on_input: true,
            input,
        }
    }

    /// Title for the modal popup.
    pub fn title(&self) -> &'static str {
        match &self.purpose {
            Purpose::Create => "Create Entry",
            Purpose::Edit { .. } => "Edit Entry",
        }
    }

    /// Returns the value of the given field from the draft.
    pub fn field_value(&self, field: Field) -> &str {
        if field == self.field {
            return self.input.as_str();
        }
        self.draft_field(field)
    }

    /// Returns the value of the given field directly from the draft (not the input buffer).
    fn draft_field(&self, field: Field) -> &str {
        match field {
            Field::Name => &self.draft.name,
            Field::Username => &self.draft.username,
            Field::Password => &self.draft.password,
            Field::Uri(i) => self.draft.uris.get(i).map(|s| s.as_str()).unwrap_or(""),
            Field::Folder => &self.draft.folder,
            Field::Notes => &self.draft.notes,
        }
    }

    /// Flushes the input buffer back into the draft for the current field.
    fn flush_input(&mut self) {
        let value = self.input.as_str().to_string();
        match self.field {
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

    /// Switches to a new field, flushing the current input and loading the new one.
    fn switch_to(&mut self, field: Field) {
        self.flush_input();
        self.field = field;
        self.input = TextInput::from_str(self.draft_field(field));
        self.replace_on_input = true;
    }

    /// Ensures the draft is up-to-date with the input buffer before saving.
    pub fn sync_draft(&mut self) {
        self.flush_input();
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
        let next = fields[(pos + 1) % fields.len()];
        self.switch_to(next);
    }

    /// Moves to the previous field in tab order, wrapping at the start.
    pub fn previous_field(&mut self) {
        let fields = self.fields();
        let pos = fields.iter().position(|f| *f == self.field).unwrap_or(0);
        let prev = fields[(pos + fields.len() - 1) % fields.len()];
        self.switch_to(prev);
    }

    /// Toggles whether the password field is visually revealed.
    pub fn toggle_password_visibility(&mut self) {
        self.show_password = !self.show_password;
    }

    /// Replaces the password field with a generated password and focuses it.
    pub fn apply_generated_password(&mut self, password: String) {
        self.flush_input();
        self.draft.password = password;
        self.field = Field::Password;
        self.input = TextInput::from_str(&self.draft.password);
        self.replace_on_input = true;
    }

    /// Adds a new URI row and focuses it.
    pub fn add_uri(&mut self) {
        self.flush_input();
        self.draft.uris.push(String::new());
        self.field = Field::Uri(self.draft.uris.len() - 1);
        self.input = TextInput::new();
        self.replace_on_input = true;
    }

    /// Removes the currently focused URI row (if there's more than one).
    pub fn remove_current_uri(&mut self) {
        if let Field::Uri(i) = self.field {
            if self.draft.uris.len() > 1 {
                self.draft.uris.remove(i);
                let new_count = self.draft.uris.len();
                self.field = Field::Uri(i.min(new_count - 1));
                self.input = TextInput::from_str(self.draft_field(self.field));
                self.replace_on_input = true;
            }
        }
    }

    /// Character offset of the cursor within the current field (for rendering).
    pub fn cursor_char_offset(&self) -> usize {
        self.input.cursor_char_offset()
    }

    /// Handles `replace_on_input` — clears the input on first edit, then delegates to the closure.
    fn edit_input(&mut self, f: impl FnOnce(&mut TextInput)) {
        if self.replace_on_input {
            self.input.clear();
            self.replace_on_input = false;
        }
        f(&mut self.input);
    }

    pub fn backspace(&mut self) {
        self.edit_input(TextInput::backspace);
    }

    pub fn delete(&mut self) {
        self.edit_input(TextInput::delete);
    }

    pub fn insert_char(&mut self, ch: char) {
        self.edit_input(|input| input.insert(ch));
    }

    pub fn delete_word_back(&mut self) {
        self.edit_input(TextInput::delete_word_back);
    }

    pub fn move_left(&mut self) {
        self.replace_on_input = false;
        self.input.move_left();
    }

    pub fn move_right(&mut self) {
        self.replace_on_input = false;
        self.input.move_right();
    }

    pub fn move_to_start(&mut self) {
        self.replace_on_input = false;
        self.input.move_to_start();
    }

    pub fn move_to_end(&mut self) {
        self.replace_on_input = false;
        self.input.move_to_end();
    }
}
