// manga-translator-gpui/src/ui/sidebar.rs
//
// Top toolbar, folder/image navigation and the bottom status bar.

use gpui::component::button::DropdownButton;
use gpui::component::checkbox::Checkbox;
use gpui::component::menu::PopupMenuItem;
use gpui::{
    ClickEvent, Context, Div, FontWeight, ObjectFit, SharedString, div, img, prelude::*, px,
    uniform_list,
};

use gpui::ImageSource;
use manga_translator_core::i18n::t;

use crate::app::{BackendState, MangaApp, format_size, shorten_path};
use crate::theme::{Theme, sizes::*};
use crate::ui::{dialogs, widgets};

impl MangaApp {
    // -----------------------------------------------------------------------
    // Header
    // -----------------------------------------------------------------------

    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let has_selection = self.selected_file_count() > 0;

        let mut actions = gpui::div().flex().flex_row().items_center().gap_2();

        actions = actions
            .child(
                widgets::button("btn-up", t("↑ Übergeordnet")).on_click(cx.listener(
                    |this, _: &ClickEvent, _window, cx| {
                        this.go_up(cx);
                    },
                )),
            )
            .child(
                widgets::button("btn-home", t("⌂ Manga")).on_click(cx.listener(
                    |this, _: &ClickEvent, _window, cx| {
                        this.go_home(cx);
                    },
                )),
            )
            .child(
                widgets::button("btn-reload", t("↻ Neu laden")).on_click(cx.listener(
                    |this, _: &ClickEvent, _window, cx| {
                        this.reload(cx);
                    },
                )),
            );

        if self.translating {
            actions = actions.child(widgets::button("btn-cancel", t("Abbrechen")).on_click(
                cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.cancel_translation(cx);
                }),
            ));
        } else {
            let mut translate = widgets::primary_button("btn-translate", t("Übersetzen"));
            if !has_selection {
                translate = translate.opacity(0.45);
            }
            actions = actions.child(translate.on_click(cx.listener(
                |this, _: &ClickEvent, _window, cx| {
                    this.start_translation(cx);
                },
            )));
        }

        actions = actions.child(menu_button(theme, cx.entity().downgrade()));

        div()
            .h(px(HEADER_H))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .bg(theme.panel)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(FONT_LG))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(t("Manga Translator")),
                    )
                    .child(
                        div()
                            .text_size(px(FONT_SM))
                            .text_color(theme.text_faint)
                            .child(shorten_path(&self.cwd)),
                    ),
            )
            .child(div().flex_1())
            .child(actions)
    }

    // -----------------------------------------------------------------------
    // Sidebar
    // -----------------------------------------------------------------------

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let visible = self.visible_images();

        let mut folders_section = div().flex().flex_col().gap(px(1.));
        if self.folders.is_empty() {
            folders_section = folders_section.child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(FONT_SM))
                    .text_color(theme.text_faint)
                    .child(t("Keine Unterordner")),
            );
        } else {
            let mut list = div()
                .id("folders-scroll")
                .flex()
                .flex_col()
                .gap(px(1.))
                .max_h(px(150.))
                .overflow_y_scroll();

            for (index, folder) in self.folders.iter().enumerate() {
                let path = folder.path.clone();
                let checked = self.is_folder_checked(&folder.path);
                list = list.child(
                    div()
                        .id(("folder", index))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(3.))
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme.panel_alt))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                            this.open_folder(path.clone(), cx);
                        }))
                        .child(
                            Checkbox::new(("folder-check", index))
                                .checked(checked)
                                .tab_stop(false)
                                .on_click(cx.listener(move |this, _: &bool, _window, cx| {
                                    cx.stop_propagation();
                                    this.toggle_folder_selection(index, cx);
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .flex_1()
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .text_color(theme.warning)
                                        .text_size(px(FONT_BASE))
                                        .child("▸"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_size(px(FONT_BASE))
                                        .text_color(theme.text_muted)
                                        .child(folder.name.clone()),
                                ),
                        ),
                );
            }
            folders_section = folders_section.child(list);
        }

        let selected = self.selected;

        let image_list = uniform_list(
            "image-list",
            visible.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                let mut rows = Vec::with_capacity(range.len());
                let current = this.visible_images();

                for flat in range {
                    let Some(&image_index) = current.get(flat) else {
                        continue;
                    };
                    let Some(image) = this.images.get(image_index) else {
                        continue;
                    };
                    let theme = this.theme;
                    let checked = this.is_file_checked(&image.path);
                    let previewed = selected == Some(image_index);
                    let thumb = this.thumbs.get(&image.path).cloned();

                    let preview: gpui::AnyElement = match thumb {
                        Some(thumb) => img(ImageSource::Image(thumb))
                            .w_full()
                            .h_full()
                            .object_fit(ObjectFit::Cover)
                            .into_any_element(),
                        None => div()
                            .w_full()
                            .h_full()
                            .bg(theme.panel_alt)
                            .child(
                                div()
                                    .text_size(px(FONT_SM))
                                    .text_color(theme.text_faint)
                                    .child("…"),
                            )
                            .into_any_element(),
                    };

                    rows.push(
                        div()
                            .id(("img", image_index))
                            .w_full()
                            .h(px(54.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .cursor_pointer()
                            .bg(if checked {
                                theme.selection
                            } else if previewed {
                                theme.panel_alt
                            } else {
                                theme.panel
                            })
                            .hover(move |style| style.bg(theme.panel_alt))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                this.toggle_image_selection(image_index, cx);
                            }))
                            .child(
                                Checkbox::new(("img-check", image_index))
                                    .checked(checked)
                                    .tab_stop(false),
                            )
                            .child(
                                div()
                                    .w(px(38.))
                                    .h(px(50.))
                                    .rounded_sm()
                                    .overflow_hidden()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(preview),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .gap(px(1.))
                                    .child(
                                        div()
                                            .text_size(px(FONT_BASE))
                                            .text_color(if checked || previewed {
                                                theme.text
                                            } else {
                                                theme.text_muted
                                            })
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .child(image.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(FONT_SM))
                                            .text_color(theme.text_faint)
                                            .child(format_size(image.size)),
                                    ),
                            ),
                    );
                }
                rows
            }),
        )
        .flex_1()
        .min_h(px(0.));

        div()
            .id("sidebar")
            .w(px(self.sidebar_width))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(widgets::section_header(theme, t("Ordner")))
                    .child(folders_section),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(widgets::section_header(theme, t("Seiten")))
                    .child(match &self.search_input {
                        Some(input) => input.clone().into_any_element(),
                        None => div().into_any_element(),
                    }),
            )
            .child(image_list)
    }

    // -----------------------------------------------------------------------
    // Status bar
    // -----------------------------------------------------------------------

    pub(crate) fn render_status_bar(&self, cx: &mut Context<Self>) -> Div {
        let _ = cx;
        let theme = self.theme;

        let (state_label, state_color) = match self.backend_state {
            BackendState::Ready => (t("Backend bereit"), theme.success),
            BackendState::Starting => (t("Backend startet…"), theme.warning),
            BackendState::Failed => (t("Backend-Fehler"), theme.danger),
            BackendState::Unknown => (t("Backend nicht gestartet"), theme.text_faint),
        };

        let checked = self.selected_file_count();
        let selection = match checked {
            0 => t("Keine Dateien ausgewählt"),
            1 => format!("1 {} {}", t("Datei"), t("ausgewählt")),
            n => format!("{} {} {}", n, t("Dateien"), t("ausgewählt")),
        };

        let percent = format!("{:.0}%", self.progress * 100.0);
        let pages = format!(
            "{} / {}",
            self.selected.map(|i| i + 1).unwrap_or(0),
            self.images.len()
        );

        div()
            .h(px(STATUS_H))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .bg(theme.panel)
            .border_t_1()
            .border_color(theme.border)
            .child(widgets::pill(state_label, state_color))
            .child(
                div()
                    .text_size(px(FONT_SM))
                    .text_color(theme.text_muted)
                    .child(selection),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(FONT_SM))
                    .text_color(theme.text_muted)
                    .child(SharedString::from(self.status.clone())),
            )
            .child(
                div()
                    .text_size(px(FONT_SM))
                    .text_color(theme.text_faint)
                    .child(pages),
            )
            .child(
                div()
                    .w(px(120.))
                    .child(widgets::progress_bar(self.progress)),
            )
            .child(
                div()
                    .text_size(px(FONT_SM))
                    .text_color(theme.text_faint)
                    .child(percent),
            )
    }
}

/// Overflow menu: help and the log viewer.
fn menu_button(theme: Theme, app: gpui::WeakEntity<MangaApp>) -> impl IntoElement {
    use gpui::component::button::Button;
    use gpui::component::Sizable as _;

    let app = app.clone();
    DropdownButton::new("btn-menu")
        .button(Button::new("btn-menu-main").label("☰").with_size(px(28.)))
        .dropdown_menu(move |menu, _window, _cx| {
            menu.item(
                PopupMenuItem::new(t("Tastenkürzel"))
                    .on_click(|_, window, cx| dialogs::open_shortcuts(window, cx)),
            )
            .item(
                PopupMenuItem::new(t("Log anzeigen"))
                    .on_click(move |_, window, cx| dialogs::open_log(theme, window, cx)),
            )
            .item(PopupMenuItem::separator())
            .item(PopupMenuItem::new(t("Akzentfarbe")).on_click({
                let app = app.clone();
                move |_, window, cx| dialogs::open_accent(theme, app.clone(), window, cx)
            }))
            .item(PopupMenuItem::new(t("Über Manga Translator"))
                    .on_click(move |_, window, cx| dialogs::open_about(theme, window, cx)),
            )
        })
}
