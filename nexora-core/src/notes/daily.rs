//! Daily notes: `YYYY-MM-DD.md` at the vault root.

use crate::{NexoraError, NoteId};
use chrono::Local;
use std::path::Path;

pub fn today_id() -> NoteId {
    NoteId::new(Local::now().format("%Y-%m-%d").to_string())
}

/// Open today's note, creating it with `body` if it does not exist yet.
/// An existing note is never rewritten (§37.1).
pub fn open_or_create_with(root: &Path, body: &str) -> Result<NoteId, NexoraError> {
    let id = today_id();
    let path = crate::vault::note_path(root, &id)?;
    if !path.exists() {
        crate::save::write_note(root, &id, body, "")?;
    }
    Ok(id)
}

/// Open today's note, creating an empty file if it does not exist yet.
pub fn open_or_create(root: &Path) -> Result<NoteId, NexoraError> {
    open_or_create_with(root, "")
}
