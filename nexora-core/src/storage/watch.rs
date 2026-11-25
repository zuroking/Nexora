//! Read-only polling monitor for changes made outside Nexora.
//!
//! The vault is the source of truth. A fingerprint of each note (content plus
//! modified time) is kept in memory and compared on a 1 Hz poll by the app.
//! See `ARCHITECTURE.md` §16 / DD-N4 for why polling was chosen over `notify`.

use crate::{vault, NexoraError, NoteId};
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Created(NoteId),
    Modified(NoteId),
    Deleted(NoteId),
}

fn fingerprint(root: &Path) -> Result<BTreeMap<NoteId, u64>, NexoraError> {
    Ok(vault::reload(root)?
        .into_iter()
        .map(|(id, note)| {
            let mut hasher = DefaultHasher::new();
            note.content.hash(&mut hasher);
            note.modified.hash(&mut hasher);
            (id, hasher.finish())
        })
        .collect())
}

/// Compares the current vault state against the last state we acknowledged.
pub struct DiskMonitor {
    root: PathBuf,
    known: BTreeMap<NoteId, u64>,
    startup_error: Option<NexoraError>,
}

impl DiskMonitor {
    pub fn new(root: &Path) -> Self {
        match fingerprint(root) {
            Ok(known) => Self {
                root: root.to_path_buf(),
                known,
                startup_error: None,
            },
            // §PROD-P3: an unreadable vault at startup must not be swallowed
            // into an empty fingerprint — that would report every note as
            // Created on the first poll and reload the index from scratch.
            Err(error) => Self {
                root: root.to_path_buf(),
                known: BTreeMap::new(),
                startup_error: Some(error),
            },
        }
    }

    /// Take the startup fingerprint error once, so the app can surface it
    /// instead of silently treating an unreadable vault as empty.
    pub fn take_startup_error(&mut self) -> Option<NexoraError> {
        self.startup_error.take()
    }

    /// Return changes since the last `refresh` / `resync` and remember them.
    pub fn refresh(&mut self) -> Result<Vec<Change>, NexoraError> {
        let current = fingerprint(&self.root)?;
        let mut changes = Vec::new();
        for (id, value) in &current {
            match self.known.get(id) {
                None => changes.push(Change::Created(id.clone())),
                Some(previous) if previous != value => {
                    changes.push(Change::Modified(id.clone()))
                }
                Some(_) => {}
            }
        }
        for id in self.known.keys() {
            if !current.contains_key(id) {
                changes.push(Change::Deleted(id.clone()));
            }
        }
        self.known = current;
        Ok(changes)
    }

    /// Acknowledge the current disk state without reporting changes. Called
    /// after Nexora itself writes, so our own autosave never looks external.
    pub fn resync(&mut self) {
        if let Ok(current) = fingerprint(&self.root) {
            self.known = current;
        }
    }
}
