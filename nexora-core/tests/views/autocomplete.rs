use nexora_core::autocomplete::{partial_at, PartialLink};

#[test]
fn detects_unclosed_link_at_cursor() {
    let source = "see [[school/alg";
    assert_eq!(
        partial_at(source, source.chars().count()),
        Some(PartialLink {
            start: 4,
            partial: "school/alg".to_string()
        })
    );
}

#[test]
fn closed_and_escaped_links_are_ignored() {
    let closed = "see [[target]]";
    assert_eq!(partial_at(closed, closed.chars().count()), None);
    let newline = "[[target\nmore";
    assert_eq!(partial_at(newline, newline.chars().count()), None);
}

#[test]
fn alias_text_is_not_part_of_the_query() {
    let source = "[[target|alias";
    assert_eq!(
        partial_at(source, source.chars().count()).unwrap().partial,
        "target"
    );
}

#[test]
fn unicode_before_cursor_is_counted_in_characters() {
    let source = "привет [[мате";
    let result = partial_at(source, source.chars().count()).unwrap();
    assert_eq!(result.start, 7);
    assert_eq!(result.partial, "мате");
}
