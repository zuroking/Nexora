use nexora_core::display::adapt_for_display;
use nexora_core::vault;
use std::fs;
use tempfile::tempdir;

fn index(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    for (name, content) in files {
        fs::write(dir.path().join(format!("{name}.md")), content).unwrap();
    }
    dir
}

#[test]
fn embed_shows_body_without_frontmatter() {
    let dir = index(&[
        ("host", "before\n![[child]]\nafter\n"),
        ("child", "---\ncreated: 2026-01-01\n---\n# Child\nbody\n"),
    ]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("before\n![[child]]\nafter\n", &index);
    assert!(adapted.contains("# Child\nbody"), "adapted={adapted:?}");
    assert!(!adapted.contains("created: 2026-01-01"));
    assert!(adapted.contains("before"));
    assert!(adapted.contains("after"));
}

#[test]
fn cyclic_embed_stops_with_marker() {
    let dir = index(&[
        ("a", "![[b]]\n"),
        ("b", "![[a]]\n"),
    ]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("![[b]]", &index);
    assert!(adapted.contains("(embed loop)"));
}

#[test]
fn missing_embed_is_left_alone() {
    let dir = index(&[("host", "![[gone]]\n")]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("![[gone]]", &index);
    assert_eq!(adapted.trim_end(), "![[gone]]");
}

#[test]
fn heading_anchor_embeds_only_that_section() {
    let dir = index(&[
        ("child", "# First\na\n\n## Second\nb\n\n## Third\nc\n"),
    ]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("![[child#Second]]", &index);
    assert!(adapted.contains("## Second\nb"), "adapted={adapted:?}");
    assert!(!adapted.contains("# First"), "adapted={adapted:?}");
    assert!(!adapted.contains("## Third"), "adapted={adapted:?}");
}

#[test]
fn block_anchor_embeds_only_that_line() {
    let dir = index(&[("child", "one ^a\ntwo ^b\n")]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("![[child#^b]]", &index);
    assert!(adapted.contains("two ^b"));
    assert!(!adapted.contains("one ^a"));
}

#[test]
fn anchored_section_expands_nested_embeds_like_any_other_embed() {
    let dir = index(&[
        ("leaf", "# Leaf\nleaf text\n"),
        ("child", "# First\nalpha\n\n## Second\n![[leaf]]\n\n## Third\ncharlie\n"),
    ]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("![[child#Second]]", &index);
    assert!(adapted.contains("# Leaf\nleaf text"), "adapted={adapted:?}");
    assert!(!adapted.contains("alpha"), "adapted={adapted:?}");
    assert!(!adapted.contains("charlie"), "adapted={adapted:?}");
}

#[test]
fn fenced_code_inside_anchored_section_is_not_expanded() {
    let dir = index(&[
        ("leaf", "leaf body\n"),
        ("child", "## Target\ntext\n\n```\n![[leaf]]\n```\n\n## End\nlast\n"),
    ]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("![[child#Target]]", &index);
    assert!(adapted.contains("![[leaf]]"), "adapted={adapted:?}");
    assert!(!adapted.contains("leaf body"), "adapted={adapted:?}");
}

#[test]
fn heading_section_ends_at_higher_level_and_keeps_subheadings() {
    let dir = index(&[(
        "child",
        "## Target\nintro\n### Sub\nsubtext\n# Parent\nafter\n",
    )]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("![[child#Target]]", &index);
    assert!(adapted.contains("### Sub\nsubtext"), "adapted={adapted:?}");
    assert!(!adapted.contains("# Parent"), "adapted={adapted:?}");
    assert!(!adapted.contains("after"), "adapted={adapted:?}");
}

#[test]
fn depth_bound_applies_to_anchored_embeds() {
    // Top-level depth is 0; the fourth hop is refused. Anchored slices must
    // carry the depth forward just like whole-note embeds (§43 review).
    let dir = index(&[
        ("a", "## Sec\n![[b#Sec]]\n"),
        ("b", "## Sec\n![[c#Sec]]\n"),
        ("c", "## Sec\n![[d#Sec]]\n"),
        ("d", "## Sec\nleaf text\n"),
    ]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("![[a#Sec]]", &index);
    // a#Sec -> b#Sec (1) -> c#Sec (2) -> d#Sec (3) is the refused hop.
    assert!(adapted.contains("(embed loop)"), "adapted={adapted:?}");
    assert!(!adapted.contains("leaf text"), "adapted={adapted:?}");
}

#[test]
fn unknown_anchor_shows_a_visible_marker() {
    let dir = index(&[("child", "only body\n")]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("![[child#Missing]]", &index);
    assert!(adapted.contains("(anchor not found)"), "adapted={adapted:?}");
    assert!(!adapted.contains("only body"));
}

#[test]
fn embed_inside_fence_is_not_expanded() {
    let dir = index(&[("child", "body\n")]);
    let index = vault::build_index(dir.path()).unwrap();
    let adapted = adapt_for_display("```\n![[child]]\n```", &index);
    assert!(adapted.contains("![[child]]"));
}
