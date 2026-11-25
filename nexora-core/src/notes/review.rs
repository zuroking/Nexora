//! Pure SM-2 scheduling and a due-note projection (§46).
//!
//! State lives in flat frontmatter fields; no decks, no automatic writes.

use crate::{frontmatter, NoteId, VaultIndex};
use chrono::{Duration, Local, NaiveDate};

/// Named SM-2 grades used by the GUI buttons (§46.1).
pub const AGAIN: u8 = 0;
pub const HARD: u8 = 3;
pub const GOOD: u8 = 4;
pub const EASY: u8 = 5;

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Domain defaults for a note that has never been reviewed (§46.1).
pub fn new_card(date: NaiveDate) -> ReviewState {
    ReviewState {
        due: date,
        ease: 2.5,
        interval: 0,
        reps: 0,
    }
}

/// A calendar bound for user-authored intervals. Without it a hand-set
/// `review_interval` can overflow chrono's date arithmetic (§53).
const MAX_INTERVAL_DAYS: u32 = 36500;

/// True when a note has any review frontmatter, even if malformed.
/// Only `review_due` decides queue membership (§46.1); grading must not treat
/// a partially populated card as fresh and overwrite the other fields (§53).
pub fn has_fields(content: &str) -> bool {
    ["review_due", "review_ease", "review_interval", "review_reps"]
        .iter()
        .any(|key| frontmatter::read_field(content, key).is_some())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReviewState {
    pub due: NaiveDate,
    pub ease: f32,
    pub interval: u32,
    pub reps: u32,
}

pub fn from_content(content: &str) -> Option<ReviewState> {
    let due = frontmatter::read_field(content, "review_due")?;
    let ease: f32 = frontmatter::read_field(content, "review_ease")?.parse().ok()?;
    let interval: u32 = frontmatter::read_field(content, "review_interval")?.parse().ok()?;
    let reps: u32 = frontmatter::read_field(content, "review_reps")?.parse().ok()?;
    if !ease.is_finite() || interval > MAX_INTERVAL_DAYS {
        return None;
    }
    Some(ReviewState {
        due: NaiveDate::parse_from_str(&due, "%Y-%m-%d").ok()?,
        ease,
        interval,
        reps,
    })
}

pub fn write_state(content: &str, state: &ReviewState) -> String {
    let mut out = content.to_string();
    out = frontmatter::write_field(&out, "review_due", &state.due.format("%Y-%m-%d").to_string());
    out = frontmatter::write_field(&out, "review_ease", &state.ease.to_string());
    out = frontmatter::write_field(&out, "review_interval", &state.interval.to_string());
    frontmatter::write_field(&out, "review_reps", &state.reps.to_string())
}

/// One SM-2 step. Grades 0–2 reset repetitions and schedule tomorrow; grades
/// 3–5 advance 1 → 6 → interval × ease. Ease never drops below 1.3.
pub fn grade(state: &ReviewState, quality: u8, today: NaiveDate) -> ReviewState {
    let quality = quality.clamp(0, 5);
    let five_minus = (5 - quality) as f32;
    let ease = (state.ease + 0.1 - five_minus * (0.08 + five_minus * 0.02)).max(1.3);
    let (reps, interval) = if quality < 3 {
        (0, 1)
    } else {
        let reps = state.reps.saturating_add(1);
        let interval = match reps {
            1 => 1,
            2 => 6,
            // f64 + clamp keeps a huge user-authored ease from saturating a
            // float-to-u32 cast into an out-of-range date (§53).
            _ => (state.interval as f64 * ease as f64)
                .round()
                .clamp(1.0, MAX_INTERVAL_DAYS as f64) as u32,
        };
        (reps, interval)
    };
    let due = match today.checked_add_signed(Duration::days(interval as i64)) {
        Some(date) => date,
        None => NaiveDate::MAX,
    };
    ReviewState {
        due,
        ease,
        interval,
        reps,
    }
}

/// Notes due today or earlier, ordered by due date then id (§46.1).
pub fn due_queue(index: &VaultIndex, today: NaiveDate) -> Vec<NoteId> {
    // Only `review_due` decides membership (§46.1); other fields may be
    // missing or malformed without hiding a genuinely due note.
    let mut due: Vec<(NaiveDate, NoteId)> = index
        .notes
        .iter()
        .filter_map(|(id, note)| {
            let raw = frontmatter::read_field(&note.content, "review_due")?;
            let date = NaiveDate::parse_from_str(&raw, "%Y-%m-%d").ok()?;
            (date <= today).then_some((date, id.clone()))
        })
        .collect();
    due.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    due.into_iter().map(|(_, id)| id).collect()
}
