// manga-translator-gpui/src/ui/preview_panel.rs
//
// Centre pane: original page, translated page, or a comparison slider.
//
// The comparison view is a `canvas` element: both images are painted into the
// same fitted rectangle and the translated one is clipped with a content mask,
// so the split is pixel-exact and follows resizes without extra layout work.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, ContentMask, Context, Corners, DispatchPhase, Div, Entity,
    FontWeight, Hsla, Image, ImageSource, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit,
    PathBuilder, Pixels, RenderImage, WeakEntity, Window, canvas, div, fill, img, point,
    prelude::*, px, size,
};

use manga_translator_core::i18n::t;

use crate::app::{MangaApp, PreviewMode};
use crate::theme::{Theme, sizes::*};
use crate::ui::canvas_text;
use crate::ui::page_turn;
use crate::ui::widgets;

impl MangaApp {
    pub(crate) fn render_preview(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let selected = self.selected_image();
        let translated_available =
            self.translated_preview.is_some() || self.translated_path_for_selected().is_some();

        // ---- toolbar -----------------------------------------------------
        let title = selected
            .map(|image| image.name.clone())
            .unwrap_or_else(|| t("Keine Seite ausgewählt"));

        let modes = [
            (PreviewMode::Original, t("Original")),
            (PreviewMode::Translated, t("Übersetzt")),
            (PreviewMode::Compare, t("Vergleich")),
        ];

        let mut tabs = vec![];
        for (mode, label) in modes {
            let active = self.preview_mode == mode;
            tabs.push(segment_button(
                theme,
                mode,
                label,
                active,
                mode != PreviewMode::Original && !translated_available,
                cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    this.set_preview_mode(mode, cx);
                }),
            ));
        }

        let mut toolbar = div()
            .h(px(34.))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(FONT_BASE))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .child(title),
            )
            .child(widgets::button("btn-prev", "‹").on_click(cx.listener(
                |this, _: &ClickEvent, _window, cx| {
                    this.select_next(-1, cx);
                },
            )))
            .child(widgets::button("btn-next", "›").on_click(cx.listener(
                |this, _: &ClickEvent, _window, cx| {
                    this.select_next(1, cx);
                },
            )));

        for tab in tabs {
            toolbar = toolbar.child(tab);
        }

        // ---- body --------------------------------------------------------
        let body: Div = match self.preview_mode {
            PreviewMode::Original => picture_body(
                self.preview.clone(),
                theme,
                translated_available,
                false,
                self.page_turn.clone(),
            ),
            PreviewMode::Translated => picture_body(
                self.translated_preview.clone(),
                theme,
                translated_available,
                true,
                self.page_turn.clone(),
            ),
            PreviewMode::Compare => match self.preview.clone() {
                Some(original) => canvas_body(
                    self.split,
                    original,
                    self.translated_preview.clone(),
                    cx.entity().downgrade(),
                ),
                None => empty_state(
                    theme,
                    empty_message(translated_available, false),
                    self.page_turn.clone(),
                ),
            },
        };

        let hint = self.compare_hint(translated_available);

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .bg(theme.bg)
            .child(toolbar)
            .when_some(hint, |this, hint| this.child(hint))
            .child(body)
    }

    fn compare_hint(&self, translated_available: bool) -> Option<Div> {
        if self.preview_mode != PreviewMode::Compare || translated_available {
            return None;
        }
        let theme = self.theme;
        Some(
            div()
                .w_full()
                .px_3()
                .py_1()
                .bg(theme.panel)
                .border_b_1()
                .border_color(theme.border)
                .text_size(px(FONT_SM))
                .text_color(theme.text_faint)
                .child(t(
                    "Noch keine Übersetzung – starte die Übersetzung über die Kopfzeile.",
                )),
        )
    }
}

// ---------------------------------------------------------------------------
// Single-image body
// ---------------------------------------------------------------------------

fn picture_body(
    image: Option<Arc<Image>>,
    theme: Theme,
    translated_available: bool,
    showing_translated: bool,
    page_turn: Entity<page_turn::PageTurnView>,
) -> Div {
    match image {
        Some(image) => div()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .p_3()
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        img(ImageSource::Image(image))
                            .w_full()
                            .h_full()
                            .object_fit(ObjectFit::Contain),
                    ),
            ),
        None => empty_state(
            theme,
            empty_message(translated_available, showing_translated),
            page_turn,
        ),
    }
}

fn empty_message(translated_available: bool, showing_translated: bool) -> String {
    if showing_translated && !translated_available {
        t("Noch keine Übersetzung – starte die Übersetzung über die Kopfzeile.")
    } else {
        t("Wähle links eine Seite aus.")
    }
}

/// Nothing selected: the animated manga book, like the GTK placeholder.
fn empty_state(theme: Theme, message: String, page_turn: Entity<page_turn::PageTurnView>) -> Div {
    div()
        .relative()
        .flex_1()
        .min_h(px(0.))
        .w_full()
        .overflow_hidden()
        .child(page_turn)
        .child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .pb_2()
                .flex()
                .justify_center()
                .child(
                    div()
                        .text_size(px(FONT_SM))
                        .text_color(theme.text_faint)
                        .child(message),
                ),
        )
}

// ---------------------------------------------------------------------------
// Comparison canvas
// ---------------------------------------------------------------------------

fn canvas_body(
    split: f32,
    original: Arc<Image>,
    translated: Option<Arc<Image>>,
    weak: WeakEntity<MangaApp>,
) -> Div {
    let has_translated = translated.is_some();
    let mut wrapper = div()
        .flex_1()
        .min_h(px(0.))
        .w_full()
        .overflow_hidden()
        .p_3()
        .child(compare_canvas(split, original, translated, weak));

    if has_translated {
        wrapper = wrapper.cursor(gpui::CursorStyle::ResizeColumn);
    }

    wrapper
}

fn compare_canvas(
    split: f32,
    original: Arc<Image>,
    translated: Option<Arc<Image>>,
    weak: WeakEntity<MangaApp>,
) -> impl IntoElement {
    canvas(
        move |_bounds, window, cx| {
            let original = original.use_render_image(window, cx);
            let translated = translated.and_then(|image| image.use_render_image(window, cx));
            (original, translated)
        },
        move |bounds, (original, translated), window, cx| {
            let Some(original) = original else {
                return;
            };
            let Some(fitted) = fitted_rect(bounds, &original) else {
                return;
            };

            // Base layer: the original page.
            let _ = window.paint_image(
                fitted,
                fitted,
                Corners::default(),
                original.clone(),
                0,
                false,
            );

            // Overlay: the translated page, clipped to the right of the divider.
            if let Some(translated) = &translated {
                let split_x = fitted.origin.x + fitted.size.width * split;
                let mask = ContentMask {
                    bounds: Bounds::new(
                        point(split_x, fitted.origin.y),
                        size(
                            fitted.size.width - (split_x - fitted.origin.x),
                            fitted.size.height,
                        ),
                    ),
                };
                window.with_content_mask(Some(mask), |window| {
                    let _ = window.paint_image(
                        fitted,
                        fitted,
                        Corners::default(),
                        translated.clone(),
                        0,
                        false,
                    );
                });

                // Slider in the GTK style: a black 4.5 px line under a 2 px
                // white one, a 12 px white handle with chevrons, and the
                // position in percent under the handle.
                let top = fitted.origin.y;
                let height = fitted.size.height;
                let cy = px(top.as_f32() + height.as_f32() / 2.0);

                window.paint_quad(fill(
                    Bounds::new(point(split_x - px(2.25), top), size(px(4.5), height)),
                    gpui::rgba(0x00000099),
                ));
                window.paint_quad(fill(
                    Bounds::new(point(split_x - px(1.), top), size(px(2.), height)),
                    gpui::rgba(0xffffffe6),
                ));

                paint_disc(
                    window,
                    split_x,
                    cy,
                    13.0,
                    Hsla::from(gpui::rgba(0x00000099)),
                );
                paint_disc(window, split_x, cy, 12.0, Hsla::from(gpui::rgb(0xfafafa)));
                if let Ok(path) = handle_arrows(split_x, cy).build() {
                    window.paint_path(path, Hsla::from(gpui::rgb(0x333333)));
                }

                let percent = format!("{}%", (split * 100.0).round() as i32);
                canvas_text::paint_text(
                    window,
                    cx,
                    &percent,
                    point(split_x, px(cy.as_f32() + 21.0)),
                    px(9.),
                    Hsla::from(gpui::rgb(0x333333)),
                    true,
                );
            }

            // ---- drag handling -------------------------------------------
            if translated.is_none() {
                return;
            }

            let origin_x = f32::from(fitted.origin.x);
            let width = f32::from(fitted.size.width);

            let weak_down = weak.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _window, cx| {
                if phase != DispatchPhase::Bubble || !fitted.contains(&event.position) {
                    return;
                }
                if let Some(app) = weak_down.upgrade() {
                    app.update(cx, |app, cx| {
                        app.comparing_drag = true;
                        app.set_split_at_x(f32::from(event.position.x), origin_x, width);
                        cx.notify();
                    });
                }
            });

            let weak_move = weak.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let Some(app) = weak_move.upgrade() else {
                    return;
                };
                if !app.read(cx).comparing_drag {
                    return;
                }
                app.update(cx, |app, cx| {
                    app.set_split_at_x(f32::from(event.position.x), origin_x, width);
                    cx.notify();
                });
            });

            let weak_up = weak.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let Some(app) = weak_up.upgrade() else {
                    return;
                };
                if !app.read(cx).comparing_drag {
                    return;
                }
                app.update(cx, |app, cx| {
                    app.comparing_drag = false;
                    // The GTK slider overshoots by 1.5 % and snaps back after
                    // 80 ms, which is what makes the release feel damped.
                    let final_pos = app.split;
                    app.split = bounce_target(final_pos);
                    cx.notify();

                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(80))
                            .await;
                        let _ = this.update(cx, |app, cx| {
                            app.split = final_pos;
                            cx.notify();
                        });
                    })
                    .detach();
                });
            });
        },
    )
    .size_full()
}

/// Largest rectangle with the image's aspect ratio that fits into `bounds`.
fn fitted_rect(bounds: Bounds<Pixels>, image: &RenderImage) -> Option<Bounds<Pixels>> {
    let natural = image.size(0);
    let image_width = natural.width.0 as f32;
    let image_height = natural.height.0 as f32;
    let available_width = f32::from(bounds.size.width);
    let available_height = f32::from(bounds.size.height);

    if image_width <= 0.0
        || image_height <= 0.0
        || available_width <= 0.0
        || available_height <= 0.0
    {
        return None;
    }

    let scale = (available_width / image_width).min(available_height / image_height);
    let width = px(image_width * scale);
    let height = px(image_height * scale);
    let x = px(f32::from(bounds.origin.x) + (available_width - f32::from(width)) / 2.0);
    let y = px(f32::from(bounds.origin.y) + (available_height - f32::from(height)) / 2.0);

    Some(Bounds::new(point(x, y), size(width, height)))
}

// ---------------------------------------------------------------------------
// Comparison slider chrome
// ---------------------------------------------------------------------------

/// Filled circle for the slider handle.
fn paint_disc(window: &mut Window, cx: Pixels, cy: Pixels, radius: f32, color: Hsla) {
    let mut builder = PathBuilder::fill();
    let mut points = Vec::with_capacity(32);
    for i in 0..32 {
        let a = i as f32 / 32.0 * std::f32::consts::TAU;
        points.push(point(
            px(cx.as_f32() + radius * a.cos()),
            px(cy.as_f32() + radius * a.sin()),
        ));
    }
    builder.add_polygon(&points, true);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// The two chevrons on the handle, as one stroked path.
fn handle_arrows(cx: Pixels, cy: Pixels) -> PathBuilder {
    let mut builder = PathBuilder::stroke(px(2.0));
    let x = cx.as_f32();
    let y = cy.as_f32();

    builder.move_to(point(px(x - 5.0), px(y)));
    builder.line_to(point(px(x - 9.0), px(y - 4.0)));
    builder.move_to(point(px(x - 5.0), px(y)));
    builder.line_to(point(px(x - 9.0), px(y + 4.0)));

    builder.move_to(point(px(x + 5.0), px(y)));
    builder.line_to(point(px(x + 9.0), px(y - 4.0)));
    builder.move_to(point(px(x + 5.0), px(y)));
    builder.line_to(point(px(x + 9.0), px(y + 4.0)));

    builder
}

/// The GTK slider overshoots by 1.5 % on release and snaps back after 80 ms,
/// which is what makes letting go feel damped.
fn bounce_target(pos: f32) -> f32 {
    if pos > 0.5 {
        (pos + 0.015).min(0.98)
    } else {
        (pos - 0.015).max(0.02)
    }
}

// ---------------------------------------------------------------------------
// Toolbar segment button
// ---------------------------------------------------------------------------

fn segment_button(
    theme: Theme,
    mode: PreviewMode,
    label: String,
    active: bool,
    dimmed: bool,
    on_click: impl Fn(&ClickEvent, &mut gpui::Window, &mut App) + 'static,
) -> AnyElement {
    let (bg, fg) = if active {
        (theme.accent, theme.accent_fg)
    } else {
        (theme.panel_alt, theme.text_muted)
    };

    div()
        .id(match mode {
            PreviewMode::Original => "tab-original",
            PreviewMode::Translated => "tab-translated",
            PreviewMode::Compare => "tab-compare",
        })
        .flex()
        .items_center()
        .justify_center()
        .h(px(24.))
        .px_3()
        .rounded_md()
        .bg(bg)
        .text_size(px(FONT_SM))
        .text_color(fg)
        .cursor_pointer()
        .hover(move |style| style.opacity(0.85))
        .on_click(on_click)
        .when(dimmed, |this| this.opacity(0.5))
        .child(label)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::bounce_target;

    #[test]
    fn bounce_overshoots_then_stays_inside_the_range() {
        // Right of the middle it overshoots to the right, left of it to the left.
        assert!(bounce_target(0.70) > 0.70);
        assert!(bounce_target(0.30) < 0.30);
        // Never past the clamp the drag uses.
        assert!(bounce_target(0.98) <= 0.98);
        assert!(bounce_target(0.02) >= 0.02);
        assert!((bounce_target(0.5) - 0.485).abs() < 1e-6);
    }
}
