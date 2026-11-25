//! Compact note preview for hover tooltips. Pure `VaultIndex` projection.

use crate::{NoteId, VaultIndex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub title: String,
    pub excerpt: String,
}

/// First `max_lines` non-empty body lines, with frontmatter removed.
pub fn for_note(index: &VaultIndex, id: &NoteId, max_lines: usize) -> Option<Preview> {
    let note = index.notes.get(id)?;
    let mut lines = note.content.lines();
    let first = lines.next().unwrap_or("");
    let mut body: Box<dyn Iterator<Item = &str>> = if first.trim().trim_start_matches('\u{feff}') == "---" {
        let rest: Vec<&str> = lines.collect();
        let mut closed = false;
        let mut kept = Vec::new();
        for line in rest {
            if !closed {
                if line.trim() == "---" {
                    closed = true;
                }
            } else {
                kept.push(line);
            }
        }
        Box::new(kept.into_iter())
    } else {
        Box::new(note.content.lines())
    };
    let excerpt = body
        .by_ref()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n");
    Some(Preview {
        title: note.title.clone(),
        excerpt,
    })
}
