use chrono::NaiveDate;
use nexora_core::review;
use nexora_core::vault;
use std::fs;
use tempfile::tempdir;

fn day(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
}

#[test]
fn sm2_advances_intervals_and_resets_on_bad_grade() {
    let today = day("2026-11-22");
    let start = review::ReviewState {
        due: today,
        ease: 2.5,
        interval: 0,
        reps: 0,
    };

    let first = review::grade(&start, 5, today);
    assert_eq!(first.interval, 1);
    assert_eq!(first.reps, 1);
    assert_eq!(first.due, day("2026-11-23"));

    let second = review::grade(&first, 5, today);
    assert_eq!(second.interval, 6);

    let third = review::grade(&second, 4, today);
    assert!(third.interval > 6);
    assert_eq!(third.reps, 3);

    let failed = review::grade(&third, 2, today);
    assert_eq!(failed.reps, 0);
    assert_eq!(failed.interval, 1);
    assert_eq!(failed.due, day("2026-11-23"));
}

#[test]
fn grade_three_is_the_advancing_boundary() {
    let today = day("2026-11-22");
    let start = review::ReviewState {
        due: today,
        ease: 2.5,
        interval: 1,
        reps: 1,
    };
    let advanced = review::grade(&start, 3, today);
    assert_eq!(advanced.reps, 2, "grade 3 must advance repetitions");
    let reset = review::grade(&start, 2, today);
    assert_eq!(reset.reps, 0, "grade 2 must reset repetitions");
}

#[test]
fn queue_uses_only_review_due_for_membership() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("partial.md"),
        "---\nreview_due: 2026-11-01\nreview_ease: broken\n---\n",
    )
    .unwrap();
    let index = vault::build_index(dir.path()).unwrap();
    let due = review::due_queue(&index, day("2026-11-22"));
    assert_eq!(due.len(), 1, "a due date is enough for queue membership");
}

#[test]
fn ease_never_falls_below_the_sm2_floor() {
    let today = day("2026-11-22");
    let start = review::ReviewState {
        due: today,
        ease: 1.35,
        interval: 10,
        reps: 3,
    };
    let graded = review::grade(&start, 0, today);
    assert!((graded.ease - 1.3).abs() < f32::EPSILON);
}

#[test]
fn queue_uses_flat_frontmatter_and_sorts_by_date_then_id() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("b.md"),
        "---\nreview_due: 2026-11-20\nreview_ease: 2.5\nreview_interval: 6\nreview_reps: 2\n---\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("a.md"),
        "---\nreview_due: 2026-11-20\nreview_ease: 2.5\nreview_interval: 6\nreview_reps: 2\n---\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("later.md"),
        "---\nreview_due: 2026-12-01\n---\n",
    )
    .unwrap();
    fs::write(dir.path().join("plain.md"), "# Plain\n").unwrap();
    let index = vault::build_index(dir.path()).unwrap();

    let due = review::due_queue(&index, day("2026-11-22"));
    assert_eq!(
        due.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        vec!["a", "b"]
    );
}

#[test]
fn state_roundtrips_through_frontmatter() {
    let today = day("2026-11-22");
    let state = review::ReviewState {
        due: today,
        ease: 2.5,
        interval: 6,
        reps: 2,
    };
    let content = review::write_state("# Note\n", &state);
    let loaded = review::from_content(&content).unwrap();
    assert_eq!(loaded.due, today);
    assert_eq!(loaded.interval, 6);
    assert_eq!(loaded.reps, 2);
    assert!(review::from_content("# No review fields\n").is_none());
}

#[test]
fn partial_review_fields_are_not_a_fresh_card() {
    // `has_fields` must see any of the four keys, not just the due date, so
    // grading cannot overwrite a card whose other values were hand-edited.
    assert!(review::has_fields("---\nreview_due: garbage\n---\n"));
    assert!(review::has_fields("---\nreview_ease: 2.5\n---\n"));
    assert!(review::has_fields("---\nreview_interval: 6\n---\n"));
    assert!(review::has_fields("---\nreview_reps: 2\n---\n"));
    assert!(!review::has_fields("# Plain\n"));
    assert!(review::from_content("---\nreview_ease: 2.5\n---\n").is_none());
}

#[test]
fn hostile_review_frontmatter_cannot_overflow_date_arithmetic() {
    let today = day("2026-11-22");
    let state = review::ReviewState {
        due: today,
        ease: f32::MAX,
        interval: u32::MAX,
        reps: u32::MAX,
    };
    let graded = review::grade(&state, review::GOOD, today);
    assert!(graded.interval <= 36500, "interval must stay bounded");
    assert_eq!(graded.reps, u32::MAX, "reps must saturate, not wrap");

    let content = "---\nreview_due: 2026-11-22\nreview_ease: 2.5\nreview_interval: 4294967295\nreview_reps: 1\n---\n";
    assert!(review::from_content(content).is_none(), "huge interval is invalid");
    let inf = "---\nreview_due: 2026-11-22\nreview_ease: inf\nreview_interval: 6\nreview_reps: 1\n---\n";
    assert!(review::from_content(inf).is_none(), "non-finite ease is invalid");
}
