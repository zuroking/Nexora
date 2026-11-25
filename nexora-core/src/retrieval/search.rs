//! In-memory ranked substring search (no external index crate).

use crate::{NoteId, VaultIndex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub id: NoteId,
    pub title: String,
    pub rank: u8,
}

/// Ranking: exact title -> title substring -> body substring, then path order.
pub fn search(index: &VaultIndex, query: &str) -> Vec<SearchResult> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    for (id, note) in &index.notes {
        let title = note.title.to_lowercase();
        let body = note.content.to_lowercase();
        let rank = if title == needle {
            0
        } else if title.contains(&needle) {
            1
        } else if body.contains(&needle) {
            2
        } else {
            continue;
        };
        results.push(SearchResult {
            id: id.clone(),
            title: note.title.clone(),
            rank,
        });
    }

    results.sort_by(|left, right| {
        left.rank
            .cmp(&right.rank)
            .then_with(|| left.id.cmp(&right.id))
    });
    results
}

// --- Boolean/field query language (§39) -----------------------------------

#[derive(Clone, Debug)]
struct Term {
    field: Option<String>,
    value: String,
}

#[derive(Clone, Debug)]
enum Token {
    Term(Term),
    And,
    Or,
    Not,
}

#[derive(Clone, Debug)]
enum Expr {
    Term(Term),
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}

fn scan_closer(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, closer: char) -> Option<String> {
    let mut value = String::new();
    for (_, inner) in chars.by_ref() {
        if inner == closer {
            return Some(value);
        }
        value.push(inner);
    }
    None
}

/// One term constructor for quoted phrases, bracket fields and bare fields.
/// `None` means the query is malformed (§39.1).
fn term_from(value: &str, split_field: bool) -> Option<Term> {
    let (field, text) = if split_field {
        match value.split_once(':') {
            Some((key, text)) => {
                if key.trim().is_empty() || text.trim().is_empty() {
                    return None;
                }
                (Some(key.trim().to_lowercase()), text.trim())
            }
            None => (None, value.trim()),
        }
    } else {
        (None, value.trim())
    };
    if text.is_empty() {
        return None;
    }
    Some(Term {
        field,
        value: text.to_lowercase(),
    })
}

/// Closed token set. Returns `None` on an unterminated quote/bracket.
fn tokenize(query: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = query.char_indices().peekable();
    while let Some((_, ch)) = chars.peek().copied() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if ch == '"' {
            chars.next();
            let Some(value) = scan_closer(&mut chars, '"') else {
                return None;
            };
            tokens.push(Token::Term(term_from(&value, false)?));
            continue;
        }
        if ch == '[' {
            chars.next();
            let Some(value) = scan_closer(&mut chars, ']') else {
                return None;
            };
            if !value.contains(':') {
                return None;
            }
            tokens.push(Token::Term(term_from(&value, true)?));
            continue;
        }
        let start = match chars.peek() {
            Some((index, _)) => *index,
            None => break,
        };
        let mut end = start;
        for (index, inner) in chars.by_ref() {
            if inner.is_whitespace() {
                break;
            }
            end = index + inner.len_utf8();
        }
        let word = &query[start..end];
        match word.to_lowercase().as_str() {
            "and" => tokens.push(Token::And),
            "or" => tokens.push(Token::Or),
            "not" => tokens.push(Token::Not),
            _ => {
                if word.ends_with(':') {
                    return None;
                }
                tokens.push(Token::Term(term_from(word, true)?));
            }
        }
    }
    Some(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn parse(&mut self) -> Option<Expr> {
        let expression = self.parse_or()?;
        if self.position == self.tokens.len() {
            Some(expression)
        } else {
            None
        }
    }

    fn parse_or(&mut self) -> Option<Expr> {
        let mut branches = vec![self.parse_and()?];
        while matches!(self.tokens.get(self.position), Some(Token::Or)) {
            self.position += 1;
            branches.push(self.parse_and()?);
        }
        if branches.len() == 1 {
            Some(branches.remove(0))
        } else {
            Some(Expr::Or(branches))
        }
    }

    fn parse_and(&mut self) -> Option<Expr> {
        let mut operands = vec![self.parse_not()?];
        loop {
            match self.tokens.get(self.position) {
                Some(Token::And) => self.position += 1,
                Some(Token::Term(_)) | Some(Token::Not) => {}
                _ => break,
            }
            operands.push(self.parse_not()?);
        }
        if operands.len() == 1 {
            Some(operands.remove(0))
        } else {
            Some(Expr::And(operands))
        }
    }

    fn parse_not(&mut self) -> Option<Expr> {
        if matches!(self.tokens.get(self.position), Some(Token::Not)) {
            self.position += 1;
            return Some(Expr::Not(Box::new(self.parse_not()?)));
        }
        match self.tokens.get(self.position) {
            Some(Token::Term(term)) => {
                self.position += 1;
                Some(Expr::Term(term.clone()))
            }
            _ => None,
        }
    }
}

fn frontmatter(content: &str) -> Vec<(String, String)> {
    let mut lines = content.lines();
    // A BOM before the opening fence is not whitespace; strip it here as
    // vault::extract_title and frontmatter::parts already do (§30/§39).
    if lines
        .next()
        .map(|line| line.trim().trim_start_matches('\u{feff}'))
        != Some("---")
    {
        return Vec::new();
    }
    let mut fields = Vec::new();
    for line in lines {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line == "---" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            fields.push((key.trim().to_lowercase(), value.trim().to_lowercase()));
        }
    }
    fields
}

fn term_matches(term: &Term, values: &[(String, String)]) -> bool {
    values
        .iter()
        .filter(|(field, _)| match &term.field {
            Some(requested) => requested == field,
            // An unqualified term searches every declared field, including
            // `path` and frontmatter keys (§39.2).
            None => true,
        })
        .any(|(_, value)| value.contains(&term.value))
}

fn eval(expr: &Expr, values: &[(String, String)]) -> bool {
    match expr {
        Expr::Term(term) => term_matches(term, values),
        Expr::Not(inner) => !eval(inner, values),
        Expr::And(items) => items.iter().all(|item| eval(item, values)),
        Expr::Or(items) => items.iter().any(|item| eval(item, values)),
    }
}

fn parse_query(query: &str) -> Option<Expr> {
    if query.trim().is_empty() {
        return None;
    }
    let tokens = tokenize(query)?;
    let mut parser = Parser {
        tokens,
        position: 0,
    };
    parser.parse()
}

/// Validate query syntax without touching a vault. An empty query is valid
/// (it simply matches nothing); malformed syntax is an error so a saved query
/// can be rejected visibly before it is applied (§41.2).
pub fn validate(query: &str) -> Result<(), crate::NexoraError> {
    if query.trim().is_empty() {
        return Ok(());
    }
    if parse_query(query).is_some() {
        Ok(())
    } else {
        Err(crate::NexoraError::InvalidInput {
            message: "malformed query".to_string(),
        })
    }
}

/// Boolean/field query over the vault. Additive to [`search`]; see §39.
/// Malformed queries return an empty vector rather than panicking.
pub fn search_advanced(index: &VaultIndex, query: &str) -> Vec<SearchResult> {
    let Some(expression) = parse_query(query) else {
        return Vec::new();
    };

    let mut results = Vec::new();
    for (id, note) in &index.notes {
        let mut values = vec![
            ("path".to_string(), id.as_str().to_lowercase()),
            ("title".to_string(), note.title.to_lowercase()),
            ("body".to_string(), note.content.to_lowercase()),
        ];
        values.extend(frontmatter(&note.content));
        if eval(&expression, &values) {
            results.push(SearchResult {
                id: id.clone(),
                title: note.title.clone(),
                rank: 0,
            });
        }
    }
    results.sort_by(|left, right| left.id.cmp(&right.id));
    results
}
