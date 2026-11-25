//! Pure transformations for plain-text markdown lists (§55).
//!
//! Every function takes a character offset because egui cursors are
//! character-indexed, and returns the edited text together with the new cursor
//! offset. Nothing here touches the filesystem or a GUI type.

/// Result of a list edit: the new text and the caret position in characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListEdit {
    pub text: String,
    pub cursor: usize,
}

#[derive(Clone, Debug)]
struct Item {
    indent: String,
    marker: String,
    next_marker: String,
    /// Start of the list content, after any BOM (§55).
    content_start: usize,
    marker_start: usize,
    empty: bool,
    task: Option<bool>,
}

/// Line boundaries for a character cursor. `end` points at `\n` or at EOF.
/// Shared with `notes::tables` (§57); both modules use character offsets.
pub(crate) fn line_bounds(chars: &[char], cursor: usize) -> (usize, usize) {
    let cursor = cursor.min(chars.len());
    let mut start = cursor;
    while start > 0 && chars[start - 1] != '\n' {
        start -= 1;
    }
    let mut end = cursor;
    while end < chars.len() && chars[end] != '\n' {
        end += 1;
    }
    (start, end)
}

/// True when the caret line is inside a fenced code block. The fence line
/// itself is not part of the check, so pressing Enter there is left to egui.
/// Shared with `notes::tables` (§57) so neither module edits fenced code.
pub(crate) fn in_fence(chars: &[char], line_start: usize) -> bool {
    let mut open = false;
    let mut i = 0;
    while i < line_start {
        let mut j = i;
        while j < line_start && chars[j] != '\n' {
            j += 1;
        }
        let line: String = chars[i..j].iter().collect();
        // U+FEFF is not whitespace for `trim_start`; strip it explicitly so a
        // BOM-prefixed fence is still counted (§55 review).
        let line = if i == 0 {
            line.strip_prefix('\u{feff}').unwrap_or(&line)
        } else {
            &line
        };
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            open = !open;
        }
        if j == line_start {
            break;
        }
        i = j + 1;
    }
    open
}

fn ordered_prefix(text: &str) -> Option<(String, u64, &str)> {
    let digits_len = text.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits_len == 0 {
        return None;
    }
    let digits = &text[..digits_len]; // ASCII only: byte len == char len
    let number = digits.parse::<u64>().ok()?;
    let rest = &text[digits_len..];
    let delimiter = rest.chars().next()?;
    if delimiter != '.' && delimiter != ')' {
        return None;
    }
    let body = &rest[1..];
    if !body.starts_with(' ') {
        return None;
    }
    Some((format!("{digits}{delimiter} "), number, &body[1..]))
}

fn parse(chars: &[char], start: usize, end: usize) -> Option<Item> {
    let mut line_end = end;
    if line_end > start && chars[line_end - 1] == '\r' {
        line_end -= 1;
    }
    let mut marker_start = start;
    let line = if start == 0 {
        let first: String = chars[start..line_end].iter().collect();
        match first.strip_prefix('\u{feff}') {
            Some(without_bom) => {
                marker_start += 1;
                without_bom.to_string()
            }
            None => first,
        }
    } else {
        chars[start..line_end].iter().collect()
    };
    let trimmed = line.trim_start_matches([' ', '\t']);
    let indent_len = line.chars().count() - trimmed.chars().count();
    let indent: String = line.chars().take(indent_len).collect();
    marker_start += indent_len;
    let content_start = marker_start - indent_len;

    if let Some(body) = trimmed.strip_prefix("- [ ] ") {
        return Some(Item {
            indent,
            marker: "- [ ] ".to_string(),
            next_marker: "- [ ] ".to_string(),
            content_start,
            marker_start,
            empty: body.trim().is_empty(),
            task: Some(false),
        });
    }
    if let Some(body) = trimmed.strip_prefix("- [x] ") {
        return Some(Item {
            indent,
            marker: "- [x] ".to_string(),
            next_marker: "- [ ] ".to_string(),
            content_start,
            marker_start,
            empty: body.trim().is_empty(),
            task: Some(true),
        });
    }
    if let Some((marker, number, body)) = ordered_prefix(trimmed) {
        let delimiter = marker.chars().nth(marker.chars().count() - 2)?;
        return Some(Item {
            indent,
            next_marker: format!("{}{delimiter} ", number.saturating_add(1)),
            marker,
            content_start,
            marker_start,
            empty: body.trim().is_empty(),
            task: None,
        });
    }
    for prefix in ["- ", "* ", "+ "] {
        if let Some(body) = trimmed.strip_prefix(prefix) {
            return Some(Item {
                indent,
                marker: prefix.to_string(),
                next_marker: prefix.to_string(),
                content_start,
                marker_start,
                empty: body.trim().is_empty(),
                task: None,
            });
        }
    }
    None
}

/// Pressing Enter on a list item. Returns `None` outside a list or inside a
/// fenced code block, leaving the key to egui.
pub fn on_enter(text: &str, cursor: usize) -> Option<ListEdit> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let (start, end) = line_bounds(&chars, cursor);
    if in_fence(&chars, start) {
        return None;
    }
    let item = parse(&chars, start, end)?;
    if item.empty {
        // An empty item exits the list instead of creating another blank one.
        let mut out = String::with_capacity(text.len());
        out.extend(chars[..item.content_start].iter());
        out.extend(chars[end..].iter());
        return Some(ListEdit {
            text: out,
            cursor: item.content_start,
        });
    }

    let crlf = end > start && chars[end - 1] == '\r';
    let content_end = if crlf { end - 1 } else { end };
    let newline = if crlf { "\r\n" } else { "\n" };
    let mut insertion = String::with_capacity(newline.len() + item.indent.len() + 8);
    insertion.push_str(newline);
    insertion.push_str(&item.indent);
    insertion.push_str(&item.next_marker);

    let prefix_end = item.marker_start + item.marker.chars().count();
    // Never split a marker if the caret is still inside it.
    let at = cursor.clamp(prefix_end, content_end);
    let mut out = String::with_capacity(text.len() + insertion.len());
    out.extend(chars[..at].iter());
    out.push_str(&insertion);
    out.extend(chars[at..].iter());
    Some(ListEdit {
        text: out,
        cursor: at + insertion.chars().count(),
    })
}

/// Tab / Shift+Tab handling for the caret line. Returns `None` outside a list.
pub fn indent(text: &str, cursor: usize, outdent: bool) -> Option<ListEdit> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let (start, end) = line_bounds(&chars, cursor);
    if in_fence(&chars, start) {
        return None;
    }
    let item = parse(&chars, start, end)?;
    if outdent {
        let remove = if item.indent.starts_with('\t') {
            1
        } else {
            item.indent.chars().count().min(2)
        };
        if remove == 0 {
            return None;
        }
        let base = item.content_start;
        let mut out = String::with_capacity(text.len());
        out.extend(chars[..base].iter());
        out.extend(chars[base + remove..].iter());
        let new_cursor = if cursor >= base + remove {
            cursor - remove
        } else {
            base
        };
        return Some(ListEdit {
            text: out,
            cursor: new_cursor,
        });
    }
    let base = item.content_start;
    let mut out = String::with_capacity(text.len() + 2);
    out.extend(chars[..base].iter());
    out.push_str("  ");
    out.extend(chars[base..].iter());
    Some(ListEdit {
        text: out,
        cursor: cursor + 2,
    })
}

/// Toggle `- [ ]` / `- [x]` on the caret line. Returns `None` for non-tasks.
pub fn toggle_task(text: &str, cursor: usize) -> Option<ListEdit> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let (start, end) = line_bounds(&chars, cursor);
    if in_fence(&chars, start) {
        return None;
    }
    let item = parse(&chars, start, end)?;
    let checked = item.task?;
    let old_marker = if checked { "- [x] " } else { "- [ ] " };
    let new_marker = if checked { "- [ ] " } else { "- [x] " };
    let marker_end = item.marker_start + old_marker.chars().count();
    let actual: String = chars[item.marker_start..marker_end].iter().collect();
    if actual != old_marker {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    out.extend(chars[..item.marker_start].iter());
    out.push_str(new_marker);
    out.extend(chars[marker_end..].iter());
    Some(ListEdit {
        text: out,
        cursor,
    })
}
