//! Pure tab-index arithmetic for the GUI shell (§73).

/// Next or previous tab index, wrapping in both directions. `None` when there
/// are no tabs. An out-of-range `active` is clamped to the last tab first.
pub fn cycle(active: usize, len: usize, backward: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let current = active.min(len - 1);
    if backward {
        Some((current + len - 1) % len)
    } else {
        Some((current + 1) % len)
    }
}
