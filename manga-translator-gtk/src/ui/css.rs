#![allow(dead_code)]
// manga-translator-gtk/src/ui/css.rs
//
// Dynamic CSS generation helpers.
//
// Provides functions to build CSS strings at runtime for:
//   - Accent color overrides
//   - Widget-specific accent color patches
//   - Animation keyframes
//   - Grid/list item selection styles

use crate::config::AccentColor;

// ---------------------------------------------------------------------------
// Accent color CSS
// ---------------------------------------------------------------------------

/// Generate the full accent color CSS override block.
///
/// Uses CSS custom properties (--accent-bg-color, --accent-fg-color)
/// which is the official libadwaita method for accent color overrides.
/// Also overrides @define-color for our custom CSS in style.css.
pub fn accent_color_css(accent: &AccentColor) -> String {
    if accent.hex.is_empty() {
        // "system" preset — no override needed
        return String::new();
    }

    let fg = if accent.fg.is_empty() {
        AccentColor::foreground_for(&accent.hex)
    } else {
        accent.fg.clone()
    };

    format!(
        r#":root {{
    --accent-bg-color: {hex};
    --accent-fg-color: {fg};
    --accent-color: oklab(from var(--accent-bg-color) var(--standalone-color-oklab));
}}

@define-color accent_color {hex};
@define-color accent_bg_color {hex};
@define-color accent_fg_color {fg};
"#,
        hex = accent.hex,
        fg = fg,
    )
}

// ---------------------------------------------------------------------------
// Forced light palette
// ---------------------------------------------------------------------------

/// Surface colors for light mode, applied only while the user forces light.
///
/// `AdwStyleManager::set_color_scheme(ForceLight)` swaps libadwaita's own
/// stylesheet, but a third-party theme can pin the *named* colors globally in
/// `~/.config/gtk-4.0/gtk.css` (Matugen does exactly that with
/// `@define-color window_bg_color #1a1110`). Those definitions are loaded after
/// libadwaita's, so they win regardless of the scheme and the window stays dark.
///
/// Re-asserting the surfaces at `STYLE_PROVIDER_PRIORITY_USER` makes the in-app
/// switch authoritative again: later definitions win, and we only do this for
/// light mode — the dark palette of the user's theme is left untouched.
///
/// Accent, destructive and error colors are deliberately not listed, so the
/// theme's (or the app's own) accent choice survives.
pub fn light_palette_css() -> &'static str {
    r#"@define-color window_bg_color #fafafa;
@define-color window_fg_color rgba(0, 0, 0, 0.8);
@define-color view_bg_color #ffffff;
@define-color view_fg_color rgba(0, 0, 0, 0.8);
@define-color headerbar_bg_color #ffffff;
@define-color headerbar_fg_color rgba(0, 0, 0, 0.8);
@define-color headerbar_border_color rgba(0, 0, 0, 0.1);
@define-color headerbar_backdrop_color @window_bg_color;
@define-color sidebar_bg_color #ebebed;
@define-color sidebar_fg_color rgba(0, 0, 0, 0.8);
@define-color secondary_sidebar_bg_color #f2f2f4;
@define-color secondary_sidebar_fg_color rgba(0, 0, 0, 0.8);
@define-color card_bg_color #ffffff;
@define-color card_fg_color rgba(0, 0, 0, 0.8);
@define-color dialog_bg_color #fafafa;
@define-color dialog_fg_color rgba(0, 0, 0, 0.8);
@define-color popover_bg_color #ffffff;
@define-color popover_fg_color rgba(0, 0, 0, 0.8);
@define-color overview_bg_color #f2f2f4;
@define-color overview_fg_color rgba(0, 0, 0, 0.8);
@define-color shade_color rgba(0, 0, 0, 0.07);
@define-color scrollbar_outline_color rgba(0, 0, 0, 0.1);

:root {
    --window-bg-color: #fafafa;
    --window-fg-color: rgba(0, 0, 0, 0.8);
    --view-bg-color: #ffffff;
    --view-fg-color: rgba(0, 0, 0, 0.8);
    --headerbar-bg-color: #ffffff;
    --headerbar-fg-color: rgba(0, 0, 0, 0.8);
    --sidebar-bg-color: #ebebed;
    --sidebar-fg-color: rgba(0, 0, 0, 0.8);
    --secondary-sidebar-bg-color: #f2f2f4;
    --secondary-sidebar-fg-color: rgba(0, 0, 0, 0.8);
    --card-bg-color: #ffffff;
    --card-fg-color: rgba(0, 0, 0, 0.8);
    --dialog-bg-color: #fafafa;
    --dialog-fg-color: rgba(0, 0, 0, 0.8);
    --popover-bg-color: #ffffff;
    --popover-fg-color: rgba(0, 0, 0, 0.8);
    --overview-bg-color: #f2f2f4;
    --overview-fg-color: rgba(0, 0, 0, 0.8);
}
"#
}

// ---------------------------------------------------------------------------
// Accent swatch CSS
// ---------------------------------------------------------------------------

/// Build CSS for all swatch buttons in the accent color dialog.
/// Uses a single CSS provider instead of one per swatch.
pub fn build_swatch_grid_css() -> &'static str {
    r#"
.accent-swatch {
    min-width: 56px;
    min-height: 56px;
    border-radius: 50%;
    padding: 0;
    transition: transform 150ms ease, box-shadow 150ms ease;
}
.accent-swatch:hover {
    transform: scale(1.1);
}
.accent-swatch:active {
    transform: scale(0.95);
}
.accent-swatch.active {
    box-shadow: 0 0 0 3px @window_bg_color, 0 0 0 5px currentColor;
}
.accent-swatch-system {
    background: alpha(@accent_bg_color, 0.15);
    border: 2px dashed @accent_bg_color;
}
.accent-swatch-label {
    font-size: 0.8em;
}
"#
}

// ---------------------------------------------------------------------------
// Animation CSS
// ---------------------------------------------------------------------------

/// CSS for the start-button pulse-glow (green).
pub fn start_button_css() -> &'static str {
    r#"
@keyframes pulse-glow-green {
    0%   { box-shadow: 0 0 4px alpha(#4CAF50, 0.3); }
    50%  { box-shadow: 0 0 16px alpha(#4CAF50, 0.7); }
    100% { box-shadow: 0 0 4px alpha(#4CAF50, 0.3); }
}

button.start-action-btn {
    background: #4CAF50;
    color: white;
    animation: pulse-glow-green 2s ease-in-out infinite;
    transition: background 200ms ease, box-shadow 200ms ease;
    border-radius: 12px;
    padding: 10px 24px;
    font-weight: 700;
    font-size: 14px;
}

button.start-action-btn:hover {
    background: #66BB6A;
    transform: scale(1.03);
    box-shadow: 0 0 20px alpha(#4CAF50, 0.8);
}

button.start-action-btn:active {
    background: #43A047;
    transform: scale(0.97);
}

button.start-action-btn:disabled {
    background: alpha(#4CAF50, 0.4);
    animation: none;
    color: alpha(white, 0.6);
}
"#
}

/// CSS for the start-button warning state (pulsing red glow when API key is missing).
pub fn start_button_warning_css() -> &'static str {
    r#"
@keyframes pulse-glow-warning {
    0%   { box-shadow: 0 0 4px alpha(#FF5722, 0.5); }
    25%  { box-shadow: 0 0 20px alpha(#FF5722, 0.9); }
    50%  { box-shadow: 0 0 30px alpha(#FF5722, 1.0); }
    75%  { box-shadow: 0 0 20px alpha(#FF5722, 0.9); }
    100% { box-shadow: 0 0 4px alpha(#FF5722, 0.5); }
}

button.start-action-btn.warning {
    background: #D32F2F;
    color: white;
    animation: pulse-glow-warning 0.8s ease-in-out infinite;
    border: 2px solid #FF5722;
}

button.start-action-btn.warning:hover {
    background: #E53935;
    box-shadow: 0 0 30px alpha(#FF5722, 1.0);
}
"#
}

/// CSS for the cancel-button pulse-glow (red).
pub fn cancel_button_css() -> &'static str {
    r#"
@keyframes pulse-glow-red {
    0%   { box-shadow: 0 0 4px alpha(#F44336, 0.3); }
    50%  { box-shadow: 0 0 16px alpha(#F44336, 0.7); }
    100% { box-shadow: 0 0 4px alpha(#F44336, 0.3); }
}

button.cancel-action-btn {
    background: #F44336;
    color: white;
    animation: pulse-glow-red 1.5s ease-in-out infinite;
    transition: background 200ms ease, box-shadow 200ms ease;
    border-radius: 12px;
    padding: 8px 16px;
    font-weight: 700;
}

button.cancel-action-btn:hover {
    background: #EF5350;
    transform: scale(1.05);
    box-shadow: 0 0 20px alpha(#F44336, 0.8);
}

button.cancel-action-btn:active {
    background: #E53935;
    transform: scale(0.95);
}

button.cancel-action-btn:disabled {
    opacity: 0.5;
    animation: none;
}
"#
}

/// CSS for selection bounce animations.
pub fn selection_animations_css() -> &'static str {
    r#"
@keyframes selection-pop {
    0%   { transform: scale(1.0); opacity: 1; }
    30%  { transform: scale(1.08); opacity: 1; }
    60%  { transform: scale(0.96); opacity: 1; }
    100% { transform: scale(1.0); opacity: 1; }
}

@keyframes check-bounce {
    0%   { transform: scale(1.0); opacity: 1; }
    25%  { transform: scale(1.3); opacity: 1; }
    50%  { transform: scale(0.9); opacity: 1; }
    75%  { transform: scale(1.1); opacity: 1; }
    100% { transform: scale(1.0); opacity: 1; }
}

@keyframes row-bounce {
    0%   { transform: scale(1.0); opacity: 1; }
    25%  { transform: scale(1.05); opacity: 1; }
    50%  { transform: scale(0.97); opacity: 1; }
    75%  { transform: scale(1.02); opacity: 1; }
    100% { transform: scale(1.0); opacity: 1; }
}

@keyframes row-appear {
    0%   { opacity: 0; transform: translateX(-8px); }
    100% { opacity: 1; transform: translateX(0); }
}

@keyframes row-appear-right {
    0%   { opacity: 0; transform: translateX(8px); }
    100% { opacity: 1; transform: translateX(0); }
}

@keyframes row-appear-left {
    0%   { opacity: 0; transform: translateX(-8px); }
    100% { opacity: 1; transform: translateX(0); }
}

.grid-item-box.accent-selected {
    animation: selection-pop 250ms ease-out;
}

.file-row.just-selected {
    animation: row-bounce 300ms ease-out;
}

.row-appear-right {
    animation: row-appear-right 300ms ease-out;
}

.row-appear-left {
    animation: row-appear-left 300ms ease-out;
}
"#
}

/// CSS for the progress bar (red-to-green gradient).
pub fn progress_bar_css() -> &'static str {
    r#"
progressbar.translation-progress {
    min-height: 6px;
    border-radius: 3px;
}

progressbar.translation-progress trough {
    min-height: 6px;
    border-radius: 3px;
    background: alpha(@borders, 0.4);
}

progressbar.translation-progress progress {
    min-height: 6px;
    border-radius: 3px;
    background: linear-gradient(to right,
        #F44336, #FF9800, #FFEB3B, #8BC34A, #4CAF50);
}

progressbar.translation-progress.active progress {
    animation: pulse-glow-green 1s ease-in-out infinite;
}
"#
}

/// CSS for the completion success animation (progress bar + button glow).
pub fn completion_css() -> &'static str {
    r#"
@keyframes success-glow {
    0%   { box-shadow: 0 0 4px alpha(#4CAF50, 0.3); }
    50%  { box-shadow: 0 0 24px alpha(#4CAF50, 0.9); }
    100% { box-shadow: 0 0 4px alpha(#4CAF50, 0.3); }
}

@keyframes progress-success-pulse {
    0%   { opacity: 1; }
    50%  { opacity: 0.7; }
    100% { opacity: 1; }
}

progressbar.translation-progress.completed progress {
    background: #4CAF50;
    animation: progress-success-pulse 0.6s ease-in-out;
}

button.start-action-btn.success {
    background: #4CAF50;
    color: white;
    animation: success-glow 0.6s ease-in-out;
    border-radius: 12px;
    padding: 10px 24px;
    font-weight: 700;
    font-size: 14px;
}
"#
}

/// CSS for suppressing GTK's internal hover/selection highlights
/// that conflict with our custom selection styles.
pub fn gtk_override_css() -> &'static str {
    r#"
/* Suppress GTK's internal hover on GridView children */
gridview child:hover {
    background: transparent;
}

/* Suppress button hover inside folder rows (double-highlight fix) */
.folder-row button.flat,
.folder-row button.flat:hover,
.folder-row button.flat:active {
    background: transparent;
}
"#
}

/// CSS for the spinner fade-out animation in the start button.
pub fn spinner_fade_css() -> &'static str {
    r#"
@keyframes spinner-fade-out {
    0%   { opacity: 1; }
    100% { opacity: 0; }
}

.spinner-fading {
    animation: spinner-fade-out 300ms ease-out forwards;
}
"#
}

// ---------------------------------------------------------------------------
// Combined builder
// ---------------------------------------------------------------------------

/// Build the complete application CSS as a single string.
/// This combines all static CSS fragments into one stylesheet.
pub fn build_combined_css() -> String {
    let mut css = String::with_capacity(8192);

    css.push_str(start_button_css());
    css.push_str(start_button_warning_css());
    css.push_str(cancel_button_css());
    css.push_str(selection_animations_css());
    css.push_str(progress_bar_css());
    css.push_str(completion_css());
    css.push_str(spinner_fade_css());

    css.push_str(gtk_override_css());

    css
}

/// Build an accent-color-only CSS string for the given accent.
pub fn build_accent_css(accent: &AccentColor) -> String {
    accent_color_css(accent)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AccentColor;

    #[test]
    fn test_accent_css_system_is_empty() {
        let system = AccentColor {
            name: "system".into(),
            hex: String::new(),
            fg: String::new(),
        };
        assert!(accent_color_css(&system).is_empty());
    }

    #[test]
    fn test_accent_css_blue_contains_hex() {
        let blue = AccentColor {
            name: "blue".into(),
            hex: "#3584e4".into(),
            fg: "#ffffff".into(),
        };
        let css = accent_color_css(&blue);
        assert!(css.contains("#3584e4"));
        assert!(css.contains("--accent-bg-color"));
        assert!(css.contains("@define-color accent_color"));
    }

    #[test]
    fn test_swatch_css() {
        let css = build_swatch_grid_css();
        assert!(css.contains(".accent-swatch"));
        assert!(css.contains("border-radius: 50%"));
    }

    #[test]
    fn test_combined_css_not_empty() {
        let css = build_combined_css();
        assert!(!css.is_empty());
        assert!(css.contains("pulse-glow-green"));
        assert!(css.contains("pulse-glow-warning"));
        assert!(css.contains("selection-pop"));
    }
}
