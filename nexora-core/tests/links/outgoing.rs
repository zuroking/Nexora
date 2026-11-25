use nexora_core::outgoing;
use nexora_core::vault;
use nexora_core::NoteId;
use std::fs;
use tempfile::tempdir;

#[test]
fn lists_resolved_broken_and_alias_links_with_lines() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("source.md"), "first\n[[target|alias]] [[gone]]\n```\n[[fenced]]\n```\n").unwrap();
    fs::write(dir.path().join("target.md"), "# Target\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let rows = outgoing::for_note(&index, &NoteId::new("source"));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].target, NoteId::new("target"));
    assert_eq!(rows[0].alias.as_deref(), Some("alias"));
    assert!(rows[0].resolved);
    assert_eq!(rows[0].line, 2);
    assert_eq!(rows[1].target, NoteId::new("gone"));
    assert!(!rows[1].resolved);
}

#[test]
fn aliases_are_resolved() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("source.md"), "[[Alg]]").unwrap();
    fs::write(dir.path().join("target.md"), "---\naliases: [Alg]\n---\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let rows = outgoing::for_note(&index, &NoteId::new("source"));
    assert_eq!(rows.len(), 1);
    assert!(rows[0].resolved);
    assert_eq!(rows[0].target, NoteId::new("target"));
}

#[test]
fn real_note_id_wins_over_alias_with_same_name() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("source.md"), "[[real]]").unwrap();
    fs::write(dir.path().join("real.md"), "# Real\n").unwrap();
    fs::write(dir.path().join("other.md"), "---\naliases: [real]\n---\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let rows = outgoing::for_note(&index, &NoteId::new("source"));
    assert_eq!(rows.len(), 1);
    assert!(rows[0].resolved);
    assert_eq!(rows[0].target, NoteId::new("real"));
}

#[test]
fn empty_target_is_not_a_link() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("source.md"), "[[]] [[|alias]] [[ok]]").unwrap();
    fs::write(dir.path().join("ok.md"), "# Ok\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let rows = outgoing::for_note(&index, &NoteId::new("source"));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].target, NoteId::new("ok"));
}

#[test]
fn missing_note_has_no_outgoing_links() {
    let dir = tempdir().unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    assert!(outgoing::for_note(&index, &NoteId::new("gone")).is_empty());
}
