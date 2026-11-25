//! Authored theme presets and their contrast checks. See `ARCHITECTURE.md`
//! §35 (historical) and §85 (DESIGN v2 identity replacement).

use crate::NexoraError;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

const MAGIC: &str = "NEXORA-THEME-v2";

pub type Rgb = [u8; 3];

/// Every colour the shell draws comes from this one table; nothing is
/// hard-coded in the egui layer. §85 adds the Obsidian-v2 roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg: Rgb,
    pub panel: Rgb,
    pub surface: Rgb,
    pub raised: Rgb,
    pub tag: Rgb,
    pub tag_hover: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub accent: Rgb,
    pub accent_dim: Rgb,
    pub info: Rgb,
    pub info_fg: Rgb,
    pub note: Rgb,
    pub link: Rgb,
    pub sage: Rgb,
    pub amber: Rgb,
    pub rust: Rgb,
    pub line: Rgb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePreset {
    Dark,
    Light,
    Sepia,
    HighContrast,
}

impl ThemePreset {
    pub const ALL: [Self; 4] = [Self::Dark, Self::Light, Self::Sepia, Self::HighContrast];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Sepia => "sepia",
            Self::HighContrast => "high-contrast",
        }
    }

    /// Strict: an unknown label is an error, never a silent fallback to Dark.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "sepia" => Some(Self::Sepia),
            "high-contrast" => Some(Self::HighContrast),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Sepia,
            Self::Sepia => Self::HighContrast,
            Self::HighContrast => Self::Dark,
        }
    }

    /// egui's own dark/light mode follows the preset, so native widgets without
    /// an explicit colour still fit the table. Sepia is a light mode.
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark | Self::HighContrast)
    }

    pub const fn palette(self) -> Palette {
        match self {
            Self::Dark => Palette {
                bg: [0x12, 0x12, 0x12],
                panel: [0x17, 0x17, 0x17],
                surface: [0x22, 0x22, 0x22],
                raised: [0x2D, 0x2D, 0x2D],
                tag: [0x2D, 0x28, 0x38],
                tag_hover: [0x45, 0x3B, 0x5F],
                text: [0xE0, 0xE0, 0xE0],
                muted: [0xC8, 0xC8, 0xC8],
                accent: [0xC9, 0xBA, 0xFF],
                accent_dim: [0x4A, 0x44, 0x5C],
                info: [0x27, 0x35, 0x35],
                info_fg: [0xA4, 0xD4, 0xF4],
                note: [0x22, 0x2B, 0x39],
                link: [0xC9, 0xBA, 0xFF],
                sage: [0x8F, 0xA9, 0x8A],
                amber: [0xD8, 0xA6, 0x57],
                rust: [0xE0, 0x6B, 0x6B],
                line: [0x36, 0x36, 0x36],
            },
            Self::Light => Palette {
                bg: [0xF7, 0xF7, 0xF9],
                panel: [0xED, 0xED, 0xF2],
                surface: [0xFF, 0xFF, 0xFF],
                raised: [0xE6, 0xE6, 0xEF],
                tag: [0xE6, 0xE0, 0xF5],
                tag_hover: [0xD8, 0xCE, 0xF0],
                text: [0x1F, 0x1D, 0x26],
                muted: [0x56, 0x51, 0x66],
                accent: [0x5B, 0x43, 0xA8],
                accent_dim: [0xC9, 0xBC, 0xEA],
                info: [0xDC, 0xEC, 0xEC],
                info_fg: [0x12, 0x48, 0x4E],
                note: [0xE6, 0xEC, 0xF6],
                link: [0x5B, 0x43, 0xA8],
                sage: [0x2E, 0x6B, 0x4F],
                amber: [0x7A, 0x52, 0x00],
                rust: [0xA3, 0x21, 0x21],
                line: [0xD2, 0xD0, 0xDA],
            },
            Self::Sepia => Palette {
                bg: [0xF4, 0xEE, 0xE3],
                panel: [0xEA, 0xE1, 0xD1],
                surface: [0xFB, 0xF6, 0xEC],
                raised: [0xE3, 0xD8, 0xC5],
                tag: [0xE5, 0xDC, 0xCB],
                tag_hover: [0xD8, 0xC9, 0xAE],
                text: [0x2B, 0x24, 0x1B],
                muted: [0x5B, 0x51, 0x44],
                accent: [0x6B, 0x4F, 0xA8],
                accent_dim: [0xCF, 0xC1, 0xE5],
                info: [0xE3, 0xED, 0xE5],
                info_fg: [0x17, 0x4B, 0x3B],
                note: [0xE5, 0xE9, 0xF0],
                link: [0x6B, 0x4F, 0xA8],
                sage: [0x3F, 0x6B, 0x3A],
                amber: [0x8A, 0x5A, 0x00],
                rust: [0x9E, 0x2E, 0x2E],
                line: [0xD4, 0xC7, 0xB2],
            },
            Self::HighContrast => Palette {
                bg: [0x00, 0x00, 0x00],
                panel: [0x00, 0x00, 0x00],
                surface: [0x0A, 0x0A, 0x0A],
                raised: [0x16, 0x16, 0x16],
                tag: [0x1C, 0x15, 0x24],
                tag_hover: [0x2A, 0x20, 0x38],
                text: [0xFF, 0xFF, 0xFF],
                muted: [0xE6, 0xE6, 0xE6],
                accent: [0xE4, 0xD7, 0xFF],
                accent_dim: [0x6B, 0x5A, 0x99],
                info: [0xB8, 0xF0, 0xF0],
                // The renderer cannot paint the callout fill, so the label
                // sits on `surface`; a dark foreground would be invisible.
                info_fg: [0xB8, 0xF0, 0xF0],
                note: [0x0A, 0x1A, 0x2A],
                link: [0xE4, 0xD7, 0xFF],
                sage: [0x66, 0xFF, 0x66],
                amber: [0xFF, 0xD1, 0x66],
                rust: [0xFF, 0x80, 0x80],
                line: [0xFF, 0xFF, 0xFF],
            },
        }
    }
}

fn luminance(rgb: Rgb) -> f64 {
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
}

/// WCAG contrast ratio between two RGB colours.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (lighter, darker) = if luminance(a) >= luminance(b) {
        (luminance(a), luminance(b))
    } else {
        (luminance(b), luminance(a))
    };
    (lighter + 0.05) / (darker + 0.05)
}

pub fn save(path: &Path, preset: ThemePreset) -> Result<(), NexoraError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| NexoraError::io_at(parent, error))?;
    }
    let text = format!("{MAGIC}\n{}\n", preset.label());
    fs::write(path, text.as_bytes()).map_err(|error| NexoraError::io_at(path, error))
}

pub fn load(path: &Path) -> Result<ThemePreset, NexoraError> {
    if !path.exists() {
        return Ok(ThemePreset::Dark);
    }
    let text = fs::read_to_string(path).map_err(|error| {
        if error.kind() == ErrorKind::InvalidData {
            NexoraError::InvalidUtf8 {
                path: path.to_path_buf(),
            }
        } else {
            NexoraError::io_at(path, error)
        }
    })?;
    let mut lines = text.lines();
    if lines.next() != Some(MAGIC) {
        return Err(NexoraError::InvalidInput {
            message: "bad theme sidecar header".to_string(),
        });
    }
    let body = lines.next().ok_or_else(|| NexoraError::InvalidInput {
        message: "missing theme preset".to_string(),
    })?;
    ThemePreset::parse(body).ok_or_else(|| NexoraError::InvalidInput {
        message: "bad theme preset".to_string(),
    })
}
