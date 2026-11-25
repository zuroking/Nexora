//! Display-only adaptation of Obsidian-flavoured Markdown.
//!
//! The result is fed to a renderer. It is NEVER written back to the vault and
//! NEVER used for indexing. Link/tag semantics still live in `wikilink.rs` and
//! `tags.rs`; this module only changes how a note looks while reading.

use crate::{outline, wikilink, NoteId, VaultIndex};
use std::collections::BTreeSet;

const MAX_EMBED_DEPTH: usize = 3;

/// Percent-encode a `NoteId` for use in a `nexora://` link destination.
/// Unreserved ASCII bytes stay as-is; everything else becomes `%XX`.
pub fn encode_target(id: &NoteId) -> String {
    encode_component(id.as_str())
}

/// Percent-encode an anchor the same way as a note id, so `#Heading` survives
/// the trip through the renderer.
pub fn encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Inverse of [`encode_target`]. Invalid escapes are kept literally rather
/// than failing, because this function only handles UI navigation.
pub fn decode_target(encoded: &str) -> String {
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            // Work on raw bytes: slicing `&str[index..index+3]` by a byte
            // offset panics when the two bytes after '%' start a multibyte
            // code point. Non-ASCII escapes stay literal (§decode_target).
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                out.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn resolve_id(raw: &str, index: &VaultIndex) -> Option<NoteId> {
    let requested = NoteId::new(raw);
    if index.notes.contains_key(&requested) {
        Some(requested)
    } else {
        index.aliases.get(requested.as_str()).cloned()
    }
}

/// Drop a leading YAML block from an embedded note; the host keeps its own.
fn strip_frontmatter(content: &str) -> String {
    let mut lines = content.split_inclusive('\n');
    let first = lines.next().unwrap_or("");
    if first.trim().trim_start_matches('\u{feff}') != "---" {
        return content.to_string();
    }
    let mut body = String::new();
    let mut closed = false;
    for line in lines {
        if !closed {
            if line.trim() == "---" {
                closed = true;
            }
            continue;
        }
        body.push_str(line);
    }
    if closed {
        body
    } else {
        content.to_string()
    }
}

/// Slice a note body to one anchored section or block (§43.1). A heading
/// section runs to the next heading of the same or higher level, or to EOF.
fn slice_by_anchor(content: &str, anchor: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    if anchor.starts_with('^') {
        // Block resolution belongs to outline (§38); do not re-implement it.
        let line = outline::line_for_anchor(content, anchor)?;
        return Some(format!("{}\n", lines[line - 1]));
    }
    let headings = outline::extract(content);
    let heading = headings.iter().find(|item| item.title == anchor)?;
    let start = heading.line - 1;
    let end = headings
        .iter()
        .skip_while(|item| item.line <= heading.line)
        .find(|item| item.level <= heading.level)
        .map(|item| item.line - 1)
        .unwrap_or(lines.len());
    Some(format!("{}\n", lines[start..end].join("\n")))
}

/// Spans of `![[inner]]` on one line: `(start, end_after_close, inner)`.
/// The single owner of embed syntax (§45 review).
pub fn embed_spans<'a>(line: &'a str) -> Vec<(usize, usize, &'a str)> {
    let mut spans = Vec::new();
    let mut rest = line;
    let mut offset = 0;
    while let Some(start) = rest.find("![[") {
        let after = &rest[start + 3..];
        let Some(end) = after.find("]]") else {
            break;
        };
        spans.push((
            offset + start,
            offset + start + 3 + end + 2,
            &after[..end],
        ));
        let advance = start + 3 + end + 2;
        offset += advance;
        rest = &rest[advance..];
    }
    spans
}

/// Visit every line with a flag telling whether it belongs to fenced code
/// (the fence marker itself included). Single owner of the fence rule here.
fn scan_lines(content: &str, mut visit: impl FnMut(&str, bool)) {
    let mut in_fence = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        let marker = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        visit(line, in_fence || marker);
        if marker {
            in_fence = !in_fence;
        }
    }
}

/// Raw `![[...]]` inners in source order, fenced code excluded.
pub fn embed_inner(content: &str) -> Vec<String> {
    let mut found = Vec::new();
    scan_lines(content, |line, in_code| {
        if !in_code {
            for (_, _, inner) in embed_spans(line) {
                found.push(inner.to_string());
            }
        }
    });
    found
}

fn expand_line(line: &str, index: &VaultIndex, stack: &mut BTreeSet<NoteId>, depth: usize) -> String {
    let mut out = String::with_capacity(line.len());
    let spans = embed_spans(line);
    let mut cursor = 0;
    for (start, end, inner) in spans {
        out.push_str(&line[cursor..start]);
        let target_part = inner.split('|').next().unwrap_or("").trim();
        let (note_target, anchor) = wikilink::split_target(target_part);
        if let Some(id) = resolve_id(&note_target, index) {
            if stack.contains(&id) || depth >= MAX_EMBED_DEPTH {
                out.push_str(&format!("![[{target_part}]] (embed loop)"));
            } else {
                stack.insert(id.clone());
                let body = strip_frontmatter(&index.notes[&id].content);
                match anchor {
                    Some(anchor) => match slice_by_anchor(&body, &anchor) {
                        // Use the line-wise body expander so fenced code
                        // inside an anchored slice is still left alone.
                        Some(sliced) => out.push_str(&expand_body(&sliced, index, stack, depth + 1)),
                        // Never feed the marker back into the expander: it
                        // would try to resolve the same target again.
                        None => out.push_str(&format!("![[{target_part}]] (anchor not found)")),
                    },
                    None => out.push_str(&expand_body(&body, index, stack, depth + 1)),
                }
                stack.remove(&id);
            }
        } else {
            out.push_str(&line[start..end]);
        }
        cursor = end;
    }
    out.push_str(&line[cursor..]);
    out
}

/// Line-wise expansion shared by the top level and every embedded body, so
/// fenced code is respected at any depth (§31, §43.1).
fn expand_body(content: &str, index: &VaultIndex, stack: &mut BTreeSet<NoteId>, depth: usize) -> String {
    let mut out = String::with_capacity(content.len());
    scan_lines(content, |line, in_code| {
        if in_code {
            out.push_str(line);
        } else {
            out.push_str(&expand_line(line, index, stack, depth));
        }
        out.push('\n');
    });
    out
}

/// Expand `![[note]]` embeds without touching the vault or any index.
pub fn expand_embeds(content: &str, index: &VaultIndex) -> String {
    let mut stack = BTreeSet::new();
    expand_body(content, index, &mut stack, 0)
}

/// Adapt raw note text for a CommonMark renderer.
///
/// - `[[target|alias]]` / `[[target]]` become `[label](nexora://target)`.
/// - `==highlight==` becomes `**highlight**` (no extra renderer feature).
/// - `> [!note] Title` becomes `> **NOTE** — Title`.
/// - Fenced code blocks are left byte-for-byte alone.
pub fn adapt_for_display(content: &str, index: &VaultIndex) -> String {
    let expanded = expand_embeds(content, index);
    let mut out = String::with_capacity(expanded.len());
    let mut in_fence = false;
    for line in expanded.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let adapted = replace_callouts(&replace_highlights(&replace_wikilinks(line, index)));
        out.push_str(&adapted);
        out.push('\n');
    }
    out
}

fn replace_wikilinks(line: &str, index: &VaultIndex) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find("[[") {
        // `![[...]]` is an embed, not a link; leave it for the embed pass.
        if start > 0 && rest.as_bytes()[start - 1] == b'!' {
            let embed_start = start - 1;
            let after_bang = &rest[embed_start + 3..];
            let Some(embed_end) = after_bang.find("]]") else {
                out.push_str(rest);
                return out;
            };
            out.push_str(&rest[..embed_start + 3 + embed_end + 2]);
            rest = &after_bang[embed_end + 2..];
            continue;
        }
        out.push_str(&rest[..start]);
        let after_open = &rest[start + 2..];
        let Some(end) = after_open.find("]]") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &after_open[..end];
        let (target_part, alias) = match inner.split_once('|') {
            Some((target, alias)) => (target.trim(), Some(alias.trim())),
            None => (inner.trim(), None),
        };
        let (raw_target, anchor) = wikilink::split_target(target_part);
        let requested = NoteId::new(&raw_target);
        // A real note id wins; aliases are a fallback, same rule as wikilink.rs.
        let resolved = if index.notes.contains_key(&requested) {
            Some(requested.clone())
        } else {
            index.aliases.get(requested.as_str()).cloned()
        };
        let destination = resolved.clone().unwrap_or_else(|| requested.clone());
        let label = if let Some(note) = resolved.and_then(|id| index.notes.get(&id)) {
            alias.map(str::to_string).unwrap_or_else(|| {
                if index.notes.contains_key(&requested) {
                    note.title.clone()
                } else {
                    requested.as_str().to_string()
                }
            })
        } else {
            match alias {
                Some(value) => format!("{value} (missing)"),
                None => match anchor {
                    Some(ref anchor) => format!("{raw_target}#{anchor} (missing)"),
                    None => format!("{raw_target} (missing)"),
                },
            }
        };
        let label = label.replace(['[', ']'], "");
        out.push('[');
        out.push_str(&label);
        out.push_str("](nexora://");
        out.push_str(&encode_target(&destination));
        if let Some(anchor) = anchor {
            out.push('#');
            out.push_str(&encode_component(&anchor));
        }
        out.push(')');
        rest = &after_open[end + 2..];
    }
    out.push_str(rest);
    out
}

fn replace_highlights(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find("==") {
        let after_open = &rest[start + 2..];
        let Some(end) = after_open.find("==") else {
            out.push_str(rest);
            return out;
        };
        if end == 0 {
            out.push_str(&rest[..start + 2]);
            rest = after_open;
            continue;
        }
        out.push_str(&rest[..start]);
        out.push_str("**");
        out.push_str(&after_open[..end]);
        out.push_str("**");
        rest = &after_open[end + 2..];
    }
    out.push_str(rest);
    out
}

fn replace_callouts(line: &str) -> String {
    let Some(after_quote) = line.strip_prefix("> ") else {
        return line.to_string();
    };
    let Some(rest) = after_quote.strip_prefix("[!") else {
        return line.to_string();
    };
    let Some(end) = rest.find(']') else {
        return line.to_string();
    };
    let kind = &rest[..end];
    if kind.is_empty() {
        return line.to_string();
    }
    let title = rest[end + 1..].trim().trim_start_matches('-').trim();
    if title.is_empty() {
        format!("> **{}**", kind.to_uppercase())
    } else {
        format!("> **{}** — {title}", kind.to_uppercase())
    }
}
