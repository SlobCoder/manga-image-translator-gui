// manga-translator-gpui/src/main.rs
//
// GPUI (Zed) frontend for the manga-image-translator backend.
//
// Shares its configuration file and the Python IPC bridge with the GTK
// frontend via `manga-translator-core`, so both GUIs stay interchangeable.

mod actions;
mod app;
mod theme;
mod ui;
mod workers;

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

use gpui::prelude::*;
use gpui::{App, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, px, size};

use manga_translator_core::config::ConfigManager;
use manga_translator_core::i18n;

/// Log file shared with the GTK frontend, so both write one history.
static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);

/// Dual logging: stderr for the terminal plus the shared log file.
fn init_logging() {
    let log_path = ConfigManager::log_file_path();

    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::rename(&log_path, log_path.with_extension("log.old"));

    if let Ok(file) = File::create(&log_path) {
        *LOG_FILE.lock().unwrap() = Some(file);
    }

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("manga_translator_gpui=info,warn"),
    )
    .format(|buf, record| {
        let line = format!(
            "[{} {:5}] {}\n",
            buf.timestamp(),
            record.level(),
            record.args()
        );
        if let Ok(mut guard) = LOG_FILE.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }
        buf.write_all(line.as_bytes())
    })
    .init();
}

fn key_bindings() -> Vec<KeyBinding> {
    use actions::*;
    // No key context: these are window-level fallbacks. A focused control's
    // own bindings win, and Escape propagates when no translation is running,
    // so dialogs and dropdowns still close.
    vec![
        KeyBinding::new("ctrl-o", OpenDirectory, None),
        KeyBinding::new("ctrl-r", Reload, None),
        KeyBinding::new("f5", Reload, None),
        KeyBinding::new("alt-left", GoUp, None),
        KeyBinding::new("ctrl-t", Translate, None),
        KeyBinding::new("f9", ToggleSettings, None),
        KeyBinding::new("ctrl-l", FocusSearch, None),
        KeyBinding::new("ctrl-a", SelectAllFiles, None),
        KeyBinding::new("ctrl-shift-a", DeselectAllFiles, None),
        KeyBinding::new("f1", ShowShortcuts, None),
        KeyBinding::new("ctrl-shift-l", ShowLog, None),
        KeyBinding::new("escape", CancelTranslation, None),
    ]
}

fn main() {
    init_logging();
    i18n::init();

    // Apply the saved UI language before the first render.
    let settings = ConfigManager::new().settings;
    if !settings.ui_language.is_empty() {
        i18n::set_language(&settings.ui_language);
    }

    // The embedded Lucide icons back every caret, check mark and tag icon in
    // gpui-component; without them those slots render empty.
    gpui::application()
        .with_assets(gpui::assets::Assets)
        .run(move |cx: &mut App| {
        gpui::init(cx);
        cx.bind_keys(key_bindings());

        // Give the component library the same palette the panels draw with.
        theme::Theme::from_settings(&settings).apply_to_component_theme(cx);

        // Optional CLI argument: the folder to open at startup.
        let start_dir = std::env::args().nth(1).map(std::path::PathBuf::from);

        // Restore the window geometry remembered by either frontend.
        let width = settings.window_width.max(980) as f32;
        let height = settings.window_height.max(620) as f32;
        let bounds = gpui::Bounds::centered(None, size(px(width), px(height)), cx);

        gpui::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(980.), px(620.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Manga Translator".into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                app_id: Some("com.mangatranslator.gpui".into()),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(move |cx| app::MangaApp::new(cx, window, start_dir.clone())),
        )
        .expect("failed to open the main window");

        cx.activate(true);
    });
}
