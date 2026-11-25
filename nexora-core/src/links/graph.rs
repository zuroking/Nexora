//! Deterministic graph layout. No RNG, no wall-clock input, no graph crate.

use crate::{Link, NoteId, VaultIndex};
use std::collections::{BTreeMap, BTreeSet};

const MAX_NODES: usize = 500;
const ITERATIONS: usize = 50;
const REPULSION: f64 = 0.0005;
const ATTRACTION: f64 = 0.01;
const TARGET_LENGTH: f64 = 0.3;
const DAMPING: f64 = 0.85;

#[derive(Clone, Debug, PartialEq)]
pub struct GraphNode {
    pub id: NoteId,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphEdge {
    pub source: NoteId,
    pub target: NoteId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphLayout {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub truncated: bool,
}

/// Stable identity of the visible node set for GUI selection reset (§77).
/// Debug of the id vector quotes/escapes separators, like §72/§76.
pub fn selection_key(layout: &GraphLayout) -> String {
    format!("{:?}", layout.nodes.iter().map(|node| &node.id).collect::<Vec<_>>())
}

/// Number of resolved edges incident to a note, outgoing plus incoming.
fn degree(index: &VaultIndex, id: &NoteId) -> usize {
    let outgoing = index
        .links
        .get(id)
        .map(|links| {
            links
                .iter()
                .filter(|link| matches!(link, Link::Resolved { .. }))
                .count()
        })
        .unwrap_or(0);
    let incoming = index
        .links
        .values()
        .flatten()
        .filter(|link| matches!(link, Link::Resolved { target, .. } if target == id))
        .count();
    outgoing + incoming
}

/// Build a layout from resolved links. When the vault is larger than 500
/// notes, only the 500 highest-degree notes are rendered.
pub fn build(index: &VaultIndex) -> GraphLayout {
    let all: Vec<&NoteId> = index.notes.keys().collect();
    let truncated = all.len() > MAX_NODES;
    let selected: Vec<NoteId> = if truncated {
        let mut scored: Vec<(usize, NoteId)> = all
            .iter()
            .map(|id| (degree(index, id), (*id).clone()))
            .collect();
        scored.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.cmp(&right.1))
        });
        scored
            .into_iter()
            .take(MAX_NODES)
            .map(|(_, id)| id)
            .collect()
    } else {
        all.into_iter().cloned().collect()
    };

    let allowed: BTreeSet<&NoteId> = selected.iter().collect();
    let mut edge_set: BTreeSet<(NoteId, NoteId)> = BTreeSet::new();
    for (source, links) in &index.links {
        if !allowed.contains(source) {
            continue;
        }
        for link in links {
            if let Link::Resolved { target, .. } = link {
                if allowed.contains(target) && source != target {
                    let pair = if source < target {
                        ((*source).clone(), (*target).clone())
                    } else {
                        ((*target).clone(), (*source).clone())
                    };
                    edge_set.insert(pair);
                }
            }
        }
    }
    let edges: Vec<GraphEdge> = edge_set
        .into_iter()
        .map(|(source, target)| GraphEdge { source, target })
        .collect();

    let count = selected.len();
    let mut positions: Vec<(f64, f64)> = selected
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let angle = 2.0 * std::f64::consts::PI * index as f64 / count as f64;
            (angle.cos(), angle.sin())
        })
        .collect();
    let positions_by_id: BTreeMap<&NoteId, usize> = selected
        .iter()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect();

    for _ in 0..ITERATIONS {
        let mut forces = vec![(0.0_f64, 0.0_f64); count];
        for first in 0..count {
            for second in (first + 1)..count {
                let dx = positions[first].0 - positions[second].0;
                let dy = positions[first].1 - positions[second].1;
                let squared = dx * dx + dy * dy;
                if squared > 1e-12 {
                    let distance = squared.sqrt();
                    let magnitude = REPULSION / squared;
                    let fx = dx / distance * magnitude;
                    let fy = dy / distance * magnitude;
                    forces[first].0 += fx;
                    forces[first].1 += fy;
                    forces[second].0 -= fx;
                    forces[second].1 -= fy;
                }
            }
        }
        for edge in &edges {
            let first = positions_by_id[&edge.source];
            let second = positions_by_id[&edge.target];
            let dx = positions[second].0 - positions[first].0;
            let dy = positions[second].1 - positions[first].1;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > 1e-12 {
                let magnitude = ATTRACTION * (distance - TARGET_LENGTH);
                let fx = dx / distance * magnitude;
                let fy = dy / distance * magnitude;
                forces[first].0 += fx;
                forces[first].1 += fy;
                forces[second].0 -= fx;
                forces[second].1 -= fy;
            }
        }
        for (position, force) in positions.iter_mut().zip(forces) {
            position.0 = (position.0 + force.0 * DAMPING).clamp(-10.0, 10.0);
            position.1 = (position.1 + force.1 * DAMPING).clamp(-10.0, 10.0);
        }
    }

    let largest = positions
        .iter()
        .flat_map(|(x, y)| [*x, *y])
        .fold(0.0_f64, |current, value| current.max(value.abs()));
    if largest > 0.0 {
        for position in &mut positions {
            position.0 /= largest;
            position.1 /= largest;
        }
    }

    let nodes = selected
        .into_iter()
        .zip(positions)
        .map(|(id, (x, y))| GraphNode { id, x, y })
        .collect();

    GraphLayout {
        nodes,
        edges,
        truncated,
    }
}
