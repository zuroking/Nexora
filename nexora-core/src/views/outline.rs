//! Heading outline for the active note. Fenced code is ignored.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub level: usize,
    pub title: String,
    /// 1-based line number.
    pub line: usize,
}

pub fn extract(content: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut in_fence = false;
    for (index, raw) in content.lines().enumerate() {
        let line = raw.trim_start();
        if line.starts_with("```") || line.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let level = line.chars().take_while(|character| *character == '#').count();
        if !(1..=6).contains(&level) {
            continue;
        }
        let rest = &line[level..];
        if !rest.starts_with(' ') && !rest.starts_with('\t') {
            continue;
        }
        let title = rest.trim();
        if !title.is_empty() {
            headings.push(Heading {
                level,
                title: title.to_string(),
                line: index + 1,
            });
        }
    }
    headings
}

/// Resolve a `[[Note#anchor]]` to a 1-based line in that note. A heading
/// anchor matches the heading text; a `^id` anchor matches a block marker.
/// Unknown anchors return `None` — navigation still opens the note (§38.1).
pub fn line_for_anchor(content: &str, anchor: &str) -> Option<usize> {
    if let Some(block) = anchor.strip_prefix('^') {
        let marker = format!("^{block}");
        return content
            .lines()
            .position(|line| line.split_whitespace().any(|word| word == marker))
            .map(|index| index + 1);
    }
    extract(content)
        .into_iter()
        .find(|heading| heading.title == anchor)
        .map(|heading| heading.line)
}
