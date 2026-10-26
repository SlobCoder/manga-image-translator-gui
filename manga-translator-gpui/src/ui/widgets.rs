// manga-translator-gpui/src/ui/widgets.rs
//
// Small, reusable visual primitives shared by all panels.
//
// Buttons, progress bars and status tags come from `gpui-component`; their
// colours follow the palette pushed into the component theme by
// `Theme::apply_to_component_theme`. The toggle and the select trigger stay
// hand-drawn: `Switch::on_click` reports a bool rather than a click, and the
// dropdown needs the caller's own popup state (a full `Select` needs an
// `Entity` that owns it, which is the next migration step).

use gpui::component::{
    Sizable as _, Size,
    button::{Button, ButtonVariants as _},
    progress::Progress,
    tag::Tag,
};
use gpui::{Div, ElementId, FontWeight, Hsla, Rgba, SharedString, Stateful, div, prelude::*, px};

use crate::theme::{Theme, sizes::*};

/// Uppercase section label with a thin rule underneath.
pub fn section_header(theme: Theme, title: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .mt_3()
        .mb_1()
        .child(
            div()
                .text_size(px(FONT_SM))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_faint)
                .child(title.into().to_uppercase()),
        )
        .child(div().h(px(1.)).w_full().bg(theme.border))
}

/// Label + optional description, used on the left of a setting row.
pub fn label_block(theme: Theme, label: impl Into<SharedString>, desc: Option<String>) -> Div {
    let mut block = div().flex().flex_col().flex_1().min_w(px(0.)).child(
        div()
            .text_size(px(FONT_BASE))
            .text_color(theme.text)
            .whitespace_nowrap()
            .overflow_hidden()
            .child(label.into()),
    );
    if let Some(desc) = desc {
        block = block.child(
            div()
                .text_size(px(FONT_SM))
                .text_color(theme.text_faint)
                .child(desc),
        );
    }
    block
}

/// Neutral button at the row height used by the panels.
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label).with_size(px(28.))
}

/// Accent-coloured primary button.
pub fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label).primary().with_size(px(28.))
}

/// Small square checkbox-style toggle.
pub fn toggle(theme: Theme, id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    let (track, knob) = if on {
        (theme.accent, theme.accent_fg)
    } else {
        (theme.border, theme.text_faint)
    };
    div()
        .id(id)
        .w(px(30.))
        .h(px(17.))
        .rounded_full()
        .bg(track)
        .cursor_pointer()
        .flex()
        .items_center()
        .child(
            div()
                .w(px(13.))
                .h(px(13.))
                .rounded_full()
                .bg(knob)
                .ml(px(if on { 15. } else { 2. })),
        )
}

/// Dropdown trigger showing the current value and a chevron.
pub fn select_trigger(
    theme: Theme,
    id: impl Into<ElementId>,
    value: impl Into<SharedString>,
    open: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .h(px(26.))
        .px_2()
        .rounded_md()
        .border_1()
        .border_color(if open { theme.accent } else { theme.border })
        .bg(theme.panel_alt)
        .cursor_pointer()
        .hover(move |this| this.border_color(theme.accent))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_size(px(FONT_BASE))
                .text_color(theme.text)
                .whitespace_nowrap()
                .overflow_hidden()
                .child(value.into()),
        )
        .child(
            div()
                .text_size(px(FONT_SM))
                .text_color(theme.text_faint)
                .child(if open { "▲" } else { "▼" }),
        )
}

/// Horizontal progress bar.
pub fn progress_bar(fraction: f64) -> Progress {
    let percent = (fraction.clamp(0.0, 1.0) * 100.0) as f32;
    Progress::new("status-progress")
        .value(percent)
        .with_size(Size::XSmall)
}

/// Muted caption text.
#[allow(dead_code)]
pub fn caption(theme: Theme, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(FONT_SM))
        .text_color(theme.text_faint)
        .child(text.into())
}

/// Coloured status pill.
pub fn pill(text: impl Into<SharedString>, color: Rgba) -> Tag {
    let color = Hsla::from(color);
    Tag::custom(color.opacity(0.18), color, color.opacity(0.35))
        .rounded_full()
        .child(text.into())
}
