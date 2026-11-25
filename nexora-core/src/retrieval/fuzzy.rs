//! Small explainable fuzzy score for the file switcher. No external crate.

/// Subsequence match with bonuses for prefix, word boundary and consecutive
/// hits. `None` means the query is not a subsequence of the text.
pub fn score(query: &str, text: &str) -> Option<u32> {
    let needle: Vec<char> = query.trim().to_lowercase().chars().collect();
    if needle.is_empty() {
        return Some(0);
    }
    let haystack: Vec<char> = text.to_lowercase().chars().collect();
    let mut result = 0u32;
    let mut cursor = 0usize;
    let mut previous: Option<usize> = None;

    for (index, wanted) in needle.iter().enumerate() {
        let mut found = None;
        while cursor < haystack.len() {
            if haystack[cursor] == *wanted {
                found = Some(cursor);
                break;
            }
            cursor += 1;
        }
        let position = found?;
        if position == 0 {
            result += 10;
        } else if !haystack[position - 1].is_alphanumeric() {
            result += 6;
        }
        if previous == Some(position.saturating_sub(1)) {
            result += 8;
        }
        if index == 0 && position == 0 {
            result += 8;
        }
        result += 1;
        previous = Some(position);
        cursor = position + 1;
    }
    Some(result)
}
