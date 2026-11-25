use nexora_core::watch::{Change, DiskMonitor};
use nexora_core::NoteId;
use std::fs;
use tempfile::tempdir;

#[test]
fn detects_external_modify_create_and_delete() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("a.md"), "# A\nold\n").unwrap();
    let mut monitor = DiskMonitor::new(root);
    assert!(monitor.refresh().unwrap().is_empty());

    fs::write(root.join("a.md"), "# A\nnew\n").unwrap();
    fs::write(root.join("b.md"), "# B\n").unwrap();
    let mut changes = monitor.refresh().unwrap();
    changes.sort_by_key(|change| format!("{change:?}"));
    assert!(changes.contains(&Change::Modified(NoteId::new("a"))));
    assert!(changes.contains(&Change::Created(NoteId::new("b"))));
    assert!(monitor.refresh().unwrap().is_empty());

    fs::remove_file(root.join("b.md")).unwrap();
    assert_eq!(
        monitor.refresh().unwrap(),
        vec![Change::Deleted(NoteId::new("b"))]
    );
}

#[test]
fn startup_fingerprint_error_is_surfaced_once() {
    // §PROD-P3: an unreadable vault must not be treated as an empty one.
    // reload() creates a missing root, so use a file where a directory is
    // required — that fails deterministically on every platform.
    let dir = tempdir().unwrap();
    let blocker = dir.path().join("not-a-dir");
    fs::write(&blocker, "x").unwrap();
    let mut monitor = DiskMonitor::new(&blocker);
    assert!(monitor.take_startup_error().is_some());
    assert!(monitor.take_startup_error().is_none());

    let mut healthy = DiskMonitor::new(dir.path());
    assert!(healthy.take_startup_error().is_none());
}

#[test]
fn resync_swallows_our_own_write() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("note.md"), "one").unwrap();
    let mut monitor = DiskMonitor::new(root);

    fs::write(root.join("note.md"), "two").unwrap();
    monitor.resync();
    assert!(monitor.refresh().unwrap().is_empty());
}

#[test]
fn unchanged_content_with_new_mtime_is_still_a_change() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let note = root.join("note.md");
    fs::write(&note, "same").unwrap();
    let mut monitor = DiskMonitor::new(root);

    // Rewriting the same bytes updates mtime; the monitor reports it so a
    // clean tab reloads and a dirty tab can raise a conflict.
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&note, "same").unwrap();
    assert_eq!(
        monitor.refresh().unwrap(),
        vec![Change::Modified(NoteId::new("note"))]
    );
}
