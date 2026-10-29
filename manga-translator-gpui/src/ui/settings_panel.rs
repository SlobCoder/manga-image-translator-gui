// manga-translator-gpui/src/ui/settings_panel.rs
//
// Right-hand settings panel. Every control writes straight into
// `ConfigManager::settings` and persists immediately, so the two frontends
// (GTK and GPUI) always share the same configuration file.

use gpui::component::select::{Select, SelectEvent};
use gpui::component::{IndexPath, select::SelectState};
use gpui::{ClickEvent, Context, Div, Entity, Window, prelude::*, px};

use manga_translator_core::config::{Settings, options};
use manga_translator_core::i18n::{SUPPORTED_LANGUAGES, t};

use crate::app::MangaApp;
use crate::theme::{Theme, adw, sizes::*};
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

    /// Subtitle under the row title, taken from the GTK preferences panel.
    fn description(self) -> Option<&'static str> {
        Some(match self {
            SelectKind::TranslationMode => "Wähle den Übersetzungsmodus",
            SelectKind::Translator => "Wähle den Übersetzungsdienst",
            SelectKind::TargetLanguage => "Sprache der Übersetzung",
            SelectKind::Detector => "Methode zur Blasenerkennung",
            SelectKind::Ocr => "Optical Character Recognition",
            SelectKind::Inpainter => "Methode zur Bildreparatur",
            SelectKind::Upscaler => "Bildvergrößerung vor der Übersetzung",
            SelectKind::Device => "Recheneinheit für die Übersetzung",
            SelectKind::Renderer => "Methode zur Texteinblendung",
            SelectKind::Alignment => "Textausrichtung in Blasen",
            SelectKind::Direction => "Erzwinge horizontale/vertikale Richtung",
            SelectKind::InpaintingPrecision => "Berechnungsgenauigkeit",
            SelectKind::VlmType => "Wähle das VLM-Backend",
            SelectKind::GeminiModel => "Modell für VLM-Korrektur",
            SelectKind::OpenRouterModel => "Vision-fähiges Modell",
            SelectKind::UiLanguage => "Wirkt sofort",
            SelectKind::Sort | SelectKind::ColorScheme => return None,
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
            SelectKind::GeminiModel => {
                if settings.gemini_models.is_empty() {
                    static_list(options::GEMINI_MODELS)
                } else {
                    settings.gemini_models.clone()
                }
            }
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
            SelectKind::GeminiModel => settings
                .gemini_models
                .iter()
                .position(|model| *model == settings.gemini_model)
                .or_else(|| {
                    options::GEMINI_MODELS
                        .iter()
                        .position(|model| *model == settings.gemini_model)
                })
                .unwrap_or(settings.gemini_model_index as usize),
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
                let models = if settings.gemini_models.is_empty() {
                    options::GEMINI_MODELS
                        .iter()
                        .map(|model| model.to_string())
                        .collect::<Vec<_>>()
                } else {
                    settings.gemini_models.clone()
                };
                if let Some(model) = models.get(index as usize) {
                    settings.gemini_model = model.clone();
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

        // ---- Übersetzung -------------------------------------------------
        let translation = vec![
            self.select_row(SelectKind::TranslationMode),
            self.select_row(SelectKind::Translator),
            self.select_row(SelectKind::TargetLanguage),
            self.toggle_row(
                t("RTL-Textrichtung"),
                Some(t("Text von rechts nach links (Arabisch, Hebräisch)")),
                settings.direction_rtl,
                "toggle-rtl",
                |s| s.direction_rtl = !s.direction_rtl,
                cx,
            ),
            self.select_row(SelectKind::Device),
        ];

        // ---- Umgebung ----------------------------------------------------
        let environment = vec![
            self.free_text_row("manga_translator_path", cx),
            self.free_text_row("virtual_env_path", cx),
        ];

        // ---- Erkennung ---------------------------------------------------
        let detection = vec![
            self.select_row(SelectKind::Detector),
            self.select_row(SelectKind::Ocr),
            self.toggle_row(
                t("BBox Zusammenführung (mocr)"),
                Some(t("Erkennungsboxen zusammenführen (mocr)")),
                settings.use_mocr_merge,
                "toggle-mocr",
                |s| s.use_mocr_merge = !s.use_mocr_merge,
                cx,
            ),
            self.stepper_row(
                t("Erkennungsgröße"),
                Some(t("Auflösung für Texterkennung")),
                settings.detection_size as i64,
                "step-detection",
                |s, delta| s.detection_size = step_u32(s.detection_size, delta, 256, 4096),
                cx,
            ),
        ];

        // ---- Inpainting --------------------------------------------------
        let inpainting = vec![
            self.select_row(SelectKind::Inpainter),
            self.select_row(SelectKind::InpaintingPrecision),
            self.stepper_row(
                t("Inpainting-Größe"),
                Some(t("Auflösung für Bildreparatur")),
                settings.inpainting_size as i64,
                "step-inpainting",
                |s, delta| s.inpainting_size = step_u32(s.inpainting_size, delta, 256, 4096),
                cx,
            ),
            self.stepper_row(
                t("Masken-Dilatation"),
                Some(t("Textmaske erweitern um (px)")),
                settings.mask_dilation_offset as i64,
                "step-mask",
                |s, delta| s.mask_dilation_offset = step_i32(s.mask_dilation_offset, delta, 0, 100),
                cx,
            ),
            self.stepper_row(
                t("Kernel-Größe"),
                None,
                settings.kernel_size as i64,
                "step-kernel",
                |s, delta| s.kernel_size = step_i32(s.kernel_size, delta, 1, 15),
                cx,
            ),
        ];

        // ---- VLM ---------------------------------------------------------
        let mut vlm = vec![self.select_row(SelectKind::VlmType)];
        match settings.vlm_type.as_str() {
            "openrouter" => {
                vlm.push(self.select_row(SelectKind::OpenRouterModel));
                vlm.push(self.secret_row("openrouter_api_key", cx));
            }
            "local" => vlm.push(self.free_text_row("local_model_path", cx)),
            _ => {
                vlm.push(self.select_row(SelectKind::GeminiModel));
                vlm.push(self.secret_row("gemini_api_key", cx));
            }
        }

        // ---- Rendering ---------------------------------------------------
        let rendering = vec![
            self.select_row(SelectKind::Renderer),
            self.select_row(SelectKind::Alignment),
            self.select_row(SelectKind::Direction),
            self.stepper_row(
                t("Schriftgröße (0 = automatisch)"),
                None,
                settings.font_size as i64,
                "step-font-size",
                |s, delta| s.font_size = step_i32(s.font_size, delta, 0, 200),
                cx,
            ),
            self.stepper_row(
                t("Schriftgröße-Offset"),
                Some(t("Positive Werte = größerer Text")),
                settings.font_size_offset as i64,
                "step-font-offset",
                |s, delta| s.font_size_offset = step_i32(s.font_size_offset, delta, -50, 50),
                cx,
            ),
            self.stepper_row(
                t("Minimale Schriftgröße"),
                None,
                settings.font_size_minimum as i64,
                "step-font-min",
                |s, delta| s.font_size_minimum = step_i32(s.font_size_minimum, delta, -1, 200),
                cx,
            ),
            self.stepper_row(
                t("Zeilenabstand (0 = automatisch)"),
                None,
                settings.line_spacing as i64,
                "step-line-spacing",
                |s, delta| s.line_spacing = step_i32(s.line_spacing, delta, 0, 50),
                cx,
            ),
            self.free_text_row("font_color", cx),
            self.toggle_row(
                t("Font-Rahmen deaktivieren"),
                Some(t("Kein Rand/Umriss um den Text")),
                settings.disable_font_border,
                "toggle-border",
                |s| s.disable_font_border = !s.disable_font_border,
                cx,
            ),
            self.toggle_row(
                t("GROSSBUCHSTABEN"),
                None,
                settings.uppercase,
                "toggle-upper",
                |s| s.uppercase = !s.uppercase,
                cx,
            ),
            self.toggle_row(
                t("kleinbuchstaben"),
                None,
                settings.lowercase,
                "toggle-lower",
                |s| s.lowercase = !s.lowercase,
                cx,
            ),
            self.toggle_row(
                t("Silbentrennung aus"),
                None,
                settings.no_hyphenation,
                "toggle-hyphen",
                |s| s.no_hyphenation = !s.no_hyphenation,
                cx,
            ),
        ];

        // ---- Upscaling ---------------------------------------------------
        let upscaling = vec![
            self.select_row(SelectKind::Upscaler),
            self.stepper_row(
                t("Skalierungsfaktor"),
                Some(t("Vergrößerungsfaktor")),
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
            ),
        ];

        // ---- Ausgabe -----------------------------------------------------
        let output = vec![
            self.free_text_row("output_directory", cx),
            self.toggle_row(
                t("Im Originalordner speichern"),
                Some(t(
                    "Speichert die Übersetzungen im selben Ordner wie die Originaldateien",
                )),
                settings.use_original_folder,
                "toggle-original-folder",
                |s| s.use_original_folder = !s.use_original_folder,
                cx,
            ),
        ];

        // ---- Darstellung -------------------------------------------------
        let appearance = vec![
            self.select_row(SelectKind::ColorScheme),
            self.select_row(SelectKind::UiLanguage),
            self.select_row(SelectKind::Sort),
            self.free_text_row("manga_home_directory", cx),
        ];

        let content = gpui::div()
            .flex()
            .flex_col()
            .px_3()
            .py_6()
            .gap_6()
            .text_size(px(FONT_BASE))
            .child(preferences_group(theme, t("Übersetzung"), translation))
            .child(preferences_group(theme, t("Umgebung"), environment))
            .child(preferences_group(theme, t("Erkennung"), detection))
            .child(preferences_group(theme, t("Inpainting"), inpainting))
            .child(preferences_group(theme, t("VLM"), vlm))
            .child(preferences_group(theme, t("Rendering"), rendering))
            .child(preferences_group(theme, t("Upscaling"), upscaling))
            .child(preferences_group(theme, t("Ausgabe"), output))
            .child(preferences_group(theme, t("Darstellung"), appearance));

        gpui::div()
            .id("settings-panel")
            .w(px(self.settings_width))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.bg)
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

    fn select_row(&self, kind: SelectKind) -> Div {
        let theme = self.theme;
        let mut row = action_row(theme, kind.label(), kind.description().map(t));

        if let Some(state) = self.selects.get(&kind) {
            // The Select root is `size_full`, so the width has to come from a
            // wrapper; setting it on the element itself is ignored.
            row = row.child(gpui::div().w(px(140.)).child(Select::new(state)));
        }

        row
    }

    fn toggle_row(
        &self,
        label: String,
        desc: Option<String>,
        value: bool,
        id: &'static str,
        apply: fn(&mut Settings),
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = self.theme;
        action_row(theme, label, desc).child(widgets::toggle(theme, id, value).on_click(
            cx.listener(move |this, _: &ClickEvent, _window, cx| {
                apply(&mut this.config.settings);
                this.config.save_settings();
                cx.notify();
            }),
        ))
    }

    fn stepper_row(
        &self,
        label: String,
        desc: Option<String>,
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
            .h(px(adw::ENTRY_H))
            .rounded(px(adw::BUTTON_RADIUS))
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

        action_row(theme, label, desc).child(stepper)
    }

    fn free_text_row(&self, key: &'static str, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let label = self.input_label(key);
        let input: Option<Entity<TextInput>> = self.inputs.get(key).cloned();

        let mut column = gpui::div()
            .flex()
            .flex_col()
            .justify_center()
            .gap_1()
            .min_h(px(adw::ACTION_ROW_H))
            .px_3()
            .py_1();

        if let Some(input) = input {
            // `AdwEntryRow`: the title doubles as the placeholder, so it only
            // appears above the field once something has been entered.
            if !input.read(cx).value(cx).is_empty() {
                column = column.child(
                    gpui::div()
                        .text_size(px(FONT_SM))
                        .text_color(theme.text_muted)
                        .child(label),
                );
            }

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

            column = column.child(line);
        }

        column
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
            self.theme = Theme::from_settings(&self.config.settings);
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
// Adwaita preferences layout
// ---------------------------------------------------------------------------

/// One `AdwActionRow`: title on the left, control on the right, 50 px tall.
/// The header box keeps 6 px between the label block and the control.
fn action_row(theme: Theme, label: impl Into<gpui::SharedString>, desc: Option<String>) -> Div {
    gpui::div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .min_h(px(adw::ACTION_ROW_H))
        .px_3()
        .child(widgets::label_block(theme, label, desc))
}

/// One `AdwPreferencesGroup`: a bold title above a card that holds the rows,
/// divided by hairlines like the boxed list in GTK.
fn preferences_group(theme: Theme, title: String, rows: Vec<Div>) -> Div {
    gpui::div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(6.))
        .child(widgets::group_heading(theme, title).px_3())
        .child(widgets::card(theme, rows))
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
