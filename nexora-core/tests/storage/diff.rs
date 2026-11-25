use nexora_core::diff::{diff, DiffKind};
use nexora_core::{save, NoteId};
use std::fs;
use tempfile::tempdir;

#[test]
fn diff_marks_added_removed_and_equal_rows() {
    let rows = diff("a\nb\nc\n", "a\nx\nc\nd\n");
    let kinds: Vec<_> = rows.iter().map(|row| row.kind).collect();
    assert_eq!(kinds, vec![
        DiffKind::Equal,
        DiffKind::Removed,
        DiffKind::Added,
        DiffKind::Equal,
        DiffKind::Added,
    ]);
    assert_eq!(rows[0].old_line, Some(1));
    assert_eq!(rows[0].new_line, Some(1));
    assert_eq!(rows[1].text, "b");
    assert_eq!(rows[3].text, "c");
    assert_eq!(rows[4].new_line, Some(4));
}

#[test]
fn identical_and_empty_inputs() {
    assert!(diff("same\n", "same\n")
        .iter()
        .all(|row| row.kind == DiffKind::Equal));
    assert!(diff("", "").is_empty());
    let rows = diff("", "new\n");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, DiffKind::Added);
    let rows = diff("old\n", "");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, DiffKind::Removed);
}

#[test]
fn huge_input_uses_bounded_fallback() {
    let old = (0..1200).map(|i| format!("line {i}\n")).collect::<String>();
    let mut new = old.clone();
    new.push_str("extra\n");
    let rows = diff(&old, &new);
    assert_eq!(rows.len(), 1201);
    assert_eq!(rows.last().unwrap().kind, DiffKind::Added);
}

#[test]
fn read_snapshot_returns_saved_bytes() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let id = NoteId::new("note");
    fs::write(root.join("note.md"), "first").unwrap();
    save::write_note(root, &id, "second", "first").unwrap();
    let stamps = save::list_snapshots(root, &id).unwrap();
    assert_eq!(stamps.len(), 1);
    assert_eq!(save::read_snapshot(root, &id, &stamps[0]).unwrap(), "first");
}
