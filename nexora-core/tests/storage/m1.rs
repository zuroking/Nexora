//! M1 gate: model + vault + save + history.

use nexora_core::{vault, NexoraError, NoteId};
use std::fs;
use std::path::Path;

#[test]
fn note_id_rejects_windows_drive_prefixes() {
    // `C:/secret` must never escape the vault through PathBuf::push (§3/§9).
    assert!(!NoteId::new("C:/secret").is_safe());
    assert!(matches!(
        vault::note_path(Path::new("C:/vault"), &NoteId::new("C:/secret")),
        Err(NexoraError::InvalidId { .. })
    ));
}

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
fn default_vault_root_uses_documents_nexora_my_first_vault() {
    let root = vault::default_vault_root();
    assert!(root.ends_with(Path::new("Documents/Nexora/my-first-vault")));
}

#[test]
fn note_id_from_path_is_canonical_and_skips_hidden() {
    let root = Path::new("C:/vault");
    assert_eq!(
        vault::note_id_from_path(root, &root.join("dir/Note.MD")),
        Some(NoteId::new("dir/Note"))
    );
    assert_eq!(
        vault::note_id_from_path(root, &root.join(".nexora/history/x.md")),
        None
    );
    assert_eq!(
        vault::note_id_from_path(root, &root.join("image.png")),
        None
    );
}

#[test]
fn note_path_roundtrip_and_traversal_rejection() {
    let root = Path::new("C:/vault");
    let id = NoteId::new("Projects/Nexora");
    let path = vault::note_path(root, &id).expect("safe id");
    assert_eq!(vault::note_id_from_path(root, &path), Some(id));
    assert!(matches!(
        vault::note_path(root, &NoteId::new("../escape")),
        Err(NexoraError::InvalidId { .. })
    ));
}

#[test]
fn note_path_preserves_interior_dots_and_reload_finds_them() {
    // §PROD-P1: `set_extension` used to turn id `v2.0` (disk file `v2.0.md`)
    // into `v2.md`; the fix appends `.md` verbatim, so vaulted notes whose
    // stems contain dots are reachable again.
    let fixture = vault_fixture();
    put(fixture.path(), "v2.0.md", "# Version 2\n");
    put(fixture.path(), "sub/dir.name.md", "# Nested\n");
    assert_eq!(
        vault::note_path(fixture.path(), &NoteId::new("v2.0")).expect("safe id"),
        fixture.path().join("v2.0.md")
    );
    let notes = vault::reload(fixture.path()).expect("reload");
    assert_eq!(
        notes.get(&NoteId::new("v2.0")).map(|note| note.content.as_str()),
        Some("# Version 2\n")
    );
    assert!(notes.contains_key(&NoteId::new("sub/dir.name")));
}

#[test]
fn reload_discovers_nested_notes_and_extracts_title() {
    let fixture = vault_fixture();
    put(
        fixture.path(),
        "Projects/Nexora.md",
        "# Nexora\n\nNotes app.\n",
    );
    put(fixture.path(), "Inbox.md", "plain text\n");
    put(fixture.path(), ".hidden/secret.md", "not a note\n");
    put(fixture.path(), "readme.txt", "not a note\n");

    let notes = vault::reload(fixture.path()).expect("reload");
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[&NoteId::new("Projects/Nexora")].title, "Nexora");
    assert_eq!(notes[&NoteId::new("Inbox")].title, "Inbox");
}

#[test]
fn invalid_utf8_is_a_typed_error() {
    let fixture = vault_fixture();
    fs::write(fixture.path().join("bad.md"), [0xff, 0xfe]).expect("bytes");
    assert!(matches!(
        vault::reload(fixture.path()),
        Err(NexoraError::InvalidUtf8 { .. })
    ));
}

#[test]
fn write_note_creates_parent_folders_for_nested_ids() {
    let fixture = vault_fixture();
    let root = fixture.path();
    let id = NoteId::new("folder/missing");

    nexora_core::save::write_note(root, &id, "created", "").expect("create nested");
    let path = root.join("folder/missing.md");
    assert!(path.is_file(), "nested note file must exist");
    let notes = vault::reload(root).expect("reload");
    assert_eq!(notes[&id].content, "created");
}

#[test]
fn write_note_creates_updates_and_snapshots_old_content() {
    let fixture = vault_fixture();
    let id = NoteId::new("Note");
    let root = fixture.path();

    nexora_core::save::write_note(root, &id, "first", "").expect("create");
    let notes = vault::reload(root).expect("reload");
    assert_eq!(notes[&id].content, "first");

    nexora_core::save::write_note(root, &id, "second", "first").expect("update");
    let snapshots = nexora_core::save::list_snapshots(root, &id).expect("snapshots");
    assert_eq!(snapshots.len(), 1);
    let old = fs::read_to_string(
        root.join(".nexora/history/Note").join(&snapshots[0]),
    )
    .expect("snapshot content");
    assert_eq!(old, "first");
}

#[test]
fn snapshot_retention_is_ten_per_note() {
    let fixture = vault_fixture();
    let root = fixture.path();
    let id = NoteId::new("Note");
    nexora_core::save::write_note(root, &id, "v0", "").expect("create");

    for version in 1..=12 {
        let previous = format!("v{}", version - 1);
        let next = format!("v{}", version);
        nexora_core::save::write_note(root, &id, &next, &previous).expect("update");
    }

    let snapshots = nexora_core::save::list_snapshots(root, &id).expect("snapshots");
    assert_eq!(snapshots.len(), 10);
}

#[test]
fn restore_snapshot_reverts_content() {
    let fixture = vault_fixture();
    let root = fixture.path();
    let id = NoteId::new("Note");
    nexora_core::save::write_note(root, &id, "original", "").expect("create");
    nexora_core::save::write_note(root, &id, "edited", "original").expect("update");
    let snapshots = nexora_core::save::list_snapshots(root, &id).expect("snapshots");

    let restored = nexora_core::save::restore_snapshot(root, &id, &snapshots[0]).expect("restore");
    assert_eq!(restored, "original");
    let notes = vault::reload(root).expect("reload");
    assert_eq!(notes[&id].content, restored);
}

#[test]
fn restore_snapshot_is_byte_exact_for_bom_crlf_and_utf8() {
    let fixture = vault_fixture();
    let root = fixture.path();
    let id = NoteId::new("Multibyte");
    let original = "\u{feff}---\r\ntitle: Тест\r\n---\r\n# Algebra\r\n\nПривет, мир!\r\n";
    nexora_core::save::write_note(root, &id, original, "").expect("create");
    nexora_core::save::write_note(root, &id, "edited", original).expect("update");
    let snapshots = nexora_core::save::list_snapshots(root, &id).expect("snapshots");

    let restored = nexora_core::save::restore_snapshot(root, &id, &snapshots[0]).expect("restore");
    assert_eq!(restored, original);
    let notes = vault::reload(root).expect("reload");
    assert_eq!(notes[&id].content, original);
}

#[test]
fn bom_before_h1_does_not_hide_the_title() {
    let fixture = vault_fixture();
    put(
        fixture.path(),
        "Bom.md",
        "\u{feff}# Тайтл\n\nbody\n",
    );
    let notes = vault::reload(fixture.path()).expect("reload");
    assert_eq!(notes[&NoteId::new("Bom")].title, "Тайтл");
}

#[test]
fn stale_loaded_content_is_a_conflict_not_an_overwrite() {
    let fixture = vault_fixture();
    let root = fixture.path();
    let id = NoteId::new("Note");
    nexora_core::save::write_note(root, &id, "disk", "").expect("create");

    let error = nexora_core::save::write_note(root, &id, "stale editor", "old").unwrap_err();
    assert!(matches!(error, NexoraError::Conflict { .. }));

    let notes = vault::reload(root).expect("reload");
    assert_eq!(notes[&id].content, "disk");
}

#[test]
fn identical_write_creates_no_snapshot() {
    let fixture = vault_fixture();
    let root = fixture.path();
    let id = NoteId::new("Note");
    nexora_core::save::write_note(root, &id, "same", "").expect("create");
    nexora_core::save::write_note(root, &id, "same", "same").expect("noop");
    assert!(nexora_core::save::list_snapshots(root, &id)
        .expect("snapshots")
        .is_empty());
}


#[test]
fn a_locked_old_snapshot_does_not_block_note_writes() {
    use std::fs;

    let fixture = vault_fixture();
    let root = fixture.path();
    let id = NoteId::new("Note");
    nexora_core::save::write_note(root, &id, "v0", "").expect("create");
    for version in 1..=10 {
        let previous = format!("v{}", version - 1);
        let next = format!("v{}", version);
        nexora_core::save::write_note(root, &id, &next, &previous).expect("update");
    }
    let oldest = nexora_core::save::list_snapshots(root, &id)
        .expect("snapshots")
        .last()
        .cloned()
        .expect("oldest stamp");
    let path = root.join(".nexora/history/Note").join(&oldest);
    let mut perms = fs::metadata(&path).expect("meta").permissions();
    perms.set_readonly(true);
    fs::set_permissions(&path, perms.clone()).expect("readonly");

    // Retention cleanup cannot delete the locked file, but the note must save.
    nexora_core::save::write_note(root, &id, "v11", "v10").expect("write despite prune");
    let disk = fs::read_to_string(root.join("Note.md")).expect("disk");
    assert_eq!(disk, "v11");

    // The file may already have been removed; cleanup is not the assertion.
    perms.set_readonly(false);
    let _ = fs::set_permissions(&path, perms);
}
