//! Data types shared by every core module.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::time::SystemTime;

/// Stable identity of a note: vault-relative path without the `.md` extension.
/// Separators are always `/`, regardless of the host OS.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NoteId(pub String);

impl NoteId {
    pub fn new(raw: impl Into<String>) -> Self {
        let normalized = raw.into().replace('\\', "/");
        Self(normalized.trim_matches('/').to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Last path component, used when a note has no H1 title.
    pub fn stem(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// Reject traversal and malformed ids before any filesystem access.
    pub fn is_safe(&self) -> bool {
        !self.0.is_empty()
            && !self
                .0
                .split('/')
                .any(|component| {
                // Windows filenames cannot contain ':'; a drive prefix such
                // as `C:` would make PathBuf::push escape the vault root.
                component.is_empty()
                    || component == "."
                    || component == ".."
                    || component.contains(':')
            })
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A parsed `[[target]]` or `[[target|alias]]` occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    Resolved {
        target: NoteId,
        alias: Option<String>,
        anchor: Option<String>,
    },
    Broken {
        target: NoteId,
        anchor: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub struct Note {
    pub id: NoteId,
    pub title: String,
    pub content: String,
    pub modified: SystemTime,
}

/// Rebuilt wholesale from disk. `BTreeMap` gives deterministic iteration order.
#[derive(Clone, Debug, Default)]
pub struct VaultIndex {
    pub notes: BTreeMap<NoteId, Note>,
    pub backlinks: BTreeMap<NoteId, BTreeSet<NoteId>>,
    pub tags: BTreeMap<String, BTreeSet<NoteId>>,
    pub links: BTreeMap<NoteId, Vec<Link>>,
    pub aliases: BTreeMap<String, NoteId>,
}
