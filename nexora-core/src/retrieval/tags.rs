//! Tag extraction. Per locked decision GM-1 there are no exclusions: any
//! `#word` is a tag, including headings and code blocks.

use crate::{Note, NoteId};
use std::collections::{BTreeMap, BTreeSet};

/// Extract every `#word` occurrence. `#` may be followed by spaces, so both
/// `#тег` and `# Заголовок` produce a tag. Unicode letters are supported.
pub fn extract_tags(content: &str) -> Vec<String> {
    let characters: Vec<char> = content.chars().collect();
    let mut tags = Vec::new();
    let mut index = 0;

    while index < characters.len() {
        if characters[index] == '#' {
            let mut cursor = index + 1;
            while cursor < characters.len() && characters[cursor] == ' ' {
                cursor += 1;
            }
            let start = cursor;
            while cursor < characters.len()
                && (characters[cursor].is_alphanumeric()
                    || characters[cursor] == '_'
                    || characters[cursor] == '-'
                    || characters[cursor] == '/')
            {
                cursor += 1;
            }
            if cursor > start {
                let tag: String = characters[start..cursor].iter().collect();
                let tag = tag.trim_end_matches('/').to_string();
                if tag.is_empty() {
                    index = cursor;
                    continue;
                }
                tags.push(tag);
                index = cursor;
                continue;
            }
        }
        index += 1;
    }
    tags
}

pub fn build_tag_index(notes: &BTreeMap<NoteId, Note>) -> BTreeMap<String, BTreeSet<NoteId>> {
    let mut index: BTreeMap<String, BTreeSet<NoteId>> = BTreeMap::new();
    for (id, note) in notes {
        for tag in extract_tags(&note.content) {
            // Full tag and every parent prefix point at the same note, so a
            // flat filter for `school` still matches `#school/math`.
            let components: Vec<&str> = tag.split('/').collect();
            for depth in 1..=components.len() {
                let key = components[..depth].join("/");
                index.entry(key).or_default().insert(id.clone());
            }
        }
    }
    index
}
