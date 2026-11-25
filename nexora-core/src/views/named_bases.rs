//! Named Base views stored in app data (§74).
//!
//! Only the view configuration is persisted. Applying a view writes the
//! active `base.idx` through the existing `bases::save` path.

use super::bases::{self, BaseConfig};
use crate::NexoraError;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

const MAGIC: &str = "NEXORA-NAMED-BASES-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedBase {
    pub name: String,
    pub config: BaseConfig,
}

fn invalid(message: &str) -> NexoraError {
    NexoraError::InvalidInput {
        message: message.to_string(),
    }
}

fn valid_name(name: &str) -> Result<String, NexoraError> {
    let name = name.trim();
    if name.is_empty() || name.contains(['\t', '\n', '\r']) {
        return Err(invalid("Base view name must be a non-empty single line"));
    }
    Ok(name.to_string())
}

/// Replace an existing view in place, or append a new one (§74).
pub fn upsert(
    views: &[NamedBase],
    name: &str,
    config: &BaseConfig,
) -> Result<Vec<NamedBase>, NexoraError> {
    let name = valid_name(name)?;
    bases::validate_config(config)?;
    let mut result = views.to_vec();
    if let Some(existing) = result.iter_mut().find(|view| view.name == name) {
        existing.config = config.clone();
    } else {
        result.push(NamedBase {
            name,
            config: config.clone(),
        });
    }
    Ok(result)
}

/// Rename a view in place. Same-name is a no-op; collisions are errors (§74).
pub fn rename(
    views: &[NamedBase],
    old_name: &str,
    new_name: &str,
) -> Result<Vec<NamedBase>, NexoraError> {
    let new_name = valid_name(new_name)?;
    if !views.iter().any(|view| view.name == old_name) {
        return Err(invalid("no saved Base view with that name"));
    }
    if views
        .iter()
        .any(|view| view.name == new_name && new_name != old_name)
    {
        return Err(invalid("a saved Base view with that name already exists"));
    }
    Ok(views
        .iter()
        .map(|view| {
            if view.name == old_name {
                NamedBase {
                    name: new_name.clone(),
                    config: view.config.clone(),
                }
            } else {
                view.clone()
            }
        })
        .collect())
}

/// Remove a view by exact name, preserving the order of the rest (§74).
pub fn remove(views: &[NamedBase], name: &str) -> Vec<NamedBase> {
    views
        .iter()
        .filter(|view| view.name != name)
        .cloned()
        .collect()
}

pub fn save(path: &Path, views: &[NamedBase]) -> Result<(), NexoraError> {
    let mut text = format!("{MAGIC}\n");
    for view in views {
        let name = valid_name(&view.name)?;
        bases::validate_config(&view.config)?;
        text.push_str(&name);
        for field in bases::to_fields(&view.config) {
            text.push('\t');
            text.push_str(&field);
        }
        text.push('\n');
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| NexoraError::io_at(parent, error))?;
    }
    fs::write(path, text.as_bytes()).map_err(|error| NexoraError::io_at(path, error))
}

pub fn load(path: &Path) -> Result<Vec<NamedBase>, NexoraError> {
    if !path.exists() {
        return Ok(Vec::new());
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
        return Err(invalid("bad named-bases sidecar header"));
    }
    let mut views = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 8 {
            return Err(invalid("bad saved Base view row"));
        }
        views.push(NamedBase {
            name: valid_name(fields[0])?,
            config: bases::from_fields(&fields[1..]),
        });
    }
    Ok(views)
}
