//! Exact-target note rename with wikilink rewriting.
//!
//! The plan is computed in memory and shown as a dry run before any write.
//! See `ARCHITECTURE.md` §18 for the locked order and accepted partial-failure
//! behaviour.

use crate::{save, vault, wikilink, NexoraError, NoteId};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEdit {
    pub id: NoteId,
    pub original: String,
    pub content: String,
    pub replacements: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenamePlan {
    pub old: NoteId,
    pub new: NoteId,
    pub renamed_content: String,
    pub edits: Vec<FileEdit>,
}

fn history_dir(root: &Path, id: &NoteId) -> PathBuf {
    let mut path = root.join(".nexora").join("history");
    for component in id.as_str().split('/') {
        path.push(component);
    }
    path
}

/// Replace only exact `[[old]]` / `[[old|alias]]` targets, keeping the alias
/// and skipping fenced code blocks.
pub fn rewrite_links(content: &str, old: &NoteId, new: &NoteId) -> (String, usize) {
    let mut out = String::with_capacity(content.len());
    let mut count = 0;
    let mut in_fence = false;
    for raw_line in content.split_inclusive('\n') {
        // Keep the original line ending: rewriting a CRLF vault into LF would
        // produce a whole-file diff unrelated to the rename.
        let (line, ending) = if let Some(body) = raw_line.strip_suffix("\r\n") {
            (body, "\r\n")
        } else if let Some(body) = raw_line.strip_suffix('\n') {
            (body, "\n")
        } else {
            (raw_line, "")
        };
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            out.push_str(ending);
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push_str(ending);
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find("[[") {
            let after_open = &rest[start + 2..];
            let Some(end) = after_open.find("]]") else {
                out.push_str(rest);
                rest = "";
                break;
            };
            let inner = &after_open[..end];
            let (target_part, alias) = match inner.split_once('|') {
                Some((target, alias)) => (target, Some(alias)),
                None => (inner, None),
            };
            // An anchor after `#` is navigation metadata, not part of the
            // note id; rename rewrites only the note part (§38.1).
            let (note_part, anchor) = wikilink::split_target(target_part);
            let target = NoteId::new(&note_part);
            out.push_str(&rest[..start]);
            out.push_str("[[");
            if target == *old {
                out.push_str(new.as_str());
                count += 1;
            } else {
                out.push_str(&note_part);
            }
            if let Some(anchor) = anchor {
                out.push('#');
                out.push_str(&anchor);
            }
            if let Some(alias) = alias {
                out.push('|');
                out.push_str(alias);
            }
            out.push_str("]]");
            rest = &after_open[end + 2..];
        }
        out.push_str(rest);
        out.push_str(ending);
    }
    (out, count)
}

/// Compute a dry-run plan. Nothing is written.
pub fn plan(root: &Path, old: &NoteId, new_raw: &str) -> Result<RenamePlan, NexoraError> {
    let new = NoteId::new(new_raw);
    if !new.is_safe() {
        return Err(NexoraError::InvalidInput {
            message: format!("unsafe note id: {new_raw}"),
        });
    }
    if new == *old {
        return Err(NexoraError::InvalidInput {
            message: "source and target are the same note".to_string(),
        });
    }
    let notes = vault::reload(root)?;
    let renamed_content = notes
        .get(old)
        .ok_or_else(|| NexoraError::NotFound { id: old.to_string() })?
        .content
        .clone();
    if notes.contains_key(&new) {
        return Err(NexoraError::InvalidInput {
            message: format!("target already exists: {new}"),
        });
    }
    let mut edits = Vec::new();
    for (id, note) in &notes {
        let (content, replacements) = rewrite_links(&note.content, old, &new);
        if replacements > 0 {
            edits.push(FileEdit {
                id: id.clone(),
                original: note.content.clone(),
                content,
                replacements,
            });
        }
    }
    Ok(RenamePlan {
        old: old.clone(),
        new,
        renamed_content,
        edits,
    })
}

/// Apply a previously previewed plan.
pub fn apply(root: &Path, plan: &RenamePlan) -> Result<(), NexoraError> {
    let old_path = vault::note_path(root, &plan.old)?;
    let new_path = vault::note_path(root, &plan.new)?;
    if new_path.exists() {
        return Err(NexoraError::InvalidInput {
            message: format!("target already exists: {}", plan.new),
        });
    }
    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent).map_err(|error| NexoraError::io_at(parent, error))?;
    }

    // Snapshot the old note at its old path before moving the file; the
    // history directory is copied to the new path afterwards.
    save::snapshot_note(root, &plan.old, &plan.renamed_content)?;
    for edit in &plan.edits {
        save::write_note(root, &edit.id, &edit.content, &edit.original)?;
    }
    fs::rename(&old_path, &new_path)
        .map_err(|error| NexoraError::io_at(&new_path, error))?;

    let old_history = history_dir(root, &plan.old);
    let new_history = history_dir(root, &plan.new);
    if old_history.is_dir() {
        fs::create_dir_all(&new_history)
            .map_err(|error| NexoraError::io_at(&new_history, error))?;
        for entry in fs::read_dir(&old_history).map_err(|error| NexoraError::io_at(&old_history, error))? {
            let entry = entry.map_err(|error| NexoraError::io_at(&old_history, error))?;
            if entry.file_type().is_ok_and(|kind| kind.is_file()) {
                fs::copy(entry.path(), new_history.join(entry.file_name()))
                    .map_err(|error| NexoraError::io_at(&entry.path(), error))?;
            }
        }
    }
    Ok(())
}
