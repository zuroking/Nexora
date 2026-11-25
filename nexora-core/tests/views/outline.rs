use nexora_core::outline::{extract, Heading};

#[test]
fn extracts_headings_with_line_numbers() {
    let content = "# Title\ntext\n## Section\n### Deep\n";
    assert_eq!(
        extract(content),
        vec![
            Heading { level: 1, title: "Title".to_string(), line: 1 },
            Heading { level: 2, title: "Section".to_string(), line: 3 },
            Heading { level: 3, title: "Deep".to_string(), line: 4 },
        ]
    );
}

#[test]
fn ignores_fenced_code_and_hash_without_space() {
    let content = "```md\n# Not a heading\n```\n#hashtag\n# Real\n";
    assert_eq!(
        extract(content),
        vec![Heading { level: 1, title: "Real".to_string(), line: 5 }]
    );
}
