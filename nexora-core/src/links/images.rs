//! Safe vault-relative image targets for `![[pic.png]]` (§45).
//!
//! Core only recognises and validates targets; reading bytes and registering
//! them with egui stays in the app. The `![[...]]` grammar itself is owned by
//! `display` — this module only filters its output.

use crate::display;

const EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "svg"];

pub fn is_image_target(raw: &str) -> bool {
    let Some((_, extension)) = raw.rsplit_once('.') else {
        return false;
    };
    EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
}

/// No absolute paths, no drive letters, no traversal, no Windows separators.
pub fn is_safe_target(raw: &str) -> bool {
    if raw.is_empty() || raw.starts_with('/') || raw.contains(['\\', ':']) {
        return false;
    }
    !raw.split('/').any(|part| part == "..")
}

/// Every `![[...]]` inner that addresses an image, including unsafe ones so
/// the app can show a marker. Inner is verbatim (possibly with `|alias`).
pub fn all_targets(content: &str) -> Vec<String> {
    display::embed_inner(content)
        .into_iter()
        .filter(|inner| {
            let target = inner.split('|').next().unwrap_or("").trim();
            is_image_target(target)
        })
        .collect()
}

/// Only safe vault-relative image targets, as promised by §45.1.
pub fn targets(content: &str) -> Vec<String> {
    all_targets(content)
        .into_iter()
        .filter(|inner| {
            let target = inner.split('|').next().unwrap_or("").trim();
            is_safe_target(target)
        })
        .collect()
}
