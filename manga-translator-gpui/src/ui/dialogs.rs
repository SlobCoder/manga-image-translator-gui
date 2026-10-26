// manga-translator-gpui/src/ui/dialogs.rs
//
// Modal dialogs: About, keyboard shortcuts and the live log viewer.
//
// They are opened through `WindowExt::open_dialog`, which renders into the
// dialog layer of the base `Root` that `gpui::open_window` installs.

use std::path::PathBuf;
use std::time::Duration;

use gpui::component::button::Button;
use gpui::component::{ActiveTheme as _, Sizable as _, WindowExt as _, switch::Switch, tag::Tag};
use gpui::{
    App, Context, IntoElement, ParentElement as _, Render, ScrollHandle, SharedString, Styled as _,
    Window, div, prelude::*, px,
};

use manga_translator_core::config::ConfigManager;
use manga_translator_core::i18n::t;

use crate::theme::{Theme, sizes::*};

/// Lines of log kept in memory; older ones are dropped.
const LOG_TAIL_LINES: usize = 800;
/// How often the viewer re-reads the file while "Auto" is on.
const LOG_POLL: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// About
// ---------------------------------------------------------------------------

pub(crate) fn open_about(theme: Theme, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(t("Über Manga Translator"))
            .w(px(460.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(FONT_LG))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Manga Translator"),
                    )
                    .child(caption(
                        t("Ein GUI für Manga Image Translator — geschrieben in Rust mit GPUI"),
                        theme.text_muted,
                    ))
                    .child(caption(
                        format!("{} {}", t("Version"), env!("CARGO_PKG_VERSION")),
                        theme.text_muted,
                    ))
                    .child(divider(theme))
                    .child(credit_line(
                        "Rust GUI",
                        "SlobCoder, Contributors",
                        theme.text_faint,
                    ))
                    .child(credit_line(
                        "Python Backend",
                        "zyddnys, Contributors",
                        theme.text_faint,
                    ))
                    .child(credit_line(
                        t("Basiert auf"),
                        "GPUI, gpui-component, gettext, MangaTranslator",
                        theme.text_faint,
                    ))
                    .child(divider(theme))
                    .child(caption(
                        "https://github.com/zyddnys/manga-image-translator",
                        theme.text_faint,
                    )),
            )
            .footer(
                Button::new("about-close")
                    .label(t("Schließen"))
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            )
    });
}

fn caption(text: impl Into<SharedString>, color: gpui::Rgba) -> impl IntoElement {
    div()
        .text_size(px(FONT_SM))
        .text_color(gpui::Hsla::from(color))
        .child(text.into())
}

fn credit_line(
    role: impl Into<SharedString>,
    names: impl Into<SharedString>,
    muted: gpui::Rgba,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .gap_2()
        .text_size(px(FONT_SM))
        .child(
            div()
                .w(px(110.))
                .text_color(gpui::Hsla::from(muted))
                .child(role.into()),
        )
        .child(div().flex_1().child(names.into()))
}

fn divider(theme: Theme) -> impl IntoElement {
    div().h(px(1.)).w_full().bg(theme.border)
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------

pub(crate) fn open_shortcuts(window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(t("Tastenkürzel"))
            .w(px(460.))
            .child(
                div()
                    .id("shortcuts-scroll")
                    .max_h(px(460.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(shortcut_group(
                        t("Allgemein"),
                        &[
                            ("Ctrl+R / F5", t("Aktualisieren")),
                            ("Alt+←", t("Zurück")),
                            ("Ctrl+T", t("Übersetzen")),
                            ("Escape", t("Übersetzung abbrechen")),
                            ("F9", t("Einstellungen ein-/ausblenden")),
                            ("Ctrl+L", t("Suche fokussieren")),
                            ("F1", t("Tastenkürzel anzeigen")),
                        ],
                    ))
                    .child(shortcut_group(
                        t("Dialoge"),
                        &[("Ctrl+Shift+L", t("Log anzeigen"))],
                    )),
            )
            .footer(
                Button::new("shortcuts-close")
                    .label(t("Schließen"))
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            )
    });
}

fn shortcut_group(title: String, rows: &[(&'static str, String)]) -> impl IntoElement {
    let mut group = div().flex().flex_col().gap(px(2.)).child(
        div()
            .mt_2()
            .mb_1()
            .text_size(px(FONT_SM))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child(title),
    );

    for (keys, description) in rows {
        group = group.child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .child(
                    div()
                        .w(px(120.))
                        .child(Tag::new().rounded_md().child(SharedString::from(*keys))),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(FONT_SM))
                        .child(description.clone()),
                ),
        );
    }

    group
}

// ---------------------------------------------------------------------------
// Log viewer
// ---------------------------------------------------------------------------

pub(crate) fn open_log(theme: Theme, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| LogView::new(theme, cx));

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(t("Log anzeigen"))
            .w(px(760.))
            .child(view.clone())
            .footer(
                Button::new("log-close")
                    .label(t("Schließen"))
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            )
    });
}

/// The log file, its tail and the controls around it.
pub struct LogView {
    path: PathBuf,
    lines: Vec<SharedString>,
    auto: bool,
    scroll: ScrollHandle,
    theme: Theme,
}

impl LogView {
    fn new(theme: Theme, cx: &mut Context<Self>) -> Self {
        let path = ConfigManager::log_file_path();
        let mut view = Self {
            path,
            lines: Vec::new(),
            auto: true,
            scroll: ScrollHandle::new(),
            theme,
        };
        view.reload(cx);

        // Poll while the view is alive; dropping the dialog drops the view,
        // and the failed upgrade ends the task.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(LOG_POLL).await;
                let alive = this
                    .update(cx, |this, cx| {
                        if this.auto {
                            this.reload(cx);
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();

        view
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let text = std::fs::read_to_string(&self.path).unwrap_or_default();
        let all: Vec<&str> = text.lines().collect();
        let start = all.len().saturating_sub(LOG_TAIL_LINES);
        let lines: Vec<SharedString> = all[start..]
            .iter()
            .map(|line| SharedString::from(*line))
            .collect();

        if lines == self.lines {
            return;
        }
        self.lines = lines;
        if self.auto {
            self.scroll.scroll_to_bottom();
        }
        cx.notify();
    }
}

impl Render for LogView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mono = cx.theme().mono_font_family.clone();
        let weak = cx.entity().downgrade();

        let mut body = div()
            .id("log-scroll")
            .flex()
            .flex_col()
            .h(px(460.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .font_family(mono)
            .text_size(px(11.));

        if self.lines.is_empty() {
            body = body.child(
                div()
                    .text_color(theme.text_faint)
                    .child(t("Die Log-Datei ist noch leer.")),
            );
        } else {
            for line in &self.lines {
                body = body.child(div().flex_none().child(line.clone()));
            }
        }

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(FONT_SM))
                            .text_color(theme.text_faint)
                            .child(self.path.display().to_string()),
                    )
                    .child(
                        Switch::new("log-auto")
                            .checked(self.auto)
                            .label(t("Auto"))
                            .on_change({
                                let weak = weak.clone();
                                move |checked, _window, cx| {
                                    if let Some(view) = weak.upgrade() {
                                        view.update(cx, |this, cx| {
                                            this.auto = *checked;
                                            if this.auto {
                                                this.scroll.scroll_to_bottom();
                                            }
                                            cx.notify();
                                        });
                                    }
                                }
                            }),
                    )
                    .child(
                        Button::new("log-refresh")
                            .label(t("Aktualisieren"))
                            .with_size(px(24.))
                            .on_click({
                                let weak = weak.clone();
                                move |_, _window, cx| {
                                    if let Some(view) = weak.upgrade() {
                                        view.update(cx, |this, cx| {
                                            this.lines.clear();
                                            this.reload(cx);
                                        });
                                    }
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.panel)
                    .p_2()
                    .child(body),
            )
    }
}
