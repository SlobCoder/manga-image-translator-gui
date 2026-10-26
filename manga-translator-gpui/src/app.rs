// manga-translator-gpui/src/app.rs
//
// Root entity: owns all UI state, drives the workers and renders the shell.
//
// The three panels (sidebar, preview, settings) are rendered by helper
// modules but keep their state here so there is exactly one owner.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_channel::Sender;
use gpui::component::select::SelectState;
use gpui::{Context, Entity, FocusHandle, Image, Window, prelude::*};

use manga_translator_core::config::ConfigManager;
use manga_translator_core::i18n::t;
use manga_translator_core::ipc_bridge::{IpcBridge, TranslationParams};

use crate::theme::{Theme, sizes};
use crate::ui::settings_panel::SelectKind;
use crate::ui::text_input::TextInput;
use crate::workers::{BridgeCmd, FolderItem, Generation, ImageItem, Job, Out, RenderCmd};

/// Size of one page as reported by the backend / filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendState {
    Unknown,
    Starting,
    Ready,
    Failed,
}

/// Which picture the centre pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    Original,
    Translated,
    /// Original and translated with a draggable divider.
    Compare,
}

impl PreviewMode {
    pub(crate) fn needs_translation(self) -> bool {
        !matches!(self, PreviewMode::Original)
    }
}

/// Which panel divider is being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitterKind {
    Sidebar,
    Settings,
}

/// State of an in-progress divider drag.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SplitDrag {
    pub kind: SplitterKind,
    /// Mouse x at the moment the drag started.
    pub start_x: f32,
    /// Panel width at the moment the drag started.
    pub start_width: f32,
}

pub struct MangaApp {
    // ---- services -------------------------------------------------------
    pub(crate) config: ConfigManager,
    pub(crate) theme: Theme,
    pub(crate) render_tx: Sender<RenderCmd>,
    pub(crate) preview_tx: Sender<PathBuf>,
    pub(crate) bridge_tx: Sender<BridgeCmd>,
    pub(crate) generation: Arc<Generation>,
    pub(crate) cancel: Arc<AtomicBool>,

    /// Focus of the panel shell; keeps the app key context in the dispatch
    /// path when no control is focused.
    pub(crate) focus_handle: FocusHandle,

    // ---- file browser ---------------------------------------------------
    pub(crate) cwd: PathBuf,
    pub(crate) folders: Vec<FolderItem>,
    pub(crate) images: Vec<ImageItem>,
    pub(crate) thumbs: HashMap<PathBuf, Arc<Image>>,
    pub(crate) selected: Option<usize>,
    pub(crate) search: String,
    pub(crate) search_input: Option<Entity<TextInput>>,

    // ---- preview --------------------------------------------------------
    pub(crate) preview: Option<Arc<Image>>,
    pub(crate) preview_path: Option<PathBuf>,
    pub(crate) translated_preview: Option<Arc<Image>>,
    pub(crate) preview_mode: PreviewMode,
    /// Divider position in compare mode, 0.0 (all original) ..= 1.0 (all translated).
    pub(crate) split: f32,
    pub(crate) comparing_drag: bool,

    // ---- panel widths ---------------------------------------------------
    pub(crate) sidebar_width: f32,
    pub(crate) settings_width: f32,
    pub(crate) settings_visible: bool,
    pub(crate) split_drag: Option<SplitDrag>,

    // ---- settings panel -------------------------------------------------
    pub(crate) selects: HashMap<SelectKind, Entity<SelectState<Vec<String>>>>,
    pub(crate) inputs: HashMap<&'static str, Entity<TextInput>>,

    // ---- translation ----------------------------------------------------
    pub(crate) translating: bool,
    pub(crate) progress: f64,
    pub(crate) status: String,
    pub(crate) outputs: Vec<PathBuf>,
    pub(crate) backend_state: BackendState,
}

impl MangaApp {
    pub fn new(cx: &mut Context<Self>, window: &mut Window, start_dir: Option<PathBuf>) -> Self {
        let config = ConfigManager::new();
        let theme = Theme::for_scheme(&config.settings.color_scheme);

        let cwd = {
            let requested = start_dir.unwrap_or_else(|| {
                let last = PathBuf::from(&config.settings.last_directory);
                if last.is_dir() {
                    last
                } else {
                    config.manga_home_dir()
                }
            });
            if requested.is_dir() {
                requested
            } else {
                config.manga_home_dir()
            }
        };

        let bridge = Arc::new(IpcBridge::new());
        bridge.configure_paths(
            config.resolve_venv_site_packages(),
            config.manga_translator_dir(),
        );

        let cancel = Arc::new(AtomicBool::new(false));
        let generation = Arc::new(Generation::new());

        // GTK and GPUI share the paned positions: both are panel widths.
        let sidebar_width = (config.settings.paned_position as f32).clamp(
            crate::theme::sizes::SIDEBAR_MIN,
            crate::theme::sizes::SIDEBAR_MAX,
        );
        let settings_width = (config.settings.right_paned_position as f32).clamp(
            crate::theme::sizes::SETTINGS_MIN,
            crate::theme::sizes::SETTINGS_MAX,
        );

        let (out_tx, out_rx) = async_channel::unbounded();
        let render_tx = crate::workers::spawn_render_worker(out_tx.clone(), generation.clone());
        let preview_tx = crate::workers::spawn_preview_worker(out_tx.clone());
        let bridge_tx = crate::workers::spawn_bridge_worker(bridge.clone(), cancel.clone(), out_tx);

        cx.spawn(async move |this, cx| {
            while let Ok(message) = out_rx.recv().await {
                if this
                    .update(cx, |app, cx| app.handle_worker_message(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        let mut app = Self {
            config,
            theme,
            render_tx,
            preview_tx,
            bridge_tx,
            generation,
            cancel,
            focus_handle: cx.focus_handle(),
            cwd: cwd.clone(),
            folders: Vec::new(),
            images: Vec::new(),
            thumbs: HashMap::new(),
            selected: None,
            search: String::new(),
            search_input: None,
            preview: None,
            preview_path: None,
            translated_preview: None,
            preview_mode: PreviewMode::Original,
            split: 0.5,
            comparing_drag: false,
            sidebar_width,
            settings_width,
            settings_visible: true,
            split_drag: None,
            selects: HashMap::new(),
            inputs: HashMap::new(),
            translating: false,
            progress: 0.0,
            status: t("Bereit"),
            outputs: Vec::new(),
            backend_state: BackendState::Unknown,
        };

        app.build_inputs(cx, window);
        app.build_selects(cx, window);
        window.focus(&app.focus_handle, cx);
        app.load_folder(cwd, cx);
        app.backend_state = BackendState::Starting;
        let _ = app.bridge_tx.send_blocking(BridgeCmd::EnsureReady);
        app
    }

    // -----------------------------------------------------------------------
    // Worker plumbing
    // -----------------------------------------------------------------------

    fn handle_worker_message(&mut self, message: Out, cx: &mut Context<Self>) {
        match message {
            Out::Folder {
                generation,
                path,
                folders,
                images,
            } => {
                if generation != self.generation.current() {
                    return;
                }
                self.cwd = path;
                self.folders = folders;
                self.images = images;
                self.thumbs.clear();
                self.selected = None;
                self.preview = None;
                self.preview_path = None;
                self.translated_preview = None;
                self.preview_mode = PreviewMode::Original;

                self.config.settings.last_directory = self.cwd.to_string_lossy().into_owned();
                self.config.save_settings();

                if !self.images.is_empty() {
                    self.select_image(0, cx);
                }
                cx.notify();
            }
            Out::Thumb {
                generation,
                path,
                png,
            } => {
                if generation != self.generation.current() {
                    return;
                }
                let image = Arc::new(Image::from_bytes(gpui::ImageFormat::Png, png));
                self.thumbs.insert(path, image);
                cx.notify();
            }
            Out::Preview { path, png } => {
                let image = Arc::new(Image::from_bytes(gpui::ImageFormat::Png, png));
                if self.preview_path.as_ref() == Some(&path) {
                    self.preview = Some(image);
                } else if self.translated_path_for_selected().as_ref() == Some(&path) {
                    self.translated_preview = Some(image);
                } else {
                    // Belongs to a page the user already navigated away from.
                    return;
                }
                cx.notify();
            }
            Out::Progress { fraction, message } => {
                self.progress = fraction;
                if !message.is_empty() {
                    self.status = message;
                }
                cx.notify();
            }
            Out::Backend { ok, message } => {
                self.backend_state = if ok {
                    BackendState::Ready
                } else {
                    BackendState::Failed
                };
                if !ok && !message.is_empty() {
                    self.status = message;
                }
                cx.notify();
            }
            Out::Finished {
                ok,
                message,
                outputs,
            } => {
                self.translating = false;
                self.backend_state = if ok {
                    BackendState::Ready
                } else {
                    BackendState::Failed
                };
                self.status = message;
                if ok {
                    self.progress = 1.0;
                    self.outputs = outputs;
                    if self.preview_mode.needs_translation() {
                        self.load_translated_preview(cx);
                    }
                } else {
                    self.progress = 0.0;
                }
                cx.notify();
            }
        }
    }

    // -----------------------------------------------------------------------
    // Navigation
    // -----------------------------------------------------------------------

    pub(crate) fn load_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let generation = self.generation.next();
        self.cwd = path.clone();
        let _ = self.render_tx.send_blocking(RenderCmd::LoadFolder {
            generation,
            path,
            sort_method: self.config.settings.sort_method,
        });
        cx.notify();
    }

    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        let cwd = self.cwd.clone();
        self.load_folder(cwd, cx);
    }

    pub(crate) fn go_up(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.cwd.parent() {
            let parent = parent.to_path_buf();
            self.load_folder(parent, cx);
        }
    }

    pub(crate) fn go_home(&mut self, cx: &mut Context<Self>) {
        let home = self.config.manga_home_dir();
        self.load_folder(home, cx);
    }

    /// Put the keyboard focus into the image filter.
    pub(crate) fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = self.search_input.clone() {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
    }

    pub(crate) fn open_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.load_folder(path, cx);
    }

    pub(crate) fn select_image(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(image) = self.images.get(index) else {
            return;
        };
        let path = image.path.clone();
        self.selected = Some(index);
        self.preview = None;
        self.translated_preview = None;
        self.preview_path = Some(path.clone());
        self.comparing_drag = false;

        let _ = self.preview_tx.send_blocking(path);

        // Stay in the current view (original / translated / compare) while
        // paging, and fetch the matching translated page if one exists.
        if self.preview_mode.needs_translation() {
            self.load_translated_preview(cx);
        }
        cx.notify();
    }

    pub(crate) fn select_next(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(current) = self.selected else {
            return;
        };
        let next = current as isize + delta;
        if next >= 0 && (next as usize) < self.images.len() {
            self.select_image(next as usize, cx);
        }
    }

    pub(crate) fn set_preview_mode(&mut self, mode: PreviewMode, cx: &mut Context<Self>) {
        self.preview_mode = mode;
        self.comparing_drag = false;
        if mode.needs_translation() && self.translated_preview.is_none() {
            self.load_translated_preview(cx);
        }
        cx.notify();
    }

    /// Move the comparison divider to `x` (window coordinates) inside `area`.
    pub(crate) fn set_split_at_x(&mut self, x: f32, area_left: f32, area_width: f32) {
        if area_width <= 0.0 {
            return;
        }
        self.split = ((x - area_left) / area_width).clamp(0.0, 1.0);
    }

    // -----------------------------------------------------------------------
    // Panel dividers
    // -----------------------------------------------------------------------

    pub(crate) fn splitter_width(&self, kind: SplitterKind) -> f32 {
        match kind {
            SplitterKind::Sidebar => self.sidebar_width,
            SplitterKind::Settings => self.settings_width,
        }
    }

    pub(crate) fn is_dragging_splitter(&self, kind: SplitterKind) -> bool {
        self.split_drag.map(|drag| drag.kind) == Some(kind)
    }

    /// Start dragging a divider. A double click resets it to its default width.
    pub(crate) fn begin_split_drag(
        &mut self,
        kind: SplitterKind,
        mouse_x: f32,
        double_click: bool,
        cx: &mut Context<Self>,
    ) {
        if double_click {
            match kind {
                SplitterKind::Sidebar => self.sidebar_width = sizes::SIDEBAR_W,
                SplitterKind::Settings => self.settings_width = sizes::SETTINGS_W,
            }
            self.persist_panel_widths();
            cx.notify();
            return;
        }

        self.split_drag = Some(SplitDrag {
            kind,
            start_x: mouse_x,
            start_width: self.splitter_width(kind),
        });
        cx.notify();
    }

    /// Apply a drag movement. `viewport_width` keeps the preview from collapsing.
    pub(crate) fn update_split_drag(
        &mut self,
        mouse_x: f32,
        viewport_width: f32,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.split_drag else {
            return;
        };
        let delta = mouse_x - drag.start_x;
        let reserved = sizes::MIN_PREVIEW + 2.0 * sizes::SPLITTER_W;

        match drag.kind {
            SplitterKind::Sidebar => {
                let upper = (viewport_width - self.settings_width - reserved)
                    .min(sizes::SIDEBAR_MAX)
                    .max(sizes::SIDEBAR_MIN);
                self.sidebar_width = (drag.start_width + delta).clamp(sizes::SIDEBAR_MIN, upper);
            }
            SplitterKind::Settings => {
                let upper = (viewport_width - self.sidebar_width - reserved)
                    .min(sizes::SETTINGS_MAX)
                    .max(sizes::SETTINGS_MIN);
                self.settings_width = (drag.start_width - delta).clamp(sizes::SETTINGS_MIN, upper);
            }
        }
        cx.notify();
    }

    pub(crate) fn end_split_drag(&mut self, cx: &mut Context<Self>) {
        if self.split_drag.take().is_some() {
            self.persist_panel_widths();
            cx.notify();
        }
    }

    /// Panel widths live in the same config keys the GTK paned uses.
    fn persist_panel_widths(&mut self) {
        self.config.settings.paned_position = self.sidebar_width.round() as i32;
        self.config.settings.right_paned_position = self.settings_width.round() as i32;
        self.config.save_settings();
    }

    fn load_translated_preview(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.translated_path_for_selected() else {
            return;
        };
        let _ = self.preview_tx.send_blocking(path);
        cx.notify();
    }

    pub(crate) fn translated_path_for_selected(&self) -> Option<PathBuf> {
        let selected = self.images.get(self.selected?)?;
        let stem = selected.path.file_stem()?;

        // 1. A translation produced during this session.
        if let Some(path) = self
            .outputs
            .iter()
            .find(|path| path.file_stem() == Some(stem))
        {
            return Some(path.clone());
        }

        // 2. A translation from an earlier run. The backend writes results into
        //    a sibling directory named "<folder>-translated".
        let parent = selected.path.parent()?;
        let folder = parent.file_name()?.to_string_lossy().into_owned();
        let candidate = parent
            .with_file_name(format!("{}-translated", folder))
            .join(selected.path.file_name()?);
        candidate.is_file().then_some(candidate)
    }

    pub(crate) fn selected_image(&self) -> Option<&ImageItem> {
        self.images.get(self.selected?)
    }

    // -----------------------------------------------------------------------
    // Settings persistence
    // -----------------------------------------------------------------------

    pub(crate) fn persist(&self) {
        self.config.save_settings();
        self.config.save_api_keys();
    }

    // -----------------------------------------------------------------------
    // Translation
    // -----------------------------------------------------------------------

    pub(crate) fn start_translation(&mut self, cx: &mut Context<Self>) {
        if self.translating {
            return;
        }
        if self.images.is_empty() {
            self.status = t("Keine Bilder zum Übersetzen in diesem Ordner");
            cx.notify();
            return;
        }

        if let Some(problem) = self.preflight_problem() {
            self.status = problem;
            cx.notify();
            return;
        }

        let files: Vec<PathBuf> = self.images.iter().map(|i| i.path.clone()).collect();
        let params = self.build_params();

        self.translating = true;
        self.progress = 0.0;
        self.status = t("Übersetzung wird vorbereitet…");
        self.backend_state = BackendState::Starting;
        self.outputs.clear();
        self.translated_preview = None;
        self.comparing_drag = false;

        let _ = self
            .bridge_tx
            .send_blocking(BridgeCmd::Translate(Box::new(Job { files, params })));
        cx.notify();
    }

    pub(crate) fn cancel_translation(&mut self, cx: &mut Context<Self>) {
        if !self.translating {
            return;
        }
        self.cancel.store(true, Ordering::SeqCst);
        self.status = t("Wird abgebrochen…");
        cx.notify();
    }

    /// Reject obviously impossible configurations before starting Python.
    fn preflight_problem(&self) -> Option<String> {
        let settings = &self.config.settings;
        let mode = settings.translation_mode.as_str();
        if mode == "vlm" {
            match settings.vlm_type.as_str() {
                "gemini" if self.config.api_keys.gemini.trim().is_empty() => {
                    return Some(t(
                        "Gemini API-Schlüssel fehlt (Einstellungen → API-Schlüssel)",
                    ));
                }
                "openrouter" if self.config.api_keys.openrouter.trim().is_empty() => {
                    return Some(t(
                        "OpenRouter API-Schlüssel fehlt (Einstellungen → API-Schlüssel)",
                    ));
                }
                _ => {}
            }
        }
        None
    }

    pub(crate) fn build_params(&self) -> TranslationParams {
        let settings = &self.config.settings;
        let keys = &self.config.api_keys;

        let mut api_keys = std::collections::HashMap::new();
        let pairs = [
            ("deepl", &keys.deepl),
            ("openai", &keys.openai),
            ("gemini", &keys.gemini),
            ("deepseek", &keys.deepseek),
            ("groq", &keys.groq),
            ("openrouter", &keys.openrouter),
            ("baidu_app_id", &keys.baidu_app_id),
            ("baidu_secret_key", &keys.baidu_secret_key),
            ("caiyun_token", &keys.caiyun_token),
        ];
        for (name, value) in pairs {
            if !value.trim().is_empty() {
                api_keys.insert(name.to_string(), value.clone());
            }
        }

        TranslationParams {
            translator: options_get(
                manga_translator_core::config::options::TRANSLATORS,
                settings.translator_index,
            ),
            target_lang: settings.target_language.clone(),
            detector: options_get(
                manga_translator_core::config::options::DETECTORS,
                settings.detector_index,
            ),
            ocr: options_get(
                manga_translator_core::config::options::OCR_MODELS,
                settings.ocr_index,
            ),
            inpainter: options_get(
                manga_translator_core::config::options::INPAINTERS,
                settings.inpainter_index,
            ),
            upscaler: options_get(
                manga_translator_core::config::options::UPSCALERS,
                settings.upscaler_index,
            ),
            direction: if settings.direction_rtl {
                "rtl".into()
            } else {
                "auto".into()
            },
            use_mocr_merge: settings.use_mocr_merge,
            device: settings.device.clone(),
            output_directory: settings.output_directory.clone(),
            use_original_folder: settings.use_original_folder,
            translation_mode: settings.translation_mode.clone(),
            vlm_type: settings.vlm_type.clone(),
            gemini_model: settings.gemini_model.clone(),
            openrouter_model: settings.openrouter_model.clone(),
            local_model_path: settings.local_model_path.clone(),
            project_name: settings.project_name.clone(),
            gemini_api_key: keys.gemini.clone(),
            openrouter_api_key: keys.openrouter.clone(),
            upscale_ratio: settings.upscale_ratio,
            renderer: options_get(
                manga_translator_core::config::options::RENDERER_MAP,
                settings.renderer_index,
            ),
            alignment: options_get(
                manga_translator_core::config::options::ALIGNMENT_MAP,
                settings.alignment_index,
            ),
            disable_font_border: settings.disable_font_border,
            font_size_offset: settings.font_size_offset,
            font_size_minimum: settings.font_size_minimum,
            render_direction: options_get(
                manga_translator_core::config::options::DIRECTION_MAP,
                settings.direction_index,
            ),
            uppercase: settings.uppercase,
            lowercase: settings.lowercase,
            font_color: settings.font_color.clone(),
            no_hyphenation: settings.no_hyphenation,
            line_spacing: settings.line_spacing,
            font_size: settings.font_size,
            rtl: settings.direction_rtl,
            mask_dilation_offset: settings.mask_dilation_offset,
            kernel_size: settings.kernel_size,
            inpainting_size: settings.inpainting_size,
            inpainting_precision: options_get(
                manga_translator_core::config::options::INPAINTING_PRECISION_MAP,
                settings.inpainting_precision_index,
            ),
            detection_size: settings.detection_size,
            api_keys,
        }
    }

    // -----------------------------------------------------------------------
    // Text inputs
    // -----------------------------------------------------------------------

    fn build_inputs(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        type Apply = fn(&mut MangaApp, &str);
        let defs: Vec<(&'static str, String, bool, Apply)> = vec![
            (
                "gemini_api_key",
                t("Gemini API-Schlüssel"),
                true,
                |app, value| app.config.api_keys.gemini = value.to_string(),
            ),
            (
                "openrouter_api_key",
                t("OpenRouter API-Schlüssel"),
                true,
                |app, value| app.config.api_keys.openrouter = value.to_string(),
            ),
            (
                "openrouter_model",
                t("OpenRouter Modell"),
                false,
                |app, value| app.config.settings.openrouter_model = value.to_string(),
            ),
            (
                "local_model_path",
                t("Pfad zum lokalen Modell"),
                false,
                |app, value| app.config.settings.local_model_path = value.to_string(),
            ),
            (
                "output_directory",
                t("Ausgabeordner"),
                false,
                |app, value| app.config.settings.output_directory = value.to_string(),
            ),
            (
                "manga_home_directory",
                t("Manga-Home-Ordner"),
                false,
                |app, value| app.config.settings.manga_home_directory = value.to_string(),
            ),
            (
                "font_color",
                t("Schriftfarbe (z. B. #ffffff) oder #fg:#bg"),
                false,
                |app, value| app.config.settings.font_color = value.to_string(),
            ),
        ];

        for (key, placeholder, masked, apply) in defs {
            let initial = self.input_initial_value(key);
            let weak_change = cx.entity().downgrade();
            let weak_commit = cx.entity().downgrade();

            let entry = cx.new(|cx| {
                TextInput::new(cx, window, initial, placeholder, masked)
                    .on_change(move |text, _window, cx| {
                        if let Some(app) = weak_change.upgrade() {
                            app.update(cx, |app, _cx| {
                                apply(app, text);
                            });
                        }
                    })
                    .on_commit(move |_text, _window, cx| {
                        if let Some(app) = weak_commit.upgrade() {
                            app.update(cx, |app, _cx| {
                                app.persist();
                            });
                        }
                    })
            });

            self.inputs.insert(key, entry);
        }

        // Image filter / search field.
        let weak_search = cx.entity().downgrade();
        let search = cx.new(|cx| {
            TextInput::new(cx, window, "", t("Bilder filtern…"), false).on_change(
                move |text, _window, cx| {
                    if let Some(app) = weak_search.upgrade() {
                        app.update(cx, |app, cx| {
                            app.search = text.to_string();
                            cx.notify();
                        });
                    }
                },
            )
        });
        self.search_input = Some(search);
    }

    fn input_initial_value(&self, key: &str) -> String {
        let settings = &self.config.settings;
        let keys = &self.config.api_keys;
        match key {
            "gemini_api_key" => keys.gemini.clone(),
            "openrouter_api_key" => keys.openrouter.clone(),
            "openrouter_model" => settings.openrouter_model.clone(),
            "local_model_path" => settings.local_model_path.clone(),
            "output_directory" => settings.output_directory.clone(),
            "manga_home_directory" => settings.manga_home_directory.clone(),
            "font_color" => settings.font_color.clone(),
            _ => String::new(),
        }
    }

    /// Push config values into the text fields (after a language reload etc.).
    pub(crate) fn sync_inputs(&self, window: &mut Window, cx: &mut Context<Self>) {
        for (key, entity) in &self.inputs {
            let value = self.input_initial_value(key);
            entity.update(cx, |input, cx| input.set_value(&value, window, cx));
        }
    }

    /// Indices of `images` that match the current filter.
    pub(crate) fn visible_images(&self) -> Vec<usize> {
        if self.search.trim().is_empty() {
            return (0..self.images.len()).collect();
        }
        let needle = self.search.to_lowercase();
        self.images
            .iter()
            .enumerate()
            .filter(|(_, image)| image.name.to_lowercase().contains(&needle))
            .map(|(index, _)| index)
            .collect()
    }
}

fn options_get(list: &'static [&'static str], index: u32) -> String {
    list.get(index as usize)
        .copied()
        .unwrap_or_else(|| list.first().copied().unwrap_or(""))
        .to_string()
}

/// Formats a byte count for the file list.
pub(crate) fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let value = bytes as f64;
    if value >= KB * KB {
        format!("{:.1} MB", value / (KB * KB))
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Shortens a path for the header, keeping the tail visible.
pub(crate) fn shorten_path(path: &Path) -> String {
    let text = path.display().to_string();
    if text.chars().count() <= 62 {
        return text;
    }
    let tail: String = text
        .chars()
        .rev()
        .take(58)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{}", tail)
}

impl Render for MangaApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::actions::*;

        let theme = self.theme;
        let weak = cx.entity().downgrade();
        let settings_visible = self.settings_visible;

        let mut body = gpui::div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h(gpui::px(0.))
            .child(self.render_sidebar(cx))
            .child(crate::ui::splitter::splitter(
                theme,
                weak.clone(),
                SplitterKind::Sidebar,
            ))
            .child(self.render_preview(cx));

        if settings_visible {
            body = body
                .child(crate::ui::splitter::splitter(
                    theme,
                    weak,
                    SplitterKind::Settings,
                ))
                .child(self.render_settings(cx));
        }

        gpui::div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.bg)
            .text_color(theme.text)
            .text_size(gpui::px(crate::theme::sizes::FONT_BASE))
            .track_focus(&self.focus_handle)
            .key_context(if self.translating {
                "MangaApp Translating"
            } else {
                "MangaApp"
            })
            .on_action(cx.listener(|this, _: &Reload, _window, cx| this.reload(cx)))
            .on_action(cx.listener(|this, _: &GoUp, _window, cx| this.go_up(cx)))
            .on_action(cx.listener(|this, _: &Translate, _window, cx| this.start_translation(cx)))
            .on_action(cx.listener(|this, _: &CancelTranslation, _window, cx| {
                if this.translating {
                    this.cancel_translation(cx);
                } else {
                    // Leave Escape to whatever dialog or dropdown is open.
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleSettings, _window, cx| {
                this.settings_visible = !this.settings_visible;
                cx.notify();
            }))
            .on_action(
                cx.listener(|this, _: &FocusSearch, window, cx| this.focus_search(window, cx)),
            )
            .on_action(cx.listener(|_this, _: &ShowShortcuts, window, cx| {
                crate::ui::dialogs::open_shortcuts(window, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowLog, window, cx| {
                crate::ui::dialogs::open_log(this.theme, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowAbout, window, cx| {
                crate::ui::dialogs::open_about(this.theme, window, cx)
            }))
            .child(self.render_header(cx))
            .child(body)
            .child(self.render_status_bar(cx))
    }
}
