use nexora_core::bookmarks::{self, Bookmarks};
use nexora_core::vault;
use nexora_core::NoteId;
use std::fs;
use tempfile::tempdir;

#[test]
fn toggle_preserves_order_and_removes() {
    let mut bookmarks = Bookmarks::default();
    let a = NoteId::new("a");
    let b = NoteId::new("b");
    assert!(bookmarks.toggle(&a));
    assert!(bookmarks.toggle(&b));
    assert!(!bookmarks.toggle(&a));
    assert_eq!(bookmarks.ids, vec![b]);
}

#[test]
fn remove_and_move_preserve_the_list() {
    let mut bookmarks = Bookmarks::default();
    for name in ["a", "b", "c"] {
        bookmarks.toggle(&NoteId::new(name));
    }
    let b = NoteId::new("b");
    assert!(bookmarks.move_by(&b, -1));
    assert_eq!(bookmarks.ids, vec![NoteId::new("b"), NoteId::new("a"), NoteId::new("c")]);
    assert!(!bookmarks.move_by(&b, -5));
    assert!(bookmarks.move_by(&b, 5));
    assert_eq!(bookmarks.ids.last(), Some(&b));
    assert!(!bookmarks.move_by(&NoteId::new("missing"), 1));
    assert!(bookmarks.remove(&b));
    assert!(!bookmarks.remove(&b));
    assert_eq!(bookmarks.ids, vec![NoteId::new("a"), NoteId::new("c")]);
}

#[test]
fn sidecar_roundtrip_and_bad_header() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bookmarks.idx");
    let mut bookmarks = Bookmarks::default();
    bookmarks.toggle(&NoteId::new("a"));
    bookmarks.toggle(&NoteId::new("folder/b"));
    bookmarks::save(&path, &bookmarks).unwrap();
    assert_eq!(bookmarks::load(&path).unwrap(), bookmarks);

    fs::write(&path, "WRONG\n").unwrap();
    assert!(bookmarks::load(&path).is_err());
}

#[test]
fn prune_and_rename_update_ids() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "# A\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let mut bookmarks = Bookmarks::default();
    bookmarks.toggle(&NoteId::new("a"));
    bookmarks.toggle(&NoteId::new("gone"));
    bookmarks.prune(&index);
    assert_eq!(bookmarks.ids, vec![NoteId::new("a")]);

    bookmarks.rename(&NoteId::new("a"), &NoteId::new("folder/a"));
    assert_eq!(bookmarks.ids, vec![NoteId::new("folder/a")]);
}
