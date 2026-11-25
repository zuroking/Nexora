//! Pure transformations for plain-text markdown blockquotes (§61).
//!
//! All offsets are character indices, matching egui cursors. No filesystem or
//! GUI types live here.

use super::lists::{in_fence, line_bounds, ListEdit};

/// Shared result type: new text and caret position in characters.
pub type QuoteEdit = ListEdit;

#[derive(Clone, Debug)]
struct Quote {
    /// Line start after a BOM, if present.
    base: usize,
    /// End of the line without a trailing `\r`.
    content_end: usize,
    /// Exact leading whitespace plus quote markers, ready to repeat.
    prefix: String,
    /// First character after the quote markers.
    content_start: usize,
    empty: bool,
}

fn parse(chars: &[char], start: usize, end: usize) -> Option<Quote> {
    let content_end = if end > start && chars[end - 1] == '\r' {
        end - 1
    } else {
        end
    };
    let mut base = start;
    if start == 0 && chars.first() == Some(&'\u{feff}') {
        base += 1;
    }
    let mut i = base;
    while i < content_end && matches!(chars[i], ' ' | '\t') {
        i += 1;
    }
    let mut prefix: String = chars[base..i].iter().collect();
    let mut markers = 0;
    while i < content_end && chars[i] == '>' {
        prefix.push('>');
        markers += 1;
        i += 1;
        // CommonMark allows one optional space after each `>`.
        if i < content_end && chars[i] == ' ' {
            prefix.push(' ');
            i += 1;
        }
    }
    if markers == 0 {
        return None;
    }
    let empty = chars[i..content_end].iter().all(|c| c.is_whitespace());
    Some(Quote {
        base,
        content_end,
        prefix,
        content_start: i,
        empty,
    })
}

/// Pressing Enter on a blockquote line. Returns `None` outside a quote or
/// inside a fenced code block, leaving the key to egui.
pub fn on_enter(text: &str, cursor: usize) -> Option<QuoteEdit> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let (start, end) = line_bounds(&chars, cursor);
    if in_fence(&chars, start) {
        return None;
    }
    let quote = parse(&chars, start, end)?;

    if quote.empty {
        // Remove the empty quote line and leave the caret on a blank line.
        // Keep the BOM before `base` and the original line ending after
        // `content_end` (§61).
        let mut out = String::with_capacity(text.len());
        out.extend(chars[..quote.base].iter());
        out.extend(chars[quote.content_end..].iter());
        return Some(QuoteEdit {
            text: out,
            cursor: quote.base,
        });
    }

    let crlf = end > start && chars[end - 1] == '\r';
    let newline = if crlf { "\r\n" } else { "\n" };
    let mut insertion = String::with_capacity(newline.len() + quote.prefix.len());
    insertion.push_str(newline);
    insertion.push_str(&quote.prefix);
    let at = cursor.clamp(quote.content_start, quote.content_end);
    let mut out = String::with_capacity(text.len() + insertion.len());
    out.extend(chars[..at].iter());
    out.push_str(&insertion);
    out.extend(chars[at..].iter());
    Some(QuoteEdit {
        text: out,
        cursor: at + insertion.chars().count(),
    })
}
