//! Ordered note bookmarks stored in app data, never in the vault.

use crate::{NexoraError, NoteId, VaultIndex};
use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

const MAGIC: &str = "NEXORA-BOOKMARKS-v1";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bookmarks {
    pub ids: Vec<NoteId>,
}

impl Bookmarks {
    pub fn contains(&self, id: &NoteId) -> bool {
        self.ids.contains(id)
    }

    /// Add or remove, preserving insertion order.
    pub fn toggle(&mut self, id: &NoteId) -> bool {
        if let Some(position) = self.ids.iter().position(|item| item == id) {
            self.ids.remove(position);
            false
        } else {
            self.ids.push(id.clone());
            true
        }
    }

    /// Keep only ids that still exist in the index.
    pub fn prune(&mut self, index: &VaultIndex) {
        self.ids.retain(|id| index.notes.contains_key(id));
    }

    /// Rewrite an id after a rename, keeping its position.
    pub fn rename(&mut self, old: &NoteId, new: &NoteId) {
        for id in &mut self.ids {
            if id == old {
                *id = new.clone();
            }
        }
    }

    /// Remove one bookmark. Returns `true` when something was removed (§71).
    pub fn remove(&mut self, id: &NoteId) -> bool {
        let before = self.ids.len();
        self.ids.retain(|item| item != id);
        self.ids.len() != before
    }

    /// Move a bookmark by `delta` positions, clamped to the list. Returns
    /// `true` when the order changed (§71).
    pub fn move_by(&mut self, id: &NoteId, delta: isize) -> bool {
        let Some(from) = self.ids.iter().position(|item| item == id) else {
            return false;
        };
        let to = (from as isize + delta).clamp(0, self.ids.len() as isize - 1) as usize;
        if to == from {
            return false;
        }
        let moved = self.ids.remove(from);
        self.ids.insert(to, moved);
        true
    }
}

pub fn save(path: &Path, bookmarks: &Bookmarks) -> Result<(), NexoraError> {
    let mut text = format!("{MAGIC}\n");
    for id in &bookmarks.ids {
        text.push_str(id.as_str());
        text.push('\n');
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| NexoraError::io_at(parent, error))?;
    }
    fs::write(path, text.as_bytes()).map_err(|error| NexoraError::io_at(path, error))
}

pub fn load(path: &Path) -> Result<Bookmarks, NexoraError> {
    if !path.exists() {
        return Ok(Bookmarks::default());
    }
    let text = fs::read_to_string(path).map_err(|error| {
        if error.kind() == ErrorKind::InvalidData {
            NexoraError::InvalidUtf8 {
                path: path.to_path_buf(),
            }
        } else {
            NexoraError::io_at(path, error)
        }
    })?;
    let mut lines = text.lines();
    if lines.next() != Some(MAGIC) {
        return Err(NexoraError::InvalidInput {
            message: "bad bookmarks sidecar header".to_string(),
        });
    }
    let mut seen = BTreeSet::new();
    let ids = lines
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let id = NoteId::new(line);
            id.is_safe().then_some(id)
        })
        .filter(|id| seen.insert(id.clone()))
        .collect();
    Ok(Bookmarks { ids })
}
