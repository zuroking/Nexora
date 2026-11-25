use nexora_core::saved_queries::{self, SavedQuery};

#[test]
fn selection_key_is_stable_and_tracks_changes() {
    let entries = vec![
        SavedQuery { name: "math".into(), text: "algebra".into() },
        SavedQuery { name: "physics".into(), text: "forces".into() },
    ];
    assert_eq!(saved_queries::selection_key(&entries), saved_queries::selection_key(&entries.clone()));

    let mut renamed = entries.clone();
    renamed[0].name = "geometry".into();
    assert_ne!(saved_queries::selection_key(&entries), saved_queries::selection_key(&renamed));

    let mut changed_text = entries.clone();
    changed_text[1].text = "motion".into();
    assert_ne!(saved_queries::selection_key(&entries), saved_queries::selection_key(&changed_text));

    let shorter = saved_queries::remove(&entries, "physics");
    assert_ne!(saved_queries::selection_key(&entries), saved_queries::selection_key(&shorter));
    assert_eq!(saved_queries::selection_key(&[]), "");
}

#[test]
fn selection_key_cannot_be_forged_by_delimiters() {
    // Under a naive `name|text;` join these two lists collide.
    let first = vec![SavedQuery { name: "a".into(), text: "b;c|d".into() }];
    let second = vec![SavedQuery { name: "a|b;c".into(), text: "d".into() }];
    assert_ne!(saved_queries::selection_key(&first), saved_queries::selection_key(&second));
}

use nexora_core::search::SearchResult;
use nexora_core::vault;
use nexora_core::NoteId;
use std::fs;
use tempfile::tempdir;

#[test]
fn upsert_replaces_in_place_and_keeps_order() {
    let entries = vec![
        SavedQuery { name: "math".into(), text: "algebra".into() },
        SavedQuery { name: "physics".into(), text: "forces".into() },
    ];
    let updated = saved_queries::upsert(&entries, "math", "geometry");
    assert_eq!(updated.len(), 2);
    assert_eq!(updated[0].text, "geometry");
    assert_eq!(updated[1].name, "physics");

    let added = saved_queries::upsert(&updated, "history", "dates");
    assert_eq!(added.len(), 3);
    assert_eq!(added[2].name, "history");
}

#[test]
fn sidecar_roundtrip_and_bad_files() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("saved_queries.idx");
    assert!(saved_queries::load(&path).unwrap().is_empty());

    let entries = vec![
        SavedQuery { name: "school".into(), text: "path:school AND NOT done".into() },
        SavedQuery { name: "ru".into(), text: "[author:Zuro] OR \"线性代数\"".into() },
    ];
    saved_queries::save(&path, &entries).unwrap();
    assert_eq!(saved_queries::load(&path).unwrap(), entries);

    fs::write(&path, "WRONG\n").unwrap();
    assert!(saved_queries::load(&path).is_err(), "bad header");
    fs::write(&path, "NEXORA-SAVED-QUERIES-v1\nno-tab\n").unwrap();
    assert!(saved_queries::load(&path).is_err(), "bad row");
}

#[test]
fn run_returns_the_named_query_results() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "# A\nalgebra\n").unwrap();
    fs::write(dir.path().join("b.md"), "# B\ngeometry\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let entries = vec![
        SavedQuery { name: "algebra".into(), text: "algebra".into() },
        SavedQuery { name: "empty".into(), text: "".into() },
    ];

    let results = saved_queries::run(&index, &entries, "algebra").unwrap();
    assert_eq!(results, vec![SearchResult { id: NoteId::new("a"), title: "A".into(), rank: 0 }]);
    assert!(saved_queries::run(&index, &entries, "empty").unwrap().is_empty());
    assert!(saved_queries::run(&index, &entries, "missing").is_err());

    let broken = vec![SavedQuery { name: "bad".into(), text: "\"unterminated".into() }];
    assert!(
        saved_queries::run(&index, &broken, "bad").is_err(),
        "malformed stored query must be a visible error, not an empty result"
    );
}

#[test]
fn rename_and_remove_preserve_order_and_text() {
    let entries = vec![
        SavedQuery { name: "math".into(), text: "algebra".into() },
        SavedQuery { name: "physics".into(), text: "forces".into() },
    ];
    let renamed = saved_queries::rename(&entries, "math", "  geometry  ").unwrap();
    assert_eq!(renamed[0].name, "geometry");
    assert_eq!(renamed[0].text, "algebra");
    assert_eq!(renamed[1].name, "physics");

    let same = saved_queries::rename(&entries, "math", "math").unwrap();
    assert_eq!(same, entries);
    let removed = saved_queries::remove(&entries, "math");
    assert_eq!(removed, vec![SavedQuery { name: "physics".into(), text: "forces".into() }]);
    assert_eq!(saved_queries::remove(&entries, "missing").len(), 2);
}

#[test]
fn rename_rejects_invalid_and_colliding_names() {
    let entries = vec![
        SavedQuery { name: "math".into(), text: "algebra".into() },
        SavedQuery { name: "physics".into(), text: "forces".into() },
    ];
    for name in ["", "  ", "bad\nname", "bad\tname", "physics"] {
        assert!(saved_queries::rename(&entries, "math", name).is_err(), "{name:?} accepted");
    }
    assert!(saved_queries::rename(&entries, "missing", "other").is_err());
}

#[test]
fn rename_and_remove_survive_sidecar_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("saved_queries.idx");
    let entries = vec![SavedQuery { name: "math".into(), text: "algebra".into() }];
    saved_queries::save(&path, &entries).unwrap();
    let renamed = saved_queries::rename(&entries, "math", "geometry").unwrap();
    saved_queries::save(&path, &renamed).unwrap();
    assert_eq!(saved_queries::load(&path).unwrap(), renamed);
    let removed = saved_queries::remove(&renamed, "geometry");
    saved_queries::save(&path, &removed).unwrap();
    assert!(saved_queries::load(&path).unwrap().is_empty());
}
