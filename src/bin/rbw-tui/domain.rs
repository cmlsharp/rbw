use crate::app::StaticLabel;

pub type Entry = rbw::decrypted::Cipher;
pub type EntryDraft = rbw::client::EntryDraft;

/// Convenience accessors for TUI display over [`rbw::decrypted::Cipher`].
pub trait EntryExt {
    fn username(&self) -> &str;
    fn password(&self) -> &str;
    fn totp(&self) -> Option<&str>;
    fn folder_str(&self) -> &str;
    fn notes_str(&self) -> &str;
    fn uri_strings(&self) -> Vec<&str>;
}

impl EntryExt for Entry {
    fn username(&self) -> &str {
        if let rbw::decrypted::Data::Login {
            username: Some(ref u),
            ..
        } = self.data
        {
            u
        } else {
            ""
        }
    }

    fn password(&self) -> &str {
        if let rbw::decrypted::Data::Login {
            password: Some(ref p),
            ..
        } = self.data
        {
            p
        } else {
            ""
        }
    }

    fn totp(&self) -> Option<&str> {
        if let rbw::decrypted::Data::Login {
            totp: Some(ref t), ..
        } = self.data
        {
            Some(t)
        } else {
            None
        }
    }

    fn folder_str(&self) -> &str {
        self.folder.as_deref().unwrap_or("")
    }

    fn notes_str(&self) -> &str {
        self.notes.as_deref().unwrap_or("")
    }

    fn uri_strings(&self) -> Vec<&str> {
        if let rbw::decrypted::Data::Login {
            uris: Some(ref uris),
            ..
        } = self.data
        {
            uris.iter().map(|u| u.uri.as_str()).collect()
        } else {
            Vec::new()
        }
    }
}

/// Prefills a create-entry draft from page context.
pub fn draft_from_seed(url: &str, username: &str) -> EntryDraft {
    let targets = crate::browser::site_targets(url);
    let name = targets
        .get(2)
        .or_else(|| targets.get(1))
        .or_else(|| targets.first())
        .cloned()
        .unwrap_or_default();

    let uris = if url.is_empty() {
        Vec::new()
    } else {
        vec![url.to_string()]
    };

    EntryDraft {
        name,
        username: username.to_string(),
        uris,
        ..EntryDraft::default()
    }
}

/// Top-level vault filter mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Vault,
    Site,
}

impl Scope {
    /// Toggles between vault-wide and site-only filtering.
    pub fn toggle(self, has_url: bool) -> Self {
        if !has_url {
            Self::Vault
        } else {
            match self {
                Self::Vault => Self::Site,
                Self::Site => Self::Vault,
            }
        }
    }
}

/// Copy/export target chosen from yank mode.
#[derive(Debug, Clone, Copy)]
pub enum YankTarget {
    Name,
    Notes,
    Folder,
    Uri,
    Username,
    Password,
    Totp,
}

impl StaticLabel for YankTarget {
    fn label(&self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Notes => "notes",
            Self::Folder => "folder",
            Self::Uri => "URI",
            Self::Username => "username",
            Self::Password => "password",
            Self::Totp => "totp",
        }
    }
}
