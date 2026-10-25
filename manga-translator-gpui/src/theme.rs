// manga-translator-gpui/src/theme.rs
//
// Design tokens for the GPUI frontend. Deliberately self-contained (no
// dependency on the GTK/adw theme or on matugen output) so the window looks
// consistent on every desktop.

use gpui::{Rgba, rgb};

/// A minimal, copyable set of design tokens.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Rgba,
    pub panel: Rgba,
    pub panel_alt: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub text_faint: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_fg: Rgba,
    pub success: Rgba,
    pub danger: Rgba,
    pub warning: Rgba,
    pub selection: Rgba,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            bg: rgb(0x141517),
            panel: rgb(0x1b1d20),
            panel_alt: rgb(0x23262a),
            border: rgb(0x33373d),
            text: rgb(0xe7e9ec),
            text_muted: rgb(0x9aa1a9),
            text_faint: rgb(0x6d747c),
            accent: rgb(0x4d8df6),
            accent_hover: rgb(0x659dfa),
            accent_fg: rgb(0xffffff),
            success: rgb(0x59b95c),
            danger: rgb(0xe25c5c),
            warning: rgb(0xd9a441),
            selection: rgb(0x2c4a7c),
        }
    }

    pub fn light() -> Self {
        Self {
            bg: rgb(0xf4f6f8),
            panel: rgb(0xffffff),
            panel_alt: rgb(0xeceff3),
            border: rgb(0xd6dbe2),
            text: rgb(0x1b1e22),
            text_muted: rgb(0x5d6570),
            text_faint: rgb(0x8a929c),
            accent: rgb(0x2f6fdd),
            accent_hover: rgb(0x4785ef),
            accent_fg: rgb(0xffffff),
            success: rgb(0x2f8f3a),
            danger: rgb(0xc0392b),
            warning: rgb(0xa9781c),
            selection: rgb(0xcfe0fb),
        }
    }

    pub fn for_scheme(scheme: &str) -> Self {
        match scheme {
            "light" => Self::light(),
            "dark" => Self::dark(),
            // "system" — GPUI exposes no portable appearance query, so a
            // dark default keeps the app readable on both setups.
            _ => Self::dark(),
        }
    }
}

/// Shared spacing / sizing constants.
pub mod sizes {
    pub const HEADER_H: f32 = 46.0;
    pub const STATUS_H: f32 = 26.0;
    pub const SIDEBAR_W: f32 = 268.0;
    pub const SETTINGS_W: f32 = 330.0;

    // Draggable panel dividers
    pub const SPLITTER_W: f32 = 6.0;
    pub const SIDEBAR_MIN: f32 = 180.0;
    pub const SIDEBAR_MAX: f32 = 460.0;
    pub const SETTINGS_MIN: f32 = 240.0;
    pub const SETTINGS_MAX: f32 = 460.0;
    /// Space the preview always keeps, so the panels cannot squeeze it away.
    pub const MIN_PREVIEW: f32 = 260.0;

    pub const FONT_SM: f32 = 11.5;
    pub const FONT_BASE: f32 = 12.5;
    pub const FONT_LG: f32 = 14.5;
    /// Thumbnails are fitted into a 112x112 box — the same geometry the GTK
    /// frontend uses, so both share one on-disk thumbnail cache.
    pub const THUMB_FIT: u32 = 112;
    pub const PREVIEW_MAX: u32 = 1600;
}
