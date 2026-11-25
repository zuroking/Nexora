use nexora_core::composer;
use nexora_core::vault;
use nexora_core::NoteId;
use std::fs;
use tempfile::tempdir;

#[test]
fn extract_removes_range_and_inserts_link() {
    let source = NoteId::new("source");
    let new_id = NoteId::new("extracted");
    let plan = composer::extract(
        "one\r\ntwo\r\nthree\r\nfour\r\n",
        &source,
        &new_id,
        2,
        3,
    )
    .unwrap();
    assert_eq!(plan.updated_source, "one\r\n[[extracted]]\r\nfour\r\n");
    assert_eq!(plan.new_content, "two\r\nthree\r\n");
}

#[test]
fn extract_rejects_invalid_ranges() {
    let id = NoteId::new("source");
    for (first, last) in [(0, 1), (2, 1), (1, 9)] {
        assert!(composer::extract("one\ntwo\n", &id, &NoteId::new("x"), first, last).is_err());
    }
}

#[test]
fn apply_extract_writes_both_notes() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("source.md"), "alpha\nbeta\ngamma\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let plan = composer::apply_extract(
        dir.path(),
        &index,
        &NoteId::new("source"),
        &NoteId::new("extracted"),
        2,
        2,
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("source.md")).unwrap(),
        "alpha\n[[extracted]]\ngamma\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("extracted.md")).unwrap(),
        plan.new_content
    );
}

#[test]
fn merge_content_appends_provenance() {
    let merged = composer::merge_content("# Target\n", "# Source\nbody\n", &NoteId::new("src"));
    assert!(merged.starts_with("# Target\n"));
    assert!(merged.contains("Merged from src"));
    assert!(merged.ends_with("# Source\nbody\n"));
}

#[test]
fn apply_merge_rewrites_links_and_removes_source() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("source.md"), "# Source\n[[source]] in source\n").unwrap();
    fs::write(dir.path().join("target.md"), "# Target\n[[source]] in target\n").unwrap();
    fs::write(
        dir.path().join("witness.md"),
        "see [[source|the source]] and [[target]]\n",
    )
    .unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let rewritten = composer::apply_merge(
        dir.path(),
        &index,
        &NoteId::new("source"),
        &NoteId::new("target"),
    )
    .unwrap();
    assert_eq!(rewritten.rewritten, 1);
    // Every changed note is returned with its exact bytes for tab refresh (§78).
    let target_id = NoteId::new("target");
    let witness_id = NoteId::new("witness");
    assert!(rewritten.changes.iter().any(|(id, _)| id == &target_id));
    assert!(rewritten.changes.iter().any(|(id, _)| id == &witness_id));
    assert!(!rewritten.changes.iter().any(|(id, _)| id == &NoteId::new("source")));
    let target_bytes = rewritten
        .changes
        .iter()
        .find(|(id, _)| id == &target_id)
        .unwrap()
        .1
        .clone();
    assert_eq!(target_bytes, fs::read_to_string(dir.path().join("target.md")).unwrap());
    assert!(!dir.path().join("source.md").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("witness.md")).unwrap(),
        "see [[target|the source]] and [[target]]\n"
    );
    let target = fs::read_to_string(dir.path().join("target.md")).unwrap();
    assert!(target.contains("Merged from source"));
    assert!(target.contains("[[target]] in target"), "target's own link rewritten");
    assert!(target.contains("[[target]] in source"), "source self-link rewritten");
    assert!(!target.contains("[[source]]"), "no link to the deleted note remains");
    assert!(dir.path().join(".nexora/history/source").exists(), "source snapshotted");
}


#[test]
fn extract_new_note_first_survives_a_source_conflict_and_retries() {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.md");
    fs::write(&source_path, "alpha\nbeta\ngamma\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    // Make the on-disk source diverge from the index anchor so only the
    // source write conflicts (§78: new note must already be on disk).
    fs::write(&source_path, "alpha\nbeta\ngamma\nexternal\n").unwrap();

    let err = composer::apply_extract(
        dir.path(),
        &index,
        &NoteId::new("source"),
        &NoteId::new("extracted"),
        2,
        2,
    )
    .unwrap_err();
    assert!(matches!(err, nexora_core::NexoraError::Conflict { .. }));
    assert_eq!(
        fs::read_to_string(dir.path().join("extracted.md")).unwrap(),
        "beta\n"
    );
    assert_eq!(
        fs::read_to_string(&source_path).unwrap(),
        "alpha\nbeta\ngamma\nexternal\n"
    );

    // A retry against a fresh index succeeds and leaves no duplicate.
    let fresh = vault::build_index(dir.path()).unwrap();
    let plan = composer::apply_extract(
        dir.path(),
        &fresh,
        &NoteId::new("source"),
        &NoteId::new("extracted-retry"),
        2,
        2,
    )
    .unwrap();
    assert_eq!(plan.new_content, "beta\n");
    assert!(fs::read_to_string(dir.path().join("source.md"))
        .unwrap()
        .contains("[[extracted-retry]]"));
}
