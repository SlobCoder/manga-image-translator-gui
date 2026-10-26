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

use crate::theme::{Theme, adw, sizes::*, typo};

/// Group heading in Adwaita's `title-4` step. Used above a card in the
/// preferences panel and as the section label in panels and dialogs.
pub fn group_heading(theme: Theme, title: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(typo::TITLE_4))
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text)
        .child(title.into())
}

/// Section label above a group of rows.
pub fn section_header(theme: Theme, title: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .justify_center()
        .min_h(px(adw::COMPACT_ROW_H))
        .mt_3()
        .mb_1()
        .child(group_heading(theme, title))
}

/// Label + optional description, used on the left of a setting row.
///
/// Mirrors `AdwActionRow`: the title and the subtitle wrap at word boundaries
/// (`wrap-mode: word-char`) instead of being cut off, they sit 3 px apart, and
/// the 6 px above and below keep a two-line title off the row edges.
pub fn label_block(theme: Theme, label: impl Into<SharedString>, desc: Option<String>) -> Div {
    let mut block = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.))
        .gap(px(3.))
        .py(px(6.))
        .child(
            div()
                .text_size(px(FONT_BASE))
                .text_color(theme.text)
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
