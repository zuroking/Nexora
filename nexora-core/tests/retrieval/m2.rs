//! M2 gate: wikilinks, tags, search, daily notes, inbox, health.

use nexora_core::health::Severity;
use nexora_core::wikilink::RawLink;
use nexora_core::{daily, health, inbox, search, tags, vault, wikilink, Link, NexoraError, NoteId};
use std::fs;
use std::path::Path;

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

fn id(raw: &str) -> NoteId {
    NoteId::new(raw)
}

#[test]
fn parse_links_handles_alias_and_ignores_fenced_code() {
    let content = "# T\n\n[[Note]] and [[dir/Other|Alias]].\n\n```\n[[Fenced]]\n```\n";
    let links = wikilink::parse_links(content);
    assert_eq!(links.len(), 2);
    assert_eq!(
        links[0],
        RawLink {
            target: "Note".to_string(),
            alias: None,
            anchor: None,
        }
    );
    assert_eq!(
        links[1],
        RawLink {
            target: "dir/Other".to_string(),
            alias: Some("Alias".to_string()),
            anchor: None,
        }
    );
}

#[test]
fn links_resolve_by_path_and_backlinks_are_derived() {
    let fixture = vault_fixture();
    put(fixture.path(), "A.md", "[[B]] [[Missing]]\n");
    put(fixture.path(), "B.md", "[[A|first]]\n");

    let index = vault::build_index(fixture.path()).expect("index");
    assert_eq!(index.links[&id("A")].len(), 2);
    assert!(matches!(index.links[&id("A")][0], Link::Resolved { .. }));
    assert!(matches!(index.links[&id("A")][1], Link::Broken { .. }));
    assert!(index.backlinks[&id("B")].contains(&id("A")));

    let errors = wikilink::validate(&index.links).expect_err("broken link");
    assert!(matches!(
        &errors[0],
        NexoraError::BrokenLink { note, target } if note == "A" && target == "Missing"
    ));
}

#[test]
fn any_hash_word_is_a_tag_including_headings_and_code() {
    let content = "# Заголовок\n## Два\n\n#тег и #work/project\n\n```\n#вкоде\n```\n";
    assert_eq!(
        tags::extract_tags(content),
        vec!["Заголовок", "Два", "тег", "work/project", "вкоде"]
    );
}

#[test]
fn tag_index_maps_tags_to_notes() {
    let fixture = vault_fixture();
    put(fixture.path(), "A.md", "#one #shared\n");
    put(fixture.path(), "B.md", "#shared\n");
    let index = vault::build_index(fixture.path()).expect("index");
    assert_eq!(index.tags["shared"].len(), 2);
    assert_eq!(index.tags["one"].len(), 1);
}

#[test]
fn search_ranks_title_exact_then_title_then_body() {
    let fixture = vault_fixture();
    put(fixture.path(), "Exact.md", "# Exact\n");
    put(fixture.path(), "Partial.md", "# Exact thing\nbody needle\n");
    put(fixture.path(), "Body.md", "# Other\nneedle here\n");
    let index = vault::build_index(fixture.path()).expect("index");

    let by_title = search::search(&index, "exact");
    assert_eq!(by_title[0].id, id("Exact"));
    assert_eq!(by_title[0].rank, 0);
    assert_eq!(by_title[1].id, id("Partial"));
    assert_eq!(by_title[1].rank, 1);

    let by_body = search::search(&index, "needle");
    assert_eq!(by_body[0].id, id("Body"));
    assert_eq!(by_body[0].rank, 2);
    assert!(search::search(&index, "  ").is_empty());
}

#[test]
fn daily_note_id_is_a_date_and_open_or_create_creates_the_file() {
    let fixture = vault_fixture();
    let expected = daily::today_id();
    assert_eq!(expected.as_str().len(), 10);
    assert!(expected
        .as_str()
        .chars()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()));

    let created = daily::open_or_create(fixture.path()).expect("daily note");
    assert_eq!(created, expected);
    assert!(fixture.path().join(format!("{}.md", created)).exists());
}

#[test]
fn quick_capture_appends_timestamped_lines() {
    let fixture = vault_fixture();
    inbox::capture(fixture.path(), "first thought").expect("capture");
    inbox::capture(fixture.path(), "second").expect("capture");

    let notes = vault::reload(fixture.path()).expect("reload");
    let content = &notes[&id("Inbox")].content;
    assert_eq!(content.lines().count(), 2);
    assert!(content.lines().all(|line| line.starts_with('[') && line.contains("] ")));
    assert!(content.contains("first thought"));
    assert!(matches!(
        inbox::capture(fixture.path(), "   "),
        Err(NexoraError::InvalidInput { .. })
    ));
}

#[test]
fn health_classifies_broken_links_orphans_and_duplicates() {
    let fixture = vault_fixture();
    put(fixture.path(), "A.md", "[[Missing]]\n");
    put(fixture.path(), "B.md", "# B\n");
    put(fixture.path(), "x/A.md", "# A\n");
    put(fixture.path(), "2026-01-01.md", "# daily\n");

    let index = vault::build_index(fixture.path()).expect("index");
    let issues = health::check(&index);

    assert!(issues.iter().any(|issue| issue.severity == Severity::Error
        && issue.target.as_deref() == Some("Missing")
        && issue.line == Some(1)));
    assert!(issues.iter().any(|issue| issue.severity == Severity::Warning
        && issue.note.as_ref().is_some_and(|note| note.as_str() == "A")));
    assert!(!issues.iter().any(|issue| {
        issue
            .note
            .as_ref()
            .is_some_and(|note| note.as_str() == "2026-01-01")
    }));
    assert_eq!(
        issues
            .iter()
            .filter(|issue| issue.severity == Severity::Info)
            .count(),
        2
    );
}
