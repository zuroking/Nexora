use nexora_core::graph;
use nexora_core::vault;
use std::fs;
use tempfile::tempdir;

fn index_with(notes: &[(&str, &str)]) -> nexora_core::VaultIndex {
    let dir = tempdir().unwrap();
    for (name, content) in notes {
        fs::write(dir.path().join(format!("{name}.md")), content).unwrap();
    }
    let index = vault::build_index(dir.path()).unwrap();
    std::mem::forget(dir);
    index
}

#[test]
fn selection_key_is_stable_and_tracks_the_node_set() {
    let index = index_with(&[("a", "a"), ("b", "b"), ("c", "c")]);
    let layout = graph::build(&index);
    assert_eq!(graph::selection_key(&layout), graph::selection_key(&graph::build(&index)));

    let smaller = index_with(&[("a", "a"), ("b", "b")]);
    assert_ne!(graph::selection_key(&layout), graph::selection_key(&graph::build(&smaller)));
}

#[test]
fn selection_key_does_not_collide_on_separator_ids() {
    let plain = index_with(&[("a", "a"), ("b", "b"), ("c", "c")]);
    let plain_key = graph::selection_key(&graph::build(&plain));
    let combined = index_with(&[("a;b", "a"), ("c", "c")]);
    assert_ne!(plain_key, graph::selection_key(&graph::build(&combined)));
}
