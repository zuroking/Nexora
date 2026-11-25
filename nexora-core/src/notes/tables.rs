//! Pure transformations for plain-text markdown pipe tables (§57).
//!
//! Character offsets are used throughout because egui cursors are
//! character-indexed. Nothing here reads files or touches a GUI type.

use super::lists::{in_fence, line_bounds, ListEdit};

/// Shared result type: a new text and the caret position in characters.
pub type TableEdit = ListEdit;

#[derive(Clone, Debug)]
struct Cell {
    end: usize,
    content_start: usize,
    content_end: usize,
    pad_before: String,
    pad_after: String,
}

#[derive(Clone, Debug)]
struct Row {
    /// Start of the line after a BOM, if present (§57 review).
    base: usize,
    /// BOM and indentation before the first cell (§57).
    indent: String,
    /// Raw end of the line without a trailing `\r`.
    content_end: usize,
    /// End after trailing whitespace, used for cell boundaries only.
    logical_end: usize,
    cells: Vec<Cell>,
    leading_pipe: bool,
    trailing_pipe: bool,
}

fn is_ws(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// A pipe preceded by an odd number of backslashes is escaped, not a delimiter.
fn escaped(chars: &[char], index: usize) -> bool {
    let mut backslashes = 0;
    let mut i = index;
    while i > 0 && chars[i - 1] == '\\' {
        backslashes += 1;
        i -= 1;
    }
    backslashes % 2 == 1
}

fn unescaped_pipes(chars: &[char], start: usize, end: usize) -> Vec<usize> {
    (start..end)
        .filter(|&i| chars[i] == '|' && !escaped(chars, i))
        .collect()
}

fn parse(chars: &[char], start: usize, end: usize) -> Option<Row> {
    let content_end = if end > start && chars[end - 1] == '\r' {
        end - 1
    } else {
        end
    };
    let mut base = start;
    if start == 0 && chars.first() == Some(&'\u{feff}') {
        base += 1;
    }
    let mut row_start = base;
    while row_start < content_end && is_ws(chars[row_start]) {
        row_start += 1;
    }
    // BOM is a file marker, not line indentation: never repeat it on a new row.
    let indent: String = chars[base..row_start].iter().collect();

    // Trailing whitespace after the closing pipe must not create a phantom
    // cell (§57 review), but stays in `content_end` as the insertion point.
    let mut logical_end = content_end;
    while logical_end > row_start && is_ws(chars[logical_end - 1]) {
        logical_end -= 1;
    }
    let positions = unescaped_pipes(chars, row_start, logical_end);
    if positions.is_empty() {
        return None;
    }
    let leading_pipe = positions[0] == row_start;
    let trailing_pipe = positions.last() == Some(&(logical_end.saturating_sub(1)));

    let mut spans = Vec::with_capacity(positions.len() + 1);
    let mut cell_start = if leading_pipe {
        positions[0] + 1
    } else {
        row_start
    };
    for &pipe in &positions {
        if pipe >= cell_start {
            spans.push((cell_start, pipe));
        }
        cell_start = pipe + 1;
    }
    if cell_start < logical_end || !trailing_pipe {
        spans.push((cell_start, logical_end));
    }
    if spans.len() < 2 {
        return None;
    }

    let mut cells = Vec::with_capacity(spans.len());
    for &(cell_start, cell_end) in &spans {
        let mut content_start = cell_start;
        while content_start < cell_end && is_ws(chars[content_start]) {
            content_start += 1;
        }
        let mut content_end = cell_end;
        while content_end > content_start && is_ws(chars[content_end - 1]) {
            content_end -= 1;
        }
        let pad_before: String = chars[cell_start..content_start].iter().collect();
        let pad_after: String = chars[content_end..cell_end].iter().collect();
        cells.push(Cell {
            end: cell_end,
            content_start,
            content_end,
            pad_before,
            pad_after,
        });
    }

    Some(Row {
        base,
        indent,
        content_end,
        logical_end,
        cells,
        leading_pipe,
        trailing_pipe,
    })
}

fn cell_index(row: &Row, cursor: usize) -> usize {
    row.cells
        .iter()
        .position(|cell| cursor < cell.end)
        .unwrap_or_else(|| row.cells.len() - 1)
}

/// Render an empty row with the same shape and per-cell padding as `row`, and
/// report the caret offset inside it for `target` (relative to the string).
fn render_empty_row(row: &Row, target: usize) -> (String, usize) {
    let mut out = String::new();
    out.push_str(&row.indent);
    let mut caret = out.chars().count();
    if row.leading_pipe {
        out.push('|');
    }
    for index in 0..row.cells.len() {
        if index > 0 {
            out.push('|');
        }
        if index == target {
            caret = out.chars().count() + row.cells[index].pad_before.chars().count();
        }
        out.push_str(&row.cells[index].pad_before);
        out.push_str(&row.cells[index].pad_after);
    }
    if row.trailing_pipe {
        out.push('|');
    }
    (out, caret)
}

fn row_is_empty(row: &Row) -> bool {
    row.cells
        .iter()
        .all(|cell| cell.content_start == cell.content_end)
}

/// Pressing Enter on a non-empty pipe-table row. Returns `None` outside a
/// table or inside a fenced code block, leaving the key to egui.
pub fn on_enter(text: &str, cursor: usize) -> Option<TableEdit> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let (start, end) = line_bounds(&chars, cursor);
    if in_fence(&chars, start) {
        return None;
    }
    let row = parse(&chars, start, end)?;

    if row_is_empty(&row) {
        // Remove the blank row and leave the caret on the resulting blank line.
        // Keep the BOM before `base` and the original line ending starting at
        // `content_end` (which excludes only the trailing `\r`).
        let mut out = String::with_capacity(text.len());
        out.extend(chars[..row.base].iter());
        out.extend(chars[row.content_end..].iter());
        return Some(TableEdit {
            text: out,
            cursor: row.base,
        });
    }

    let crlf = end > start && chars[end - 1] == '\r';
    let newline = if crlf { "\r\n" } else { "\n" };
    let target = cell_index(&row, cursor);
    let (new_row, row_caret) = render_empty_row(&row, target);
    let mut insertion = String::with_capacity(newline.len() + new_row.len() + 1);
    insertion.push_str(newline);
    insertion.push_str(&new_row);
    let at = row.content_end;
    let mut out = String::with_capacity(text.len() + insertion.len());
    out.extend(chars[..at].iter());
    out.push_str(&insertion);
    out.extend(chars[at..].iter());
    Some(TableEdit {
        text: out,
        cursor: at + insertion.chars().count() - new_row.chars().count() + row_caret,
    })
}

/// Tab / Shift+Tab between cells. Tab on the last cell appends one; Shift+Tab
/// on the first cell is a no-op. Returns `None` outside a table or in a fence.
pub fn on_tab(text: &str, cursor: usize, shift: bool) -> Option<TableEdit> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let (start, end) = line_bounds(&chars, cursor);
    if in_fence(&chars, start) {
        return None;
    }
    let row = parse(&chars, start, end)?;
    let index = cell_index(&row, cursor);

    if shift {
        // Still return a handled event on the first cell so the GUI consumes
        // the Tab and egui cannot outdent the row (§57 review).
        if index == 0 {
            return Some(TableEdit {
                text: text.to_string(),
                cursor,
            });
        }
        return Some(TableEdit {
            text: text.to_string(),
            cursor: row.cells[index - 1].content_start,
        });
    }

    if index + 1 < row.cells.len() {
        return Some(TableEdit {
            text: text.to_string(),
            cursor: row.cells[index + 1].content_start,
        });
    }

    // Append a new cell after the last one, before a closing pipe if present.
    let last = &row.cells[row.cells.len() - 1];
    let at = if row.trailing_pipe {
        row.logical_end - 1
    } else {
        row.content_end
    };
    let mut insertion = String::new();
    if row.trailing_pipe {
        insertion.push('|');
    } else {
        insertion.push_str(" |");
    }
    insertion.push_str(&last.pad_before);
    insertion.push_str(&last.pad_after);
    let mut out = String::with_capacity(text.len() + insertion.len());
    out.extend(chars[..at].iter());
    out.push_str(&insertion);
    out.extend(chars[at..].iter());
    let prefix_len = if row.trailing_pipe { 1 } else { 2 };
    Some(TableEdit {
        text: out,
        cursor: at + prefix_len + last.pad_before.chars().count(),
    })
}
