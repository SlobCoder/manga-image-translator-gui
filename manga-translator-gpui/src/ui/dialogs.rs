// manga-translator-gpui/src/ui/dialogs.rs
//
// Modal dialogs: About, keyboard shortcuts and the live log viewer.
//
// They are opened through `WindowExt::open_dialog`, which renders into the
// dialog layer of the base `Root` that `gpui::open_window` installs.

use std::path::PathBuf;
use std::time::Duration;

use gpui::component::button::Button;
use gpui::component::input::{Input, InputState};
use gpui::component::scroll::ScrollableElement as _;
use gpui::component::{ActiveTheme as _, Sizable as _, WindowExt as _, switch::Switch, tag::Tag};
use gpui::{
    App, ClickEvent, Context, Div, ElementId, Entity, IntoElement, ParentElement as _, Render,
    ScrollHandle, SharedString, Styled as _, WeakEntity, Window, div, prelude::*, px,
};

use manga_translator_core::config::{ApiKeys, ConfigManager, accent_display_name, accent_presets};
use manga_translator_core::i18n::t;

use crate::app::MangaApp;
use crate::theme::{Theme, adw, parse_hex, sizes::*, typo};
use crate::ui::text_input::TextInput;
use crate::ui::widgets;

/// Lines of log kept in memory; older ones are dropped.
const LOG_TAIL_LINES: usize = 800;
/// How often the viewer re-reads the file while "Auto" is on.
const LOG_POLL: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// About
// ---------------------------------------------------------------------------

/// The icon the desktop entry ships, embedded so the About dialog shows the
/// same artwork as the installed application.
const APP_ICON: &[u8] = include_bytes!("../../../packaging/manga-translator-gtk.svg");

const APP_WEBSITE: &str = "https://github.com/zyddnys/manga-image-translator";

/// Mirrors `AdwAboutDialog`'s main page: a large centred icon, the application
/// name in `title-1`, the developer line, and the version as an accent pill,
/// followed by the credits as property rows in a card.
pub(crate) fn open_about(theme: Theme, window: &mut Window, cx: &mut App) {
    // `svg()` only rasterises an alpha mask, which collapses a coloured icon to
    // its silhouette. Rasterising the SVG ourselves keeps the artwork's colours.
    let icon = gpui::Image::from_bytes(gpui::ImageFormat::Svg, APP_ICON.to_vec())
        .to_image_data(cx.svg_renderer())
        .ok();

    window.open_dialog(cx, move |dialog, _window, _cx| {
        let credits = widgets::card(
            theme,
            vec![
                property_row(theme, "Rust GUI", "SlobCoder, Contributors"),
                property_row(theme, "Python Backend", "zyddnys, Contributors"),
                property_row(
                    theme,
                    t("Basiert auf"),
                    "GPUI, gpui-component, gettext, MangaTranslator",
                ),
                property_row(theme, t("Lizenz"), "MIT"),
                website_row(theme),
            ],
        );

        dialog
            // Adwaita titles the dialog "About"; the application name is the
            // heading in the body, so repeating it here would say it twice.
            .title(t("Über"))
            .w(px(360.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.))
                    .child(match icon.clone() {
                        Some(icon) => gpui::img(gpui::ImageSource::Render(icon))
                            .flex_none()
                            .w(px(128.))
                            .h(px(128.))
                            .into_any_element(),
                        None => div().w(px(128.)).h(px(128.)).into_any_element(),
                    })
                    .child(
                        div()
                            .text_size(px(typo::TITLE_1))
                            .font_weight(gpui::FontWeight::EXTRA_BOLD)
                            .text_color(gpui::Hsla::from(theme.text))
                            .text_center()
                            .child("Manga Translator"),
                    )
                    .child(about_line(
                        t("Ein GUI für Manga Image Translator — geschrieben in Rust mit GPUI"),
                        theme.text_muted,
                    ))
                    .child(about_line("Manga Translator Team", theme.text_muted))
                    .child(version_pill(theme))
                    .child(div().w_full().mt(px(18.)).mb(px(6.)).child(credits)),
            )
            .footer(
                Button::new("about-close")
                    .label(t("Schließen"))
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            )
    });
}

/// Centred line under the app name — body size, not a caption.
fn about_line(text: impl Into<SharedString>, color: gpui::Rgba) -> impl IntoElement {
    div()
        .text_size(px(FONT_BASE))
        .text_color(gpui::Hsla::from(color))
        .text_center()
        .child(text.into())
}

/// `AdwAboutDialog`'s version button: accent-coloured text on a tinted pill.
fn version_pill(theme: Theme) -> impl IntoElement {
    div()
        .mt(px(3.))
        .px(px(18.))
        .py(px(3.))
        .rounded(px(999.))
        .bg(gpui::Hsla::from(theme.accent).opacity(0.14))
        .text_size(px(FONT_BASE))
        .text_color(gpui::Hsla::from(theme.accent))
        .child(format!("{} {}", t("Version"), env!("CARGO_PKG_VERSION")))
}

/// One `row.property`: a dimmed caption label on the left, the value beside it.
fn property_row(theme: Theme, label: impl Into<SharedString>, value: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap_3()
        .px_3()
        .py_2()
        .min_h(px(adw::ACTION_ROW_H))
        .child(
            div()
                .w(px(110.))
                .flex_none()
                .text_size(px(FONT_SM))
                .text_color(gpui::Hsla::from(theme.text_faint))
                .child(label.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_size(px(FONT_BASE))
                .text_color(gpui::Hsla::from(theme.text))
                .child(value),
        )
}

fn website_row(theme: Theme) -> Div {
    property_row(
        theme,
        t("Website"),
        div()
            .id("about-website")
            .text_color(gpui::Hsla::from(theme.accent))
            .cursor_pointer()
            .on_click(|_, _, cx| cx.open_url(APP_WEBSITE))
            .child(APP_WEBSITE),
    )
}

fn caption(text: impl Into<SharedString>, color: gpui::Rgba) -> impl IntoElement {
    div()
        .text_size(px(FONT_SM))
        .text_color(gpui::Hsla::from(color))
        .child(text.into())
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
                            ("Ctrl+O", t("Verzeichnis öffnen")),
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
                        &[
                            ("Ctrl+K", t("API-Schlüssel bearbeiten")),
                            ("Ctrl+Shift+L", t("Log anzeigen")),
                        ],
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
                            .with_size(px(adw::BUTTON_H))
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

// ---------------------------------------------------------------------------
// API keys
// ---------------------------------------------------------------------------

/// The keys in the same order and grouping the GTK dialog uses:
/// `(label, config field, belongs to the secondary group)`.
const API_KEY_FIELDS: &[(&str, &str, bool)] = &[
    ("DeepL", "deepl", false),
    ("OpenAI", "openai", false),
    ("Gemini", "gemini", false),
    ("DeepSeek", "deepseek", false),
    ("Groq", "groq", false),
    ("OpenRouter", "openrouter", true),
    ("Baidu App ID", "baidu_app_id", true),
    ("Baidu Secret Key", "baidu_secret_key", true),
    ("Caiyun Token", "caiyun_token", true),
];

pub(crate) fn open_api_keys(
    theme: Theme,
    keys: ApiKeys,
    app: WeakEntity<MangaApp>,
    window: &mut Window,
    cx: &mut App,
) {
    let dialog = cx.new(|cx| ApiKeysDialog::new(theme, keys, app, window, cx));

    window.open_dialog(cx, move |shell, _window, _cx| {
        let save = dialog.clone();
        let confirm = dialog.clone();
        shell
            .title(t("API Schlüssel"))
            .w(px(480.))
            .child(dialog.clone())
            // Enter is the dialog's default action; without this the shell
            // would close and drop the edits.
            .on_ok(move |_, window, cx| {
                confirm.update(cx, |this, cx| this.save_all(window, cx));
                true
            })
            .footer(div().flex().flex_row().justify_end().child(
                widgets::primary_button("api-keys-save", t("Speichern")).on_click(
                    move |_, window, cx| {
                        save.update(cx, |this, cx| this.save_all(window, cx));
                        window.close_dialog(cx);
                    },
                ),
            ))
    });
}

/// All nine keys as masked fields. Every edit goes straight into the shared
/// config; the row button and the footer button only decide when it is
/// written to disk, like the apply button and header button in GTK.
struct ApiKeysDialog {
    app: WeakEntity<MangaApp>,
    fields: Vec<(&'static str, Entity<TextInput>)>,
    fallback: Theme,
}

impl ApiKeysDialog {
    fn new(
        theme: Theme,
        keys: ApiKeys,
        app: WeakEntity<MangaApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let fields = API_KEY_FIELDS
            .iter()
            .map(|(label, field, _)| {
                let value = keys.get_field(field).to_string();
                let placeholder = (*label).to_string();
                let input = cx.new(|cx| TextInput::new(cx, window, value, placeholder, true));
                (*field, input)
            })
            .collect();

        Self {
            app,
            fields,
            fallback: theme,
        }
    }

    fn theme(&self, cx: &App) -> Theme {
        self.app
            .upgrade()
            .map(|app| app.read(cx).theme)
            .unwrap_or(self.fallback)
    }

    /// Write one row without closing, so a single key can be saved in place.
    fn save_one(&mut self, field: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self
            .fields
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, input)| input.clone())
        else {
            return;
        };
        let value = input.read(cx).value(cx);
        self.commit(&[(field, value)], window, cx);
    }

    fn save_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let values: Vec<(&'static str, String)> = self
            .fields
            .iter()
            .map(|(field, input)| (*field, input.read(cx).value(cx)))
            .collect();

        self.commit(&values, window, cx);
    }

    fn commit(
        &mut self,
        values: &[(&'static str, String)],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.update(cx, |app, cx| {
            for (field, value) in values {
                app.config.api_keys.set_field(field, value);
            }
            app.config.save_api_keys();
            // The settings panel keeps inline fields for the two VLM keys.
            app.sync_inputs(window, cx);
            cx.notify();
        });
    }
}

/// Green tick when the field holds something, amber warning when it is empty.
fn status_mark(theme: Theme, has_key: bool) -> Div {
    div()
        .w(px(16.))
        .text_center()
        .text_size(px(FONT_BASE))
        .text_color(if has_key {
            theme.success
        } else {
            theme.warning
        })
        .child(if has_key { "✓" } else { "⚠" })
}

impl Render for ApiKeysDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme(cx);
        let mut primary = div().flex().flex_col().gap_2();
        let mut secondary = div().flex().flex_col().gap_2();

        for (label, field, is_secondary) in API_KEY_FIELDS {
            let field = *field;
            let Some(input) = self
                .fields
                .iter()
                .find(|(f, _)| *f == field)
                .map(|(_, input)| input.clone())
            else {
                continue;
            };
            let has_key = !input.read(cx).value(cx).trim().is_empty();

            let row = div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(FONT_SM))
                        .text_color(theme.text_muted)
                        .child(*label),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().child(input))
                        .child(status_mark(theme, has_key))
                        .child(
                            widgets::button(
                                ElementId::Name(format!("api-apply-{field}").into()),
                                t("Übernehmen"),
                            )
                            .on_click(cx.listener(
                                move |this, _: &ClickEvent, window, cx| {
                                    this.save_one(field, window, cx);
                                },
                            )),
                        ),
                );

            if *is_secondary {
                secondary = secondary.child(row);
            } else {
                primary = primary.child(row);
            }
        }

        div()
            .id("api-keys-scroll")
            .max_h(px(610.))
            .overflow_y_scrollbar()
            .flex()
            .flex_col()
            .gap_1()
            .child(widgets::section_header(theme, t("API Schlüssel")))
            .child(caption(
                t("Schlüssel für kostenpflichtige Übersetzungsdienste"),
                theme.text_faint,
            ))
            .child(primary)
            .child(widgets::section_header(theme, t("Weitere Dienste")))
            .child(secondary)
            .child(divider(theme))
            .child(caption(
                t("Hinweis: Die Schlüssel werden lokal gespeichert."),
                theme.text_faint,
            ))
    }
}

// ---------------------------------------------------------------------------
// Accent colour
// ---------------------------------------------------------------------------

pub(crate) fn open_accent(
    theme: Theme,
    app: WeakEntity<MangaApp>,
    window: &mut Window,
    cx: &mut App,
) {
    let picker = cx.new(|cx| AccentPicker::new(theme, app, window, cx));

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(t("Akzentfarbe"))
            .w(px(440.))
            .child(picker.clone())
            .footer(
                Button::new("accent-done")
                    .label(t("Fertig"))
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            )
    });
}

/// Accept `#rrggbb` and the bare `rrggbb` people paste from a colour picker.
fn normalize_hex(value: &str) -> Option<String> {
    let value = value.trim();
    let candidate = if value.starts_with('#') {
        value.to_string()
    } else {
        format!("#{value}")
    };
    parse_hex(&candidate).map(|_| candidate)
}

/// Preset swatches plus a free `#rrggbb` field. Every choice is applied
/// immediately, so the whole window previews it.
struct AccentPicker {
    selected: String,
    app: WeakEntity<MangaApp>,
    hex: Entity<InputState>,
    fallback: Theme,
}

impl AccentPicker {
    fn new(
        fallback: Theme,
        app: WeakEntity<MangaApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let selected = app
            .upgrade()
            .map(|app| app.read(cx).config.settings.accent_color.clone())
            .unwrap_or_default();
        let hex = cx.new(|cx| InputState::new(window, cx).placeholder("#3584e4"));

        Self {
            selected,
            app,
            hex,
            fallback,
        }
    }

    fn apply(&mut self, value: String, cx: &mut Context<Self>) {
        self.selected = value.clone();
        if let Some(app) = self.app.upgrade() {
            app.update(cx, |app, cx| app.set_accent(value, cx));
        }
        cx.notify();
    }

    fn theme(&self, cx: &App) -> Theme {
        self.app
            .upgrade()
            .map(|app| app.read(cx).theme)
            .unwrap_or(self.fallback)
    }
}

impl Render for AccentPicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme(cx);
        // "System" previews the accent the scheme would have without an override.
        let builtin_accent = if theme.dark {
            Theme::dark().accent
        } else {
            Theme::light().accent
        };
        let mut grid = div().flex().flex_col().gap_2();

        for chunk in accent_presets().chunks(3) {
            let mut row = div().flex().flex_row().gap_2();

            for preset in chunk {
                let name = preset.name.clone();
                let label = accent_display_name(&name);
                let color = if preset.hex.is_empty() {
                    builtin_accent
                } else {
                    parse_hex(&preset.hex).unwrap_or(theme.accent)
                };
                let selected = self.selected == name;
                let id = ElementId::Name(format!("accent-{name}").into());

                row = row.child(
                    div()
                        .id(id)
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                            this.apply(name.clone(), cx);
                        }))
                        .child(
                            div()
                                .w(px(64.))
                                .h(px(36.))
                                .rounded(px(adw::CARD_RADIUS))
                                .bg(color)
                                .border_2()
                                .border_color(if selected { theme.text } else { theme.border }),
                        )
                        .child(
                            div()
                                .text_size(px(FONT_SM))
                                .text_color(if selected {
                                    theme.text
                                } else {
                                    theme.text_muted
                                })
                                .child(label),
                        ),
                );
            }

            grid = grid.child(row);
        }

        let weak = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(grid)
            .child(divider(theme))
            .child(caption(t("Eigene Farbe als #rrggbb"), theme.text_muted))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.hex).h(px(adw::ENTRY_H))),
                    )
                    .child(
                        Button::new("accent-hex-apply")
                            .label(t("Anwenden"))
                            .with_size(px(adw::BUTTON_H))
                            .on_click(move |_, _window, cx| {
                                if let Some(picker) = weak.upgrade() {
                                    picker.update(cx, |this, cx| {
                                        let value = this.hex.read(cx).value().to_string();
                                        if let Some(hex) = normalize_hex(&value) {
                                            this.apply(hex, cx);
                                        }
                                    });
                                }
                            }),
                    ),
            )
    }
}
