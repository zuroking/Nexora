use nexora_core::tree::{build, NodeKind};
use nexora_core::NoteId;

fn ids(values: &[&str]) -> Vec<NoteId> {
    values.iter().map(|value| NoteId::new(*value)).collect()
}

#[test]
fn folder_and_note_with_same_path_have_distinct_ids() {
    let tree = build(ids(&["foo", "foo/bar"]));
    let mut keys = Vec::new();
    fn collect(nodes: &[nexora_core::tree::TreeNode], keys: &mut Vec<String>) {
        for node in nodes {
            keys.push(node.id_key());
            collect(&node.children, keys);
        }
    }
    collect(&tree, &mut keys);
    let mut unique = keys.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(keys.len(), unique.len());
}

#[test]
fn nested_paths_become_folders() {
    let tree = build(ids(&["school/math/algebra", "school/history"]));
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].name, "school");
    assert_eq!(tree[0].kind, NodeKind::Folder);
    assert_eq!(tree[0].children.len(), 2);
    // Folders precede notes, so math (folder) comes before history (note).
    assert_eq!(tree[0].children[0].name, "math");
    assert_eq!(tree[0].children[1].name, "history");
    assert_eq!(tree[0].children[0].children[0].path, "school/math/algebra");
    assert!(matches!(tree[0].children[1].kind, NodeKind::Note(_)));
}

#[test]
fn folders_come_before_notes_and_sort_alphabetically() {
    let tree = build(ids(&["z.md", "a.md", "b/note", "a/inside"]));
    let names: Vec<_> = tree.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "a.md", "z.md"]);
}

#[test]
fn duplicate_basenames_stay_distinct() {
    let tree = build(ids(&["work/notes", "personal/notes"]));
    let first = &tree[0].children[0];
    let second = &tree[1].children[0];
    assert_eq!(first.path, "personal/notes");
    assert_eq!(second.path, "work/notes");
    assert_ne!(first.kind, second.kind);
}

#[test]
fn folder_and_note_may_share_a_name() {
    let tree = build(ids(&["foo", "foo/bar"]));
    assert_eq!(tree.len(), 2);
    assert_eq!(tree[0].name, "foo");
    assert_eq!(tree[0].kind, NodeKind::Folder);
    assert_eq!(tree[1].name, "foo");
    assert!(matches!(tree[1].kind, NodeKind::Note(_)));
}

#[test]
fn empty_input_produces_empty_tree() {
    assert!(build(ids(&[])).is_empty());
}
