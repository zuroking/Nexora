use nexora_core::bases::{self, BaseConfig, SortBy};
use nexora_core::named_bases;
use std::fs;
use tempfile::tempdir;

fn config(filter: &str) -> BaseConfig {
    BaseConfig {
        filter_tag: "school".to_string(),
        sort_by: SortBy::Field,
        group_by: "tags".to_string(),
        extra_columns: vec!["subject".to_string()],
        sort_field: "subject".to_string(),
        sort_desc: true,
        filter_text: filter.to_string(),
    }
}

#[test]
fn upsert_rename_and_remove_preserve_config() {
    let mut views = named_bases::upsert(&[], " first ", &config("physics")).unwrap();
    assert_eq!(views[0].name, "first");
    assert!(named_bases::upsert(&views, "first", &config("math"))
        .unwrap()
        .len()
        == 1);
    views = named_bases::upsert(&views, "first", &config("math")).unwrap();
    assert_eq!(views[0].config.filter_text, "math");
    views = named_bases::rename(&views, "first", "second").unwrap();
    assert_eq!(views[0].name, "second");
    assert_eq!(views[0].config.filter_text, "math");
    assert!(named_bases::rename(&views, "second", "second").is_ok());
    assert!(named_bases::rename(&views, "missing", "third").is_err());
    let two = named_bases::upsert(&views, "other", &config("other")).unwrap();
    assert!(named_bases::rename(&two, "second", "other").is_err());
    let views = named_bases::remove(&views, "second");
    assert!(views.is_empty());
}

#[test]
fn names_and_config_are_validated() {
    for name in ["", "  ", "a\tb", "a\nb"] {
        assert!(named_bases::upsert(&[], name, &config("x")).is_err(), "{name:?}");
    }
    let mut invalid = config("x");
    invalid.filter_text = "has\ttab".to_string();
    assert!(named_bases::upsert(&[], "view", &invalid).is_err());
    invalid.filter_text = "has\nnewline".to_string();
    assert!(named_bases::upsert(&[], "view", &invalid).is_err());

    let mut invalid = config("x");
    invalid.extra_columns = vec!["bad\tkey".to_string()];
    assert!(named_bases::upsert(&[], "view", &invalid).is_err());

    let mut invalid = config("x");
    invalid.sort_field = "bad,key".to_string();
    assert!(named_bases::upsert(&[], "view", &invalid).is_err());

    let mut invalid = config("x");
    invalid.sort_field = " subject ".to_string();
    assert!(named_bases::upsert(&[], "view", &invalid).is_err());

    let mut invalid = config("x");
    invalid.extra_columns = vec![" subject ".to_string()];
    assert!(named_bases::upsert(&[], "view", &invalid).is_err());
}

#[test]
fn sidecar_roundtrip_and_bad_files() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("named_bases.idx");
    let views = named_bases::upsert(&[], "physics", &config("physics")).unwrap();
    named_bases::save(&path, &views).unwrap();
    let loaded = named_bases::load(&path).unwrap();
    assert_eq!(loaded, views);
    assert_eq!(loaded[0].config, config("physics"));

    fs::write(&path, "WRONG\n").unwrap();
    assert!(named_bases::load(&path).is_err());
    fs::write(&path, "NEXORA-NAMED-BASES-v1\nbroken\n").unwrap();
    assert!(named_bases::load(&path).is_err());
}

#[test]
fn applying_a_view_writes_the_active_base_sidecar() {
    let dir = tempdir().unwrap();
    let view_path = dir.path().join("named_bases.idx");
    let base_path = dir.path().join("base.idx");
    let views = named_bases::upsert(&[], "physics", &config("physics")).unwrap();
    named_bases::save(&view_path, &views).unwrap();

    let loaded = named_bases::load(&view_path).unwrap();
    bases::save(&base_path, &loaded[0].config).unwrap();
    let active = bases::load(&base_path).unwrap();
    assert_eq!(active, config("physics"));
}
