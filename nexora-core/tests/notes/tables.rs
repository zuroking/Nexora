use nexora_core::tables::{on_enter, on_tab};

#[test]
fn enter_continues_table_with_same_shape_and_column() {
    let edited = on_enter("| a | b |\nrest", 5).expect("table row");
    assert_eq!(edited.text, "| a | b |\n|  |  |\nrest");
    assert_eq!(edited.cursor, 15);
}

#[test]
fn enter_on_empty_row_exits_table() {
    let edited = on_enter("|  |  |\nrest", 3).expect("empty row");
    assert_eq!(edited.text, "\nrest");
    assert_eq!(edited.cursor, 0);
}

#[test]
fn enter_does_not_copy_alignment_markers() {
    let edited = on_enter("| --- | :---: |\n| a | b |", 3).expect("separator row");
    assert_eq!(edited.text, "| --- | :---: |\n|  |  |\n| a | b |");
    assert_eq!(edited.text.lines().nth(1), Some("|  |  |"));
}

#[test]
fn escaped_pipe_is_not_a_cell_boundary() {
    let edited = on_enter("| a \\| b | c |", 2).expect("row with escaped pipe");
    assert_eq!(edited.text, "| a \\| b | c |\n|  |  |");
}

#[test]
fn enter_preserves_crlf() {
    let edited = on_enter("| a | b |\r\nrest", 5).expect("CRLF row");
    assert_eq!(edited.text, "| a | b |\r\n|  |  |\r\nrest");
    assert_eq!(edited.cursor, 16);
}

#[test]
fn enter_preserves_bom() {
    let input = "\u{feff}| a | b |";
    let edited = on_enter(input, 3).expect("BOM row");
    assert!(edited.text.starts_with('\u{feff}'));
    assert_eq!(edited.text, "\u{feff}| a | b |\n|  |  |");
    assert_eq!(edited.cursor, 13);
}

#[test]
fn enter_ignores_single_cell_and_non_table_lines() {
    assert!(on_enter("| only |", 2).is_none());
    assert!(on_enter("plain text", 4).is_none());
}

#[test]
fn enter_inside_fence_is_ignored() {
    assert!(on_enter("```\n| a | b |\n```", 9).is_none());
}

#[test]
fn tab_moves_between_cells() {
    let first = on_tab("| a | b |", 2, false).expect("next cell");
    assert_eq!(first.cursor, 6);
    let back = on_tab("| a | b |", 6, true).expect("previous cell");
    assert_eq!(back.cursor, 2);
}

#[test]
fn shift_tab_on_first_cell_is_noop() {
    let handled = on_tab("| a | b |", 2, true).expect("handled first cell");
    assert_eq!(handled.text, "| a | b |");
    assert_eq!(handled.cursor, 2);
}

#[test]
fn tab_on_last_cell_appends_one() {
    let edited = on_tab("| a | b |", 6, false).expect("append cell");
    assert_eq!(edited.text, "| a | b |  |");
    assert_eq!(edited.cursor, 10);
}

#[test]
fn table_keys_are_ignored_outside_table() {
    assert!(on_tab("plain text", 3, false).is_none());
}

#[test]
fn new_row_keeps_indentation() {
    let edited = on_enter("  | a | b |", 4).expect("indented row");
    assert_eq!(edited.text, "  | a | b |\n  |  |  |");
    assert_eq!(edited.cursor, 16);
}

#[test]
fn padding_is_per_cell() {
    let edited = on_enter("|a|  b  |", 5).expect("mixed padding");
    assert_eq!(edited.text, "|a|  b  |\n||    |");
}

#[test]
fn empty_row_exit_keeps_bom_and_crlf() {
    let edited = on_enter("\u{feff}|  |  |\r\nrest", 4).expect("empty CRLF row");
    assert_eq!(edited.text, "\u{feff}\r\nrest");
    assert_eq!(edited.cursor, 1);
}

#[test]
fn trailing_whitespace_is_not_a_cell() {
    let edited = on_enter("| a | b | ", 5).expect("trailing space");
    assert_eq!(edited.text, "| a | b | \n|  |  |");
    let appended = on_tab("| a | b | ", 6, false).expect("append before pipe");
    assert_eq!(appended.text, "| a | b |  | ");
}
