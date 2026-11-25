use nexora_core::{canvas, vault, Note, NoteId, VaultIndex};
use std::fs;
use tempfile::tempdir;

fn index_with(notes: &[(&str, &str)]) -> VaultIndex {
    let dir = tempdir().unwrap();
    for (name, content) in notes {
        fs::write(dir.path().join(format!("{name}.md")), content).unwrap();
    }
    let index = vault::build_index(dir.path()).unwrap();
    std::mem::forget(dir);
    index
}

#[test]
fn initial_layout_is_deterministic() {
    let index = index_with(&[("a", "a"), ("b", "b"), ("c", "c"), ("d", "d")]);
    let first = canvas::initial(&index);
    let second = canvas::initial(&index);
    assert_eq!(first, second);
    assert_eq!(first.nodes.len(), 4);
    let a = first.nodes.get(&NoteId::new("a")).unwrap();
    let b = first.nodes.get(&NoteId::new("b")).unwrap();
    assert_eq!(a.y, b.y);
    assert_eq!(b.x - a.x, 260.0);
}

#[test]
fn selection_key_tracks_node_set_not_positions() {
    let index = index_with(&[("a", "a"), ("b", "b")]);
    let first = canvas::initial(&index);
    let mut three = first.clone();
    three.nodes.insert(NoteId::new("c"), canvas::Pos { x: 1.0, y: 1.0 });
    let raw = canvas::selection_key(&three);
    // Under raw `";"` joining, {"a","b","c"} and {"a;b","c"} both render
    // "a;b;c"; Debug quoting must keep them distinct (§76 review).
    let mut collision = three.clone();
    collision.nodes.clear();
    for name in ["a;b", "c"] {
        collision
            .nodes
            .insert(NoteId::new(name), canvas::Pos { x: 0.0, y: 0.0 });
    }
    assert_ne!(raw, canvas::selection_key(&collision));
    let mut moved = first.clone();
    moved.nodes.insert(NoteId::new("a"), canvas::Pos { x: 9.0, y: 7.0 });
    assert_eq!(canvas::selection_key(&first), canvas::selection_key(&moved));

    let mut changed = moved;
    changed.nodes.remove(&NoteId::new("b"));
    assert_ne!(canvas::selection_key(&first), canvas::selection_key(&changed));
}

#[test]
fn reconcile_keeps_known_positions_and_prunes_missing() {
    let mut index = index_with(&[("a", "a"), ("b", "b")]);
    let mut stored = canvas::initial(&index);
    stored.nodes.insert(NoteId::new("a"), canvas::Pos { x: 7.0, y: 9.0 });
    stored.nodes.insert(NoteId::new("gone"), canvas::Pos { x: 1.0, y: 1.0 });

    let reconciled = canvas::reconcile(Some(stored), &index);
    assert_eq!(reconciled.nodes.len(), 2);
    assert_eq!(reconciled.nodes.get(&NoteId::new("a")).unwrap().x, 7.0);
    assert!(!reconciled.nodes.contains_key(&NoteId::new("gone")));

    index.notes.insert(
        NoteId::new("new"),
        Note {
            id: NoteId::new("new"),
            title: "new".to_string(),
            content: String::new(),
            modified: std::time::SystemTime::now(),
        },
    );
    assert_eq!(canvas::reconcile(Some(reconciled), &index).nodes.len(), 3);
}

#[test]
fn edges_are_resolved_and_deduplicated() {
    let index = index_with(&[
        ("a", "[[b]] and [[b|alias]] and [[missing]]"),
        ("b", "[[a]]"),
        ("c", "plain"),
    ]);
    let edges = canvas::edges(&index);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0], (NoteId::new("a"), NoteId::new("b")));
}

#[test]
fn sidecar_roundtrip_and_bad_header() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("canvas.idx");
    let index = index_with(&[("a", "a")]);
    let layout = canvas::initial(&index);

    canvas::save(&path, &layout).unwrap();
    assert_eq!(canvas::load(&path).unwrap(), layout);

    fs::write(&path, "WRONG 1\n").unwrap();
    assert!(canvas::load(&path).is_err());
}
