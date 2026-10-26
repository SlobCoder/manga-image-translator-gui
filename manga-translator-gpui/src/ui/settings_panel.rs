// manga-translator-gpui/src/ui/settings_panel.rs
//
// Right-hand settings panel. Every control writes straight into
// `ConfigManager::settings` and persists immediately, so the two frontends
// (GTK and GPUI) always share the same configuration file.

use gpui::component::select::{Select, SelectEvent};
use gpui::component::{IndexPath, select::SelectState};
use gpui::{ClickEvent, Context, Div, Entity, FontWeight, Window, prelude::*, px};

use manga_translator_core::config::{Settings, options};
use manga_translator_core::i18n::{SUPPORTED_LANGUAGES, t};

use crate::app::MangaApp;
use crate::theme::{Theme, sizes::*};
use crate::ui::text_input::TextInput;
use crate::ui::widgets;

// ---------------------------------------------------------------------------
// Select definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectKind {
    TranslationMode,
    Translator,
    TargetLanguage,
    Detector,
    Ocr,
    Inpainter,
    Upscaler,
    Device,
    Sort,
    Renderer,
    Alignment,
    Direction,
    InpaintingPrecision,
    VlmType,
    GeminiModel,
    OpenRouterModel,
    UiLanguage,
    ColorScheme,
}

impl SelectKind {
    /// Every dropdown, in panel order.
    pub const ALL: [SelectKind; 18] = [
        SelectKind::TranslationMode,
        SelectKind::Translator,
        SelectKind::TargetLanguage,
        SelectKind::Detector,
        SelectKind::Ocr,
        SelectKind::Inpainter,
        SelectKind::Upscaler,
        SelectKind::Device,
        SelectKind::Sort,
        SelectKind::Renderer,
        SelectKind::Alignment,
        SelectKind::Direction,
        SelectKind::InpaintingPrecision,
        SelectKind::VlmType,
        SelectKind::GeminiModel,
        SelectKind::OpenRouterModel,
        SelectKind::UiLanguage,
        SelectKind::ColorScheme,
    ];

    fn label(self) -> String {
        t(match self {
            SelectKind::TranslationMode => "Übersetzungsmodus",
            SelectKind::Translator => "Übersetzer",
            SelectKind::TargetLanguage => "Zielsprache",
            SelectKind::Detector => "Texterkennung (Detector)",
            SelectKind::Ocr => "OCR-Modell",
            SelectKind::Inpainter => "Inpainting",
            SelectKind::Upscaler => "Upscaler",
            SelectKind::Device => "Gerät",
            SelectKind::Sort => "Sortierung",
            SelectKind::Renderer => "Renderer",
            SelectKind::Alignment => "Textausrichtung",
            SelectKind::Direction => "Textrichtung",
            SelectKind::InpaintingPrecision => "Inpainting-Präzision",
            SelectKind::VlmType => "VLM-Anbieter",
            SelectKind::GeminiModel => "Gemini-Modell",
            SelectKind::OpenRouterModel => "OpenRouter-Modell",
            SelectKind::UiLanguage => "Sprache der Oberfläche",
            SelectKind::ColorScheme => "Farbschema",
        })
    }

    fn options(self, settings: &Settings) -> Vec<String> {
        let static_list =
            |list: &'static [&'static str]| -> Vec<String> { list.iter().map(|s| t(s)).collect() };
        match self {
            SelectKind::TranslationMode => static_list(options::TRANSLATION_MODES),
            SelectKind::Translator => static_list(options::TRANSLATORS),
            SelectKind::TargetLanguage => static_list(options::TARGET_LANGUAGES),
            SelectKind::Detector => static_list(options::DETECTORS),
            SelectKind::Ocr => static_list(options::OCR_MODELS),
            SelectKind::Inpainter => static_list(options::INPAINTERS),
            SelectKind::Upscaler => static_list(options::UPSCALERS),
            SelectKind::Device => static_list(options::DEVICES),
            SelectKind::Sort => static_list(options::SORT_METHODS),
            SelectKind::Renderer => static_list(options::RENDERERS),
            SelectKind::Alignment => static_list(options::ALIGNMENTS),
            SelectKind::Direction => static_list(options::DIRECTIONS),
            SelectKind::InpaintingPrecision => static_list(options::INPAINTING_PRECISIONS),
            SelectKind::VlmType => static_list(options::VLM_TYPES),
            SelectKind::GeminiModel => static_list(options::GEMINI_MODELS),
            SelectKind::OpenRouterModel => {
                if settings.openrouter_model_cache.is_empty() {
                    vec![t("(Liste wird beim Start geladen)")]
                } else {
                    settings.openrouter_model_cache.clone()
                }
            }
            SelectKind::UiLanguage => SUPPORTED_LANGUAGES
                .iter()
                .map(|(_, name)| name.to_string())
                .collect(),
            SelectKind::ColorScheme => vec![t("System"), t("Hell"), t("Dunkel")],
        }
    }

    fn selected(self, settings: &Settings) -> usize {
        match self {
            SelectKind::TranslationMode => settings.translation_mode_index as usize,
            SelectKind::Translator => settings.translator_index as usize,
            SelectKind::TargetLanguage => options::TARGET_LANGUAGES
                .iter()
                .position(|l| *l == settings.target_language)
                .unwrap_or(0),
            SelectKind::Detector => settings.detector_index as usize,
            SelectKind::Ocr => settings.ocr_index as usize,
            SelectKind::Inpainter => settings.inpainter_index as usize,
            SelectKind::Upscaler => settings.upscaler_index as usize,
            SelectKind::Device => {
                if settings.device == "cpu" {
                    1
                } else {
                    0
                }
            }
            SelectKind::Sort => settings.sort_method as usize,
            SelectKind::Renderer => settings.renderer_index as usize,
            SelectKind::Alignment => settings.alignment_index as usize,
            SelectKind::Direction => settings.direction_index as usize,
            SelectKind::InpaintingPrecision => settings.inpainting_precision_index as usize,
            SelectKind::VlmType => settings.vlm_type_index as usize,
            SelectKind::GeminiModel => settings.gemini_model_index as usize,
            SelectKind::OpenRouterModel => settings
                .openrouter_model_cache
                .iter()
                .position(|m| *m == settings.openrouter_model)
                .unwrap_or(0),
            SelectKind::UiLanguage => SUPPORTED_LANGUAGES
                .iter()
                .position(|(code, _)| *code == settings.ui_language)
                .unwrap_or(0),
            SelectKind::ColorScheme => match settings.color_scheme.as_str() {
                "light" => 1,
                "dark" => 2,
                _ => 0,
            },
        }
    }

    fn apply(self, settings: &mut Settings, index: usize) -> Option<String> {
        let index = index as u32;
        match self {
            SelectKind::TranslationMode => {
                settings.translation_mode_index = index;
                settings.translation_mode = options::TRANSLATION_MODE_MAP
                    .get(index as usize)
                    .copied()
                    .unwrap_or("standard")
                    .to_string();
            }
            SelectKind::Translator => settings.translator_index = index,
            SelectKind::TargetLanguage => {
                if let Some(lang) = options::TARGET_LANGUAGES.get(index as usize) {
                    settings.target_language = lang.to_string();
                }
            }
            SelectKind::Detector => settings.detector_index = index,
            SelectKind::Ocr => settings.ocr_index = index,
            SelectKind::Inpainter => settings.inpainter_index = index,
            SelectKind::Upscaler => {
                settings.upscaler_index = index;
                let ratios = options::UPSCALER_RATIOS
                    .get(index as usize)
                    .copied()
                    .unwrap_or(&[0]);
                if !ratios.contains(&settings.upscale_ratio) {
                    settings.upscale_ratio = *ratios.last().unwrap_or(&0);
                }
            }
            SelectKind::Device => {
                settings.device = if index == 0 { "cuda" } else { "cpu" }.to_string();
            }
            SelectKind::Sort => settings.sort_method = index,
            SelectKind::Renderer => settings.renderer_index = index,
            SelectKind::Alignment => settings.alignment_index = index,
            SelectKind::Direction => settings.direction_index = index,
            SelectKind::InpaintingPrecision => settings.inpainting_precision_index = index,
            SelectKind::VlmType => {
                settings.vlm_type_index = index;
                settings.vlm_type = options::VLM_TYPE_MAP
                    .get(index as usize)
                    .copied()
                    .unwrap_or("gemini")
                    .to_string();
            }
            SelectKind::GeminiModel => {
                settings.gemini_model_index = index;
                if let Some(model) = options::GEMINI_MODELS.get(index as usize) {
                    settings.gemini_model = model.to_string();
                }
            }
            SelectKind::OpenRouterModel => {
                if let Some(model) = settings.openrouter_model_cache.get(index as usize) {
                    settings.openrouter_model = model.clone();
                }
            }
            SelectKind::UiLanguage => {
                let code = SUPPORTED_LANGUAGES
                    .get(index as usize)
                    .map(|(code, _)| code.to_string())
                    .unwrap_or_default();
                settings.ui_language = code.clone();
                return Some(code);
            }
            SelectKind::ColorScheme => {
                settings.color_scheme = match index {
                    1 => "light",
                    2 => "dark",
                    _ => "system",
                }
                .to_string();
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------

impl MangaApp {
    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let settings = &self.config.settings;

        let mut content = gpui::div()
            .flex()
            .flex_col()
            .p_3()
            .gap_1()
            .text_size(px(FONT_BASE));

        // ---- Translation -------------------------------------------------
        content = content
            .child(widgets::section_header(theme, t("Übersetzung")))
            .child(self.select_row(SelectKind::TranslationMode, cx))
            .child(self.select_row(SelectKind::Translator, cx))
            .child(self.select_row(SelectKind::TargetLanguage, cx))
            .child(self.toggle_row(
                t("RTL-Textrichtung"),
                settings.direction_rtl,
                "toggle-rtl",
                |s| s.direction_rtl = !s.direction_rtl,
                cx,
            ))
            .child(self.select_row(SelectKind::Device, cx));

        // ---- Detection ---------------------------------------------------
        content = content
            .child(widgets::section_header(theme, t("Umgebung")))
            .child(self.free_text_row("manga_translator_path", cx))
            .child(self.free_text_row("virtual_env_path", cx))
            .child(widgets::section_header(theme, t("Erkennung")))
            .child(self.select_row(SelectKind::Detector, cx))
            .child(self.select_row(SelectKind::Ocr, cx))
            .child(self.toggle_row(
                t("BBox Zusammenführung (mocr)"),
                settings.use_mocr_merge,
                "toggle-mocr",
                |s| s.use_mocr_merge = !s.use_mocr_merge,
                cx,
            ))
            .child(self.stepper_row(
                t("Erkennungsgröße"),
                settings.detection_size as i64,
                "step-detection",
                |s, delta| s.detection_size = step_u32(s.detection_size, delta, 256, 4096),
                cx,
            ));

        // ---- Inpainting --------------------------------------------------
        content = content
            .child(widgets::section_header(theme, t("Inpainting")))
            .child(self.select_row(SelectKind::Inpainter, cx))
            .child(self.select_row(SelectKind::InpaintingPrecision, cx))
            .child(self.stepper_row(
                t("Inpainting-Größe"),
                settings.inpainting_size as i64,
                "step-inpainting",
                |s, delta| s.inpainting_size = step_u32(s.inpainting_size, delta, 256, 4096),
                cx,
            ))
            .child(self.stepper_row(
                t("Masken-Dilatation"),
                settings.mask_dilation_offset as i64,
                "step-mask",
                |s, delta| s.mask_dilation_offset = step_i32(s.mask_dilation_offset, delta, 0, 100),
                cx,
            ))
            .child(self.stepper_row(
                t("Kernel-Größe"),
                settings.kernel_size as i64,
                "step-kernel",
                |s, delta| s.kernel_size = step_i32(s.kernel_size, delta, 1, 15),
                cx,
            ));

        // ---- VLM ---------------------------------------------------------
        content = content
            .child(widgets::section_header(theme, t("VLM")))
            .child(self.select_row(SelectKind::VlmType, cx));

        match settings.vlm_type.as_str() {
            "openrouter" => {
                content = content
                    .child(self.select_row(SelectKind::OpenRouterModel, cx))
                    .child(self.secret_row("openrouter_api_key", cx));
            }
            "local" => {
                content = content.child(self.free_text_row("local_model_path", cx));
            }
            _ => {
                content = content
                    .child(self.select_row(SelectKind::GeminiModel, cx))
                    .child(self.secret_row("gemini_api_key", cx));
            }
        }

        // ---- Rendering ---------------------------------------------------
        content = content
            .child(widgets::section_header(theme, t("Rendering")))
            .child(self.select_row(SelectKind::Renderer, cx))
            .child(self.select_row(SelectKind::Alignment, cx))
            .child(self.select_row(SelectKind::Direction, cx))
            .child(self.stepper_row(
                t("Schriftgröße (0 = automatisch)"),
                settings.font_size as i64,
                "step-font-size",
                |s, delta| s.font_size = step_i32(s.font_size, delta, 0, 200),
                cx,
            ))
            .child(self.stepper_row(
                t("Schriftgröße-Offset"),
                settings.font_size_offset as i64,
                "step-font-offset",
                |s, delta| s.font_size_offset = step_i32(s.font_size_offset, delta, -50, 50),
                cx,
            ))
            .child(self.stepper_row(
                t("Minimale Schriftgröße"),
                settings.font_size_minimum as i64,
                "step-font-min",
                |s, delta| s.font_size_minimum = step_i32(s.font_size_minimum, delta, -1, 200),
                cx,
            ))
            .child(self.stepper_row(
                t("Zeilenabstand (0 = automatisch)"),
                settings.line_spacing as i64,
                "step-line-spacing",
                |s, delta| s.line_spacing = step_i32(s.line_spacing, delta, 0, 50),
                cx,
            ))
            .child(self.free_text_row("font_color", cx))
            .child(self.toggle_row(
                t("Font-Rahmen deaktivieren"),
                settings.disable_font_border,
                "toggle-border",
                |s| s.disable_font_border = !s.disable_font_border,
                cx,
            ))
            .child(self.toggle_row(
                t("GROSSBUCHSTABEN"),
                settings.uppercase,
                "toggle-upper",
                |s| s.uppercase = !s.uppercase,
                cx,
            ))
            .child(self.toggle_row(
                t("kleinbuchstaben"),
                settings.lowercase,
                "toggle-lower",
                |s| s.lowercase = !s.lowercase,
                cx,
            ))
            .child(self.toggle_row(
                t("Silbentrennung aus"),
                settings.no_hyphenation,
                "toggle-hyphen",
                |s| s.no_hyphenation = !s.no_hyphenation,
                cx,
            ));

        // ---- Upscaling ---------------------------------------------------
        content = content
            .child(widgets::section_header(theme, t("Upscaling")))
            .child(self.select_row(SelectKind::Upscaler, cx))
            .child(self.stepper_row(
                t("Skalierungsfaktor"),
                settings.upscale_ratio as i64,
                "step-upscale",
                |s, delta| {
                    let ratios = options::UPSCALER_RATIOS
                        .get(s.upscaler_index as usize)
                        .copied()
                        .unwrap_or(&[0]);
                    let current = ratios
                        .iter()
                        .position(|r| *r == s.upscale_ratio)
                        .unwrap_or(0) as i64;
                    let next = (current + delta).clamp(0, ratios.len().saturating_sub(1) as i64);
                    s.upscale_ratio = ratios[next as usize];
                },
                cx,
            ));

        // ---- Output ------------------------------------------------------
        content = content
            .child(widgets::section_header(theme, t("Ausgabe")))
            .child(self.free_text_row("output_directory", cx))
            .child(self.toggle_row(
                t("Im Originalordner speichern"),
                settings.use_original_folder,
                "toggle-original-folder",
                |s| s.use_original_folder = !s.use_original_folder,
                cx,
            ));

        // ---- Appearance --------------------------------------------------
        content = content
            .child(widgets::section_header(theme, t("Darstellung")))
            .child(self.select_row(SelectKind::ColorScheme, cx))
            .child(self.select_row(SelectKind::UiLanguage, cx))
            .child(self.select_row(SelectKind::Sort, cx))
            .child(self.free_text_row("manga_home_directory", cx));

        gpui::div()
            .id("settings-panel")
            .w(px(self.settings_width))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.panel)
            .border_l_1()
            .border_color(theme.border)
            .child(
                gpui::div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .child(content),
            )
    }

    // -----------------------------------------------------------------------
    // Rows
    // -----------------------------------------------------------------------

    fn select_row(&self, kind: SelectKind, _cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let mut column = gpui::div()
            .flex()
            .flex_col()
            .gap_1()
            .py_1()
            .child(widgets::label_block(theme, kind.label(), None));

        if let Some(state) = self.selects.get(&kind) {
            column = column.child(Select::new(state).w_full());
        }

        column
    }

    fn toggle_row(
        &self,
        label: String,
        value: bool,
        id: &'static str,
        apply: fn(&mut Settings),
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = self.theme;
        gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_3()
            .py_1()
            .child(widgets::label_block(theme, label, None))
            .child(widgets::toggle(theme, id, value).on_click(cx.listener(
                move |this, _: &ClickEvent, _window, cx| {
                    apply(&mut this.config.settings);
                    this.config.save_settings();
                    cx.notify();
                },
            )))
    }

    fn stepper_row(
        &self,
        label: String,
        value: i64,
        id: &'static str,
        apply: fn(&mut Settings, i64),
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let dec_id = match id {
            "step-detection" => "dec-detection",
            "step-inpainting" => "dec-inpainting",
            "step-mask" => "dec-mask",
            "step-kernel" => "dec-kernel",
            "step-font-size" => "dec-font-size",
            "step-font-offset" => "dec-font-offset",
            "step-font-min" => "dec-font-min",
            "step-line-spacing" => "dec-line-spacing",
            "step-upscale" => "dec-upscale",
            _ => "dec-other",
        };

        let stepper = gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(26.))
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.panel_alt)
            .child(
                gpui::div()
                    .id(dec_id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(24.))
                    .h_full()
                    .text_color(theme.text_muted)
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.border))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        apply(&mut this.config.settings, -1);
                        this.config.save_settings();
                        cx.notify();
                    }))
                    .child("−"),
            )
            .child(
                gpui::div()
                    .min_w(px(48.))
                    .text_center()
                    .text_size(px(FONT_BASE))
                    .text_color(theme.text)
                    .child(format!("{}", value)),
            )
            .child(
                gpui::div()
                    .id(id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(24.))
                    .h_full()
                    .text_color(theme.text_muted)
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.border))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        apply(&mut this.config.settings, 1);
                        this.config.save_settings();
                        cx.notify();
                    }))
                    .child("+"),
            );

        gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_3()
            .py_1()
            .child(widgets::label_block(theme, label, None))
            .child(stepper)
    }

    fn free_text_row(&self, key: &'static str, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let label = self.input_label(key);
        let input: Option<Entity<TextInput>> = self.inputs.get(key).cloned();

        let mut row = gpui::div().flex().flex_col().gap_1().py_1().child(
            gpui::div()
                .text_size(px(FONT_SM))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_muted)
                .child(label),
        );

        if let Some(input) = input {
            let mut line = gpui::div().flex().flex_row().items_center().gap_1();
            line = line.child(gpui::div().flex_1().child(input));

            // Path settings get a picker next to the text field.
            if has_folder_picker(key) {
                line = line.child(
                    widgets::button(gpui::ElementId::Name(format!("pick-{key}").into()), "…")
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.pick_path_setting(key, window, cx);
                        })),
                );
            }

            row = row.child(line);
        }

        row
    }

    fn secret_row(&self, key: &'static str, cx: &mut Context<Self>) -> Div {
        self.free_text_row(key, cx)
    }

    fn input_label(&self, key: &str) -> String {
        match key {
            "gemini_api_key" => t("Gemini API-Schlüssel"),
            "openrouter_api_key" => t("OpenRouter API-Schlüssel"),
            "openrouter_model" => t("OpenRouter Modell (ID)"),
            "local_model_path" => t("Pfad zum lokalen Modell"),
            "output_directory" => t("Ausgabeordner"),
            "manga_home_directory" => t("Manga-Home-Ordner"),
            "font_color" => t("Schriftfarbe (#ffffff oder #fg:#bg)"),
            "manga_translator_path" => t("Pfad zu manga-image-translator"),
            "virtual_env_path" => t("Python-Umgebung (venv)"),
            other => other.to_string(),
        }
    }

    /// Create one component `Select` per setting and route its confirmations
    /// back into `apply_select`.
    pub(crate) fn build_selects(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        for kind in SelectKind::ALL {
            let options = kind.options(&self.config.settings);
            let selected = kind.selected(&self.config.settings);
            let state =
                cx.new(|cx| SelectState::new(options, Some(IndexPath::new(selected)), window, cx));

            cx.subscribe_in(
                &state,
                window,
                move |this, _state, event: &SelectEvent<Vec<String>>, window, cx| {
                    if let SelectEvent::Confirm(Some(value)) = event {
                        this.confirm_select(kind, value.clone(), window, cx);
                    }
                },
            )
            .detach();

            self.selects.insert(kind, state);
        }
    }

    fn confirm_select(
        &mut self,
        kind: SelectKind,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let options = kind.options(&self.config.settings);
        let Some(index) = options.iter().position(|option| *option == value) else {
            return;
        };
        self.apply_select(kind, index, window, cx);
        self.sync_select(kind, window, cx);
    }

    /// Rebuild one dropdown from the settings, keeping the selection.
    fn sync_select(&self, kind: SelectKind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.selects.get(&kind).cloned() else {
            return;
        };
        let options = kind.options(&self.config.settings);
        let selected = kind.selected(&self.config.settings);
        state.update(cx, |state, cx| {
            state.set_items(options, window, cx);
            state.set_selected_index(Some(IndexPath::new(selected)), window, cx);
        });
    }

    /// Rebuild every dropdown, e.g. after the UI language changed.
    pub(crate) fn refresh_selects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for kind in SelectKind::ALL {
            self.sync_select(kind, window, cx);
        }
    }

    fn apply_select(
        &mut self,
        kind: SelectKind,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let language = kind.apply(&mut self.config.settings, index);
        self.config.save_settings();

        if let Some(code) = language {
            manga_translator_core::i18n::set_language(&code);
            self.sync_inputs(window, cx);
            // The dropdown labels are translated, so they have to be rebuilt.
            cx.defer_in(window, |this, window, cx| this.refresh_selects(window, cx));
        }
        if kind == SelectKind::ColorScheme {
            self.theme = Theme::for_scheme(&self.config.settings.color_scheme);
            self.theme.apply_to_component_theme(cx);
            self.sync_inputs(window, cx);
        }
        if kind == SelectKind::Sort {
            let cwd = self.cwd.clone();
            self.load_folder(cwd, cx);
        }

        cx.notify();
    }
}

// ---------------------------------------------------------------------------
// Small numeric helpers
// ---------------------------------------------------------------------------

/// Settings that name a folder and therefore offer the native picker.
fn has_folder_picker(key: &str) -> bool {
    matches!(
        key,
        "manga_home_directory"
            | "output_directory"
            | "local_model_path"
            | "virtual_env_path"
            | "manga_translator_path"
    )
}

fn step_i32(value: i32, delta: i64, min: i32, max: i32) -> i32 {
    ((value as i64 + delta).clamp(min as i64, max as i64)) as i32
}

fn step_u32(value: u32, delta: i64, min: u32, max: u32) -> u32 {
    ((value as i64 + delta * 256).clamp(min as i64, max as i64)) as u32
}
