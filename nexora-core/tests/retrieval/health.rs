use nexora_core::health::{selection_key, HealthIssue, Severity};
use nexora_core::NoteId;

fn issue(note: &str, message: &str) -> HealthIssue {
    HealthIssue {
        severity: Severity::Error,
        message: message.to_string(),
        note: Some(NoteId::new(note)),
        target: Some("missing".to_string()),
        line: Some(3),
    }
}

#[test]
fn selection_key_is_stable_for_the_same_issues() {
    let issues = vec![issue("a", "broken"), issue("b", "orphan")];
    assert_eq!(selection_key(&issues), selection_key(&issues.clone()));
}

#[test]
fn selection_key_changes_when_an_issue_changes_or_disappears() {
    let first = vec![issue("a", "broken"), issue("b", "orphan")];
    let changed = vec![issue("a", "broken"), issue("b", "different")];
    let shorter = vec![issue("a", "broken")];
    assert_ne!(selection_key(&first), selection_key(&changed));
    assert_ne!(selection_key(&first), selection_key(&shorter));
}

#[test]
fn selection_key_distinguishes_issue_order() {
    let first = vec![issue("a", "broken"), issue("b", "orphan")];
    let reversed = vec![issue("b", "orphan"), issue("a", "broken")];
    assert_ne!(selection_key(&first), selection_key(&reversed));
}

#[test]
fn empty_report_has_an_empty_key() {
    assert_eq!(selection_key(&[]), "");
}

#[test]
fn key_changes_with_severity_target_and_line() {
    let base = issue("a", "broken");
    let mut changed = base.clone();
    changed.severity = Severity::Warning;
    assert_ne!(selection_key(&[base.clone()]), selection_key(&[changed]));

    let mut changed = base.clone();
    changed.target = Some("other-missing".to_string());
    assert_ne!(selection_key(&[base.clone()]), selection_key(&[changed]));

    let mut changed = base.clone();
    changed.line = Some(9);
    assert_ne!(selection_key(&[base.clone()]), selection_key(&[changed]));

    let mut changed = base.clone();
    changed.note = Some(NoteId::new("other-note"));
    assert_ne!(selection_key(&[base.clone()]), selection_key(&[changed]));

    let mut changed = base.clone();
    changed.target = Some("a;b|c".to_string());
    assert_ne!(selection_key(&[base]), selection_key(&[changed]));
}

#[test]
fn key_cannot_be_forged_by_field_separators() {
    // The old `field|field|...` format made these two rows collide: the pipe
    // in the message shifts where `note` starts. Debug quoting must keep them
    // distinct (§72 review).
    let mut first = issue("b|c", "a");
    first.target = Some("d".to_string());
    let mut second = issue("c", "a|b");
    second.target = Some("d".to_string());

    assert_ne!(selection_key(&[first]), selection_key(&[second]));
}
