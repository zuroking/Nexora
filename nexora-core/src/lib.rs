//! Testable core of Nexora. No GUI dependencies live here.
//!
//! Modules are grouped by responsibility on disk, then re-exported at the
//! crate root so every `crate::foo` / `nexora_core::foo` path stays stable.

pub mod links;
pub mod notes;
pub mod retrieval;
pub mod storage;
pub mod views;

pub use links::{display, graph, images, mentions, outgoing, preview, related, wikilink};
pub use notes::{
    composer, daily, inbox, lists, model, quotes, rename, review, status, tables, templates,
};
pub use retrieval::{fuzzy, health, search, semantic, tags};
pub use storage::{diff, frontmatter, save, vault, watch};
pub use views::{
    autocomplete, bases, bookmarks, canvas, commands, named_bases, outline, saved_queries, tabs,
    theme, tree, workspace,
};

pub use model::{Link, Note, NoteId, VaultIndex};

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Every fallible operation in the core returns this type. No string errors.
#[derive(Debug, Error)]
pub enum NexoraError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid UTF-8 in {path}")]
    InvalidUtf8 { path: PathBuf },
    #[error("note not found: {id}")]
    NotFound { id: String },
    #[error("invalid note id: {id}")]
    InvalidId { id: String },
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("conflict: {path} was modified outside Nexora")]
    Conflict { path: PathBuf },
    #[error("broken link in note {note}: [[{target}]]")]
    BrokenLink { note: String, target: String },
}

impl NexoraError {
    pub fn io_at(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}
