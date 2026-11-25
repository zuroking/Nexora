//! M3 gate: deterministic graph layout.

use nexora_core::{graph, vault, NoteId};
use std::fs;
use std::path::Path;

fn vault_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    vault::ensure_vault(dir.path()).expect("vault root");
    dir
}

fn put(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir");
    }
    fs::write(path, content).expect("write note");
}

#[test]
fn layout_is_deterministic() {
    let fixture = vault_fixture();
    put(fixture.path(), "A.md", "[[B]]\n");
    put(fixture.path(), "B.md", "[[A]]\n");
    put(fixture.path(), "C.md", "# C\n");
    let index = vault::build_index(fixture.path()).expect("index");

    assert_eq!(graph::build(&index), graph::build(&index));
}

#[test]
fn edges_come_from_resolved_links_and_are_deduplicated() {
    let fixture = vault_fixture();
    put(fixture.path(), "A.md", "[[B]] [[B]] [[Missing]]\n");
    put(fixture.path(), "B.md", "[[A]]\n");
    let index = vault::build_index(fixture.path()).expect("index");
    let layout = graph::build(&index);

    assert_eq!(layout.nodes.len(), 2);
    assert_eq!(layout.edges.len(), 1);
    assert_eq!(layout.edges[0].source, NoteId::new("A"));
    assert_eq!(layout.edges[0].target, NoteId::new("B"));
    assert!(!layout.truncated);
}

#[test]
fn large_vault_keeps_highest_degree_notes() {
    let fixture = vault_fixture();
    let root = fixture.path();
    let mut hub = "# Hub\n".to_string();
    for n in 0..501 {
        hub.push_str(&format!("[[N{}]]\n", n));
    }
    put(root, "Hub.md", &hub);
    for n in 0..501 {
        put(root, &format!("N{}.md", n), "# node\n");
    }

    let index = vault::build_index(root).expect("index");
    let layout = graph::build(&index);
    assert!(layout.truncated);
    assert_eq!(layout.nodes.len(), 500);
    assert!(layout
        .nodes
        .iter()
        .any(|node| node.id == NoteId::new("Hub")));
}

#[test]
fn empty_vault_produces_empty_layout() {
    let fixture = vault_fixture();
    let index = vault::build_index(fixture.path()).expect("index");
    let layout = graph::build(&index);
    assert!(layout.nodes.is_empty());
    assert!(layout.edges.is_empty());
    assert!(!layout.truncated);
}
