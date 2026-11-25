use nexora_core::display::{adapt_for_display, decode_target, encode_component};
use nexora_core::outline::line_for_anchor;
use nexora_core::rename;
use nexora_core::vault;
use nexora_core::{wikilink, NoteId};
use std::fs;
use tempfile::tempdir;

#[test]
fn decode_target_keeps_invalid_multibyte_escapes() {
    assert_eq!(decode_target("%€"), "%€");
    assert_eq!(decode_target("a%Жb"), "a%Жb");
    assert_eq!(decode_target("%41"), "A");
}

#[test]
fn parser_splits_heading_and_block_anchors() {
    let links = wikilink::parse_links("[[Note#Heading]] [[Other#^blk|alias]] [[Plain]] [[Empty#]]");
    assert_eq!(links.len(), 4);
    assert_eq!(links[0].target, "Note");
    assert_eq!(links[0].anchor.as_deref(), Some("Heading"));
    assert_eq!(links[1].target, "Other");
    assert_eq!(links[1].anchor.as_deref(), Some("^blk"));
    assert_eq!(links[1].alias.as_deref(), Some("alias"));
    assert_eq!(links[2].anchor, None);
    assert_eq!(links[3].anchor, None);
}

#[test]
fn links_keep_the_anchor_for_resolved_and_broken_targets() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("source.md"),
        "[[target#Heading]] [[gone#^blk]]",
    )
    .unwrap();
    fs::write(dir.path().join("target.md"), "# Heading\n\ntext ^blk\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let links = &index.links[&NoteId::new("source")];
    assert_eq!(links.len(), 2);
    match &links[0] {
        nexora_core::Link::Resolved { target, anchor, .. } => {
            assert_eq!(target, &NoteId::new("target"));
            assert_eq!(anchor.as_deref(), Some("Heading"));
        }
        other => panic!("expected resolved link, got {other:?}"),
    }
    match &links[1] {
        nexora_core::Link::Broken { target, anchor } => {
            assert_eq!(target, &NoteId::new("gone"));
            assert_eq!(anchor.as_deref(), Some("^blk"));
        }
        other => panic!("expected broken link, got {other:?}"),
    }
}

#[test]
fn rename_rewrites_note_part_and_preserves_anchor() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("source.md"), "[[old#Heading|label]]").unwrap();
    fs::write(dir.path().join("old.md"), "# Old\n").unwrap();
    let plan = rename::plan(dir.path(), &NoteId::new("old"), "new").unwrap();
    let edited = plan
        .edits
        .iter()
        .find(|edit| edit.id == NoteId::new("source"))
        .unwrap();
    assert_eq!(edited.content, "[[new#Heading|label]]");
    assert_eq!(edited.replacements, 1);
}

#[test]
fn unknown_anchor_still_resolves_and_alias_keeps_its_hash() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("source.md"),
        "[[target#Missing]] [[target|alias#part]]",
    )
    .unwrap();
    fs::write(dir.path().join("target.md"), "# Heading\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let links = &index.links[&NoteId::new("source")];
    assert_eq!(links.len(), 2);
    match &links[0] {
        nexora_core::Link::Resolved { anchor, .. } => {
            assert_eq!(anchor.as_deref(), Some("Missing"));
        }
        other => panic!("expected resolved link, got {other:?}"),
    }
    match &links[1] {
        nexora_core::Link::Resolved { alias, anchor, .. } => {
            assert_eq!(alias.as_deref(), Some("alias#part"));
            assert_eq!(anchor, &None, "a hash inside an alias is not an anchor");
        }
        other => panic!("expected resolved link, got {other:?}"),
    }
    assert!(nexora_core::health::check(&index)
        .iter()
        .all(|issue| issue.severity != nexora_core::health::Severity::Error));
}

#[test]
fn broken_anchor_link_keeps_its_source_line() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("source.md"), "first\n[[gone#Heading]]\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let issues = nexora_core::health::check(&index);
    let error = issues
        .iter()
        .find(|issue| issue.target.as_deref() == Some("gone"))
        .expect("broken anchored link must be reported");
    assert_eq!(error.line, Some(2));
}

#[test]
fn rendered_link_carries_the_anchor() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("source.md"), "[[target#My Heading|go]]").unwrap();
    fs::write(dir.path().join("target.md"), "# My Heading\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let note = &index.notes[&NoteId::new("source")];
    let rendered = adapt_for_display(&note.content, &index);
    let encoded = encode_component("My Heading");
    assert!(
        rendered.contains(&format!("nexora://target#{encoded}")),
        "raw: {rendered}"
    );
    assert_eq!(decode_target("target%23X"), "target#X");
}

#[test]
fn anchor_resolves_to_heading_or_block_line() {
    let content = "# First\n\nbody\n\n## Target Heading\n\nmore ^blk1\n";
    assert_eq!(line_for_anchor(content, "Target Heading"), Some(5));
    assert_eq!(line_for_anchor(content, "^blk1"), Some(7));
    assert_eq!(line_for_anchor(content, "missing"), None);
}
