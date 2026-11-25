use nexora_core::bases::{self, BaseConfig, SortBy};
use nexora_core::frontmatter::{read_field, read_list};
use nexora_core::vault;
use std::fs::{self, File};
use std::time::{Duration, SystemTime};
use tempfile::tempdir;

fn fixture() -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("b.md"),
        "---\ncreated: 2026-01-02\ntags: [school]\n---\n# Bravo\n#school/math\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("a.md"),
        "---\ncreated: 2026-01-01\ntags: [home, school]\n---\n# Alpha\n#school\n",
    )
    .unwrap();
    fs::write(dir.path().join("c.md"), "# Charlie\n").unwrap();
    dir
}

#[test]
fn text_filter_matches_title_path_and_values_case_insensitively() {
    let dir = fixture();
    let index = vault::build_index(dir.path()).unwrap();
    let rows_for = |text: &str| {
        bases::build(
            &index,
            &BaseConfig {
                filter_text: text.to_string(),
                ..BaseConfig::default()
            },
        )
        .into_iter()
        .map(|row| row.id.as_str().to_string())
        .collect::<Vec<_>>()
    };
    assert_eq!(rows_for("ALPHA"), ["a"]);
    assert_eq!(rows_for("  ALPHA  "), ["a"]);
    assert_eq!(rows_for("b"), ["b"]);
    assert_eq!(rows_for("SCHOOL"), ["a", "b"]);
    assert!(rows_for("zzz").is_empty());
    assert_eq!(bases::build(&index, &BaseConfig::default()).len(), 3);
}

#[test]
fn text_filter_matches_path_when_title_and_values_do_not() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("pathonly.md"),
        "---\nsubject: Physics\n---\n# Zed\n",
    )
    .unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let rows = bases::build(
        &index,
        &BaseConfig {
            filter_text: "pathonly".to_string(),
            ..BaseConfig::default()
        },
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id.as_str(), "pathonly");
}

#[test]
fn sidecar_text_filter_is_backward_compatible() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("filter.idx");
    fs::write(&path, "NEXORA-BASE-v1\n\ntitle\n\n\n\nasc\n").unwrap();
    assert!(bases::load(&path).unwrap().filter_text.is_empty());
    fs::write(&path, "NEXORA-BASE-v1\n\ntitle\n\n\n\nasc\nphysics, 2026\n").unwrap();
    assert_eq!(bases::load(&path).unwrap().filter_text, "physics, 2026");
}

#[test]
fn tag_filter_uses_index_and_nested_parents() {
    let dir = fixture();
    let index = vault::build_index(dir.path()).unwrap();
    let config = BaseConfig {
        filter_tag: "school".to_string(),
        sort_by: SortBy::Title,
        group_by: String::new(),
        ..BaseConfig::default()
    };
    let rows = bases::build(&index, &config);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.title == "Alpha" || row.title == "Bravo"));
}

#[test]
fn sorting_by_created_and_path_is_deterministic() {
    let dir = fixture();
    let index = vault::build_index(dir.path()).unwrap();
    let created = bases::build(
        &index,
        &BaseConfig {
            sort_by: SortBy::Created,
            ..BaseConfig::default()
        },
    );
    assert_eq!(created[0].title, "Alpha");
    assert_eq!(created[1].title, "Bravo");
    let paths = bases::build(
        &index,
        &BaseConfig {
            sort_by: SortBy::Path,
            ..BaseConfig::default()
        },
    );
    assert_eq!(paths[0].id.as_str(), "a");
}

#[test]
fn group_by_list_expands_rows_without_changing_ids() {
    let dir = fixture();
    let index = vault::build_index(dir.path()).unwrap();
    let rows = bases::build(
        &index,
        &BaseConfig {
            group_by: "tags".to_string(),
            ..BaseConfig::default()
        },
    );
    let alpha: Vec<_> = rows.iter().filter(|row| row.id.as_str() == "a").collect();
    assert_eq!(alpha.len(), 2);
    assert!(alpha.iter().any(|row| row.group == "home"));
    assert!(alpha.iter().any(|row| row.group == "school"));
}

#[test]
fn group_by_keeps_notes_without_the_field() {
    let dir = fixture();
    let index = vault::build_index(dir.path()).unwrap();
    for group in ["subject", "tags", " subject "] {
        let rows = bases::build(
            &index,
            &BaseConfig {
                group_by: group.to_string(),
                ..BaseConfig::default()
            },
        );
        let expected = if group.trim() == "tags" { 4 } else { 3 };
        assert_eq!(rows.len(), expected, "group_by {group:?} dropped notes");
        let ids: Vec<_> = rows.iter().map(|row| row.id.as_str()).collect();
        assert!(ids.contains(&"a") && ids.contains(&"b") && ids.contains(&"c"));
    }
}

#[test]
fn group_key_is_projected_into_row_values() {
    let dir = fixture();
    let index = vault::build_index(dir.path()).unwrap();
    let rows = bases::build(
        &index,
        &BaseConfig {
            group_by: "subject".to_string(),
            ..BaseConfig::default()
        },
    );
    assert!(rows.iter().all(|row| row.values.contains_key("subject")));
}

#[test]
fn edit_cell_preserves_unknown_keys_bom_and_crlf() {
    let original = "\u{feff}---\r\ncreated: 2026-01-01\r\nzebra: keep me\r\ntags:\r\n  - school\r\n---\r\n# Body\r\nline two\r\n";
    let updated = bases::edit_cell(original, "created", "2027-05-01");
    assert_eq!(read_field(&updated, "created").as_deref(), Some("2027-05-01"));
    assert_eq!(read_field(&updated, "zebra").as_deref(), Some("keep me"));
    assert_eq!(read_list(&updated, "tags"), vec!["school".to_string()]);
    assert!(updated.starts_with('\u{feff}'));
    assert!(updated.contains("\r\n"));
    assert!(updated.ends_with("# Body\r\nline two\r\n"));
    assert_eq!(bases::edit_cell(original, "created", "2026-01-01"), original);
}

#[test]
fn edit_cell_writes_lists_and_group_key() {
    let mut content = "# Plain\n".to_string();
    content = bases::edit_cell(&content, "tags", "school, exam, school,  ");
    assert_eq!(
        read_list(&content, "tags"),
        vec!["school".to_string(), "exam".to_string(), "school".to_string()]
    );
    content = bases::edit_cell(&content, "aliases", "Алгебра, algebra");
    assert_eq!(
        read_list(&content, "aliases"),
        vec!["Алгебра".to_string(), "algebra".to_string()]
    );
    content = bases::edit_cell(&content, "subject", "math");
    assert_eq!(read_field(&content, "subject").as_deref(), Some("math"));
    content = bases::edit_cell(&content, "tags", "");
    assert_eq!(read_list(&content, "tags"), Vec::<String>::new());
}

#[test]
fn editable_columns_excludes_modified_and_dedupes() {
    let config = BaseConfig {
        group_by: "tags".to_string(),
        ..BaseConfig::default()
    };
    assert_eq!(bases::editable_columns(&config), vec!["created", "tags", "aliases"]);
    let config = BaseConfig {
        group_by: "modified".to_string(),
        ..BaseConfig::default()
    };
    assert_eq!(bases::editable_columns(&config), vec!["created", "tags", "aliases"]);
    let config = BaseConfig {
        group_by: "subject".to_string(),
        ..BaseConfig::default()
    };
    assert_eq!(
        bases::editable_columns(&config),
        vec!["created", "tags", "aliases", "subject"]
    );
}

#[test]
fn sidecar_roundtrip_and_bad_header() {
    let dir = fixture();
    let path = dir.path().join("base.idx");
    let config = BaseConfig {
        filter_tag: "school/math".to_string(),
        sort_by: SortBy::Modified,
        group_by: "created".to_string(),
        extra_columns: vec!["subject".to_string()],
        sort_field: "teacher".to_string(),
        sort_desc: true,
        filter_text: "school".to_string(),
    };
    bases::save(&path, &config).unwrap();
    assert_eq!(bases::load(&path).unwrap(), config);

    fs::write(&path, "WRONG\n").unwrap();
    assert!(bases::load(&path).is_err());
}

#[test]
fn custom_columns_validate_and_dedupe() {
    let mut config = BaseConfig::default();
    assert!(bases::add_column(&mut config, " subject "));
    assert!(!bases::add_column(&mut config, "subject"));
    for invalid in ["", "bad,key", "bad\nkey", "created", "tags", "aliases", "modified"] {
        assert!(!bases::add_column(&mut config, invalid), "{invalid:?} accepted");
    }
    assert!(!bases::add_column(&mut config, "bad\rkey"));
    assert_eq!(config.extra_columns, vec!["subject".to_string()]);
    assert!(bases::add_column(&mut config, "teacher"));
    config.group_by = "subject".to_string();
    assert_eq!(
        bases::editable_columns(&config),
        vec!["created", "tags", "aliases", "subject", "teacher"]
    );
    assert!(bases::remove_column(&mut config, "subject"));
    assert!(bases::remove_column(&mut config, "teacher"));
    assert!(!bases::remove_column(&mut config, "subject"));
    assert!(config.extra_columns.is_empty());
}

#[test]
fn legacy_sidecar_without_extra_columns_loads() {
    let dir = fixture();
    let path = dir.path().join("legacy.idx");
    fs::write(&path, "NEXORA-BASE-v1\nschool\ntitle\nsubject\n").unwrap();
    let config = bases::load(&path).unwrap();
    assert!(config.extra_columns.is_empty());
    assert_eq!(config.filter_tag, "school");
    assert_eq!(config.group_by, "subject");
}

#[test]
fn sidecar_drops_invalid_extra_columns_on_load() {
    let dir = fixture();
    let path = dir.path().join("invalid.idx");
    fs::write(
        &path,
        "NEXORA-BASE-v1\n\ntitle\n\nsubject, created, tags, modified, subject, \n",
    )
    .unwrap();
    let config = bases::load(&path).unwrap();
    assert_eq!(config.extra_columns, vec!["subject".to_string()]);
}

#[test]
fn sort_by_field_is_deterministic_and_missing_last() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("b.md"),
        "---\nsubject: Bravo\n---\n# Zulu\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("a.md"),
        "---\nsubject: Alpha\n---\n# Yankee\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("d.md"),
        "---\nsubject: Alpha\n---\n# Xray\n",
    )
    .unwrap();
    fs::write(dir.path().join("c.md"), "# Whiskey\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let config = BaseConfig {
        sort_by: SortBy::Field,
        sort_field: "subject".to_string(),
        ..BaseConfig::default()
    };
    let ids: Vec<_> = bases::build(&index, &config)
        .into_iter()
        .map(|row| row.id.as_str().to_string())
        .collect();
    assert_eq!(ids, ["a", "d", "b", "c"]);
}

#[test]
fn empty_sort_field_falls_back_to_title() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "---\nsubject: Alpha\n---\n# Zulu\n").unwrap();
    fs::write(dir.path().join("b.md"), "---\nsubject: Bravo\n---\n# Alpha\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let config = BaseConfig {
        sort_by: SortBy::Field,
        ..BaseConfig::default()
    };
    let titles: Vec<_> = bases::build(&index, &config)
        .into_iter()
        .map(|row| row.title)
        .collect();
    assert_eq!(titles, ["Alpha", "Zulu"]);
}

#[test]
fn descending_sort_keeps_id_tiebreak_ascending() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "# Same\n").unwrap();
    fs::write(dir.path().join("b.md"), "# Same\n").unwrap();
    fs::write(dir.path().join("c.md"), "# Zebra\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let config = BaseConfig {
        sort_desc: true,
        ..BaseConfig::default()
    };
    let ids: Vec<_> = bases::build(&index, &config)
        .into_iter()
        .map(|row| row.id.as_str().to_string())
        .collect();
    assert_eq!(ids, ["c", "a", "b"]);
}

#[test]
fn descending_field_sort_keeps_missing_values_last() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "---\nsubject: Alpha\n---\n# A\n").unwrap();
    fs::write(dir.path().join("b.md"), "---\nsubject: Bravo\n---\n# B\n").unwrap();
    fs::write(dir.path().join("c.md"), "# C\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let config = BaseConfig {
        sort_by: SortBy::Field,
        sort_field: "subject".to_string(),
        sort_desc: true,
        ..BaseConfig::default()
    };
    let ids: Vec<_> = bases::build(&index, &config)
        .into_iter()
        .map(|row| row.id.as_str().to_string())
        .collect();
    assert_eq!(ids, ["b", "a", "c"]);
}

#[test]
fn descending_sort_honours_path_created_and_modified() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "---\ncreated: 2026-01-01\n---\n# A\n").unwrap();
    fs::write(dir.path().join("b.md"), "---\ncreated: 2026-01-02\n---\n# B\n").unwrap();
    fs::write(dir.path().join("c.md"), "---\ncreated: 2026-01-03\n---\n# C\n").unwrap();
    // Newest first for c, oldest for a, so desc Modified is c, b, a.
    for (name, age) in [("a.md", 300), ("b.md", 200), ("c.md", 100)] {
        File::options()
            .write(true)
            .open(dir.path().join(name))
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(age))
            .unwrap();
    }
    let index = vault::build_index(dir.path()).unwrap();
    for sort_by in [SortBy::Path, SortBy::Created, SortBy::Modified] {
        let config = BaseConfig {
            sort_by,
            sort_desc: true,
            ..BaseConfig::default()
        };
        let ids: Vec<_> = bases::build(&index, &config)
            .into_iter()
            .map(|row| row.id.as_str().to_string())
            .collect();
        assert_eq!(ids, ["c", "b", "a"], "desc sort {sort_by:?}");
    }
}

#[test]
fn sidecar_sort_direction_is_backward_compatible() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("direction.idx");
    fs::write(&path, "NEXORA-BASE-v1\n\ntitle\n").unwrap();
    assert!(!bases::load(&path).unwrap().sort_desc);
    fs::write(&path, "NEXORA-BASE-v1\n\ntitle\n\n").unwrap();
    assert!(!bases::load(&path).unwrap().sort_desc);
    fs::write(&path, "NEXORA-BASE-v1\n\ntitle\n\n\n\n").unwrap();
    assert!(!bases::load(&path).unwrap().sort_desc);
    fs::write(&path, "NEXORA-BASE-v1\n\ntitle\n\n\n\ndesc\n").unwrap();
    assert!(bases::load(&path).unwrap().sort_desc);
    fs::write(&path, "NEXORA-BASE-v1\n\ntitle\n\n\n\nsurprise\n").unwrap();
    assert!(!bases::load(&path).unwrap().sort_desc);
}

#[test]
fn set_sort_field_normalises_invalid_input() {
    let mut config = BaseConfig::default();
    assert!(bases::set_sort_field(&mut config, " subject "));
    assert_eq!(config.sort_field, "subject");
    assert!(!bases::set_sort_field(&mut config, "bad,key"));
    assert!(config.sort_field.is_empty());
    assert!(!bases::set_sort_field(&mut config, "created"));
    assert!(config.sort_field.is_empty());
}

#[test]
fn invalid_sort_field_falls_back_to_title_in_session() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "---\nsubject: Alpha\n---\n# Zulu\n").unwrap();
    fs::write(dir.path().join("b.md"), "---\nsubject: Bravo\n---\n# Alpha\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let config = BaseConfig {
        sort_by: SortBy::Field,
        sort_field: "bad,key".to_string(),
        ..BaseConfig::default()
    };
    let titles: Vec<_> = bases::build(&index, &config)
        .into_iter()
        .map(|row| row.title)
        .collect();
    assert_eq!(titles, ["Alpha", "Zulu"]);
}

#[test]
fn sidecar_sort_field_is_backward_compatible_and_validated() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("five.idx");
    fs::write(&path, "NEXORA-BASE-v1\n\ntitle\n\nsubject\n").unwrap();
    assert!(bases::load(&path).unwrap().sort_field.is_empty());

    fs::write(&path, "NEXORA-BASE-v1\n\nfield\n\ntitle\nbad,key\n").unwrap();
    assert!(bases::load(&path).unwrap().sort_field.is_empty());

    fs::write(&path, "NEXORA-BASE-v1\n\nfield\n\n\nsubject\n").unwrap();
    assert_eq!(bases::load(&path).unwrap().sort_field, "subject");
}

#[test]
fn grid_selection_moves_and_clamps() {
    use nexora_core::bases::{move_selection, Move};

    assert_eq!(move_selection(0, 3, None, Move::Right), None);
    assert_eq!(move_selection(2, 0, None, Move::Down), None);
    assert_eq!(move_selection(3, 4, None, Move::Down), Some((0, 0)));
    assert_eq!(move_selection(3, 4, Some((1, 2)), Move::Down), Some((2, 2)));
    assert_eq!(move_selection(3, 4, Some((2, 2)), Move::Down), Some((2, 2)));
    assert_eq!(move_selection(3, 4, Some((1, 2)), Move::Up), Some((0, 2)));
    assert_eq!(move_selection(3, 4, Some((0, 2)), Move::Up), Some((0, 2)));
    assert_eq!(move_selection(3, 4, Some((1, 2)), Move::Right), Some((1, 3)));
    assert_eq!(move_selection(3, 4, Some((1, 3)), Move::Right), Some((1, 3)));
    assert_eq!(move_selection(3, 4, Some((1, 1)), Move::Left), Some((1, 0)));
    assert_eq!(move_selection(3, 4, Some((1, 0)), Move::Left), Some((1, 0)));
    // Out-of-range input is clamped before moving.
    assert_eq!(move_selection(3, 4, Some((9, 9)), Move::Left), Some((2, 2)));
}

#[test]
fn filter_reset_keeps_view_settings() {
    let mut config = BaseConfig {
        filter_tag: "school".to_string(),
        filter_text: "physics".to_string(),
        sort_by: SortBy::Field,
        group_by: "tags".to_string(),
        extra_columns: vec!["subject".to_string()],
        sort_field: "subject".to_string(),
        sort_desc: true,
    };
    assert!(bases::has_filters(&config));
    bases::clear_filters(&mut config);
    assert!(!bases::has_filters(&config));
    assert!(config.filter_tag.is_empty());
    assert!(config.filter_text.is_empty());
    assert_eq!(config.sort_by, SortBy::Field);
    assert_eq!(config.group_by, "tags");
    assert_eq!(config.extra_columns, vec!["subject".to_string()]);
    assert_eq!(config.sort_field, "subject");
    assert!(config.sort_desc);
    bases::clear_filters(&mut config);
    assert!(!bases::has_filters(&config));
}

#[test]
fn trimmed_whitespace_is_not_a_filter() {
    let config = BaseConfig {
        filter_tag: "  ".to_string(),
        filter_text: "\t".to_string(),
        ..BaseConfig::default()
    };
    assert!(!bases::has_filters(&config));
    assert!(bases::has_filters(&BaseConfig {
        filter_tag: "school".to_string(),
        ..BaseConfig::default()
    }));
    assert!(bases::has_filters(&BaseConfig {
        filter_text: "physics".to_string(),
        ..BaseConfig::default()
    }));
    let dir = fixture();
    let index = vault::build_index(dir.path()).unwrap();
    assert_eq!(bases::build(&index, &config).len(), 3);
    let padded = BaseConfig {
        filter_tag: " school ".to_string(),
        ..BaseConfig::default()
    };
    assert_eq!(bases::build(&index, &padded).len(), 2);
}

#[test]
fn custom_column_is_projected_and_editable() {
    let dir = fixture();
    fs::write(
        dir.path().join("d.md"),
        "---\nsubject: Physics\nzebra: keep\n---\n# Delta\n",
    )
    .unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let config = BaseConfig {
        extra_columns: vec!["subject".to_string()],
        ..BaseConfig::default()
    };
    let rows = bases::build(&index, &config);
    let delta = rows.iter().find(|row| row.id.as_str() == "d").unwrap();
    assert_eq!(delta.values.get("subject").map(String::as_str), Some("Physics"));
    assert!(rows.iter().all(|row| row.values.contains_key("subject")));
    let updated = bases::edit_cell(
        "---\nsubject: Physics\nzebra: keep\n---\n# Delta\n",
        "subject",
        "Math",
    );
    assert_eq!(read_field(&updated, "subject").as_deref(), Some("Math"));
    assert_eq!(read_field(&updated, "zebra").as_deref(), Some("keep"));
}
