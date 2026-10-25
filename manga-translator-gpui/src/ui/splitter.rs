// manga-translator-gpui/src/ui/splitter.rs
//
// Draggable vertical divider between the three panels.
//
// Uses window-level mouse listeners (like the comparison slider) so the drag
// keeps working when the pointer moves faster than the re-layout, i.e. when it
// briefly leaves the 6px hit area. A double click restores the default width.

use gpui::{
    Bounds, DispatchPhase, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    WeakEntity, canvas, div, fill, point, prelude::*, px, size,
};

use crate::app::{MangaApp, SplitterKind};
use crate::theme::{Theme, sizes::*};

pub(crate) fn splitter(
    theme: Theme,
    weak: WeakEntity<MangaApp>,
    kind: SplitterKind,
) -> impl IntoElement {
    let idle: Hsla = theme.border.into();
    let active: Hsla = theme.accent.into();
    let track: Hsla = theme.panel_alt.into();

    div()
        .w(px(SPLITTER_W))
        .h_full()
        .cursor(gpui::CursorStyle::ResizeColumn)
        .child(
            canvas(
                move |bounds, _window, _cx| bounds,
                move |bounds, _, window, cx| {
                    window.paint_quad(fill(bounds, track));

                    let line_x =
                        px(f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.0);
                    let dragging = weak
                        .upgrade()
                        .map(|app| app.read(cx).is_dragging_splitter(kind))
                        .unwrap_or(false);
                    window.paint_quad(fill(
                        Bounds::new(
                            point(line_x, bounds.origin.y),
                            size(px(1.), bounds.size.height),
                        ),
                        if dragging { active } else { idle },
                    ));

                    let viewport_width = f32::from(window.viewport_size().width);

                    let weak_down = weak.clone();
                    window.on_mouse_event(move |event: &MouseDownEvent, phase, _window, cx| {
                        if phase != DispatchPhase::Bubble
                            || event.button != MouseButton::Left
                            || !bounds.contains(&event.position)
                        {
                            return;
                        }
                        if let Some(app) = weak_down.upgrade() {
                            app.update(cx, |app, cx| {
                                app.begin_split_drag(
                                    kind,
                                    f32::from(event.position.x),
                                    event.click_count >= 2,
                                    cx,
                                );
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
                        if !app.read(cx).is_dragging_splitter(kind) {
                            return;
                        }
                        app.update(cx, |app, cx| {
                            app.update_split_drag(f32::from(event.position.x), viewport_width, cx);
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
                        if !app.read(cx).is_dragging_splitter(kind) {
                            return;
                        }
                        app.update(cx, |app, cx| app.end_split_drag(cx));
                    });
                },
            )
            .w_full()
            .h_full(),
        )
}
