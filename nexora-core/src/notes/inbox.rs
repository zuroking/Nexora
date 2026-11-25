//! Quick capture into `Inbox.md`.

use crate::{NexoraError, NoteId};
use chrono::Local;
use std::path::Path;

pub const INBOX_ID: &str = "Inbox";

/// Append one timestamped line through the guarded autosave write path.
pub fn capture(root: &Path, text: &str) -> Result<(), NexoraError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(NexoraError::InvalidInput {
            message: "quick capture text is empty".to_string(),
        });
    }

    let id = NoteId::new(INBOX_ID);
    let path = crate::vault::note_path(root, &id)?;
    let existing = if path.exists() {
        crate::save::read_disk(&path)?
    } else {
        String::new()
    };

    let separator = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let stamp = Local::now().format("%Y-%m-%d %H:%M").to_string();
    let updated = format!("{}{}[{}] {}\n", existing, separator, stamp, text);
    crate::save::write_note(root, &id, &updated, &existing)
}
