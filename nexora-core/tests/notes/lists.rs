use nexora_core::lists::{indent, on_enter, toggle_task, ListEdit};

#[test]
fn enter_continues_unordered_item() {
    let edit = on_enter("- first", 7).expect("list item");
    assert_eq!(
        edit,
        ListEdit {
            text: "- first\n- ".to_string(),
            cursor: 10,
        }
    );
}

#[test]
fn enter_preserves_indent_and_continues_task_unchecked() {
    let edit = on_enter("  - [ ] done", 12).expect("task item");
    assert_eq!(
        edit,
        ListEdit {
            text: "  - [ ] done\n  - [ ] ".to_string(),
            cursor: 21,
        }
    );
}

#[test]
fn enter_increments_ordered_item() {
    let edit = on_enter("3) item", 7).expect("ordered item");
    assert_eq!(
        edit,
        ListEdit {
            text: "3) item\n4) ".to_string(),
            cursor: 11,
        }
    );
}

#[test]
fn enter_on_empty_item_exits_list() {
    let edit = on_enter("- \nnext", 2).expect("empty item");
    assert_eq!(
        edit,
        ListEdit {
            text: "\nnext".to_string(),
            cursor: 0,
        }
    );
}

#[test]
fn enter_uses_crlf_when_the_line_uses_crlf() {
    let before_cr = on_enter("- a\r\nrest", 3).expect("crlf item");
    assert_eq!(before_cr.text, "- a\r\n- \r\nrest");
    // egui's End key can leave the caret after `\r`; insertion must still go
    // before it, otherwise the file gains a stray `\r`.
    let after_cr = on_enter("- a\r\nrest", 4).expect("crlf item");
    assert_eq!(after_cr.text, before_cr.text);
    assert_eq!(after_cr.cursor, before_cr.cursor);
}

#[test]
fn enter_increments_dot_ordered_items_too() {
    let edit = on_enter("1. item", 7).expect("ordered item");
    assert_eq!(edit.text, "1. item\n2. ");
}

#[test]
fn enter_inside_a_fenced_code_block_is_left_to_egui() {
    assert!(on_enter("```\n- item\n```", 8).is_none());
    assert!(on_enter("\u{feff}```\n- item\n```", 9).is_none());
}

#[test]
fn enter_in_the_middle_splits_after_the_marker() {
    let edit = on_enter("- abc", 3).expect("list item");
    assert_eq!(edit.text, "- a\n- bc");
    assert_eq!(edit.cursor, 6);
}

#[test]
fn bom_before_the_first_item_does_not_hide_the_marker() {
    let text = "\u{feff}- [ ] first";
    let edit = on_enter(text, 12).expect("first list item after BOM");
    assert_eq!(edit.text, "\u{feff}- [ ] first\n- [ ] ");
    assert_eq!(edit.cursor, 19);
}

#[test]
fn cyrillic_text_uses_character_offsets() {
    let edit = on_enter("- Привет", 8).expect("cyrillic item");
    assert_eq!(
        edit,
        ListEdit {
            text: "- Привет\n- ".to_string(),
            cursor: 11,
        }
    );
}

#[test]
fn indent_and_outdent_keep_the_caret_with_the_text() {
    let indented = indent("- item", 4, false).expect("indent");
    assert_eq!(
        indented,
        ListEdit {
            text: "  - item".to_string(),
            cursor: 6,
        }
    );
    let outdented = indent(&indented.text, indented.cursor, true).expect("outdent");
    assert_eq!(
        outdented,
        ListEdit {
            text: "- item".to_string(),
            cursor: 4,
        }
    );
}

#[test]
fn indenting_a_bom_prefixed_line_keeps_the_bom_first() {
    let text = "\u{feff}  - item";
    let indented = indent(text, 9, false).expect("indent");
    assert_eq!(indented.text, "\u{feff}    - item");
    assert_eq!(indented.cursor, 11);
    let outdented = indent(&indented.text, indented.cursor, true).expect("outdent");
    assert_eq!(outdented.text, text);
    assert_eq!(outdented.cursor, 9);
}

#[test]
fn indent_inside_a_fence_is_left_to_egui() {
    assert!(indent("```\n- item\n```", 8, false).is_none());
}

#[test]
fn exiting_an_empty_bom_item_preserves_the_bom() {
    let edit = on_enter("\u{feff}- ", 3).expect("empty item");
    assert_eq!(edit.text, "\u{feff}");
    assert_eq!(edit.cursor, 1);
}

#[test]
fn outdent_without_indentation_is_a_no_op() {
    assert!(indent("- item", 4, true).is_none());
}

#[test]
fn toggle_task_works_in_both_directions() {
    let checked = toggle_task("- [ ] task", 5).expect("unchecked task");
    assert_eq!(
        checked,
        ListEdit {
            text: "- [x] task".to_string(),
            cursor: 5,
        }
    );
    let unchecked = toggle_task(&checked.text, checked.cursor).expect("checked task");
    assert_eq!(unchecked.text, "- [ ] task");
}

#[test]
fn toggle_task_ignores_plain_list_items_and_code() {
    assert!(toggle_task("- plain", 3).is_none());
    assert!(toggle_task("```\n- [ ] x\n```", 10).is_none());
}
