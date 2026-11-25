//! Vault discovery, path conversion, and note loading.

use crate::{NexoraError, Note, NoteId, VaultIndex};
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

/// `%USERPROFILE%\Documents\Nexora\my-first-vault` on Windows.
pub fn default_vault_root() -> PathBuf {
    let base = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Documents").join("Nexora").join("my-first-vault")
}

/// Create the vault root if it does not exist yet (first-run behavior).
pub fn ensure_vault(root: &Path) -> Result<(), NexoraError> {
    fs::create_dir_all(root).map_err(|error| NexoraError::io_at(root, error))
}

/// Convert a filesystem path inside `root` into a canonical `NoteId`.
/// Returns `None` for non-Markdown files and any dot-prefixed component.
pub fn note_id_from_path(root: &Path, path: &Path) -> Option<NoteId> {
    let relative = path.strip_prefix(root).ok()?;
    if !relative
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
    {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => {
                let name = value.to_string_lossy();
                if name.starts_with('.') {
                    return None;
                }
                parts.push(name.into_owned());
            }
            Component::CurDir => {}
            _ => return None,
        }
    }

    let last = parts.last_mut()?;
    *last = Path::new(last.as_str())
        .file_stem()?
        .to_string_lossy()
        .into_owned();

    let id = NoteId::new(parts.join("/"));
    id.is_safe().then_some(id)
}

/// Convert a `NoteId` back into an OS path. Traversal is rejected.
pub fn note_path(root: &Path, id: &NoteId) -> Result<PathBuf, NexoraError> {
    if !id.is_safe() {
        return Err(NexoraError::InvalidId {
            id: id.as_str().to_string(),
        });
    }
    let components: Vec<&str> = id.as_str().split('/').collect();
    let mut path = root.to_path_buf();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        path.push(component);
    }
    // Append the final component with a real `.md` suffix: `set_extension`
    // replaces the last dot in the stem, so id `v2.0` (disk file `v2.0.md`)
    // mapped to `v2.md` and notes with interior dots were unreachable (§PROD-P1).
    match components.last() {
        Some(last) => path.push(format!("{last}.md")),
        None => return Err(NexoraError::InvalidId { id: id.as_str().to_string() }),
    }
    Ok(path)
}

/// First Markdown H1, falling back to the filename stem.
fn extract_title(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        line.trim()
            .trim_start_matches('\u{feff}')
            .strip_prefix("# ")
            .map(|title| title.trim().to_string())
            .filter(|title| !title.is_empty())
    })
}

/// Read one note from disk.
pub fn load_note(root: &Path, id: &NoteId) -> Result<Note, NexoraError> {
    let path = note_path(root, id)?;
    let content = fs::read_to_string(&path).map_err(|error| {
        if error.kind() == ErrorKind::InvalidData {
            NexoraError::InvalidUtf8 {
                path: path.clone(),
            }
        } else {
            NexoraError::io_at(&path, error)
        }
    })?;
    let modified = fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| NexoraError::io_at(&path, error))?;

    Ok(Note {
        id: id.clone(),
        title: extract_title(&content).unwrap_or_else(|| id.stem().to_string()),
        content,
        modified,
    })
}

/// Full rebuild of the note map from disk.
pub fn reload(root: &Path) -> Result<BTreeMap<NoteId, Note>, NexoraError> {
    ensure_vault(root)?;
    let mut notes = BTreeMap::new();
    walk(root, root, &mut notes)?;
    Ok(notes)
}

/// Rebuild every derived index in one pass. This is the entry point used by
/// the GUI after any filesystem change.
pub fn build_index(root: &Path) -> Result<VaultIndex, NexoraError> {
    let notes = reload(root)?;
    let mut aliases = BTreeMap::new();
    for (id, note) in &notes {
        for alias in crate::frontmatter::read_list(&note.content, "aliases") {
            aliases.insert(alias, id.clone());
        }
    }
    let (links, backlinks) = crate::wikilink::build_links(&notes, &aliases);
    let tags = crate::tags::build_tag_index(&notes);
    Ok(VaultIndex {
        notes,
        backlinks,
        tags,
        links,
        aliases,
    })
}

fn walk(
    directory: &Path,
    root: &Path,
    notes: &mut BTreeMap<NoteId, Note>,
) -> Result<(), NexoraError> {
    let entries =
        fs::read_dir(directory).map_err(|error| NexoraError::io_at(directory, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| NexoraError::io_at(directory, error))?;
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| NexoraError::io_at(&path, error))?;
        if file_type.is_dir() {
            walk(&path, root, notes)?;
        } else if file_type.is_file() {
            if let Some(id) = note_id_from_path(root, &path) {
                notes.insert(id.clone(), load_note(root, &id)?);
            }
        }
    }
    Ok(())
}
