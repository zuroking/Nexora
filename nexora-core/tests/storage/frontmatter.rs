use nexora_core::frontmatter::{read_field, read_list, write_field, write_list};

#[test]
fn creates_frontmatter_when_absent() {
    let updated = write_field("# Note\nbody\n", "created", "2026-11-22");
    assert!(updated.starts_with("---\ncreated: \"2026-11-22\"\n---\n"));
    assert!(updated.ends_with("# Note\nbody\n"));
    assert_eq!(read_field(&updated, "created").unwrap(), "2026-11-22");
}

#[test]
fn unknown_lines_and_crlf_survive_edits() {
    let source = "---\r\n# comment\r\ncreated: \"old\"\r\ncustom: keep: me\r\n---\r\nbody\r\n";
    let updated = write_field(source, "created", "new");
    assert!(updated.contains("# comment\r\n"));
    assert!(updated.contains("custom: keep: me\r\n"));
    assert!(updated.contains("created: \"new\"\r\n"));
    assert!(updated.ends_with("body\r\n"));
}

#[test]
fn reads_inline_and_block_lists() {
    let inline = "---\ntags: [school, \"math 9в\"]\n---\n";
    assert_eq!(
        read_list(inline, "tags"),
        vec!["school".to_string(), "math 9в".to_string()]
    );
    let block = "---\naliases:\n  - Alg\n  - Algebra\n---\n";
    assert_eq!(
        read_list(block, "aliases"),
        vec!["Alg".to_string(), "Algebra".to_string()]
    );
}

#[test]
fn writes_inline_list_and_keeps_other_keys() {
    let source = "---\ncreated: \"x\"\naliases: [old]\ncustom: yes\n---\nbody\n";
    let updated = write_list(source, "aliases", &["New".to_string(), "Другой".to_string()]);
    assert!(updated.contains("aliases: [\"New\", \"Другой\"]\n"));
    assert!(updated.contains("created: \"x\"\n"));
    assert!(updated.contains("custom: yes\n"));
    assert_eq!(read_list(&updated, "aliases").len(), 2);
}

#[test]
fn bom_is_preserved_and_not_duplicated() {
    let source = "\u{feff}---\ncreated: \"old\"\ncustom: keep\n---\nbody\n";
    let updated = write_field(source, "created", "new");
    assert!(updated.starts_with('\u{feff}'));
    assert_eq!(updated.matches("---").count(), 2);
    assert!(updated.contains("custom: keep\n"));
    assert_eq!(read_field(&updated, "created").unwrap(), "new");
}

#[test]
fn empty_fields_are_not_inserted() {
    let source = "# Note\nbody\n";
    assert_eq!(write_field(source, "created", ""), source);
    assert_eq!(write_list(source, "tags", &[]), source);
}

#[test]
fn missing_field_returns_none() {
    assert_eq!(read_field("---\ncreated: x\n---\n", "modified"), None);
    assert!(read_list("no frontmatter", "tags").is_empty());
}
