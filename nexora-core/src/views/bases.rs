//! Read-only frontmatter views ("Bases-lite"). See `ARCHITECTURE.md` §29.
//!
//! A view is a rebuildable projection of `VaultIndex`; the vault is never
//! modified and no view file is written inside it.

use crate::frontmatter::{read_field, read_list, write_field, write_list};
use crate::{NexoraError, NoteId, VaultIndex};
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

const MAGIC: &str = "NEXORA-BASE-v1";
const FIELDS: [&str; 4] = ["created", "modified", "tags", "aliases"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy {
    Title,
    Path,
    Created,
    Modified,
    /// Sort by `BaseConfig::sort_field` (§59).
    Field,
}

impl SortBy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Path => "path",
            Self::Created => "created",
            Self::Modified => "modified",
            Self::Field => "field",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "path" => Self::Path,
            "created" => Self::Created,
            "modified" => Self::Modified,
            "field" => Self::Field,
            _ => Self::Title,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseConfig {
    pub filter_tag: String,
    pub sort_by: SortBy,
    pub group_by: String,
    /// Scalar frontmatter keys shown as extra table columns (§58).
    pub extra_columns: Vec<String>,
    /// Scalar frontmatter key used when `sort_by` is `Field` (§59).
    pub sort_field: String,
    /// Reverse the primary sort key while keeping the id tiebreak stable (§60).
    pub sort_desc: bool,
    /// Case-insensitive substring over title, path and projected values (§62).
    pub filter_text: String,
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self {
            filter_tag: String::new(),
            sort_by: SortBy::Title,
            group_by: String::new(),
            extra_columns: Vec::new(),
            sort_field: String::new(),
            sort_desc: false,
            filter_text: String::new(),
        }
    }
}

/// Reserved built-in columns cannot be added twice (§58, §59).
pub fn valid_column_key(key: &str) -> bool {
    !key.is_empty()
        && !key.contains([',', '\t', '\n', '\r'])
        && !FIELDS.contains(&key)
        && key != "modified"
}

/// Direction for keyboard navigation of a Base grid (§80).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
}

/// Move a grid selection, clamping at the edges (§80). `None` selects (0, 0)
/// when the grid is non-empty. Empty axes always yield `None`.
pub fn move_selection(
    rows: usize,
    columns: usize,
    current: Option<(usize, usize)>,
    direction: Move,
) -> Option<(usize, usize)> {
    if rows == 0 || columns == 0 {
        return None;
    }
    let Some((row, column)) = current else {
        return Some((0, 0));
    };
    let row = row.min(rows - 1);
    let column = column.min(columns - 1);
    Some(match direction {
        Move::Up => (row.saturating_sub(1), column),
        Move::Down => ((row + 1).min(rows - 1), column),
        Move::Left => (row, column.saturating_sub(1)),
        Move::Right => (row, (column + 1).min(columns - 1)),
    })
}

/// Apply the configured direction to a total ordering (§60).
fn with_direction(order: std::cmp::Ordering, desc: bool) -> std::cmp::Ordering {
    if desc { order.reverse() } else { order }
}

/// Lexicographic comparison where missing values always sort last, in both
/// directions (§59, §60).
fn compare_empty_last(left: &str, right: &str, desc: bool) -> std::cmp::Ordering {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        (false, false) => with_direction(left.cmp(right), desc),
    }
}

/// Add a custom column, returning `false` for invalid or duplicate keys.
pub fn add_column(config: &mut BaseConfig, key: &str) -> bool {
    let key = key.trim();
    if !valid_column_key(key) || config.extra_columns.iter().any(|existing| existing == key) {
        return false;
    }
    config.extra_columns.push(key.to_string());
    true
}

/// Set the sort field, normalising invalid or empty keys to `""` (§59 review).
/// The GUI keeps its raw draft in a separate buffer, so a half-typed key never
/// changes the effective sort or gets persisted.
pub fn set_sort_field(config: &mut BaseConfig, key: &str) -> bool {
    let key = key.trim();
    if !valid_column_key(key) {
        config.sort_field.clear();
        return false;
    }
    config.sort_field = key.to_string();
    true
}

/// Remove a custom column. Returns `true` when something was removed.
pub fn remove_column(config: &mut BaseConfig, key: &str) -> bool {
    let before = config.extra_columns.len();
    config.extra_columns.retain(|existing| existing != key.trim());
    config.extra_columns.len() != before
}

/// True when the projection is narrowed by a tag or text filter (§63).
pub fn has_filters(config: &BaseConfig) -> bool {
    !config.filter_tag.trim().is_empty() || !config.filter_text.trim().is_empty()
}

/// Clear only the two filters, preserving sort, group and column settings (§63).
pub fn clear_filters(config: &mut BaseConfig) {
    config.filter_tag.clear();
    config.filter_text.clear();
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: NoteId,
    pub title: String,
    pub values: BTreeMap<String, String>,
    /// Empty when no grouping is active; otherwise one value per row.
    pub group: String,
}

fn field_values(content: &str, config: &BaseConfig) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let group_key = config.group_by.trim();
    let sort_key = config.sort_field.trim();
    let keys = FIELDS
        .iter()
        .copied()
        .chain((!group_key.is_empty()).then_some(group_key))
        .chain(config.extra_columns.iter().map(String::as_str))
        .chain((valid_column_key(sort_key)).then_some(sort_key));
    for key in keys {
        if values.contains_key(key) {
            continue;
        }
        let value = if key == "tags" || key == "aliases" {
            read_list(content, key).join(", ")
        } else {
            read_field(content, key).unwrap_or_default()
        };
        values.insert(key.to_string(), value);
    }
    values
}

/// Project the vault into rows. A grouped list field expands a note into one
/// row per value; that is display-only and never changes `NoteId`.
pub fn build(index: &VaultIndex, config: &BaseConfig) -> Vec<Row> {
    // Match `has_filters` (§63): a whitespace-only tag is not a filter.
    let tag = config.filter_tag.trim();
    let allowed = if tag.is_empty() {
        None
    } else {
        index.tags.get(tag).cloned()
    };
    let needle = config.filter_text.trim().to_lowercase();
    let mut rows = Vec::new();
    for (id, note) in &index.notes {
        if let Some(ids) = &allowed {
            if !ids.contains(id) {
                continue;
            }
        }
        let values = field_values(&note.content, config);
        if !needle.is_empty() {
            let matches = note.title.to_lowercase().contains(&needle)
                || id.as_str().to_lowercase().contains(&needle)
                || values
                    .values()
                    .any(|value| value.to_lowercase().contains(&needle));
            if !matches {
                continue;
            }
        }
        let group_key = config.group_by.trim();
        let groups: Vec<String> = if group_key.is_empty() {
            vec![String::new()]
        } else if group_key == "tags" || group_key == "aliases" {
            let values = read_list(&note.content, group_key);
            if values.is_empty() {
                vec![String::new()]
            } else {
                values
            }
        } else {
            read_field(&note.content, group_key)
                .map(|value| vec![value])
                .unwrap_or_else(|| vec![String::new()])
        };
        for group in groups {
            rows.push(Row {
                id: id.clone(),
                title: note.title.clone(),
                values: values.clone(),
                group,
            });
        }
    }
    // A stale or half-typed key must behave exactly like an empty one, both in
    // this frame and after a sidecar reload (§59 review).
    let candidate = config.sort_field.trim();
    let sort_field = if valid_column_key(candidate) { candidate } else { "" };
    rows.sort_by(|left, right| {
        let desc = config.sort_desc;
        let primary = match config.sort_by {
            SortBy::Title => with_direction(left.title.cmp(&right.title), desc),
            SortBy::Path => with_direction(left.id.cmp(&right.id), desc),
            SortBy::Created => compare_empty_last(
                left.values.get("created").map(String::as_str).unwrap_or(""),
                right.values.get("created").map(String::as_str).unwrap_or(""),
                desc,
            ),
            SortBy::Modified => {
                let a = index.notes.get(&left.id).map(|note| note.modified);
                let b = index.notes.get(&right.id).map(|note| note.modified);
                with_direction(a.cmp(&b), desc)
            }
            SortBy::Field => {
                if sort_field.is_empty() {
                    with_direction(left.title.cmp(&right.title), desc)
                } else {
                    compare_empty_last(
                        left.values.get(sort_field).map(String::as_str).unwrap_or(""),
                        right.values.get(sort_field).map(String::as_str).unwrap_or(""),
                        desc,
                    )
                }
            }
        };
        // Only the chosen key reverses; equal rows stay id-ascending (§60).
        primary.then_with(|| left.id.cmp(&right.id))
    });
    rows
}

/// Columns a Base table may edit. `modified` is file-derived and stays
/// display-only; duplicates from `group_by` are collapsed (§56).
pub fn editable_columns(config: &BaseConfig) -> Vec<String> {
    let mut columns = vec!["created", "tags", "aliases"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    for key in &config.extra_columns {
        if !columns.iter().any(|column| column == key) {
            columns.push(key.clone());
        }
    }
    let group = config.group_by.trim();
    if !group.is_empty()
        && group != "modified"
        && !columns.iter().any(|column| column == group)
    {
        columns.push(group.to_string());
    }
    columns
}

/// Apply one Base cell edit to note content. List keys are comma-split;
/// duplicates are kept because they are meaningful in list fields (e.g. a
/// note can carry the same tag twice in its source). No I/O and no projection
/// state is involved — the caller owns the guarded write (§56).
pub fn edit_cell(content: &str, key: &str, value: &str) -> String {
    if key == "tags" || key == "aliases" {
        let values = value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if read_list(content, key) == values {
            return content.to_string();
        }
        write_list(content, key, &values)
    } else {
        let value = value.trim();
        if read_field(content, key).as_deref() == Some(value) {
            return content.to_string();
        }
        write_field(content, key, value)
    }
}

/// Seven single-line fields of the Base sidecar format, in stable order
/// (§56–§62, §74). Shared by the active view and named views.
pub fn to_fields(config: &BaseConfig) -> Vec<String> {
    vec![
        config.filter_tag.clone(),
        config.sort_by.label().to_string(),
        config.group_by.clone(),
        config.extra_columns.join(","),
        config.sort_field.clone(),
        if config.sort_desc { "desc" } else { "asc" }.to_string(),
        config.filter_text.clone(),
    ]
}

/// Reject fields that cannot be stored as one TSV row (§74).
pub fn validate_config(config: &BaseConfig) -> Result<(), NexoraError> {
    for value in [
        &config.filter_tag,
        &config.group_by,
        &config.sort_field,
        &config.filter_text,
    ] {
        if value.contains(['\t', '\n', '\r']) {
            return Err(crate::NexoraError::InvalidInput {
                message: "Base view fields must be single-line".to_string(),
            });
        }
    }
    // `from_fields` trims keys and revalidates them; reject untrimmed values
    // here so `load(save(view))` is byte-for-byte stable (§74 review).
    let trimmed_key = |key: &str| {
        !key.is_empty() && valid_column_key(key) && key == key.trim()
    };
    for key in &config.extra_columns {
        if !trimmed_key(key) {
            return Err(crate::NexoraError::InvalidInput {
                message: "invalid Base column key".to_string(),
            });
        }
    }
    if !config.sort_field.is_empty() && !trimmed_key(&config.sort_field) {
        return Err(crate::NexoraError::InvalidInput {
            message: "invalid Base sort field".to_string(),
        });
    }
    Ok(())
}

/// Parse a Base config from its fields. Missing trailing fields use defaults,
/// preserving the 4/5/6/7-line sidecar compatibility (§56–§62).
pub fn from_fields(fields: &[&str]) -> BaseConfig {
    let get = |index: usize| fields.get(index).copied().unwrap_or("");
    let filter_tag = get(0).to_string();
    let sort_by = SortBy::parse(get(1));
    let group_by = get(2).to_string();
    // Invalid keys are skipped instead of making an otherwise valid config
    // unreadable (§56 review).
    let mut extra_columns: Vec<String> = Vec::new();
    for key in get(3).split(',').map(str::trim) {
        if valid_column_key(key) && !extra_columns.iter().any(|existing| existing == key) {
            extra_columns.push(key.to_string());
        }
    }
    let sort_field = get(4).trim();
    let sort_field = if valid_column_key(sort_field) {
        sort_field.to_string()
    } else {
        String::new()
    };
    let sort_desc = matches!(get(5).trim(), "desc");
    let filter_text = get(6).to_string();
    BaseConfig {
        filter_tag,
        sort_by,
        group_by,
        extra_columns,
        sort_field,
        sort_desc,
        filter_text,
    }
}

pub fn save(path: &Path, config: &BaseConfig) -> Result<(), NexoraError> {
    let fields = to_fields(config);
    let text = format!("{MAGIC}\n{}\n", fields.join("\n"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| NexoraError::io_at(parent, error))?;
    }
    fs::write(path, text.as_bytes()).map_err(|error| NexoraError::io_at(path, error))
}

pub fn load(path: &Path) -> Result<BaseConfig, NexoraError> {
    if !path.exists() {
        return Ok(BaseConfig::default());
    }
    let text = fs::read_to_string(path).map_err(|error| {
        if error.kind() == ErrorKind::InvalidData {
            NexoraError::InvalidUtf8 {
                path: path.to_path_buf(),
            }
        } else {
            NexoraError::io_at(path, error)
        }
    })?;
    let mut lines = text.lines();
    if lines.next() != Some(MAGIC) {
        return Err(NexoraError::InvalidInput {
            message: "bad base sidecar header".to_string(),
        });
    }
    Ok(from_fields(&lines.collect::<Vec<_>>()))
}
