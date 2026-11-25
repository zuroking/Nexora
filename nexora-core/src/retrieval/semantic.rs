//! Part A of the local vector index: pure Rust, no model, no network.
//!
//! Embeddings are hashed character 3-grams plus word tokens. This is a lexical
//! vector space — it cannot infer that "аттестация" means "exam". The UI must
//! label it honestly. See `ARCHITECTURE.md` §25 / DD-N5.

use crate::search::SearchResult;
use crate::{NexoraError, NoteId, VaultIndex};
use std::collections::BTreeMap;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

pub const DEFAULT_DIMS: usize = 384;
const CHARS_PER_CHUNK: usize = 400;
const OVERLAP_CHARS: usize = 80;
const FILE_MAGIC: &str = "NEXORA-SEM-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub id: NoteId,
    pub text: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorEntry {
    pub id: NoteId,
    pub line: usize,
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticIndex {
    pub dims: usize,
    pub fingerprint: u64,
    pub entries: Vec<VectorEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticHit {
    pub id: NoteId,
    pub line: usize,
    pub score: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RankSource {
    Keyword,
    Semantic,
    Both,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub id: NoteId,
    pub score: f32,
    pub source: RankSource,
}

/// Split a note into overlapping text windows with their starting line.
pub fn chunk_note(id: &NoteId, content: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut buffer = String::new();
    let mut start_line = 1;
    for (offset, line) in content.lines().enumerate() {
        if !buffer.is_empty() && buffer.len() + line.len() + 1 > CHARS_PER_CHUNK {
            chunks.push(Chunk {
                id: id.clone(),
                text: buffer.trim().to_string(),
                line: start_line,
            });
            let keep = tail_on_word_boundary(&buffer, OVERLAP_CHARS);
            buffer = keep.to_string();
            start_line = offset + 1;
        }
        buffer.push_str(line);
        buffer.push('\n');
    }
    if !buffer.trim().is_empty() {
        chunks.push(Chunk {
            id: id.clone(),
            text: buffer.trim().to_string(),
            line: start_line,
        });
    }
    chunks
}

fn tail_on_word_boundary(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut start = text.len() - limit;
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    let tail = &text[start..];
    match tail.find(char::is_whitespace) {
        // `space` is a byte offset; a non-ASCII whitespace (NBSP, EM SPACE)
        // is wider than one byte and `space + 1` would split a code point.
        Some(space) => match tail[space..].chars().next() {
            Some(separator) => &tail[space + separator.len_utf8()..],
            None => tail,
        },
        None => tail,
    }
}

fn add_token(vector: &mut [f32], token: &str, weight: f32) {
    if token.is_empty() {
        return;
    }
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in token.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let index = (hash as usize) % vector.len();
    let sign = if (hash >> 63) & 1 == 0 { 1.0 } else { -1.0 };
    vector[index] += sign * weight;
}

/// Deterministic lexical embedding, L2-normalised. Empty text yields zeros.
pub fn hash_embed(text: &str, dims: usize) -> Vec<f32> {
    if dims == 0 {
        return Vec::new();
    }
    let mut vector = vec![0.0_f32; dims];
    let lower = text.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    for window in chars.windows(3) {
        add_token(&mut vector, &window.iter().collect::<String>(), 1.0);
    }
    for word in lower.split_whitespace() {
        add_token(&mut vector, word, 1.5);
    }
    let norm: f32 = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

pub fn cosine(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for (a, b) in left.iter().zip(right) {
        dot += a * b;
        left_norm += a * a;
        right_norm += b * b;
    }
    let denominator = left_norm.sqrt() * right_norm.sqrt();
    if denominator == 0.0 {
        0.0
    } else {
        dot / denominator
    }
}

pub fn vault_fingerprint(index: &VaultIndex) -> u64 {
    let mut hasher = DefaultHasher::new();
    for (id, note) in &index.notes {
        id.hash(&mut hasher);
        note.content.hash(&mut hasher);
    }
    hasher.finish()
}

pub fn build(index: &VaultIndex, dims: usize) -> SemanticIndex {
    let mut entries = Vec::new();
    for (id, note) in &index.notes {
        for chunk in chunk_note(id, &note.content) {
            entries.push(VectorEntry {
                id: id.clone(),
                line: chunk.line,
                vector: hash_embed(&chunk.text, dims),
            });
        }
    }
    SemanticIndex {
        dims,
        fingerprint: vault_fingerprint(index),
        entries,
    }
}

/// One best hit per note, highest cosine first.
pub fn search_semantic(index: &SemanticIndex, query: &[f32], limit: usize) -> Vec<SemanticHit> {
    let mut best: BTreeMap<NoteId, SemanticHit> = BTreeMap::new();
    for entry in &index.entries {
        let score = cosine(query, &entry.vector);
        if score <= 0.0 {
            continue;
        }
        let insert = match best.get(&entry.id) {
            Some(current) => score > current.score,
            None => true,
        };
        if insert {
            best.insert(
                entry.id.clone(),
                SemanticHit {
                    id: entry.id.clone(),
                    line: entry.line,
                    score,
                },
            );
        }
    }
    let mut hits: Vec<SemanticHit> = best.into_values().collect();
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.id.cmp(&b.id))
    });
    hits.truncate(limit);
    hits
}

/// Blend ranked keyword results with semantic hits. Keyword scores come from
/// the locked rank 0/1/2 in §7.5; semantic scores are cosine similarities.
pub fn blend(
    keyword: &[SearchResult],
    semantic: &[SemanticHit],
    semantic_weight: f32,
) -> Vec<Ranked> {
    let weight = semantic_weight.clamp(0.0, 1.0);
    let mut out: BTreeMap<NoteId, Ranked> = BTreeMap::new();
    for result in keyword {
        out.insert(
            result.id.clone(),
            Ranked {
                id: result.id.clone(),
                score: 1.0 / (result.rank as f32 + 1.0),
                source: RankSource::Keyword,
            },
        );
    }
    for hit in semantic {
        let semantic_score = hit.score * weight;
        match out.get_mut(&hit.id) {
            Some(existing) => {
                existing.score = existing.score * (1.0 - weight) + semantic_score;
                existing.source = RankSource::Both;
            }
            None => {
                out.insert(
                    hit.id.clone(),
                    Ranked {
                        id: hit.id.clone(),
                        score: semantic_score,
                        source: RankSource::Semantic,
                    },
                );
            }
        }
    }
    let mut ranked: Vec<Ranked> = out.into_values().collect();
    ranked.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.id.cmp(&b.id))
    });
    ranked
}

pub fn save(path: &Path, index: &SemanticIndex) -> Result<(), NexoraError> {
    let mut out = format!("{FILE_MAGIC} {} {}\n", index.dims, index.fingerprint);
    for entry in &index.entries {
        let vector = entry
            .vector
            .iter()
            .map(|value| format!("{:08x}", value.to_bits()))
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!("{}\t{}\t{}\n", entry.id, entry.line, vector));
    }
    fs::write(path, out.as_bytes()).map_err(|error| NexoraError::io_at(path, error))
}

pub fn load(path: &Path) -> Result<SemanticIndex, NexoraError> {
    let raw = fs::read_to_string(path).map_err(|error| NexoraError::io_at(path, error))?;
    let mut lines = raw.lines();
    let header = lines.next().ok_or_else(|| NexoraError::InvalidInput {
        message: "empty semantic index".to_string(),
    })?;
    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() != 3 || parts[0] != FILE_MAGIC {
        return Err(NexoraError::InvalidInput {
            message: "invalid semantic index header".to_string(),
        });
    }
    let dims = parts[1].parse::<usize>().map_err(|_| NexoraError::InvalidInput {
        message: "invalid semantic index dimensions".to_string(),
    })?;
    let fingerprint = parts[2]
        .parse::<u64>()
        .map_err(|_| NexoraError::InvalidInput {
            message: "invalid semantic index fingerprint".to_string(),
        })?;
    let mut entries = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.splitn(3, '\t').collect();
        if fields.len() != 3 {
            continue;
        }
        let line_number = fields[1].parse::<usize>().unwrap_or(1);
        let mut vector = Vec::with_capacity(dims);
        for word in fields[2].split_whitespace() {
            let bits = u32::from_str_radix(word, 16).ok();
            if let Some(bits) = bits {
                vector.push(f32::from_bits(bits));
            }
        }
        if vector.len() == dims {
            entries.push(VectorEntry {
                id: NoteId::new(fields[0]),
                line: line_number,
                vector,
            });
        }
    }
    Ok(SemanticIndex {
        dims,
        fingerprint,
        entries,
    })
}

/// Re-export for callers that only need to check against the live index.
pub fn is_stale(saved: &SemanticIndex, index: &VaultIndex) -> bool {
    saved.fingerprint != vault_fingerprint(index)
}
