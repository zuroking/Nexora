//! Small line diff for snapshot review. No external crate, bounded memory.

const MAX_CELLS: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Equal,
    Added,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffRow {
    pub kind: DiffKind,
    pub text: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}

fn row(kind: DiffKind, text: &str, old_line: Option<usize>, new_line: Option<usize>) -> DiffRow {
    DiffRow {
        kind,
        text: text.to_string(),
        old_line,
        new_line,
    }
}

/// Common prefix + one replace block + common suffix. Used when the full LCS
/// table would be too large.
fn fallback(a: &[&str], b: &[&str]) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    let mut start = 0;
    while start < a.len().min(b.len()) && a[start] == b[start] {
        rows.push(row(DiffKind::Equal, a[start], Some(start + 1), Some(start + 1)));
        start += 1;
    }
    let mut end_a = a.len();
    let mut end_b = b.len();
    while end_a > start && end_b > start && a[end_a - 1] == b[end_b - 1] {
        end_a -= 1;
        end_b -= 1;
    }
    for (offset, line) in a[start..end_a].iter().enumerate() {
        rows.push(row(DiffKind::Removed, line, Some(start + offset + 1), None));
    }
    for (offset, line) in b[start..end_b].iter().enumerate() {
        rows.push(row(DiffKind::Added, line, None, Some(start + offset + 1)));
    }
    for offset in 0..(a.len() - end_a) {
        let line = a[end_a + offset];
        rows.push(row(
            DiffKind::Equal,
            line,
            Some(end_a + offset + 1),
            Some(end_b + offset + 1),
        ));
    }
    rows
}

/// Line-by-line diff. `old_line` / `new_line` are 1-based and `None` for rows
/// that exist on only one side.
pub fn diff(old: &str, new: &str) -> Vec<DiffRow> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    if a.len().saturating_mul(b.len()) > MAX_CELLS {
        return fallback(&a, &b);
    }

    let mut dp = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut rows = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            rows.push(row(DiffKind::Equal, a[i], Some(i + 1), Some(j + 1)));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            rows.push(row(DiffKind::Removed, a[i], Some(i + 1), None));
            i += 1;
        } else {
            rows.push(row(DiffKind::Added, b[j], None, Some(j + 1)));
            j += 1;
        }
    }
    while i < a.len() {
        rows.push(row(DiffKind::Removed, a[i], Some(i + 1), None));
        i += 1;
    }
    while j < b.len() {
        rows.push(row(DiffKind::Added, b[j], None, Some(j + 1)));
        j += 1;
    }
    rows
}
