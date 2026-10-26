// manga-translator-gpui/src/main.rs
//
// GPUI (Zed) frontend for the manga-image-translator backend.
//
// Shares its configuration file and the Python IPC bridge with the GTK
// frontend via `manga-translator-core`, so both GUIs stay interchangeable.

mod app;
mod theme;
mod ui;
mod workers;

use gpui::prelude::*;
use gpui::{App, TitlebarOptions, WindowBounds, WindowOptions, px, size};

use manga_translator_core::config::ConfigManager;
use manga_translator_core::i18n;

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("manga_translator_gpui=info,warn"),
    )
    .init();

    i18n::init();

    // Apply the saved UI language before the first render.
    let settings = ConfigManager::new().settings;
    if !settings.ui_language.is_empty() {
        i18n::set_language(&settings.ui_language);
    }

    gpui::application().run(move |cx: &mut App| {
        gpui::init(cx);
        cx.bind_keys(ui::text_input::key_bindings());

        // Give the component library the same palette the panels draw with.
        theme::Theme::for_scheme(&settings.color_scheme).apply_to_component_theme(cx);

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
            |_window, cx| cx.new(move |cx| app::MangaApp::new(cx, start_dir.clone())),
        )
        .expect("failed to open the main window");

        cx.activate(true);
    });
}
