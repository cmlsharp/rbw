use crate::{app::input::StaticLabel, rbw};

use crate::{
    domain::{Entry, EntryDraft, EntryExt as _, YankTarget},
    generator,
};

/// Side effect requested by one reducer step.
pub(crate) enum Effect {
    SyncVault,
    CopyTarget {
        value: String,
        label: YankTarget,
    },
    ResolveSelection(Entry),
    GeneratePassword {
        settings: generator::Settings,
    },
    CreateEntry(EntryDraft),
    EditEntry {
        entry_id: String,
        draft: EntryDraft,
    },
    DeleteEntry(String, String),
}

impl Effect {
    /// Human-readable label for operations slow enough to warrant a status notification.
    /// Returns `None` for fast local operations (copy, generate).
    pub(crate) fn pending_label(&self) -> Option<&'static str> {
        match self {
            Self::SyncVault => Some("Syncing..."),
            Self::CreateEntry(_) => Some("Creating..."),
            Self::EditEntry { .. } => Some("Saving..."),
            Self::DeleteEntry(_, _) => Some("Deleting..."),
            Self::CopyTarget { .. }
            | Self::ResolveSelection(..)
            | Self::GeneratePassword { .. } => None,
        }
    }

    pub(crate) fn copy_target(
        entry: &Entry,
        target: YankTarget,
    ) -> Effect {
        use YankTarget as Y;
        let value = match target {
            Y::Name => entry.name.clone(),
            Y::Notes => entry.notes_str().to_string(),
            Y::Folder => entry.folder_str().to_string(),
            Y::Uri => entry
                .uri_strings()
                .first()
                .map(|u| u.to_string())
                .unwrap_or_default(),
            Y::Username => entry.username().to_string(),
            Y::Password => entry.password().to_string(),
            Y::Totp => entry.totp().unwrap_or("").to_string(),
        };
        Self::CopyTarget {
            value,
            label: target,
        }
    }

    pub(crate) fn delete_entry(entry: &Entry) -> Self {
        Self::DeleteEntry(entry.id.clone(), entry.name.clone())
    }
}

/// Successful output produced by one executed side effect.
pub(crate) enum EffectOutcome {
    Synced(Vec<Entry>),
    Copied(&'static str),
    SelectionReady(String),
    GeneratedPassword {
        password: String,
    },
    Created {
        draft: EntryDraft,
        entries: Vec<Entry>,
    },
    Edited {
        name: String,
        entries: Vec<Entry>,
    },
    Deleted {
        entry_name: String,
        entries: Vec<Entry>,
    },
}

pub(crate) type EffectResult = Result<EffectOutcome, String>;

fn to_selection_json(entry: &Entry) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Output<'a> {
        status: &'static str,
        cipher: &'a ::rbw::decrypted::Cipher,
    }
    serde_json::to_string(&Output {
        status: "ok",
        cipher: entry,
    })
    .map_err(|err| format!("Selection failed: {err}"))
}

/// Executes one requested effect and returns its reducer-facing result.
impl Effect {
    pub(super) fn run(self) -> EffectResult {
        match self {
            Self::SyncVault => rbw::sync_vault()
                .and_then(|()| rbw::list_entries())
                .map_err(|err| format!("Sync failed: {err}"))
                .map(EffectOutcome::Synced),
            Self::CopyTarget { value, label } => {
                if value.is_empty() {
                    return Err(format!("No {} for entry", label.label()));
                }
                rbw::clipboard_store(&value)
                    .map_err(|err| format!("Copy failed: {err}"))?;
                Ok(EffectOutcome::Copied(label.label()))
            }
            Self::ResolveSelection(entry) => {
                to_selection_json(&entry).map(EffectOutcome::SelectionReady)
            }
            Self::GeneratePassword { settings } => {
                Ok(EffectOutcome::GeneratedPassword {
                    password: settings.generate(),
                })
            }
            Self::CreateEntry(draft) => {
                rbw::add_entry(&draft)
                    .and_then(|()| rbw::sync_vault())
                    .and_then(|()| rbw::list_entries())
                    .map_err(|err| format!("Create failed: {err}"))
                    .map(|entries| EffectOutcome::Created { draft, entries })
            }
            Self::EditEntry { entry_id, draft } => {
                let name = draft.name.clone();
                rbw::edit_entry(&entry_id, &draft)
                    .and_then(|()| rbw::sync_vault())
                    .and_then(|()| rbw::list_entries())
                    .map_err(|err| format!("Edit failed: {err}"))
                    .map(|entries| EffectOutcome::Edited { name, entries })
            }
            Self::DeleteEntry(id, name) => {
                rbw::remove_entry(&id)
                    .and_then(|()| rbw::sync_vault())
                    .and_then(|()| rbw::list_entries())
                    .map_err(|err| format!("Delete failed: {err}"))
                    .map(|entries| EffectOutcome::Deleted {
                        entry_name: name,
                        entries,
                    })
            }
        }
    }
}
