use nexora_core::templates::{self, Fields, Template};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

/// Sidecar bytes are written by hand: v2 has no template writer (§36.2).
fn write_sidecar(path: &Path, entries: &[Template]) {
    let mut bytes = b"NEXORA-TEMPLATES-v1\n".to_vec();
    for template in entries {
        bytes.extend_from_slice(format!("{}|{}\n", template.name, template.body.len()).as_bytes());
        bytes.extend_from_slice(template.body.as_bytes());
        bytes.push(b'\n');
    }
    fs::write(path, bytes).unwrap();
}

#[test]
fn renders_only_closed_placeholders() {
    let fields = Fields {
        date: "2027-05-12",
        title: "Notes",
    };
    let out = templates::render(
        "{{date}} / {{title}} / {{author}} / {{unclosed",
        &fields,
    );
    assert_eq!(out, "2027-05-12 / Notes / {{author}} / {{unclosed");
}

#[test]
fn substituted_values_are_not_re_expanded() {
    let fields = Fields {
        date: "{{title}}",
        title: "Final",
    };
    assert_eq!(templates::render("{{date}}", &fields), "{{title}}");
}

#[test]
fn builtin_template_renders_date_and_title() {
    let fields = Fields {
        date: "2027-05-12",
        title: "2027-05-12",
    };
    let body = templates::render(&templates::builtin().body, &fields);
    assert!(body.starts_with("# 2027-05-12\n"));
    assert!(body.contains("Created 2027-05-12"));
}

#[test]
fn daily_body_selects_template_and_resolves_both_placeholders() {
    let date = nexora_core::daily::today_id();
    let entries = vec![
        Template {
            name: "other".to_string(),
            body: "first".to_string(),
        },
        Template {
            name: templates::DAILY_TEMPLATE.to_string(),
            body: "# {{title}} / {{date}}".to_string(),
        },
    ];
    assert_eq!(
        templates::daily_body(&entries),
        format!("# {} / {}", date.as_str(), date.as_str())
    );

    let first_only = vec![Template {
        name: "other".to_string(),
        body: "# {{date}}".to_string(),
    }];
    assert_eq!(templates::daily_body(&first_only), format!("# {}", date.as_str()));
    assert_eq!(templates::daily_body(&[]), "");
}

#[test]
fn insert_at_separates_a_mid_line_block_and_tracks_the_caret() {
    let edit = templates::insert_at("# Old\nbody", 5, "# New\nline");
    assert_eq!(edit.text, "# Old\n\n# New\nline\n\nbody");
    assert_eq!(edit.cursor, 18);
}

#[test]
fn insert_at_handles_empty_note_bom_and_crlf() {
    assert_eq!(templates::insert_at("", 0, "# New").text, "# New\n");
    assert_eq!(templates::insert_at("", 0, "# New").cursor, 6);
    let bom = templates::insert_at("\u{feff}", 1, "# New");
    assert_eq!(bom.text, "\u{feff}# New\n");
    assert_eq!(bom.cursor, 7);
    let crlf = templates::insert_at("# Old\r\nbody", 5, "# New\nline");
    assert_eq!(crlf.text, "# Old\r\n\r\n# New\r\nline\r\n\r\nbody");
    assert!(!crlf.text.contains("\r\n\r\n\r\nbody"));
}

#[test]
fn insert_at_appends_to_a_trailing_blank_line_without_excess_gaps() {
    let edit = templates::insert_at("a\n\n", 3, "# New");
    assert_eq!(edit.text, "a\n\n# New\n");
    assert_eq!(edit.cursor, 9);
    let eof = templates::insert_at("a", 1, "# New");
    assert_eq!(eof.text, "a\n\n# New\n\n");

    let before_blank = templates::insert_at("a\n\nb", 1, "# New");
    assert_eq!(before_blank.text, "a\n\n# New\n\nb");
    assert_eq!(before_blank.cursor, 8);
    let crlf_blank = templates::insert_at("a\r\n\r\nb", 1, "# New");
    assert_eq!(crlf_blank.text, "a\r\n\r\n# New\r\n\r\nb");
}

#[test]
fn insert_at_keeps_text_when_body_is_empty() {
    let edit = templates::insert_at("keep me", 3, "");
    assert_eq!(edit.text, "keep me");
    assert_eq!(edit.cursor, 3);
}

#[test]
fn template_management_validates_and_keeps_daily() {
    let base = vec![
        templates::builtin(),
        Template { name: "exam".into(), body: "# {{title}}".into() },
    ];
    let added = templates::upsert(&base, Template { name: " essay ".into(), body: "draft".into() }).unwrap();
    assert_eq!(added.last().unwrap().name, "essay");
    for invalid in ["", "a|b", "a\nb", "a\rb", "a\tb"] {
        assert!(templates::upsert(&base, Template { name: invalid.into(), body: String::new() }).is_err());
    }
    let updated = templates::upsert(
        &base,
        Template { name: "exam".into(), body: "# {{title}}\nNew".into() },
    )
    .unwrap();
    assert_eq!(updated.len(), 2);
    assert_eq!(updated[1].body, "# {{title}}\nNew");

    let renamed = templates::rename(&base, "exam", " test ").unwrap();
    assert_eq!(renamed[1].name, "test");
    assert_eq!(templates::rename(&base, "exam", "daily").is_err(), true);
    assert!(templates::rename(&base, "daily", "other").is_err());
    assert!(templates::rename(&base, "missing", "other").is_err());
    assert!(templates::rename(&base, "exam", "exam").is_ok());

    let daily_edited = templates::upsert(
        &base,
        Template {
            name: templates::DAILY_TEMPLATE.into(),
            body: "# {{title}}\nEdited daily".into(),
        },
    )
    .unwrap();
    assert_eq!(daily_edited.len(), 2);
    assert_eq!(daily_edited[0].name, templates::DAILY_TEMPLATE);
    assert_eq!(daily_edited[0].body, "# {{title}}\nEdited daily");

    let removed = templates::remove(&base, "exam").unwrap();
    assert_eq!(removed.len(), 1);
    assert!(templates::remove(&base, "daily").is_err());
    assert!(templates::remove(&base, "missing").is_err());
}

#[test]
fn save_writes_the_same_bytes_load_reads() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested/templates.idx");
    let entries = vec![
        templates::builtin(),
        Template { name: "ru".into(), body: "# {{title}}\nДата: {{date}}\n".into() },
    ];
    templates::save(&path, &entries).unwrap();
    assert_eq!(templates::load(&path).unwrap(), entries);
    let bytes = fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"NEXORA-TEMPLATES-v1\n"));
    assert!(String::from_utf8_lossy(&bytes).contains("Дата: {{date}}"));
}

#[test]
fn sidecar_roundtrip_and_bad_files() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("templates.idx");

    // Missing sidecar yields the shipped default.
    assert_eq!(templates::load(&path).unwrap(), vec![templates::builtin()]);

    let entries = vec![
        templates::builtin(),
        Template {
            name: "ru".to_string(),
            body: "# {{title}}\nДата: {{date}}\n\n---\n".to_string(),
        },
    ];
    write_sidecar(&path, &entries);
    assert_eq!(templates::load(&path).unwrap(), entries);

    fs::write(&path, "WRONG\n").unwrap();
    assert!(templates::load(&path).is_err(), "bad header");
    fs::write(&path, "NEXORA-TEMPLATES-v1\ndaily|x\nbody\n").unwrap();
    assert!(templates::load(&path).is_err(), "bad length");
    fs::write(&path, "NEXORA-TEMPLATES-v1\nbadheader\n").unwrap();
    assert!(templates::load(&path).is_err(), "bad entry header");
}
