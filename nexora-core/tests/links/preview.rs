use nexora_core::preview;
use nexora_core::vault;
use nexora_core::NoteId;
use std::fs;
use tempfile::tempdir;

#[test]
fn preview_skips_frontmatter_and_blank_lines() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("note.md"),
        "---\ncreated: 2026-01-01\n---\n\n# Title\nfirst\n\nsecond\nthird\n",
    )
    .unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let preview = preview::for_note(&index, &NoteId::new("note"), 2).unwrap();
    assert_eq!(preview.title, "Title");
    assert_eq!(preview.excerpt, "# Title\nfirst");
}

#[test]
fn preview_of_missing_note_is_none() {
    let dir = tempdir().unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    assert!(preview::for_note(&index, &NoteId::new("gone"), 5).is_none());
}
