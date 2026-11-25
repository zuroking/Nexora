use nexora_core::theme::{self, ThemePreset};
use std::fs;
use tempfile::tempdir;

#[test]
fn every_preset_has_readable_body_text() {
    for preset in ThemePreset::ALL {
        let palette = preset.palette();
        // Body text is drawn on all five backgrounds: panel/surface/raised
        // fills plus accent_dim for active widgets and selection.
        for background in [
            palette.bg,
            palette.panel,
            palette.surface,
            palette.raised,
            palette.accent_dim,
        ] {
            let ratio = theme::contrast(palette.text, background);
            assert!(ratio >= 4.5, "{} text on background: {ratio:.2}", preset.label());
        }
        // Muted labels never land on accent_dim (active/selection foreground
        // is `text`), so they are checked only on the surfaces that use them.
        for background in [palette.bg, palette.panel, palette.surface, palette.raised] {
            let ratio = theme::contrast(palette.muted, background);
            assert!(ratio >= 4.5, "{} muted on background: {ratio:.2}", preset.label());
        }
        // DESIGN v2 roles (§85): tags, links and callout foregrounds.
        assert!(theme::contrast(palette.text, palette.tag) >= 4.5);
        assert!(theme::contrast(palette.link, palette.surface) >= 4.5);
        // egui_commonmark 0.26 draws the accent on the transparent surface,
        // not on `info`; test the pairing that actually reaches the screen.
        assert!(theme::contrast(palette.info_fg, palette.surface) >= 4.5);
        assert!(theme::contrast(palette.text, palette.note) >= 4.5);
    }
}

#[test]
fn high_contrast_beats_dark_on_panel_text() {
    let dark = ThemePreset::Dark.palette();
    let high = ThemePreset::HighContrast.palette();
    assert!(
        theme::contrast(high.text, high.panel) > theme::contrast(dark.text, dark.panel)
    );
}

#[test]
fn cycle_and_sidecar_roundtrip() {
    assert_eq!(ThemePreset::Dark.next(), ThemePreset::Light);
    assert_eq!(ThemePreset::Light.next(), ThemePreset::Sepia);
    assert_eq!(ThemePreset::Sepia.next(), ThemePreset::HighContrast);
    assert_eq!(ThemePreset::HighContrast.next(), ThemePreset::Dark);
    assert_eq!(ThemePreset::ALL.len(), 4);

    let dir = tempdir().unwrap();
    let path = dir.path().join("theme.idx");
    theme::save(&path, ThemePreset::HighContrast).unwrap();
    assert_eq!(theme::load(&path).unwrap(), ThemePreset::HighContrast);

    fs::write(&path, "WRONG\n").unwrap();
    assert!(theme::load(&path).is_err());
    fs::write(&path, "NEXORA-THEME-v1\ndark\n").unwrap();
    assert!(theme::load(&path).is_err(), "legacy v1 sidecar must not be read");
    fs::write(&path, "NEXORA-THEME-v2\n").unwrap();
    assert!(theme::load(&path).is_err(), "missing preset label");
    fs::write(&path, "NEXORA-THEME-v2\nultraviolet\n").unwrap();
    assert!(theme::load(&path).is_err(), "unknown preset label");
    fs::write(&path, "NEXORA-THEME-v2\nsepia\n").unwrap();
    assert_eq!(theme::load(&path).unwrap(), ThemePreset::Sepia);
}
