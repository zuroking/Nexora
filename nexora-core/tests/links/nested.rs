use nexora_core::{tags, vault, Link, NoteId};
use std::fs;
use tempfile::tempdir;

#[test]
fn nested_tag_registers_full_path_and_parents() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("a.md"), "#school/math/algebra").unwrap();
    fs::write(root.join("b.md"), "#school").unwrap();
    let index = vault::build_index(root).unwrap();
    let a = NoteId::new("a");
    let b = NoteId::new("b");

    for key in ["school/math/algebra", "school/math", "school"] {
        assert!(index.tags.get(key).is_some_and(|set| set.contains(&a)));
    }
    assert!(index.tags.get("school").is_some_and(|set| set.contains(&b)));
    assert!(!index
        .tags
        .get("school/math")
        .is_some_and(|set| set.contains(&b)));
}

#[test]
fn aliases_resolve_wikilinks_and_backlinks() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("target.md"), "---\naliases: [Alg, Linear]\n---\n# Algebra\n").unwrap();
    fs::write(root.join("source.md"), "see [[Alg]] and [[Linear|алгебра]]").unwrap();
    let index = vault::build_index(root).unwrap();

    assert_eq!(
        index.aliases.get("Alg"),
        Some(&NoteId::new("target"))
    );
    let links = index.links.get(&NoteId::new("source")).unwrap();
    assert_eq!(links.len(), 2);
    for link in links {
        match link {
            Link::Resolved { target, .. } => assert_eq!(target, &NoteId::new("target")),
            other => panic!("alias stayed unresolved: {other:?}"),
        }
    }
    assert!(index
        .backlinks
        .get(&NoteId::new("target"))
        .is_some_and(|set| set.contains(&NoteId::new("source"))));
}

#[test]
fn real_note_id_beats_alias_with_same_name() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("real.md"), "# Real\n").unwrap();
    fs::write(root.join("other.md"), "---\naliases: [real]\n---\n").unwrap();
    fs::write(root.join("source.md"), "[[real]]").unwrap();
    let index = vault::build_index(root).unwrap();
    let link = &index.links.get(&NoteId::new("source")).unwrap()[0];
    match link {
        Link::Resolved { target, .. } => assert_eq!(target, &NoteId::new("real")),
        other => panic!("unexpected link: {other:?}"),
    }
}

#[test]
fn extract_tags_ignores_trailing_slash() {
    assert_eq!(tags::extract_tags("#school/ #a/b"), vec!["school", "a/b"]);
}
