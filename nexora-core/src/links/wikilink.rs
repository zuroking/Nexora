//! `[[wikilink]]` parsing, path resolution, and backlinks.

use crate::{Link, NexoraError, Note, NoteId, VaultIndex};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawLink {
    pub target: String,
    pub alias: Option<String>,
    /// Heading (`Heading`) or block (`^id`) after the first `#`, if any.
    pub anchor: Option<String>,
}

/// Split a raw target into note id and optional anchor (§38.1). This is the
/// single owner of the `#` grammar; every scanner calls it.
pub fn split_target(raw: &str) -> (String, Option<String>) {
    match raw.split_once('#') {
        Some((note, anchor)) if !anchor.trim().is_empty() => {
            (note.trim().to_string(), Some(anchor.trim().to_string()))
        }
        Some((note, _)) => (note.trim().to_string(), None),
        None => (raw.trim().to_string(), None),
    }
}

/// Parse the contents of one `[[...]]` occurrence. Returns the link and the
/// number of consumed characters in the slice after the opening `[[`.
fn parse_one(slice: &str) -> Option<(RawLink, usize)> {
    let close = slice.find("]]")?;
    let inner = &slice[..close];
    let (target_part, alias) = match inner.find('|') {
        Some(position) => (
            inner[..position].trim(),
            Some(inner[position + 1..].trim().to_string()),
        ),
        None => (inner.trim(), None),
    };
    let (target, anchor) = split_target(target_part);
    if target.is_empty() {
        return None;
    }
    Some((RawLink { target, alias, anchor }, close + 2))
}

/// Extract links line by line. Fenced code blocks are ignored; nested brackets
/// are not supported in v1.
pub fn parse_links(content: &str) -> Vec<RawLink> {
    parse_links_with_lines(content)
        .into_iter()
        .map(|(_, link)| link)
        .collect()
}

/// Same parser as [`parse_links`], but keeps the 1-based source line of each
/// occurrence. Used by the outgoing-links projection; there is only one
/// definition of what counts as a wikilink.
pub fn parse_links_with_lines(content: &str) -> Vec<(usize, RawLink)> {
    let mut links = Vec::new();
    let mut in_fence = false;
    for (line_number, line) in content.lines().enumerate() {
        // One fence definition for the whole crate: rename, mentions,
        // lists, outline and display already treat `~~~` as a fence too.
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let bytes = line.as_bytes();
        let mut index = 0;
        while index + 1 < bytes.len() {
            if bytes[index] == b'[' && bytes[index + 1] == b'[' {
                if let Some((raw, consumed)) = parse_one(&line[index + 2..]) {
                    links.push((line_number + 1, raw));
                    index += 2 + consumed;
                    continue;
                }
            }
            index += 1;
        }
    }
    links
}

/// Resolution rule shared by `build_links` and `resolve_target`: a real
/// `NoteId` always wins over an alias with the same name.
fn resolve_in(
    notes: &BTreeMap<NoteId, Note>,
    aliases: &BTreeMap<String, NoteId>,
    requested: &NoteId,
) -> Option<NoteId> {
    if notes.contains_key(requested) {
        Some(requested.clone())
    } else {
        aliases.get(requested.as_str()).cloned()
    }
}

/// Resolve one raw link target against a whole index.
pub fn resolve_target(index: &VaultIndex, raw: &str) -> Option<NoteId> {
    let requested = NoteId::new(raw);
    resolve_in(&index.notes, &index.aliases, &requested)
}

/// Build resolved links and backlinks. A target is resolved by exact `NoteId`
/// lookup — basename matching is deliberately absent.
pub fn build_links(
    notes: &BTreeMap<NoteId, Note>,
    aliases: &BTreeMap<String, NoteId>,
) -> (
    BTreeMap<NoteId, Vec<Link>>,
    BTreeMap<NoteId, BTreeSet<NoteId>>,
) {
    let mut links = BTreeMap::new();
    let mut backlinks: BTreeMap<NoteId, BTreeSet<NoteId>> = BTreeMap::new();

    for (id, note) in notes {
        let mut note_links = Vec::new();
        for raw in parse_links(&note.content) {
            let requested = NoteId::new(raw.target);
            let resolved = resolve_in(notes, aliases, &requested);
            if let Some(target) = resolved {
                backlinks
                    .entry(target.clone())
                    .or_default()
                    .insert(id.clone());
                note_links.push(Link::Resolved {
                    target,
                    alias: raw.alias,
                    anchor: raw.anchor,
                });
            } else {
                note_links.push(Link::Broken {
                    target: requested,
                    anchor: raw.anchor,
                });
            }
        }
        links.insert(id.clone(), note_links);
    }
    (links, backlinks)
}

/// Broken links are validation errors (locked decision GM-2).
pub fn validate(links: &BTreeMap<NoteId, Vec<Link>>) -> Result<(), Vec<NexoraError>> {
    let mut errors = Vec::new();
    for (id, note_links) in links {
        for link in note_links {
            if let Link::Broken { target, .. } = link {
                errors.push(NexoraError::BrokenLink {
                    note: id.as_str().to_string(),
                    target: target.as_str().to_string(),
                });
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
