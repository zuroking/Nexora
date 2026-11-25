//! Pure cursor analysis for inline `[[` autocomplete. No GUI dependencies.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialLink {
    /// Character index of the opening `[[`.
    pub start: usize,
    /// Text already typed after `[[`, before an optional `|alias`.
    pub partial: String,
}

/// Find an unclosed wikilink immediately before the cursor.
pub fn partial_at(content: &str, cursor_char: usize) -> Option<PartialLink> {
    let chars: Vec<char> = content.chars().collect();
    let cursor = cursor_char.min(chars.len());
    let prefix: String = chars[..cursor].iter().collect();
    let byte_start = prefix.rfind("[[")?;
    let start = prefix[..byte_start].chars().count();
    let tail: String = chars[start + 2..cursor].iter().collect();
    if tail.contains("]]") || tail.contains('\n') {
        return None;
    }
    let partial = tail
        .split('|')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    Some(PartialLink { start, partial })
}
