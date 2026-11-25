//! Vault health report: broken links, orphans, duplicate basenames.

use crate::{wikilink, Link, NoteId, VaultIndex};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthIssue {
    pub severity: Severity,
    pub message: String,
    pub note: Option<NoteId>,
    pub target: Option<String>,
    /// 1-based source line for broken links; `None` for note-level findings.
    pub line: Option<usize>,
}

/// Find the 1-based line containing `[[target]]` / `[[target|alias]]`.
fn line_of_link(content: &str, target: &NoteId) -> Option<usize> {
    for (index, line) in content.lines().enumerate() {
        let mut rest = line;
        while let Some(start) = rest.find("[[") {
            let after_open = &rest[start + 2..];
            let Some(end) = after_open.find("]]") else {
                break;
            };
            let inner = &after_open[..end];
            let raw = inner.split_once('|').map(|(value, _)| value).unwrap_or(inner);
            let (found, _) = wikilink::split_target(raw);
            if NoteId::new(&found) == *target {
                return Some(index + 1);
            }
            rest = &after_open[end + 2..];
        }
    }
    None
}

/// `YYYY-MM-DD` shape without a regex dependency.
fn is_daily(stem: &str) -> bool {
    stem.len() == 10
        && stem.as_bytes()[4] == b'-'
        && stem.as_bytes()[7] == b'-'
        && stem
            .chars()
            .enumerate()
            .all(|(index, c)| index == 4 || index == 7 || c.is_ascii_digit())
}

/// Stable identity of an issue list, used to reset GUI selection when the
/// report changes (§72). Deliberately includes every field that can move an
/// issue's meaning, but not its position in the vector.
pub fn selection_key(issues: &[HealthIssue]) -> String {
    // Debug of a tuple quotes and escapes fields, so a `;` or `|` inside a
    // message or target cannot forge a field boundary (§72 review).
    issues
        .iter()
        .map(|issue| {
            format!(
                "{:?}",
                (
                    issue.severity,
                    &issue.message,
                    issue.note.as_ref().map(NoteId::as_str),
                    issue.target.as_deref(),
                    issue.line,
                )
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

pub fn check(index: &VaultIndex) -> Vec<HealthIssue> {
    let mut issues = Vec::new();

    for (id, links) in &index.links {
        for link in links {
            if let Link::Broken { target, .. } = link {
                issues.push(HealthIssue {
                    severity: Severity::Error,
                    message: format!("[[{}]] not found", target),
                    note: Some(id.clone()),
                    target: Some(target.as_str().to_string()),
                    line: index
                        .notes
                        .get(id)
                        .and_then(|note| line_of_link(&note.content, target)),
                });
            }
        }
    }

    for (id, _) in &index.notes {
        let stem = id.stem();
        if stem == crate::inbox::INBOX_ID || is_daily(stem) {
            continue;
        }
        let has_incoming = index
            .backlinks
            .get(id)
            .is_some_and(|sources| !sources.is_empty());
        if !has_incoming {
            issues.push(HealthIssue {
                severity: Severity::Warning,
                message: "no incoming links".to_string(),
                note: Some(id.clone()),
                target: None,
                line: None,
            });
        }
    }

    let mut by_stem: BTreeMap<String, Vec<&NoteId>> = BTreeMap::new();
    for id in index.notes.keys() {
        by_stem.entry(id.stem().to_string()).or_default().push(id);
    }
    for (stem, ids) in by_stem {
        if ids.len() > 1 {
            for id in ids {
                issues.push(HealthIssue {
                    severity: Severity::Info,
                    message: format!("duplicate basename '{}'", stem),
                    note: Some(id.clone()),
                    target: None,
                    line: None,
                });
            }
        }
    }

    issues
}
