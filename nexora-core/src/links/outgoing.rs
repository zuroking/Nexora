//! Outgoing-link projection for the Context panel. Pure, no cache.
//!
//! It reuses `wikilink` parsing and resolution so the Context panel can never
//! disagree with backlinks or health validation. See `ARCHITECTURE.md` §34.

use crate::{wikilink, NoteId, VaultIndex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outgoing {
    pub target: NoteId,
    pub alias: Option<String>,
    pub anchor: Option<String>,
    pub resolved: bool,
    pub line: usize,
}

/// Every `[[target]]` / `[[target|alias]]` of `id`, with source line numbers.
pub fn for_note(index: &VaultIndex, id: &NoteId) -> Vec<Outgoing> {
    let Some(note) = index.notes.get(id) else {
        return Vec::new();
    };
    wikilink::parse_links_with_lines(&note.content)
        .into_iter()
        .map(|(line, raw)| {
            let resolved = wikilink::resolve_target(index, &raw.target);
            Outgoing {
                target: resolved
                    .clone()
                    .unwrap_or_else(|| NoteId::new(&raw.target)),
                alias: raw.alias,
                anchor: raw.anchor,
                resolved: resolved.is_some(),
                line,
            }
        })
        .collect()
}
