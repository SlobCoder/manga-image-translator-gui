// manga-translator-gpui/src/ui/widgets.rs
//
// Small, reusable visual primitives shared by all panels.

use gpui::{Div, ElementId, FontWeight, SharedString, Stateful, div, prelude::*, px, relative};

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

/// Solid button. The caller attaches the click handler.
pub fn button(
    theme: Theme,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(28.))
        .px_3()
        .rounded_md()
        .bg(theme.panel_alt)
        .border_1()
        .border_color(theme.border)
        .text_size(px(FONT_BASE))
        .text_color(theme.text)
        .cursor_pointer()
        .hover(move |this| this.bg(theme.border))
        .child(label.into())
}

/// Accent-coloured primary button.
pub fn primary_button(
    theme: Theme,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(28.))
        .px_4()
        .rounded_md()
        .bg(theme.accent)
        .text_size(px(FONT_BASE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.accent_fg)
        .cursor_pointer()
        .hover(move |this| this.bg(theme.accent_hover))
        .child(label.into())
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
pub fn progress_bar(theme: Theme, fraction: f64) -> Div {
    let f = fraction.clamp(0.0, 1.0) as f32;
    div()
        .h(px(4.))
        .w_full()
        .rounded_full()
        .bg(theme.border)
        .child(
            div()
                .h(px(4.))
                .w(relative(if f > 0.0 { f } else { 0.0001 }))
                .rounded_full()
                .bg(theme.accent),
        )
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
pub fn pill(_theme: Theme, text: impl Into<SharedString>, color: gpui::Rgba) -> Div {
    let background = gpui::Hsla::from(color).opacity(0.18);
    div()
        .px_2()
        .py(px(1.))
        .rounded_full()
        .bg(background)
        .text_size(px(FONT_SM))
        .text_color(color)
        .child(text.into())
}
