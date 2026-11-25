use nexora_core::quotes::on_enter;

#[test]
fn enter_continues_quote_marker() {
    let edited = on_enter("> alpha\nrest", 2).expect("quote line");
    assert_eq!(edited.text, "> \n> alpha\nrest");
    assert_eq!(edited.cursor, 5);
}

#[test]
fn nested_and_compact_markers_are_kept_exactly() {
    let spaced = on_enter("> > alpha", 4).expect("nested quote");
    assert_eq!(spaced.text, "> > \n> > alpha");
    let compact = on_enter(">>> alpha", 4).expect("compact quote");
    assert_eq!(compact.text, ">>> \n>>> alpha");
}

#[test]
fn empty_quote_exits() {
    let edited = on_enter("> \nrest", 2).expect("empty quote");
    assert_eq!(edited.text, "\nrest");
    assert_eq!(edited.cursor, 0);
}

#[test]
fn empty_quote_exit_keeps_bom_and_crlf() {
    let edited = on_enter("\u{feff}> \r\nrest", 3).expect("empty BOM CRLF quote");
    assert_eq!(edited.text, "\u{feff}\r\nrest");
    assert_eq!(edited.cursor, 1);
}

#[test]
fn enter_splits_at_caret() {
    let edited = on_enter("> alpha beta", 7).expect("mid-line split");
    assert_eq!(edited.text, "> alpha\n>  beta");
}

#[test]
fn crlf_is_preserved() {
    let edited = on_enter("> alpha\r\nrest", 2).expect("CRLF quote");
    assert_eq!(edited.text, "> \r\n> alpha\r\nrest");
}

#[test]
fn bom_is_not_repeated() {
    let edited = on_enter("\u{feff}> alpha", 3).expect("BOM quote");
    assert_eq!(edited.text, "\u{feff}> \n> alpha");
    assert_eq!(edited.cursor, 6);
}

#[test]
fn indentation_is_preserved() {
    let edited = on_enter("  > alpha", 4).expect("indented quote");
    assert_eq!(edited.text, "  > \n  > alpha");
    assert_eq!(edited.cursor, 9);
}

#[test]
fn fenced_and_non_quote_lines_are_ignored() {
    assert!(on_enter("```\n> alpha\n```", 9).is_none());
    assert!(on_enter("plain text", 4).is_none());
}
