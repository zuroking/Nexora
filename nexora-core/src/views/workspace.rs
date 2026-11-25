//! Saved workspaces in app data, keyed by vault (§42).

use crate::NexoraError;
use std::fs;
use std::path::Path;

const MAGIC: &str = "NEXORA-WORKSPACES-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceTab {
    pub kind: String,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    pub vault: String,
    pub name: String,
    pub tabs: Vec<WorkspaceTab>,
    pub active: usize,
}

fn invalid(message: &str) -> NexoraError {
    NexoraError::InvalidInput {
        message: message.to_string(),
    }
}

fn single_line(value: &str) -> bool {
    !value.is_empty() && !value.contains(['\t', '\n', '\r'])
}

pub fn capture(vault: &str, name: &str, tabs: &[(String, String)], active: usize) -> Workspace {
    Workspace {
        vault: vault.to_string(),
        name: name.to_string(),
        tabs: tabs
            .iter()
            .map(|(kind, id)| WorkspaceTab {
                kind: kind.clone(),
                id: id.clone(),
            })
            .collect(),
        active,
    }
}

pub fn upsert(workspaces: &[Workspace], workspace: Workspace) -> Vec<Workspace> {
    let mut result = workspaces.to_vec();
    if let Some(existing) = result.iter_mut().find(|item| {
        item.vault == workspace.vault && item.name == workspace.name
    }) {
        *existing = workspace;
    } else {
        result.push(workspace);
    }
    result
}

pub fn for_vault<'a>(workspaces: &'a [Workspace], vault: &str) -> Vec<&'a Workspace> {
    workspaces.iter().filter(|item| item.vault == vault).collect()
}

/// Rename one workspace, scoped to its vault. Tabs and active index are kept;
/// a same-name request is an explicit no-op (§66).
pub fn rename(
    workspaces: &[Workspace],
    vault: &str,
    old: &str,
    new: &str,
) -> Result<Vec<Workspace>, NexoraError> {
    let new = new.trim();
    if !single_line(new) {
        return Err(invalid("workspace name must be a single non-empty line"));
    }
    let mut result = workspaces.to_vec();
    let Some(index) = result
        .iter()
        .position(|item| item.vault == vault && item.name == old)
    else {
        return Err(invalid("workspace not found"));
    };
    if old == new {
        return Ok(result);
    }
    if result
        .iter()
        .any(|item| item.vault == vault && item.name == new)
    {
        return Err(invalid("workspace name already exists"));
    }
    result[index].name = new.to_string();
    Ok(result)
}

/// Remove one workspace by `(vault, name)`, preserving all others (§66).
pub fn remove(workspaces: &[Workspace], vault: &str, name: &str) -> Vec<Workspace> {
    workspaces
        .iter()
        .filter(|item| !(item.vault == vault && item.name == name))
        .cloned()
        .collect()
}

/// Restore index after missing notes have been skipped (§42.1). `kept[i]`
/// says whether descriptor `i` survived; the saved index is remapped to the
/// same position among survivors, not merely length-clamped.
pub fn remap_active(kept: &[bool], active: usize) -> usize {
    let survivors = kept.iter().filter(|flag| **flag).count();
    let before = kept
        .iter()
        .take(active.min(kept.len()))
        .filter(|flag| **flag)
        .count();
    // Clamp to the number of tabs that actually opened, not descriptors.
    before.min(survivors.saturating_sub(1))
}

pub fn save(path: &Path, workspaces: &[Workspace]) -> Result<(), NexoraError> {
    let mut text = format!("{MAGIC}\n");
    for workspace in workspaces {
        if !single_line(&workspace.vault) || !single_line(&workspace.name) {
            return Err(invalid("workspace vault and name must be single-line"));
        }
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            workspace.vault,
            workspace.name,
            workspace.active,
            workspace.tabs.len()
        ));
        for tab in &workspace.tabs {
            if tab.kind.contains(['\t', '\n', '\r']) || tab.id.contains(['\t', '\n', '\r']) {
                return Err(invalid("workspace tab fields must be single-line"));
            }
            text.push_str(&tab.kind);
            text.push('\t');
            text.push_str(&tab.id);
            text.push('\n');
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| NexoraError::io_at(parent, error))?;
    }
    fs::write(path, text.as_bytes()).map_err(|error| NexoraError::io_at(path, error))
}

pub fn load(path: &Path) -> Result<Vec<Workspace>, NexoraError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            NexoraError::InvalidUtf8 {
                path: path.to_path_buf(),
            }
        } else {
            NexoraError::io_at(path, error)
        }
    })?;
    let mut lines = text.lines();
    if lines.next() != Some(MAGIC) {
        return Err(invalid("bad workspaces sidecar header"));
    }
    let mut workspaces = Vec::new();
    while let Some(header) = lines.next() {
        let fields: Vec<&str> = header.split('\t').collect();
        if fields.len() != 4 || fields[0].is_empty() || fields[1].is_empty() {
            return Err(invalid("bad workspace row"));
        }
        let active: usize = fields[2].parse().map_err(|_| invalid("bad workspace active index"))?;
        let count: usize = fields[3].parse().map_err(|_| invalid("bad workspace tab count"))?;
        // Never trust an app-data count for allocation: a corrupt huge value
        // must return an error, not abort the process (§83).
        let mut tabs = Vec::new();
        for _ in 0..count {
            let line = lines.next().ok_or_else(|| invalid("truncated workspace"))?;
            let (kind, id) = line.split_once('\t').ok_or_else(|| invalid("bad workspace tab"))?;
            if kind.is_empty() {
                return Err(invalid("bad workspace tab kind"));
            }
            tabs.push(WorkspaceTab {
                kind: kind.to_string(),
                id: id.to_string(),
            });
        }
        workspaces.push(Workspace {
            vault: fields[0].to_string(),
            name: fields[1].to_string(),
            tabs,
            active,
        });
    }
    Ok(workspaces)
}
