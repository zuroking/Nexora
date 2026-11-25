//! Conservative, lossless editing of four frontmatter keys.
//!
//! Only lines owned by Nexora are rewritten; all other lines inside the
//! `---` block are kept byte-for-byte. See `ARCHITECTURE.md` §22.

fn line_text(line: &str) -> &str {
    line.trim_end_matches('\n').trim_end_matches('\r')
}

fn line_ending(line: &str) -> &str {
    if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        "\n"
    }
}

/// Split a note into its frontmatter body (if any), the remaining lines, and
/// whether the file started with a UTF-8 BOM (which must be preserved).
fn parts(content: &str) -> (Option<Vec<String>>, Vec<String>, bool) {
    let bom = content.starts_with('\u{feff}');
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let lines: Vec<String> = content.split_inclusive('\n').map(str::to_string).collect();
    if lines
        .first()
        .is_some_and(|line| line_text(line).trim() == "---")
    {
        if let Some(close) = lines
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, line)| line_text(line).trim() == "---")
            .map(|(index, _)| index)
        {
            return (Some(lines[1..close].to_vec()), lines[close + 1..].to_vec(), bom);
        }
    }
    (None, lines, bom)
}

fn assemble(body: &[String], rest: &[String], bom: bool) -> String {
    // The delimiters are consumed by `parts`, so infer their line endings
    // from the lines that surrounded them. A CRLF note must stay CRLF even
    // at the frontmatter boundaries (§56).
    let opening = body
        .first()
        .or_else(|| rest.first())
        .map(|line| line_ending(line))
        .unwrap_or("\n");
    let closing = body
        .last()
        .or_else(|| rest.first())
        .map(|line| line_ending(line))
        .unwrap_or("\n");
    let mut out = String::new();
    if bom {
        out.push('\u{feff}');
    }
    out.push_str("---");
    out.push_str(opening);
    for line in body {
        out.push_str(line);
        if !line.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push_str("---");
    out.push_str(closing);
    for line in rest {
        out.push_str(line);
    }
    out
}

fn key_of(line: &str) -> Option<&str> {
    if line.starts_with(char::is_whitespace) || line_text(line).starts_with('#') {
        return None;
    }
    let (key, _) = line_text(line).split_once(':')?;
    let key = key.trim();
    (!key.is_empty()).then_some(key)
}

fn value_of(line: &str) -> &str {
    line_text(line)
        .split_once(':')
        .map(|(_, value)| value.trim())
        .unwrap_or("")
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1].replace("\\\"", "\"")
    } else {
        value.to_string()
    }
}

fn quote(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn find_key(body: &[String], key: &str) -> Option<usize> {
    body.iter()
        .position(|line| key_of(line) == Some(key))
}

/// Read a scalar field from frontmatter.
pub fn read_field(content: &str, key: &str) -> Option<String> {
    let (body, _, _) = parts(content);
    let body = body?;
    let index = find_key(&body, key)?;
    Some(unquote(value_of(&body[index])))
}

/// Read `aliases` / `tags` from an inline `[a, b]` array or a following
/// block-style `- item` list.
pub fn read_list(content: &str, key: &str) -> Vec<String> {
    let Some(body) = parts(content).0 else {
        return Vec::new();
    };
    let Some(index) = find_key(&body, key) else {
        return Vec::new();
    };
    let value = value_of(&body[index]);
    if value.starts_with('[') && value.ends_with(']') {
        return value[1..value.len() - 1]
            .split(',')
            .map(|item| unquote(item))
            .filter(|item| !item.is_empty())
            .collect();
    }
    if value.is_empty() {
        let mut items = Vec::new();
        for line in body.iter().skip(index + 1) {
            if key_of(line).is_some() {
                break;
            }
            if let Some(item) = line_text(line).trim_start().strip_prefix("- ") {
                items.push(unquote(item));
            }
        }
        return items;
    }
    Vec::new()
}

/// Write a scalar field, preserving every line we do not own.
pub fn write_field(content: &str, key: &str, value: &str) -> String {
    if value.trim().is_empty() && read_field(content, key).is_none() {
        return content.to_string();
    }
    let (body, rest, bom) = parts(content);
    let mut body = body.unwrap_or_default();
    let written = format!("{key}: {}", quote(value));
    if let Some(index) = find_key(&body, key) {
        let ending = line_ending(&body[index]).to_string();
        body[index] = format!("{written}{ending}");
    } else {
        let ending = body.last()
            .map(|line| line_ending(line))
            .unwrap_or("\n")
            .to_string();
        body.push(format!("{written}{ending}"));
    }
    assemble(&body, &rest, bom)
}

/// Write `aliases` / `tags` as an inline array.
pub fn write_list(content: &str, key: &str, values: &[String]) -> String {
    if values.is_empty() && read_list(content, key).is_empty() {
        return content.to_string();
    }
    let (body, rest, bom) = parts(content);
    let mut body = body.unwrap_or_default();
    let items: Vec<String> = values
        .iter()
        .map(|value| quote(value.trim()))
        .filter(|value| value != "\"\"")
        .collect();
    let written = format!("{key}: [{}]", items.join(", "));
    if let Some(index) = find_key(&body, key) {
        let ending = line_ending(&body[index]).to_string();
        body[index] = format!("{written}{ending}");
    } else {
        let ending = body.last()
            .map(|line| line_ending(line))
            .unwrap_or("\n")
            .to_string();
        body.push(format!("{written}{ending}"));
    }
    assemble(&body, &rest, bom)
}
