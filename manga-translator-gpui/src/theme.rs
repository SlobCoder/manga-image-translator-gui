// manga-translator-gpui/src/theme.rs
//
// Design tokens for the GPUI frontend. Deliberately self-contained (no
// dependency on the GTK/adw theme or on matugen output) so the window looks
// consistent on every desktop.
//
// The palette is mirrored into the `gpui-component` theme (see
// `apply_to_component_theme`) so widgets taken from that library look like the
// hand-drawn panels instead of falling back to its default colours.

use gpui::{App, Rgba, px, rgb};

/// A minimal, copyable set of design tokens.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Whether this palette belongs to the dark scheme.
    pub dark: bool,
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
            dark: true,
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
            dark: false,
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

    /// Push this palette into the `gpui-component` global theme.
    ///
    /// Call once after `gpui::init`, and again whenever the scheme changes.
    /// Library widgets (`Button`, `Input`, `Select`, `Dialog`, …) read their
    /// colours from that global, so this is what keeps them in step with the
    /// panels that still draw themselves.
    pub fn apply_to_component_theme(&self, cx: &mut App) {
        use gpui::component::{Theme as ComponentTheme, ThemeMode};

        ComponentTheme::change(
            if self.dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            None,
            cx,
        );

        let palette = *self;
        ComponentTheme::update(cx, move |theme| {
            // Component defaults are 16 px; the panels run at 12.5.
            theme.font_size = px(sizes::FONT_BASE);
            let c = &mut theme.colors;
            let bg = palette.bg.into();
            let panel = palette.panel.into();
            let panel_alt = palette.panel_alt.into();
            let border = palette.border.into();
            let text = palette.text.into();
            let text_muted = palette.text_muted.into();
            let text_faint = palette.text_faint.into();
            let accent = palette.accent.into();
            let accent_hover = palette.accent_hover.into();
            let accent_fg = palette.accent_fg.into();
            let success = palette.success.into();
            let danger = palette.danger.into();
            let selection = palette.selection.into();

            // Surfaces
            c.background = bg;
            c.foreground = text;
            c.border = border;
            c.popover = panel;
            c.popover_foreground = text;
            c.muted = panel_alt;
            c.muted_foreground = text_muted;
            c.skeleton = panel_alt;
            c.drop_target = selection;
            c.drag_border = accent;

            // Text
            c.secondary_foreground = text_muted;
            c.caret = accent;

            // Accent / primary
            c.primary = accent;
            c.primary_hover = accent_hover;
            c.primary_active = accent_hover;
            c.primary_foreground = accent_fg;
            c.accent = selection;
            c.accent_foreground = text;
            c.ring = accent;
            c.selection = selection;

            // Secondary + plain buttons
            c.secondary = panel_alt;
            c.secondary_hover = border;
            c.secondary_active = border;
            c.button = panel_alt;
            c.button_hover = border;
            c.button_active = border;
            c.button_foreground = text;

            // Status colours
            c.success = success;
            c.success_foreground = accent_fg;
            c.success_hover = success;
            c.success_active = success;
            c.danger = danger;
            c.danger_foreground = accent_fg;
            c.danger_hover = danger;
            c.danger_active = danger;

            // Inputs, lists and tabs
            c.input = border;
            c.list = panel;
            c.list_even = panel;
            c.list_head = panel_alt;
            c.list_hover = panel_alt;
            c.list_active = selection;
            c.list_active_border = accent;
            c.tab = panel_alt;
            c.tab_bar = panel;
            c.tab_foreground = text_muted;
            c.tab_active = panel_alt;
            c.tab_active_foreground = text;
            c.table = panel;

            // Sidebar
            c.sidebar = panel;
            c.sidebar_foreground = text;
            c.sidebar_border = border;
            c.sidebar_accent = selection;
            c.sidebar_accent_foreground = text;
            c.sidebar_primary = accent;
            c.sidebar_primary_foreground = accent_fg;

            // Indicators
            c.progress_bar = border;
            c.scrollbar = panel;
            c.scrollbar_thumb = text_faint;
            c.scrollbar_thumb_hover = text_muted;
            c.slider_bar = border;
            c.slider_thumb = accent;
            c.switch = border;
        });
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
