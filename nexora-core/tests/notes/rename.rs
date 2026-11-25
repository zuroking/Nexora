use nexora_core::{rename, save, vault, wikilink, NoteId, NexoraError};
use std::fs;
use tempfile::tempdir;

#[test]
fn tilde_fences_agree_between_parser_and_rewriter() {
    let content = "~~~\n[[old]]\n```\n";
    assert!(wikilink::parse_links_with_lines(content).is_empty());
    let (rewritten, count) =
        rename::rewrite_links(content, &NoteId::new("old"), &NoteId::new("new"));
    assert_eq!(count, 0);
    assert_eq!(rewritten, content);
}

#[test]
fn plan_rewrites_exact_targets_and_keeps_aliases() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("a.md"), "[[b]] and [[b|alias]] and [[other]]\n").unwrap();
    fs::write(root.join("b.md"), "# B").unwrap();
    fs::write(root.join("other.md"), "# Other").unwrap();

    let plan = rename::plan(root, &NoteId::new("b"), "folder/c").unwrap();
    assert_eq!(plan.edits.len(), 1);
    assert_eq!(plan.edits[0].replacements, 2);
    assert_eq!(
        plan.edits[0].content,
        "[[folder/c]] and [[folder/c|alias]] and [[other]]\n"
    );
}

#[test]
fn plan_ignores_fenced_code_and_rejects_bad_targets() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("a.md"), "```\n[[b]]\n```\n[[b]]").unwrap();
    fs::write(root.join("b.md"), "# B").unwrap();

    let plan = rename::plan(root, &NoteId::new("b"), "c").unwrap();
    assert_eq!(plan.edits.len(), 1);
    assert_eq!(plan.edits[0].replacements, 1);

    assert!(matches!(
        rename::plan(root, &NoteId::new("b"), "../escape"),
        Err(NexoraError::InvalidInput { .. })
    ));
    assert!(matches!(
        rename::plan(root, &NoteId::new("b"), "b"),
        Err(NexoraError::InvalidInput { .. })
    ));
    assert!(matches!(
        rename::plan(root, &NoteId::new("b"), "a"),
        Err(NexoraError::InvalidInput { .. })
    ));
}

#[test]
fn apply_moves_file_rewrites_links_and_keeps_history() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("a.md"), "see [[b]]").unwrap();
    fs::write(root.join("b.md"), "# B").unwrap();

    let plan = rename::plan(root, &NoteId::new("b"), "folder/c").unwrap();
    rename::apply(root, &plan).unwrap();

    assert!(!root.join("b.md").exists());
    assert!(root.join("folder").join("c.md").exists());
    let a = fs::read_to_string(root.join("a.md")).unwrap();
    assert!(a.contains("[[folder/c]]"));
    assert!(!save::list_snapshots(root, &NoteId::new("folder/c"))
        .unwrap()
        .is_empty());

    let index = vault::build_index(root).unwrap();
    assert!(index.notes.contains_key(&NoteId::new("folder/c")));
    assert!(index.backlinks.get(&NoteId::new("folder/c")).is_some());
}

#[test]
fn crlf_line_endings_survive_rewriting() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("a.md"), "first\r\nsee [[b]]\r\nlast\r\n").unwrap();
    fs::write(root.join("b.md"), "# B").unwrap();

    let plan = rename::plan(root, &NoteId::new("b"), "c").unwrap();
    rename::apply(root, &plan).unwrap();
    let bytes = fs::read(root.join("a.md")).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("[[c]]\r\n"));
    assert!(!text.contains('\n') || text.replace("\r\n", "").contains('\n') == false);
}

#[test]
fn dry_run_does_not_write() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("a.md"), "[[b]]").unwrap();
    fs::write(root.join("b.md"), "# B").unwrap();
    let before = fs::read_to_string(root.join("a.md")).unwrap();

    let _ = rename::plan(root, &NoteId::new("b"), "c").unwrap();
    assert_eq!(fs::read_to_string(root.join("a.md")).unwrap(), before);
    assert!(root.join("b.md").exists());
}
