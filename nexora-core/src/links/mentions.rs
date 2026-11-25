//! Unlinked mentions: prose occurrences of a note's title or aliases.
//!
//! Pure projection of `VaultIndex`; no writes. See `ARCHITECTURE.md` §30.

use crate::{NoteId, VaultIndex};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    pub note: NoteId,
    pub line: usize,
    pub snippet: String,
}

/// Blank out `[[...]]` spans so wikilinks are not reported as prose mentions.
fn mask_links(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut masked = vec![' '; chars.len()];
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '[' && index + 1 < chars.len() && chars[index + 1] == '[' {
            let mut end = index + 2;
            while end + 1 < chars.len() && !(chars[end] == ']' && chars[end + 1] == ']') {
                end += 1;
            }
            index = if end + 1 < chars.len() { end + 2 } else { chars.len() };
        } else {
            masked[index] = chars[index];
            index += 1;
        }
    }
    masked.into_iter().collect()
}

fn needles(index: &VaultIndex, target: &NoteId) -> Vec<String> {
    let mut values = vec![index
        .notes
        .get(target)
        .map(|note| note.title.clone())
        .unwrap_or_else(|| target.stem().to_string())];
    values.extend(
        index
            .aliases
            .iter()
            .filter(|(_, id)| *id == target)
            .map(|(alias, _)| alias.clone()),
    );
    let mut lower: Vec<String> = values
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect();
    lower.sort();
    lower.dedup();
    lower
}

/// Find prose mentions of `target` across all other notes.
pub fn find(index: &VaultIndex, target: &NoteId) -> Vec<Mention> {
    let needles = needles(index, target);
    if needles.is_empty() {
        return Vec::new();
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for (source, note) in &index.notes {
        if source == target {
            continue;
        }
        let mut in_fence = false;
        let mut in_frontmatter = false;
        for (position, raw) in note.content.split_inclusive('\n').enumerate() {
            let line_number = position + 1;
            let line = raw
                .strip_suffix("\r\n")
                .or_else(|| raw.strip_suffix('\n'))
                .unwrap_or(raw);
            let trimmed = line.trim().trim_start_matches('\u{feff}');
            if line_number == 1 && trimmed == "---" {
                in_frontmatter = true;
                continue;
            }
            if in_frontmatter {
                if trimmed == "---" {
                    in_frontmatter = false;
                }
                continue;
            }
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                continue;
            }
            let masked = mask_links(line);
            let lower = masked.to_lowercase();
            for needle in &needles {
                let mut offset = 0;
                while let Some(found) = lower[offset..].find(needle) {
                    let start = offset + found;
                    let end = start + needle.len();
                    if seen.insert((source.clone(), line_number, start)) {
                        result.push(Mention {
                            note: source.clone(),
                            line: line_number,
                            snippet: lower[start..end].to_string(),
                        });
                    }
                    offset = end;
                }
            }
        }
    }
    result.sort_by(|a, b| a.note.cmp(&b.note).then(a.line.cmp(&b.line)));
    result
}
