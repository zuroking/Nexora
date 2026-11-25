use nexora_core::search;
use nexora_core::vault;
use nexora_core::NoteId;
use std::fs;
use tempfile::tempdir;

fn build(entries: &[(&str, &str)]) -> (tempfile::TempDir, nexora_core::VaultIndex) {
    let dir = tempdir().unwrap();
    for (name, body) in entries {
        fs::write(dir.path().join(format!("{name}.md")), body).unwrap();
    }
    let index = vault::build_index(dir.path()).unwrap();
    (dir, index)
}

#[test]
fn bom_prefixed_frontmatter_is_searchable() {
    let (_dir, index) = build(&[("bom", "\u{feff}---\nauthor: Zuro\n---\n# BOM\n")]);
    let results = search::search_advanced(&index, "author:Zuro");
    assert_eq!(ids(&results), vec!["bom"]);
}

fn ids(results: &[search::SearchResult]) -> Vec<String> {
    results.iter().map(|result| result.id.as_str().to_string()).collect()
}

#[test]
fn boolean_operators_and_implicit_and() {
    let (_dir, index) = build(&[
        ("both", "# Both\n\nalgebra and geometry\n"),
        ("only_algebra", "# Algebra\n\nalgebra only\n"),
        ("only_geometry", "# Geometry\n\ngeometry only\n"),
    ]);

    assert_eq!(ids(&search::search_advanced(&index, "algebra AND geometry")), ["both"]);
    let or = search::search_advanced(&index, "algebra OR geometry");
    assert_eq!(or.len(), 3);
    assert_eq!(
        ids(&search::search_advanced(&index, "algebra geometry")),
        ["both"]
    );
    assert_eq!(
        ids(&search::search_advanced(&index, "NOT algebra geometry")),
        ["only_geometry"]
    );
}

#[test]
fn quoted_phrase_is_one_term() {
    let (_dir, index) = build(&[
        ("exact", "# Exact\n\nlinear algebra today\n"),
        ("split", "# Split\n\nlinear vectors algebra\n"),
    ]);
    assert_eq!(
        ids(&search::search_advanced(&index, "\"linear algebra\"")),
        ["exact"]
    );
}

#[test]
fn path_and_frontmatter_fields() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("school")).unwrap();
    fs::write(
        dir.path().join("school/math.md"),
        "---\nauthor: Zuro\n---\n# Math\nalgebra\n",
    )
    .unwrap();
    fs::write(dir.path().join("other.md"), "# Other\nalgebra\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();

    assert_eq!(
        ids(&search::search_advanced(&index, "path:school")),
        ["school/math"]
    );
    assert_eq!(
        ids(&search::search_advanced(&index, "[author:Zuro]")),
        ["school/math"]
    );
    assert_eq!(
        ids(&search::search_advanced(&index, "author:Zuro OR path:other")),
        ["other", "school/math"]
    );
}

#[test]
fn empty_and_malformed_queries_do_not_panic() {
    let (_dir, index) = build(&[("a", "# A\nalgebra\n")]);
    assert!(search::search_advanced(&index, "").is_empty());
    assert!(search::search_advanced(&index, "   ").is_empty());
    assert!(search::search_advanced(&index, "\"unterminated").is_empty());
    assert!(search::search_advanced(&index, "AND OR NOT").is_empty());
}

#[test]
fn and_binds_tighter_than_or() {
    let (_dir, index) = build(&[
        ("a", "# A\nalpha beta\n"),
        ("b", "# B\nalpha gamma\n"),
        ("c", "# C\nbeta gamma\n"),
    ]);
    // AND binds tighter: alpha OR (beta AND gamma). `a` has alpha but no
    // gamma, so a left-to-right parser would drop it.
    let results = ids(&search::search_advanced(&index, "alpha OR beta AND gamma"));
    assert_eq!(results, ["a", "b", "c"]);
}

#[test]
fn results_are_deterministically_sorted_by_id() {
    let dir = tempdir().unwrap();
    for name in ["zeta", "alpha", "mid"] {
        fs::write(dir.path().join(format!("{name}.md")), format!("# {name}\ncommon\n")).unwrap();
    }
    let index = vault::build_index(dir.path()).unwrap();
    assert_eq!(
        ids(&search::search_advanced(&index, "common")),
        ["alpha", "mid", "zeta"]
    );
}

#[test]
fn degenerate_queries_return_nothing() {
    let (_dir, index) = build(&[
        ("a", "# A\nalgebra\n"),
        ("b", "# B\ngeometry\n"),
    ]);
    for query in ["\"\"", "path:", "[foo]", "[author:]", "\"unterminated"] {
        assert!(
            search::search_advanced(&index, query).is_empty(),
            "query {query:?} should be malformed"
        );
    }
}

#[test]
fn bare_term_searches_path_and_frontmatter() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("school")).unwrap();
    fs::write(
        dir.path().join("school/math.md"),
        "---\nauthor: Zuro\n---\n# Math\n",
    )
    .unwrap();
    fs::write(dir.path().join("other.md"), "# Other\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    assert_eq!(ids(&search::search_advanced(&index, "school")), ["school/math"]);
    assert_eq!(ids(&search::search_advanced(&index, "Zuro")), ["school/math"]);
}

#[test]
fn validate_distinguishes_malformed_from_empty() {
    assert!(search::validate("").is_ok());
    assert!(search::validate("algebra AND geometry").is_ok());
    assert!(search::validate("\"unterminated").is_err());
    assert!(search::validate("path:").is_err());
}

#[test]
fn plain_search_keeps_its_behaviour() {
    let (_dir, index) = build(&[("a", "# Algebra\n\nbody\n")]);
    let results = search::search(&index, "algebra");
    assert_eq!(results[0].id, NoteId::new("a"));
    assert_eq!(results[0].rank, 0); // exact title match
}
