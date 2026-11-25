use nexora_core::images;

#[test]
fn targets_detects_image_embeds_only() {
    let content = "![[pic.PNG]]\n![[note]]\n![[sub/photo.jpeg]]\n```\n![[fenced.png]]\n```\n";
    assert_eq!(images::targets(content), vec!["pic.PNG", "sub/photo.jpeg"]);
}

#[test]
fn unsafe_and_non_image_targets_are_rejected() {
    for target in ["ok.png", "photo.JPG", "a.webp"] {
        assert!(images::is_image_target(target));
        assert!(images::is_safe_target(target));
    }
    assert!(!images::is_image_target("note"));
    assert!(!images::is_image_target("file.pdf"));
    // Path safety is asserted directly, not hidden behind a disjunction.
    for target in ["../secret.png", "C:/secret.png", "/abs.png", "a\\b.png", ""] {
        assert!(!images::is_safe_target(target), "unsafe: {target}");
    }
}

#[test]
fn targets_excludes_unsafe_paths() {
    let content = "![[ok.png]]\n![[../secret.png]]\n![[C:/secret.png]]\n";
    assert_eq!(images::targets(content), vec!["ok.png"]);
    assert_eq!(images::all_targets(content).len(), 3);
}

#[test]
fn aliases_stay_in_the_inner_target() {
    let content = "![[pic.png|300]]\n";
    assert_eq!(images::targets(content), vec!["pic.png|300"]);
}
