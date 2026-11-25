//! Templates with a closed placeholder set, stored in app data (§36).
//!
//! Only `{{date}}` and `{{title}}` are substituted. Any other `{{...}}`
//! sequence is left verbatim — there is no scripting and no user-defined
//! placeholder syntax.

use super::lists::line_bounds;
use crate::{daily, NexoraError};
use std::fs;
use std::path::Path;

const MAGIC: &str = "NEXORA-TEMPLATES-v1";
const OPEN: &str = "{{";

/// Name of the template auto-applied to a new daily note (§37.1).
pub const DAILY_TEMPLATE: &str = "daily";

/// Resolver table: the single definition of the closed placeholder set. Both
/// [`render`] and the GUI's advertised list come from here.
const RESOLVERS: [(&str, for<'a, 'b> fn(&'a Fields<'b>) -> &'b str); 2] = [
    ("date", |fields| fields.date),
    ("title", |fields| fields.title),
];

/// Placeholders recognised by [`render`]. Exposed so the GUI can display the
/// closed set without inventing its own list.
pub const PLACEHOLDERS: [&str; 2] = [RESOLVERS[0].0, RESOLVERS[1].0];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    pub name: String,
    pub body: String,
}

/// Values substituted into a template body.
pub struct Fields<'a> {
    pub date: &'a str,
    pub title: &'a str,
}

/// The template shipped when there is no sidecar yet.
pub fn builtin() -> Template {
    Template {
        name: DAILY_TEMPLATE.to_string(),
        body: "# {{title}}\n\nCreated {{date}}\n\n".to_string(),
    }
}

/// Single-pass substitution. Substituting values never re-enters the scanner,
/// so a date containing `{{title}}` is not expanded a second time.
pub fn render(body: &str, fields: &Fields<'_>) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        let after = &rest[start + OPEN.len()..];
        let Some(end) = after.find("}}") else {
            out.push_str(OPEN);
            rest = after;
            continue;
        };
        let key = &after[..end];
        match RESOLVERS.iter().find(|(name, _)| *name == key) {
            Some((_, resolve)) => out.push_str(resolve(fields)),
            None => {
                out.push_str(OPEN);
                out.push_str(key);
                out.push_str("}}");
            }
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Result of inserting a rendered template: the new text and caret (§75).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateEdit {
    pub text: String,
    pub cursor: usize,
}

/// Insert a rendered template block at a character-indexed caret. The host
/// note's line-ending style is kept, and a mid-line caret gets blank-line
/// separation so the template never merges into surrounding text (§75).
pub fn insert_at(text: &str, cursor: usize, body: &str) -> TemplateEdit {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    if body.is_empty() {
        return TemplateEdit {
            text: text.to_string(),
            cursor,
        };
    }

    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let block = body
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>()
        .join(newline);
    let after_starts_newline = chars
        .get(cursor)
        .is_some_and(|character| matches!(character, '\n' | '\r'));
    // A blank line immediately after the caret already provides the trailing
    // separation; adding another would produce two blank lines (§75 review).
    let blank_after = if chars.get(cursor) == Some(&'\n') {
        chars.get(cursor + 1) == Some(&'\n')
    } else if chars.get(cursor) == Some(&'\r')
        && chars.get(cursor + 1) == Some(&'\n')
    {
        chars.get(cursor + 2) == Some(&'\r') && chars.get(cursor + 3) == Some(&'\n')
    } else {
        false
    };

    let (line_start, _) = line_bounds(&chars, cursor);
    let before: String = chars[line_start..cursor].iter().collect();
    let mid_line = cursor > line_start
        && before
            .chars()
            .filter(|character| *character != '\u{feff}')
            .any(|character| !character.is_whitespace());

    // §75: a mid-line caret gets a blank line on both sides of the block.
    let mut insertion = String::with_capacity(block.len() + 4 * newline.len());
    if mid_line {
        insertion.push_str(newline);
        insertion.push_str(newline);
    }
    insertion.push_str(&block);
    if mid_line && !blank_after {
        insertion.push_str(newline);
        if !after_starts_newline {
            insertion.push_str(newline);
        }
    } else if !mid_line && !after_starts_newline {
        insertion.push_str(newline);
    }

    let mut out = String::with_capacity(text.len() + insertion.len());
    out.extend(chars[..cursor].iter());
    out.push_str(&insertion);
    out.extend(chars[cursor..].iter());
    TemplateEdit {
        text: out,
        cursor: cursor + insertion.chars().count(),
    }
}

/// Body for a newly created daily note (§37.1). The choice is deterministic
/// and lives here so it can be tested without a GUI: the `daily` template if
/// present, otherwise the first one, otherwise no body. Both placeholders
/// resolve to today's date.
pub fn daily_body(templates: &[Template]) -> String {
    let template = templates
        .iter()
        .find(|template| template.name == DAILY_TEMPLATE)
        .or_else(|| templates.first());
    let date = daily::today_id();
    match template {
        Some(template) => render(
            &template.body,
            &Fields {
                date: date.as_str(),
                title: date.as_str(),
            },
        ),
        None => String::new(),
    }
}

fn invalid(message: &str) -> NexoraError {
    NexoraError::InvalidInput {
        message: message.to_string(),
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['|', '\t', '\n', '\r'])
}

/// Insert or replace a template by name, preserving the order of the rest (§70).
pub fn upsert(templates: &[Template], template: Template) -> Result<Vec<Template>, NexoraError> {
    let mut template = template;
    template.name = template.name.trim().to_string();
    if !valid_name(&template.name) {
        return Err(invalid("template name must be one line without '|'"));
    }
    let mut result = templates.to_vec();
    if let Some(existing) = result.iter_mut().find(|item| item.name == template.name) {
        *existing = template;
    } else {
        result.push(template);
    }
    Ok(result)
}

/// Rename a template. `daily` is protected because §37 resolves it by name (§70).
pub fn rename(
    templates: &[Template],
    old: &str,
    new: &str,
) -> Result<Vec<Template>, NexoraError> {
    let new = new.trim();
    if !valid_name(new) {
        return Err(invalid("template name must be one line without '|'"));
    }
    if old == DAILY_TEMPLATE || new == DAILY_TEMPLATE {
        return Err(invalid("the daily template cannot be renamed"));
    }
    let mut result = templates.to_vec();
    let Some(index) = result.iter().position(|item| item.name == old) else {
        return Err(invalid("template not found"));
    };
    if old == new {
        return Ok(result);
    }
    if result.iter().any(|item| item.name == new) {
        return Err(invalid("template name already exists"));
    }
    result[index].name = new.to_string();
    Ok(result)
}

/// Remove a template. `daily` is protected (§37/§70).
pub fn remove(templates: &[Template], name: &str) -> Result<Vec<Template>, NexoraError> {
    if name == DAILY_TEMPLATE {
        return Err(invalid("the daily template cannot be removed"));
    }
    let result: Vec<_> = templates
        .iter()
        .filter(|item| item.name != name)
        .cloned()
        .collect();
    if result.len() == templates.len() {
        Err(invalid("template not found"))
    } else {
        Ok(result)
    }
}

pub fn load(path: &Path) -> Result<Vec<Template>, NexoraError> {
    if !path.exists() {
        return Ok(vec![builtin()]);
    }
    let bytes = fs::read(path).map_err(|error| NexoraError::io_at(path, error))?;
    let mut rest = bytes.as_slice();
    let newline = rest.iter().position(|byte| *byte == b'\n').ok_or_else(|| invalid("bad templates sidecar header"))?;
    if &rest[..newline] != MAGIC.as_bytes() {
        return Err(invalid("bad templates sidecar header"));
    }
    rest = &rest[newline + 1..];

    let mut templates = Vec::new();
    while !rest.is_empty() {
        let newline = rest.iter().position(|byte| *byte == b'\n').ok_or_else(|| invalid("truncated template header"))?;
        let header = std::str::from_utf8(&rest[..newline])
            .map_err(|_| NexoraError::InvalidUtf8 { path: path.to_path_buf() })?;
        let (name, length) = header.split_once('|').ok_or_else(|| invalid("bad template header"))?;
        let length: usize = length.parse().map_err(|_| invalid("bad template body length"))?;
        let body_start = newline + 1;
        let body_end = body_start.checked_add(length).ok_or_else(|| invalid("bad template body length"))?;
        if body_end >= rest.len() || rest[body_end] != b'\n' {
            return Err(invalid("truncated template body"));
        }
        let body = std::str::from_utf8(&rest[body_start..body_end])
            .map_err(|_| NexoraError::InvalidUtf8 { path: path.to_path_buf() })?;
        templates.push(Template {
            name: name.to_string(),
            body: body.to_string(),
        });
        rest = &rest[body_end + 1..];
    }
    Ok(templates)
}

/// Write the byte-length sidecar used by [`load`] (§70). Names are validated so
/// a malformed header can never make the file unreadable on the next launch.
pub fn save(path: &Path, templates: &[Template]) -> Result<(), NexoraError> {
    let mut bytes = format!("{MAGIC}\n").into_bytes();
    for template in templates {
        if !valid_name(&template.name) {
            return Err(invalid("template name must be one line without '|'"));
        }
        bytes.extend_from_slice(
            format!("{}|{}\n", template.name, template.body.len()).as_bytes(),
        );
        bytes.extend_from_slice(template.body.as_bytes());
        bytes.push(b'\n');
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| NexoraError::io_at(parent, error))?;
    }
    fs::write(path, &bytes).map_err(|error| NexoraError::io_at(path, error))
}
