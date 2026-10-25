// manga-translator-gpui/src/ui/preview_panel.rs
//
// Centre pane: original page, translated page, or a comparison slider.
//
// The comparison view is a `canvas` element: both images are painted into the
// same fitted rectangle and the translated one is clipped with a content mask,
// so the split is pixel-exact and follows resizes without extra layout work.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, ContentMask, Context, Corners, DispatchPhase, Div,
    FontWeight, Image, ImageSource, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit,
    Pixels, RenderImage, WeakEntity, canvas, div, fill, img, point, prelude::*, px, size,
};

use manga_translator_core::i18n::t;

use crate::app::{MangaApp, PreviewMode};
use crate::theme::{Theme, sizes::*};
use crate::ui::widgets;

impl MangaApp {
    pub(crate) fn render_preview(&self, cx: &mut Context<Self>) -> Div {
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
            .child(
                widgets::button(theme, "btn-prev", "‹").on_click(cx.listener(
                    |this, _: &ClickEvent, _window, cx| {
                        this.select_next(-1, cx);
                    },
                )),
            )
            .child(
                widgets::button(theme, "btn-next", "›").on_click(cx.listener(
                    |this, _: &ClickEvent, _window, cx| {
                        this.select_next(1, cx);
                    },
                )),
            );

        for tab in tabs {
            toolbar = toolbar.child(tab);
        }

        // ---- body --------------------------------------------------------
        let body: Div = match self.preview_mode {
            PreviewMode::Original => picture_body(
                self.preview.clone(),
                theme,
                &self.status,
                translated_available,
                false,
            ),
            PreviewMode::Translated => picture_body(
                self.translated_preview.clone(),
                theme,
                &self.status,
                translated_available,
                true,
            ),
            PreviewMode::Compare => match self.preview.clone() {
                Some(original) => canvas_body(
                    theme,
                    self.split,
                    original,
                    self.translated_preview.clone(),
                    cx.entity().downgrade(),
                ),
                None => empty_state(theme, &self.status, translated_available, false),
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
    status: &str,
    translated_available: bool,
    showing_translated: bool,
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
        None => empty_state(theme, status, translated_available, showing_translated),
    }
}

fn empty_state(
    theme: Theme,
    status: &str,
    translated_available: bool,
    showing_translated: bool,
) -> Div {
    let message = if showing_translated && !translated_available {
        t("Noch keine Übersetzung – starte die Übersetzung über die Kopfzeile.")
    } else {
        t("Wähle links eine Seite aus.")
    };

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .w_full()
        .items_center()
        .justify_center()
        .gap_2()
        .child(div().text_size(px(42.)).text_color(theme.border).child("▤"))
        .child(
            div()
                .text_size(px(FONT_BASE))
                .text_color(theme.text_faint)
                .child(message),
        )
        .child(
            div()
                .text_size(px(FONT_SM))
                .text_color(theme.text_faint)
                .child(status.to_string()),
        )
}

// ---------------------------------------------------------------------------
// Comparison canvas
// ---------------------------------------------------------------------------

fn canvas_body(
    theme: Theme,
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
        .child(compare_canvas(theme, split, original, translated, weak));

    if has_translated {
        wrapper = wrapper.cursor(gpui::CursorStyle::ResizeColumn);
    }

    wrapper
}

fn compare_canvas(
    theme: Theme,
    split: f32,
    original: Arc<Image>,
    translated: Option<Arc<Image>>,
    weak: WeakEntity<MangaApp>,
) -> impl IntoElement {
    let line: gpui::Hsla = theme.accent_fg.into();
    let halo: gpui::Hsla = gpui::Hsla::from(theme.bg).opacity(0.55);
    let handle: gpui::Hsla = theme.accent.into();

    canvas(
        move |_bounds, window, cx| {
            let original = original.use_render_image(window, cx);
            let translated = translated.and_then(|image| image.use_render_image(window, cx));
            (original, translated)
        },
        move |bounds, (original, translated), window, _cx| {
            let Some(original) = original else {
                return;
            };
            let Some(fitted) = fitted_rect(bounds, &original) else {
                return;
            };

            // Base layer: the original page.
            let _ = window.paint_image(fitted, Corners::default(), original.clone(), 0, false);

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
                        Corners::default(),
                        translated.clone(),
                        0,
                        false,
                    );
                });

                // Divider: a dark halo with a light core so it stays visible on
                // both light and dark manga pages.
                let top = fitted.origin.y;
                let height = fitted.size.height;
                window.paint_quad(fill(
                    Bounds::new(point(split_x - px(1.5), top), size(px(3.), height)),
                    halo,
                ));
                window.paint_quad(fill(
                    Bounds::new(point(split_x - px(0.5), top), size(px(1.), height)),
                    line,
                ));

                let handle_bounds = Bounds::new(
                    point(
                        split_x - px(5.),
                        px(f32::from(top) + f32::from(height) / 2.0 - 22.0),
                    ),
                    size(px(10.), px(44.)),
                );
                window.paint_quad(fill(handle_bounds, handle).corner_radii(px(5.)));
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
                    cx.notify();
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
