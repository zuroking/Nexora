//! Explainable related-note suggestions built on existing local indexes.
//!
//! No new dependency, no network, no writes. See `ARCHITECTURE.md` §26.

use crate::semantic::{cosine, SemanticIndex};
use crate::{Link, NoteId, VaultIndex};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const W_SEMANTIC: f32 = 0.5;
const W_TAG: f32 = 0.25;
const W_GRAPH: f32 = 0.25;
const MAX_DEPTH: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Factors {
    pub semantic: f32,
    pub tag: f32,
    pub graph: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Related {
    pub id: NoteId,
    pub score: f32,
    pub factors: Factors,
}

fn adjacency(index: &VaultIndex) -> BTreeMap<NoteId, BTreeSet<NoteId>> {
    let mut edges: BTreeMap<NoteId, BTreeSet<NoteId>> = BTreeMap::new();
    for (source, links) in &index.links {
        for link in links {
            if let Link::Resolved { target, .. } = link {
                edges.entry(source.clone()).or_default().insert(target.clone());
                edges.entry(target.clone()).or_default().insert(source.clone());
            }
        }
    }
    edges
}

/// Undirected hop distance, capped at `MAX_DEPTH`. `None` means unreachable.
fn distance(edges: &BTreeMap<NoteId, BTreeSet<NoteId>>, from: &NoteId, to: &NoteId) -> Option<usize> {
    if from == to {
        return Some(0);
    }
    let mut seen: BTreeSet<NoteId> = BTreeSet::new();
    let mut queue: VecDeque<(NoteId, usize)> = VecDeque::new();
    queue.push_back((from.clone(), 0));
    seen.insert(from.clone());
    while let Some((node, depth)) = queue.pop_front() {
        if depth >= MAX_DEPTH {
            continue;
        }
        for next in edges.get(&node).into_iter().flatten() {
            if next == to {
                return Some(depth + 1);
            }
            if seen.insert(next.clone()) {
                queue.push_back((next.clone(), depth + 1));
            }
        }
    }
    None
}

fn tag_set(index: &VaultIndex, id: &NoteId) -> BTreeSet<String> {
    index
        .tags
        .iter()
        .filter(|(_, notes)| notes.contains(id))
        .map(|(tag, _)| tag.clone())
        .collect()
}

fn jaccard(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count();
    let union = left.union(right).count();
    intersection as f32 / union as f32
}

fn semantic_similarity(semantic: &SemanticIndex, from: &NoteId, to: &NoteId) -> f32 {
    let left: Vec<&[f32]> = semantic
        .entries
        .iter()
        .filter(|entry| &entry.id == from)
        .map(|entry| entry.vector.as_slice())
        .collect();
    let right: Vec<&[f32]> = semantic
        .entries
        .iter()
        .filter(|entry| &entry.id == to)
        .map(|entry| entry.vector.as_slice())
        .collect();
    let mut best = 0.0_f32;
    for a in &left {
        for b in &right {
            best = best.max(cosine(a, b));
        }
    }
    best.clamp(0.0, 1.0)
}

/// Pure ranking. Returns at most `limit` notes, best first.
pub fn suggest(
    index: &VaultIndex,
    semantic: &SemanticIndex,
    target: &NoteId,
    limit: usize,
) -> Vec<Related> {
    if limit == 0 || !index.notes.contains_key(target) {
        return Vec::new();
    }
    let edges = adjacency(index);
    let own_tags = tag_set(index, target);
    let mut rows = Vec::new();
    for id in index.notes.keys() {
        if id == target {
            continue;
        }
        let sem = semantic_similarity(semantic, target, id);
        let tag = jaccard(&own_tags, &tag_set(index, id));
        let graph = match distance(&edges, target, id) {
            Some(depth) if depth > 0 => 1.0 / depth as f32,
            _ => 0.0,
        };
        let score = W_SEMANTIC * sem + W_TAG * tag + W_GRAPH * graph;
        if score <= 0.001 {
            continue;
        }
        rows.push(Related {
            id: id.clone(),
            score,
            factors: Factors { semantic: sem, tag, graph },
        });
    }
    rows.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.id.cmp(&b.id))
    });
    rows.truncate(limit);
    rows
}
