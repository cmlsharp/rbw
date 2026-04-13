use anyhow::Result;

use crate::domain::{Entry, EntryDraft};

fn client() -> rbw::client::Client<rbw::client::AgentClient> {
    let agent =
        rbw::client::AgentClient::new(rbw::protocol::Environment::from_current());
    rbw::client::Client::new(agent)
}

/// Ensures the agent is running, started if necessary.
pub fn ensure_agent() -> Result<()> {
    rbw::config::Config::validate()?;
    client().ensure_agent()
}

/// Ensures the vault is unlocked (starts agent + logs in if needed).
pub fn ensure_unlocked() -> Result<()> {
    ensure_agent()?;
    let c = client();
    c.login()?;
    c.unlock()?;
    Ok(())
}

/// Lists all login entries from the vault.
pub fn list_entries() -> Result<Vec<Entry>> {
    client().list_entries()
}

/// Syncs the vault via the agent.
pub fn sync_vault() -> Result<()> {
    client().sync()
}

/// Creates a new login entry in the vault.
pub fn add_entry(draft: &EntryDraft) -> Result<()> {
    client().add_entry(draft)
}

/// Edits an existing entry by id, replacing its data with the draft contents.
pub fn edit_entry(entry_id: &str, draft: &EntryDraft) -> Result<()> {
    let c = client();
    let db = c.load_db()?;
    let entry = db
        .entries
        .iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| anyhow::anyhow!("entry not found"))?;

    c.edit_entry(entry, &draft.to_data(), draft.notes_option())
}

/// Copies text to the clipboard via the agent.
pub fn clipboard_store(text: &str) -> Result<()> {
    client().clipboard_store(text)
}

/// Removes one entry by id.
pub fn remove_entry(entry_id: &str) -> Result<()> {
    client().remove_entry(entry_id)
}
