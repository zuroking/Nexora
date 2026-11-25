//! Autosave write order and per-note version history.
//!
//! Windows cannot rename over an existing file, so the sequence is:
//! snapshot old content -> write temp -> remove target -> rename temp -> target.
//! A crash between remove and rename loses the current write, but the snapshot
//! preserves the previous content (locked decision GM-3).

use crate::{NexoraError, NoteId};
use chrono::Local;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const MAX_SNAPSHOTS_PER_NOTE: usize = 10;

pub(crate) fn read_disk(path: &Path) -> Result<String, NexoraError> {
    fs::read_to_string(path).map_err(|error| {
        if error.kind() == ErrorKind::InvalidData {
            NexoraError::InvalidUtf8 {
                path: path.to_path_buf(),
            }
        } else {
            NexoraError::io_at(path, error)
        }
    })
}

fn history_dir(root: &Path, id: &NoteId) -> Result<PathBuf, NexoraError> {
    if !id.is_safe() {
        return Err(NexoraError::InvalidId {
            id: id.as_str().to_string(),
        });
    }
    let mut path = root.join(".nexora").join("history");
    for component in id.as_str().split('/') {
        path.push(component);
    }
    Ok(path)
}

/// Snapshot timestamps, newest first.
pub fn list_snapshots(root: &Path, id: &NoteId) -> Result<Vec<String>, NexoraError> {
    let directory = history_dir(root, id)?;
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(&directory).map_err(|error| NexoraError::io_at(&directory, error))? {
        let entry = entry.map_err(|error| NexoraError::io_at(&directory, error))?;
        if entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort_unstable();
    names.reverse();
    Ok(names)
}

/// Best-effort retention cleanup: a locked old snapshot must never block
/// writing the note itself (§79).
fn prune(root: &Path, id: &NoteId) -> Result<(), NexoraError> {
    // Every step is best-effort (§79): even a read_dir failure must not
    // block the note write that called us.
    let Ok(names) = list_snapshots(root, id) else {
        return Ok(());
    };
    if names.len() <= MAX_SNAPSHOTS_PER_NOTE {
        return Ok(());
    }
    let Ok(directory) = history_dir(root, id) else {
        return Ok(());
    };
    for name in &names[MAX_SNAPSHOTS_PER_NOTE..] {
        let _ = fs::remove_file(directory.join(name));
    }
    Ok(())
}

fn snapshot(root: &Path, id: &NoteId, old_content: &str) -> Result<(), NexoraError> {
    let directory = history_dir(root, id)?;
    fs::create_dir_all(&directory).map_err(|error| NexoraError::io_at(&directory, error))?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S%9f").to_string();
    let target = directory.join(stamp);
    fs::write(&target, old_content.as_bytes())
        .map_err(|error| NexoraError::io_at(&target, error))?;
    prune(root, id)
}

/// Store one snapshot of known content without touching the note file.
/// Used by rename before moving the file so history survives the path change.
pub fn snapshot_note(root: &Path, id: &NoteId, content: &str) -> Result<(), NexoraError> {
    snapshot(root, id, content)
}

/// Write a note. `loaded_content` is what the editor saw when it opened the
/// note; a mismatch with the current disk content is a conflict, never an
/// overwrite (locked decision GM-4).
pub fn write_note(
    root: &Path,
    id: &NoteId,
    new_content: &str,
    loaded_content: &str,
) -> Result<(), NexoraError> {
    let path = crate::vault::note_path(root, id)?;
    let existing = if path.exists() {
        Some(read_disk(&path)?)
    } else {
        None
    };

    match &existing {
        Some(disk) => {
            if disk != loaded_content {
                return Err(NexoraError::Conflict { path });
            }
            if disk == new_content {
                return Ok(());
            }
            snapshot(root, id, disk)?;
        }
        None => {
            // A new note is created even when empty — daily notes rely on this.
        }
    }

    // A NoteId may contain '/', so a new note can live in a folder that does
    // not exist yet (e.g. a broken link to `folder/missing`). The id has
    // already been validated by note_path, so creating parents is safe.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| NexoraError::io_at(parent, error))?;
    }
    let temp = path.with_extension("nexora-tmp");
    fs::write(&temp, new_content.as_bytes()).map_err(|error| NexoraError::io_at(&temp, error))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| NexoraError::io_at(&path, error))?;
    }
    if let Err(error) = fs::rename(&temp, &path) {
        let _ = fs::remove_file(&temp);
        return Err(NexoraError::io_at(&path, error));
    }
    Ok(())
}

/// Read one snapshot's content for the diff view. Does not write anything.
pub fn read_snapshot(root: &Path, id: &NoteId, stamp: &str) -> Result<String, NexoraError> {
    if !stamp
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(NexoraError::InvalidId {
            id: stamp.to_string(),
        });
    }
    let path = history_dir(root, id)?.join(stamp);
    if !path.is_file() {
        return Err(NexoraError::NotFound {
            id: format!("{}@{}", id, stamp),
        });
    }
    read_disk(&path)
}

/// Restore a snapshot by writing it back through the same guarded path.
/// Returns the exact bytes written so callers can refresh their buffers from
/// the write itself instead of a possibly stale index (§54).
pub fn restore_snapshot(root: &Path, id: &NoteId, stamp: &str) -> Result<String, NexoraError> {
    if !stamp
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(NexoraError::InvalidId {
            id: stamp.to_string(),
        });
    }
    let directory = history_dir(root, id)?;
    let snapshot_path = directory.join(stamp);
    if !snapshot_path.is_file() {
        return Err(NexoraError::NotFound {
            id: format!("{}@{}", id, stamp),
        });
    }
    let content = read_disk(&snapshot_path)?;
    let path = crate::vault::note_path(root, id)?;
    let disk = if path.exists() {
        read_disk(&path)?
    } else {
        String::new()
    };
    write_note(root, id, &content, &disk)?;
    Ok(content)
}
