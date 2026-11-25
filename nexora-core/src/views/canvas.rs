//! Deterministic canvas layout and its rebuildable sidecar.
//!
//! Positions are UI state, not vault data: the sidecar lives in app data and
//! can be deleted at any time without losing a note. See `ARCHITECTURE.md` §27.

use crate::{NexoraError, NoteId, VaultIndex};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

const MAGIC: &str = "NEXORA-CANVAS-v1";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pos {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Canvas {
    pub nodes: BTreeMap<NoteId, Pos>,
}

/// Stable identity of the node set for GUI selection reset (§76). BTreeMap
/// order makes it identical across frames and independent of drag positions.
pub fn selection_key(canvas: &Canvas) -> String {
    // Debug of the key vector quotes and escapes ids, so a `;` inside a
    // Windows filename cannot forge a node boundary (§76 review, cf. §72).
    format!("{:?}", canvas.nodes.keys().collect::<Vec<_>>())
}

/// Grid layout in `NoteId` order: `ceil(sqrt(n))` columns, 260×110 spacing.
pub fn initial(index: &VaultIndex) -> Canvas {
    let count = index.notes.len();
    if count == 0 {
        return Canvas::default();
    }
    let columns = (count as f64).sqrt().ceil() as usize;
    let mut nodes = BTreeMap::new();
    for (position, id) in index.notes.keys().enumerate() {
        nodes.insert(
            id.clone(),
            Pos {
                x: (position % columns) as f64 * 260.0 + 130.0,
                y: (position / columns) as f64 * 110.0 + 70.0,
            },
        );
    }
    Canvas { nodes }
}

/// Keep known positions, add new notes to the grid, prune removed notes.
pub fn reconcile(stored: Option<Canvas>, index: &VaultIndex) -> Canvas {
    let fresh = initial(index);
    match stored {
        None => fresh,
        Some(stored) => {
            let mut nodes = BTreeMap::new();
            for (id, pos) in fresh.nodes {
                nodes.insert(
                    id.clone(),
                    stored.nodes.get(&id).copied().unwrap_or(pos),
                );
            }
            Canvas { nodes }
        }
    }
}

/// Undirected, deduplicated pairs of resolved wikilinks.
pub fn edges(index: &VaultIndex) -> Vec<(NoteId, NoteId)> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for (source, links) in &index.links {
        for link in links {
            if let crate::Link::Resolved { target, .. } = link {
                let pair = if source <= target {
                    (source.clone(), target.clone())
                } else {
                    (target.clone(), source.clone())
                };
                if seen.insert(pair.clone()) {
                    result.push(pair);
                }
            }
        }
    }
    result
}

pub fn save(path: &Path, canvas: &Canvas) -> Result<(), NexoraError> {
    let mut text = format!("{MAGIC} {}\n", canvas.nodes.len());
    for (id, pos) in &canvas.nodes {
        text.push_str(&format!(
            "{}\t{}\t{}\n",
            id.as_str(),
            pos.x.to_bits(),
            pos.y.to_bits()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| NexoraError::io_at(parent, error))?;
    }
    fs::write(path, text.as_bytes()).map_err(|error| NexoraError::io_at(path, error))
}

pub fn load(path: &Path) -> Result<Canvas, NexoraError> {
    if !path.exists() {
        return Ok(Canvas::default());
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
    let header = lines.next().unwrap_or("");
    let mut parts = header.split_whitespace();
    if parts.next() != Some(MAGIC) {
        return Err(NexoraError::InvalidInput {
            message: "bad canvas sidecar header".to_string(),
        });
    }
    let declared: usize = parts
        .next()
        .ok_or_else(|| NexoraError::InvalidInput {
            message: "canvas sidecar has no count".to_string(),
        })?
        .parse()
        .map_err(|_| NexoraError::InvalidInput {
            message: "canvas sidecar count is not a number".to_string(),
        })?;
    let mut nodes = BTreeMap::new();
    for line in lines {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 {
            continue;
        }
        let x_bits: u64 = match fields[1].parse() {
            Ok(value) => value,
            Err(_) => continue,
        };
        let y_bits: u64 = match fields[2].parse() {
            Ok(value) => value,
            Err(_) => continue,
        };
        nodes.insert(
            NoteId::new(fields[0]),
            Pos {
                x: f64::from_bits(x_bits),
                y: f64::from_bits(y_bits),
            },
        );
    }
    if nodes.len() != declared {
        return Err(NexoraError::InvalidInput {
            message: "canvas sidecar count mismatch".to_string(),
        });
    }
    Ok(Canvas { nodes })
}
