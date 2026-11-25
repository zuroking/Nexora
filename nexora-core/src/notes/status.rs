//! Human-readable relative age for the status bar. No clock dependency.

/// Label for a save that happened `seconds` ago.
pub fn saved_label(seconds: u64) -> String {
    if seconds < 2 {
        return "just now".to_string();
    }
    if seconds < 60 {
        return format!("{seconds}s ago");
    }
    let minutes = seconds / 60;
    let rest = seconds % 60;
    if rest == 0 {
        format!("{minutes}m ago")
    } else {
        format!("{minutes}m {rest}s ago")
    }
}
