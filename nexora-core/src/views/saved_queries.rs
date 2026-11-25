//! Named boolean queries stored in app data (§41).
//!
//! Only source text is persisted; the parser runs on every use so a grammar
//! change can never leave a stale AST in the sidecar.

use crate::{search, NexoraError, VaultIndex};
use std::fs;
use std::path::Path;

const MAGIC: &str = "NEXORA-SAVED-QUERIES-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedQuery {
    pub name: String,
    pub text: String,
}

fn invalid(message: &str) -> NexoraError {
    NexoraError::InvalidInput {
        message: message.to_string(),
    }
}

/// Replace an existing name in place, or append a new one.
pub fn upsert(queries: &[SavedQuery], name: &str, text: &str) -> Vec<SavedQuery> {
    let mut result = queries.to_vec();
    if let Some(existing) = result.iter_mut().find(|query| query.name == name) {
        existing.text = text.to_string();
    } else {
        result.push(SavedQuery {
            name: name.to_string(),
            text: text.to_string(),
        });
    }
    result
}

/// Rename a saved query in place. Returns an error for an empty, multiline or
/// colliding name; renaming to the same name is a no-op (§65).
pub fn rename(
    queries: &[SavedQuery],
    old_name: &str,
    new_name: &str,
) -> Result<Vec<SavedQuery>, NexoraError> {
    let new_name = new_name.trim();
    if new_name.is_empty() || new_name.contains(['\t', '\n', '\r']) {
        return Err(invalid("query name must be a non-empty single line"));
    }
    if !queries.iter().any(|query| query.name == old_name) {
        return Err(invalid("no saved query with that name"));
    }
    if queries
        .iter()
        .any(|query| query.name == new_name && new_name != old_name)
    {
        return Err(invalid("a saved query with that name already exists"));
    }
    Ok(queries
        .iter()
        .map(|query| {
            if query.name == old_name {
                SavedQuery {
                    name: new_name.to_string(),
                    text: query.text.clone(),
                }
            } else {
                query.clone()
            }
        })
        .collect())
}

/// Remove a saved query by exact name, preserving the order of the rest (§65).
pub fn remove(queries: &[SavedQuery], name: &str) -> Vec<SavedQuery> {
    queries
        .iter()
        .filter(|query| query.name != name)
        .cloned()
        .collect()
}

pub fn save(path: &Path, queries: &[SavedQuery]) -> Result<(), NexoraError> {
    let mut text = format!("{MAGIC}\n");
    for query in queries {
        if query.name.is_empty()
            || query.name.contains(['\t', '\n', '\r'])
            || query.text.contains(['\t', '\n', '\r'])
        {
            return Err(invalid("query name and text must be single-line"));
        }
        text.push_str(&query.name);
        text.push('\t');
        text.push_str(&query.text);
        text.push('\n');
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| NexoraError::io_at(parent, error))?;
    }
    fs::write(path, text.as_bytes()).map_err(|error| NexoraError::io_at(path, error))
}

pub fn load(path: &Path) -> Result<Vec<SavedQuery>, NexoraError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            NexoraError::InvalidUtf8 {
                path: path.to_path_buf(),
            }
        } else {
            NexoraError::io_at(path, error)
        }
    })?;
    let mut lines = text.lines();
    if lines.next() != Some(MAGIC) {
        return Err(invalid("bad saved-queries sidecar header"));
    }
    let mut queries = Vec::new();
    for line in lines {
        let (name, text) = line.split_once('\t').ok_or_else(|| invalid("bad saved query row"))?;
        if name.is_empty() {
            return Err(invalid("saved query name is empty"));
        }
        queries.push(SavedQuery {
            name: name.to_string(),
            text: text.to_string(),
        });
    }
    Ok(queries)
}

/// Stable identity of a query list, used to reset GUI selection after a
/// rename/remove (§82). Debug quoting prevents delimiter collisions.
pub fn selection_key(queries: &[SavedQuery]) -> String {
    queries
        .iter()
        .map(|query| format!("{:?}", (&query.name, &query.text)))
        .collect::<Vec<_>>()
        .join(";")
}

/// Reparse and run a saved query. Unknown names are an error, not a silent
/// empty result, so a typo is visible (§41.1).
pub fn run(
    index: &VaultIndex,
    queries: &[SavedQuery],
    name: &str,
) -> Result<Vec<search::SearchResult>, NexoraError> {
    let query = queries
        .iter()
        .find(|query| query.name == name)
        .ok_or_else(|| invalid("no saved query with that name"))?;
    // A malformed stored query is an error, not a silent empty result (§41.2).
    search::validate(&query.text)?;
    Ok(search::search_advanced(index, &query.text))
}
