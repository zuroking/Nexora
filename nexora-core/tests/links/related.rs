use nexora_core::related::suggest;
use nexora_core::semantic::{build as build_semantic, SemanticIndex};
use nexora_core::{Link, Note, NoteId, VaultIndex};
use std::collections::{BTreeMap, BTreeSet};
use std::time::SystemTime;

fn note(id: &str, content: &str) -> (NoteId, Note) {
    let id = NoteId::new(id);
    (
        id.clone(),
        Note {
            id,
            title: content.to_string(),
            content: content.to_string(),
            modified: SystemTime::now(),
        },
    )
}

fn fixture() -> (VaultIndex, SemanticIndex) {
    let mut notes = BTreeMap::new();
    for (id, content) in [
        ("a", "algebra equations exam"),
        ("b", "algebra equations revision"),
        ("c", "unrelated cooking recipe"),
    ] {
        let (id, value) = note(id, content);
        notes.insert(id, value);
    }
    let mut links: BTreeMap<NoteId, Vec<Link>> = BTreeMap::new();
    links.insert(
        NoteId::new("a"),
        vec![Link::Resolved {
            target: NoteId::new("c"),
            alias: None,
            anchor: None,
        }],
    );
    let mut backlinks: BTreeMap<NoteId, BTreeSet<NoteId>> = BTreeMap::new();
    backlinks.insert(NoteId::new("c"), BTreeSet::from([NoteId::new("a")]));
    let mut tags: BTreeMap<String, BTreeSet<NoteId>> = BTreeMap::new();
    tags.insert(
        "math".to_string(),
        BTreeSet::from([NoteId::new("a"), NoteId::new("b")]),
    );
    tags.insert("home".to_string(), BTreeSet::from([NoteId::new("c")]));
    let index = VaultIndex {
        notes,
        backlinks,
        links,
        tags,
        ..VaultIndex::default()
    };
    let semantic = build_semantic(&index, 384);
    (index, semantic)
}

#[test]
fn ranks_shared_tags_and_text_above_distant_notes() {
    let (index, semantic) = fixture();
    let rows = suggest(&index, &semantic, &NoteId::new("a"), 3);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id.as_str(), "b");
    assert!(rows[0].factors.tag > 0.0);
    assert!(rows[0].factors.semantic > 0.0);
    assert_eq!(rows[1].id.as_str(), "c");
    assert!(rows[1].factors.graph > 0.0);
    assert!(rows[0].score > rows[1].score);
}

#[test]
fn excludes_target_and_honours_limit() {
    let (index, semantic) = fixture();
    let rows = suggest(&index, &semantic, &NoteId::new("a"), 1);
    assert_eq!(rows.len(), 1);
    assert_ne!(rows[0].id.as_str(), "a");
    assert!(suggest(&index, &semantic, &NoteId::new("a"), 0).is_empty());
    assert!(suggest(&index, &semantic, &NoteId::new("missing"), 5).is_empty());
}

#[test]
fn empty_vector_index_still_uses_tags_and_graph() {
    let (index, _) = fixture();
    let empty = SemanticIndex {
        dims: 384,
        fingerprint: 0,
        entries: Vec::new(),
    };
    let rows = suggest(&index, &empty, &NoteId::new("a"), 5);
    let tags_only = rows.iter().find(|row| row.id.as_str() == "b").unwrap();
    assert_eq!(tags_only.factors.semantic, 0.0);
    assert!(tags_only.factors.tag > 0.0);
}

#[test]
fn ranking_is_deterministic() {
    let (index, semantic) = fixture();
    let first = suggest(&index, &semantic, &NoteId::new("a"), 5);
    let second = suggest(&index, &semantic, &NoteId::new("a"), 5);
    assert_eq!(first, second);
}
