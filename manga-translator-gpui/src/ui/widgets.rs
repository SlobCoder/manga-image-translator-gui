// manga-translator-gpui/src/ui/widgets.rs
//
// Small, reusable visual primitives shared by all panels.
//
// Buttons, progress bars and status tags come from `gpui-component`; their
// colours follow the palette pushed into the component theme by
// `Theme::apply_to_component_theme`. The toggle stays hand-drawn because
// `Switch::on_click` reports a bool rather than a click event.

use gpui::component::{
    Sizable as _, Size,
    button::{Button, ButtonVariants as _},
    progress::Progress,
    tag::Tag,
};
use gpui::{Div, ElementId, FontWeight, Hsla, Rgba, SharedString, Stateful, div, prelude::*, px};

use crate::theme::{Theme, adw, sizes::*};

/// Uppercase section label with a thin rule underneath.
pub fn section_header(theme: Theme, title: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .justify_center()
        .min_h(px(adw::COMPACT_ROW_H))
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
    Button::new(id).label(label).with_size(px(adw::BUTTON_H))
}

/// Accent-coloured primary button.
pub fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .label(label)
        .primary()
        .with_size(px(adw::BUTTON_H))
}

/// Pill-shaped on/off switch, sized like `AdwSwitchRow`.
pub fn toggle(theme: Theme, id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    let (track, knob) = if on {
        (theme.accent, theme.accent_fg)
    } else {
        (theme.border, theme.text_faint)
    };
    let knob_size = adw::SWITCH_H - 6.0;
    div()
        .id(id)
        .w(px(adw::SWITCH_W))
        .h(px(adw::SWITCH_H))
        .rounded_full()
        .bg(track)
        .cursor_pointer()
        .flex()
        .items_center()
        .child(
            div()
                .w(px(knob_size))
                .h(px(knob_size))
                .rounded_full()
                .bg(knob)
                .ml(px(if on {
                    adw::SWITCH_W - knob_size - 3.0
                } else {
                    3.0
                })),
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

/// Hairline row divider, used inside [`card`].
fn row_divider(theme: Theme) -> Div {
    div().h(px(1.)).w_full().bg(theme.border)
}

/// A card holding rows separated by hairlines — the boxed list Adwaita draws
/// inside a preferences group.
pub fn card(theme: Theme, rows: Vec<Div>) -> Div {
    let mut card = div()
        .flex()
        .flex_col()
        .w_full()
        .rounded(px(adw::CARD_RADIUS))
        .bg(theme.card)
        .overflow_hidden();

    for (index, row) in rows.into_iter().enumerate() {
        if index > 0 {
            card = card.child(row_divider(theme));
        }
        card = card.child(row);
    }

    card
}

/// Coloured status pill.
pub fn pill(text: impl Into<SharedString>, color: Rgba) -> Tag {
    let color = Hsla::from(color);
    Tag::custom(color.opacity(0.18), color, color.opacity(0.35))
        .rounded_full()
        .child(text.into())
}
