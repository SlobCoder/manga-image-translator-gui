// manga-translator-gtk/src/ui/mod.rs
//
// UI module — re-exports all UI components.

mod css;
mod dialogs;
mod file_browser;
mod main_window;
mod marquee_label;
mod preview;
mod settings_panel;
mod spring_splitter;

pub use main_window::build_main_window;
