use nexora_core::workspace::{self, WorkspaceTab};
use std::fs;
use tempfile::tempdir;

fn tab(kind: &str, id: &str) -> (String, String) {
    (kind.to_string(), id.to_string())
}

fn wt(kind: &str, id: &str) -> WorkspaceTab {
    WorkspaceTab {
        kind: kind.to_string(),
        id: id.to_string(),
    }
}

#[test]
fn corrupt_huge_tab_count_is_an_error_not_an_allocation() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("workspaces.idx");
    fs::write(
        &path,
        "NEXORA-WORKSPACES-v1\nvault\tname\t0\t18446744073709551615\n",
    )
    .unwrap();
    assert!(workspace::load(&path).is_err());
}

#[test]
fn capture_and_upsert_replace_only_matching_vault_and_name() {
    let ws = workspace::capture(
        "C:/vault-a",
        "writing",
        &[tab("note", "a"), tab("graph", "graph")],
        1,
    );
    assert_eq!(ws.vault, "C:/vault-a");
    assert_eq!(ws.active, 1);

    let other = workspace::capture("C:/vault-b", "writing", &[tab("note", "b")], 0);
    let mut all = vec![ws, other];
    all = workspace::upsert(
        &all,
        workspace::capture("C:/vault-a", "writing", &[tab("note", "new")], 0),
    );
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].tabs, vec![wt("note", "new")]);
    assert_eq!(all[1].tabs, vec![wt("note", "b")]);

    let only_a = workspace::for_vault(&all, "C:/vault-a");
    assert_eq!(only_a.len(), 1);
}

#[test]
fn active_index_is_remapped_after_missing_tabs() {
    // Second tab survived, first did not: saved index 1 must become 0.
    assert_eq!(workspace::remap_active(&[false, true], 1), 0);
    assert_eq!(workspace::remap_active(&[true, true], 1), 1);
    assert_eq!(workspace::remap_active(&[true, false, true], 2), 1);
    // The saved active descriptor was skipped and was the last one: only
    // index 0 exists, so the returned index must be clamped to 0 (§52).
    assert_eq!(workspace::remap_active(&[true, false], 1), 0);
    assert_eq!(workspace::remap_active(&[true, false, false], 2), 0);
    // Boundary guards for the changed clamp (§52 review).
    assert_eq!(workspace::remap_active(&[false, false], 2), 0);
    assert_eq!(workspace::remap_active(&[true], 5), 0);
    assert_eq!(workspace::remap_active(&[true, false, true], 1), 1);
    assert_eq!(workspace::remap_active(&[], 0), 0);
}

#[test]
fn rename_validates_and_preserves_payload() {
    let first = workspace::capture("vault-a", "writing", &[tab("note", "a")], 0);
    let other_vault = workspace::capture("vault-b", "writing", &[tab("note", "b")], 0);
    let collision = workspace::capture("vault-a", "reading", &[tab("note", "c")], 0);
    let all = vec![first.clone(), other_vault, collision];

    let renamed = workspace::rename(&all, "vault-a", "writing", "  draft  ").unwrap();
    assert_eq!(renamed[0].name, "draft");
    assert_eq!(renamed[0].tabs, first.tabs);
    assert_eq!(renamed[0].active, first.active);
    assert_eq!(renamed[1].name, "writing");
    assert_eq!(renamed[2].name, "reading");

    assert_eq!(workspace::rename(&all, "vault-a", "writing", "writing").unwrap(), all);
    assert!(workspace::rename(&all, "vault-a", "writing", "reading").is_err());
    assert!(workspace::rename(&all, "vault-a", "writing", " reading ").is_err());
    assert!(workspace::rename(&all, "vault-a", "missing", "new").is_err());
    for invalid in ["", "  ", "bad\tname", "bad\nname", "bad\rname"] {
        assert!(workspace::rename(&all, "vault-a", "writing", invalid).is_err());
    }
}

#[test]
fn remove_is_scoped_to_vault_and_name() {
    let all = vec![
        workspace::capture("vault-a", "writing", &[tab("note", "a")], 0),
        workspace::capture("vault-b", "writing", &[tab("note", "b")], 0),
        workspace::capture("vault-a", "reading", &[tab("note", "c")], 0),
    ];
    let left = workspace::remove(&all, "vault-a", "writing");
    assert_eq!(left.len(), 2);
    assert_eq!(left[0].vault, "vault-b");
    assert_eq!(left[1].name, "reading");
    assert_eq!(workspace::remove(&all, "vault-a", "missing"), all);
}

#[test]
fn sidecar_roundtrip_and_bad_files() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("workspaces.idx");
    assert!(workspace::load(&path).unwrap().is_empty());

    let entries = vec![workspace::capture(
        "C:/vault-a",
        "writing",
        &[tab("note", "a/b"), tab("search", "")],
        1,
    )];
    workspace::save(&path, &entries).unwrap();
    assert_eq!(workspace::load(&path).unwrap(), entries);

    fs::write(&path, "WRONG\n").unwrap();
    assert!(workspace::load(&path).is_err(), "bad header");
    fs::write(&path, "NEXORA-WORKSPACES-v1\nvault\tname\n").unwrap();
    assert!(workspace::load(&path).is_err(), "bad workspace row");
}
