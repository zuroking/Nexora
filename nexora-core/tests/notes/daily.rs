use nexora_core::daily;
use std::fs;
use tempfile::tempdir;

#[test]
fn creates_with_body_and_never_overwrites() {
    let dir = tempdir().unwrap();
    let id = daily::open_or_create_with(dir.path(), "# {{title}}\n").unwrap();
    let path = dir.path().join(format!("{}.md", id.as_str()));

    assert_eq!(fs::read_to_string(&path).unwrap(), "# {{title}}\n");

    // The caller renders placeholders; core writes the body verbatim.
    daily::open_or_create_with(dir.path(), "REPLACED").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# {{title}}\n");
}

#[test]
fn plain_open_or_create_still_produces_an_empty_note() {
    let dir = tempdir().unwrap();
    let id = daily::open_or_create(dir.path()).unwrap();
    let path = dir.path().join(format!("{}.md", id.as_str()));
    assert_eq!(fs::read_to_string(&path).unwrap(), "");
}
