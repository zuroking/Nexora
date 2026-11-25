use nexora_core::mentions;
use nexora_core::vault;
use nexora_core::NoteId;
use std::fs;
use tempfile::tempdir;

#[test]
fn bom_frontmatter_aliases_are_not_prose_mentions() {
    let dir = index_with(&[
        ("target", "# Target\n"),
        ("source", "\u{feff}---\naliases: [Target]\n---\n# Source\n"),
    ]);
    let index = vault::build_index(dir.path()).unwrap();
    assert!(mentions::find(&index, &NoteId::new("target")).is_empty());
}

fn index_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    for (name, content) in files {
        fs::write(dir.path().join(format!("{name}.md")), content).unwrap();
    }
    dir
}

#[test]
fn finds_prose_mention_with_line_number() {
    let dir = index_with(&[
        ("target", "# Algebra\n"),
        ("source", "first line\nAlgebra is hard\n"),
    ]);
    let index = vault::build_index(dir.path()).unwrap();
    let found = mentions::find(&index, &NoteId::new("target"));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].note, NoteId::new("source"));
    assert_eq!(found[0].line, 2);
    assert_eq!(found[0].snippet, "algebra");
}

#[test]
fn ignores_wikilinks_frontmatter_and_fences() {
    let dir = index_with(&[
        ("target", "# Algebra\n"),
        (
            "source",
            "---\naliases: [Algebra]\n---\n[[Algebra]]\n```\nAlgebra\n```\nAlgebra\n",
        ),
    ]);
    let index = vault::build_index(dir.path()).unwrap();
    let found = mentions::find(&index, &NoteId::new("target"));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 8);
}

#[test]
fn aliases_are_mentioned_too_and_case_does_not_matter() {
    let dir = index_with(&[
        ("target", "---\naliases: [Alg]\n---\n# Algebra\n"),
        ("a", "see ALG here\n"),
        ("b", "see algebra here\n"),
    ]);
    let index = vault::build_index(dir.path()).unwrap();
    let found = mentions::find(&index, &NoteId::new("target"));
    assert_eq!(found.len(), 2);
    assert!(found.iter().any(|m| m.note == NoteId::new("a")));
    assert!(found.iter().any(|m| m.note == NoteId::new("b")));
}

#[test]
fn target_never_mentions_itself() {
    let dir = index_with(&[("target", "# Algebra\nAlgebra\n")]);
    let index = vault::build_index(dir.path()).unwrap();
    assert!(mentions::find(&index, &NoteId::new("target")).is_empty());
}
