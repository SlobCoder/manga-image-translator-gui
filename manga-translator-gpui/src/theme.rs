// manga-translator-gpui/src/theme.rs
//
// Design tokens for the GPUI frontend. Deliberately self-contained (no
// dependency on the GTK/adw theme or on matugen output) so the window looks
// consistent on every desktop.
//
// The palette is mirrored into the `gpui-component` theme (see
// `apply_to_component_theme`) so widgets taken from that library look like the
// hand-drawn panels instead of falling back to its default colours.

use gpui::{App, Hsla, Rgba, px, rgb};

use manga_translator_core::config::{AccentColor, Settings, find_preset};

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
    /// Surface of a preferences card, one step above the page background.
    pub card: Rgba,
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
            card: rgb(0x202327),
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
            card: rgb(0xffffff),
        }
    }

    /// Scheme and accent from the shared configuration.
    pub fn from_settings(settings: &Settings) -> Self {
        Self::for_scheme(&settings.color_scheme).with_accent(&settings.accent_color)
    }

    /// Replace the accent with a preset name, a `#rrggbb` string, or
    /// `system`/empty, which keeps the built-in per-scheme accent.
    pub fn with_accent(mut self, name: &str) -> Self {
        if let Some((accent, foreground)) = resolve_accent(name) {
            self.accent = accent;
            self.accent_fg = foreground;
            self.accent_hover = hover_for(accent, self.dark);
        }
        self
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
            // Adwaita's control radii. `radius` reaches buttons, inputs,
            // dropdowns and menus; `radius_lg` dialogs and notifications.
            theme.radius = px(adw::BUTTON_RADIUS);
            theme.radius_lg = px(adw::DIALOG_RADIUS);
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

            // Buttons read the legacy `tokens` surface rather than `colors`.
            // Without these they fall back to the library palette, which made
            // "primary" black in the light scheme instead of the accent.
            theme.tokens.button = panel_alt.into();
            theme.tokens.button_hover = border.into();
            theme.tokens.button_active = border.into();
            theme.tokens.button_foreground = text.into();
            theme.tokens.button_primary = accent.into();
            theme.tokens.button_primary_hover = accent_hover.into();
            theme.tokens.button_primary_active = accent_hover.into();
            theme.tokens.button_primary_foreground = accent_fg.into();
            theme.tokens.button_secondary = panel_alt.into();
            theme.tokens.button_secondary_hover = border.into();
            theme.tokens.button_secondary_active = border.into();
            theme.tokens.button_secondary_foreground = text.into();
            theme.tokens.button_danger = danger.into();
            theme.tokens.button_danger_hover = danger.into();
            theme.tokens.button_danger_active = danger.into();
            theme.tokens.button_danger_foreground = accent_fg.into();
        });
    }
}

/// The accent a setting names, as `(accent, foreground)`.
fn resolve_accent(name: &str) -> Option<(Rgba, Rgba)> {
    let name = name.trim();
    if name.is_empty() || name == "system" {
        return None;
    }

    let (hex, foreground) = match find_preset(name) {
        Some(preset) if !preset.hex.is_empty() => (preset.hex, preset.fg),
        _ if is_hex(name) => (name.to_string(), AccentColor::foreground_for(name)),
        _ => return None,
    };

    Some((parse_hex(&hex)?, parse_hex(&foreground)?))
}

fn is_hex(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn parse_hex(value: &str) -> Option<Rgba> {
    if !is_hex(value) {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&value[range], 16)
            .ok()
            .map(|v| v as f32 / 255.0)
    };
    Some(Rgba {
        r: channel(1..3)?,
        g: channel(3..5)?,
        b: channel(5..7)?,
        a: 1.0,
    })
}

/// A slightly brighter accent for dark schemes, a little darker for light.
fn hover_for(accent: Rgba, dark: bool) -> Rgba {
    let mut hsla: Hsla = accent.into();
    hsla.l = if dark {
        (hsla.l + 0.10).min(1.0)
    } else {
        (hsla.l - 0.08).max(0.0)
    };
    hsla.into()
}

/// Adwaita control metrics, read from the libadwaita stylesheet in
/// `../libadwaita/src/stylesheet`. Kept apart from [`sizes`], which holds the
/// panel layout constants: these are the numbers GNOME draws its widgets with.
pub mod adw {
    /// `$button_radius` — buttons, entries, menus.
    pub const BUTTON_RADIUS: f32 = 9.0;
    /// `$card_radius` — boxed lists and preference groups.
    pub const CARD_RADIUS: f32 = 12.0;
    /// `$popover_radius` and `$dialog_radius` (`$button_radius + 6`).
    pub const DIALOG_RADIUS: f32 = 15.0;

    /// `button { min-height: 24px; padding: 5px 10px }`.
    pub const BUTTON_H: f32 = 34.0;
    /// `entry { min-height: 34px; padding: 0 9px }`.
    pub const ENTRY_H: f32 = 34.0;
    /// `switch { padding: 3px } > slider { min-width: 20px }`.
    pub const SWITCH_H: f32 = 26.0;
    pub const SWITCH_W: f32 = 46.0;

    /// `headerbar { min-height: 47px }`.
    pub const HEADER_H: f32 = 47.0;

    /// `row { min-height: 50px }` — action rows with a title and subtitle.
    pub const ACTION_ROW_H: f32 = 50.0;
    /// Single-line preference rows and group headers.
    pub const COMPACT_ROW_H: f32 = 34.0;
}

/// Adwaita's type scale as percentages of the body size (`_labels.scss`).
/// One constant moves the whole UI, and the steps are the ones GNOME uses.
pub mod typo {
    /// 100 % — `body`.
    pub const BODY: f32 = 12.5;
    /// 82 % — `caption` and `caption-heading`.
    pub const CAPTION: f32 = BODY * 0.82;
    /// 118 % — `title-4`.
    pub const TITLE_4: f32 = BODY * 1.18;
    /// 136 % — `title-3` and `title-2`.
    pub const TITLE_3: f32 = BODY * 1.36;
}

/// Shared spacing / sizing constants.
pub mod sizes {
    pub const HEADER_H: f32 = super::adw::HEADER_H;
    pub const STATUS_H: f32 = 26.0;
    pub const SIDEBAR_W: f32 = 268.0;
    pub const SETTINGS_W: f32 = 360.0;

    // Draggable panel dividers
    pub const SPLITTER_W: f32 = 6.0;
    pub const SIDEBAR_MIN: f32 = 180.0;
    pub const SIDEBAR_MAX: f32 = 460.0;
    pub const SETTINGS_MIN: f32 = 320.0;
    pub const SETTINGS_MAX: f32 = 520.0;
    /// Space the preview always keeps, so the panels cannot squeeze it away.
    pub const MIN_PREVIEW: f32 = 260.0;

    pub const FONT_SM: f32 = super::typo::CAPTION;
    pub const FONT_BASE: f32 = super::typo::BODY;
    pub const FONT_LG: f32 = super::typo::TITLE_4;
    /// Thumbnails are fitted into a 112x112 box — the same geometry the GTK
    /// frontend uses, so both share one on-disk thumbnail cache.
    pub const THUMB_FIT: u32 = 112;
    pub const PREVIEW_MAX: u32 = 1600;
}
