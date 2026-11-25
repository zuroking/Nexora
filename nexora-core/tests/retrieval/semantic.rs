use nexora_core::search::SearchResult;
use nexora_core::semantic::{
    blend, build, chunk_note, cosine, hash_embed, is_stale, load, save, search_semantic,
    RankSource, DEFAULT_DIMS,
};
use nexora_core::{Note, NoteId, VaultIndex};
use std::collections::BTreeMap;
use std::fs;
use std::time::SystemTime;
use tempfile::tempdir;

#[test]
fn chunking_accepts_multibyte_whitespace() {
    // NBSP is two bytes; advancing by 1 would slice mid-code-point.
    let body = format!("{}10\u{00A0}kg {}", "a".repeat(450), "b".repeat(40));
    let chunks = chunk_note(&NoteId::new("n"), &body);
    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|chunk| chunk.text.is_char_boundary(0)));
}

fn index_with(notes: &[(&str, &str)]) -> VaultIndex {
    let mut map = BTreeMap::new();
    for (id, content) in notes {
        let id = NoteId::new(*id);
        map.insert(
            id.clone(),
            Note {
                title: id.stem().to_string(),
                id,
                content: (*content).to_string(),
                modified: SystemTime::now(),
            },
        );
    }
    VaultIndex {
        notes: map,
        ..VaultIndex::default()
    }
}

#[test]
fn chunking_tracks_lines_and_unicode() {
    let content = "первая строка\nвторая строка\nтретья строка";
    let chunks = chunk_note(&NoteId::new("note"), content);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].line, 1);
    assert!(chunks[0].text.contains("третья строка"));
    assert!(chunk_note(&NoteId::new("empty"), "").is_empty());
}

#[test]
fn hash_embedding_is_deterministic_and_normalised() {
    let a = hash_embed("аттестация по математике", DEFAULT_DIMS);
    let b = hash_embed("аттестация по математике", DEFAULT_DIMS);
    let other = hash_embed("совсем другой текст", DEFAULT_DIMS);
    assert_eq!(a, b);
    assert_eq!(a.len(), DEFAULT_DIMS);
    assert!((cosine(&a, &b) - 1.0).abs() < 1e-5);
    assert!(cosine(&a, &other) < 0.5);
    assert_eq!(cosine(&[], &a), 0.0);
}

#[test]
fn build_search_and_staleness() {
    let live = index_with(&[
        ("school/math", "# Математика\nАттестация по алгебре\n"),
        ("personal/shop", "# Список\nХлеб и молоко\n"),
    ]);
    let index = build(&live, DEFAULT_DIMS);
    assert_eq!(index.entries.len(), 2);
    assert!(!is_stale(&index, &live));

    let query = hash_embed("аттестация алгебра", DEFAULT_DIMS);
    let hits = search_semantic(&index, &query, 10);
    assert_eq!(hits[0].id, NoteId::new("school/math"));
    assert_eq!(hits[0].line, 1);

    let changed = index_with(&[("school/math", "другой текст")]);
    assert!(is_stale(&index, &changed));
}

#[test]
fn sidecar_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("semantic.idx");
    let live = index_with(&[("a", "hello world\n")]);
    let index = build(&live, 32);
    save(&path, &index).unwrap();
    let loaded = load(&path).unwrap();
    assert_eq!(loaded, index);
    assert!(!is_stale(&loaded, &live));
}

#[test]
fn blend_keeps_sources_and_sorts() {
    let keyword = vec![SearchResult {
        id: NoteId::new("a"),
        title: "a".to_string(),
        rank: 0,
    }];
    let semantic = vec![
        nexora_core::semantic::SemanticHit {
            id: NoteId::new("a"),
            line: 1,
            score: 0.9,
        },
        nexora_core::semantic::SemanticHit {
            id: NoteId::new("b"),
            line: 3,
            score: 0.4,
        },
    ];
    let ranked = blend(&keyword, &semantic, 0.5);
    assert_eq!(ranked[0].id, NoteId::new("a"));
    assert_eq!(ranked[0].source, RankSource::Both);
    assert_eq!(ranked[1].source, RankSource::Semantic);
    assert!(ranked[0].score > ranked[1].score);
}

#[test]
fn corrupt_header_is_an_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad.idx");
    fs::write(&path, "garbage\n").unwrap();
    assert!(load(&path).is_err());
}
