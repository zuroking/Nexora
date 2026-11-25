//! Note Composer: extract a line range or merge notes (§40).
//!
//! The pure helpers are testable; the `apply_*` functions are the only ones
//! that write, and they go through `save` so existing snapshots are kept.

use crate::{rename, save, vault, NexoraError, NoteId, VaultIndex};
use std::fs;
use std::path::Path;

fn invalid(message: &str) -> NexoraError {
    NexoraError::InvalidInput {
        message: message.to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractPlan {
    pub source: NoteId,
    pub new_id: NoteId,
    pub updated_source: String,
    pub new_content: String,
}

/// Result of a merge: the number of rewritten links and the exact bytes
/// written for each note, so the GUI can refresh its tabs (§78).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergePlan {
    pub rewritten: usize,
    /// Every note whose file the merge changed (target included).
    pub changes: Vec<(NoteId, String)>,
}

/// Remove an inclusive 1-based line range, leaving a `[[new_id]]` link at the
/// removal point. Line endings are preserved byte-for-byte (§40.2).
pub fn extract(
    content: &str,
    source: &NoteId,
    new_id: &NoteId,
    first: usize,
    last: usize,
) -> Result<ExtractPlan, NexoraError> {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    if first == 0 || last < first || last > lines.len() {
        return Err(invalid("line range is outside the note"));
    }
    let start = first - 1;
    let end = last;
    let new_content = lines[start..end].concat();
    let ending = if new_content.ends_with("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut updated = String::with_capacity(content.len());
    updated.push_str(&lines[..start].concat());
    updated.push_str(&format!("[[{new_id}]]{ending}"));
    updated.push_str(&lines[end..].concat());
    Ok(ExtractPlan {
        source: source.clone(),
        new_id: new_id.clone(),
        updated_source: updated,
        new_content,
    })
}

/// Append the source body to the target, with a plain-text provenance line.
pub fn merge_content(target: &str, source: &str, source_id: &NoteId) -> String {
    let mut out = target.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    // Plain text, not a heading: under GM-1 a heading would register tags.
    out.push_str(&format!("\nMerged from {source_id}\n\n"));
    out.push_str(source);
    out
}

pub fn apply_extract(
    root: &Path,
    index: &VaultIndex,
    source: &NoteId,
    new_id: &NoteId,
    first: usize,
    last: usize,
) -> Result<ExtractPlan, NexoraError> {
    let note = index.notes.get(source).ok_or_else(|| invalid("source note missing"))?;
    if index.notes.contains_key(new_id) {
        return Err(invalid("target note already exists"));
    }
    let plan = extract(&note.content, source, new_id, first, last)?;
    // Write the new note first: if the source write later fails, the extracted
    // lines still exist and a retry is idempotent (§78).
    save::write_note(root, new_id, &plan.new_content, "")?;
    save::write_note(root, source, &plan.updated_source, &note.content)?;
    Ok(plan)
}

/// Rewrite every link to `source` so it points at `target`, append the source
/// body, then delete the source file. Returns the rewritten-link count and the
/// exact bytes written for each changed note (§78). Partial failure is
/// surfaced as an error (§40.1).
pub fn apply_merge(
    root: &Path,
    index: &VaultIndex,
    source: &NoteId,
    target: &NoteId,
) -> Result<MergePlan, NexoraError> {
    let source_note = index
        .notes
        .get(source)
        .ok_or_else(|| invalid("source note missing"))?;
    let target_note = index
        .notes
        .get(target)
        .ok_or_else(|| invalid("target note missing"))?;
    // Rewrite the target's and the source's own links before merging: once
    // the source file is gone, any `[[source]]` left inside either body would
    // become a broken link (GM-2).
    let (target_body, _) = rename::rewrite_links(&target_note.content, source, target);
    let (source_body, _) = rename::rewrite_links(&source_note.content, source, target);
    let merged = merge_content(&target_body, &source_body, source);

    let mut rewritten = 0;
    let mut changes = Vec::new();
    for (id, note) in &index.notes {
        if id == source {
            continue;
        }
        let (content, count) = rename::rewrite_links(&note.content, source, target);
        let content = if id == target { merged.clone() } else { content };
        if content != note.content {
            save::write_note(root, id, &content, &note.content)?;
            if id != target {
                rewritten += count;
            }
            changes.push((id.clone(), content));
        }
    }
    // Keep the disappearing note recoverable, same as rename's §18 order.
    save::snapshot_note(root, source, &source_note.content)?;
    let source_path = vault::note_path(root, source)?;
    fs::remove_file(source_path).map_err(|error| NexoraError::io_at(root, error))?;
    Ok(MergePlan {
        rewritten,
        changes,
    })
}
