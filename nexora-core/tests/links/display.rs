use nexora_core::display::{adapt_for_display, decode_target, encode_target};
use nexora_core::{Note, NoteId, VaultIndex};
use std::collections::BTreeMap;
use std::time::SystemTime;

fn index_with(ids: &[&str]) -> VaultIndex {
    let mut notes = BTreeMap::new();
    for id in ids {
        let id = NoteId::new(*id);
        notes.insert(
            id.clone(),
            Note {
                title: id.stem().to_string(),
                id,
                content: String::new(),
                modified: SystemTime::now(),
            },
        );
    }
    VaultIndex {
        notes,
        ..VaultIndex::default()
    }
}

#[test]
fn wikilink_becomes_markdown_link_with_alias() {
    let index = index_with(&["folder/target"]);
    let adapted = adapt_for_display("See [[folder/target|the target]] now", &index);
    assert_eq!(
        adapted.trim_end(),
        "See [the target](nexora://folder/target) now"
    );
}

#[test]
fn broken_wikilink_is_marked_missing() {
    let index = index_with(&[]);
    let adapted = adapt_for_display("[[gone]]", &index);
    assert_eq!(adapted.trim_end(), "[gone (missing)](nexora://gone)");
}

#[test]
fn highlights_and_callouts_are_adapted() {
    let index = index_with(&[]);
    let adapted = adapt_for_display("==важно==\n> [!note] Заголовок", &index);
    assert!(adapted.contains("**важно**"));
    assert!(adapted.contains("> **NOTE** — Заголовок"));
}

#[test]
fn fenced_code_is_untouched() {
    let index = index_with(&["target"]);
    let source = "```md\n[[target]] and ==x==\n```\n[[target]]";
    let adapted = adapt_for_display(source, &index);
    assert!(adapted.contains("```md\n[[target]] and ==x==\n```"));
    assert!(adapted.contains("[target](nexora://target)"));
}

#[test]
fn alias_link_renders_as_resolved() {
    let mut index = index_with(&["target"]);
    index.aliases.insert("Alg".to_string(), NoteId::new("target"));
    let adapted = adapt_for_display("see [[Alg]]", &index);
    assert_eq!(adapted.trim_end(), "see [Alg](nexora://target)");
}

#[test]
fn target_encoding_roundtrips_unicode() {
    let id = NoteId::new("школа/математика 9В");
    let encoded = encode_target(&id);
    assert!(!encoded.contains(' '));
    assert_eq!(decode_target(&encoded), id.as_str());
}
