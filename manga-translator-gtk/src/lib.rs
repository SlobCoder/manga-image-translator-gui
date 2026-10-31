// manga-translator-gtk/src/lib.rs
//
// Library root — exposes public modules so that integration tests
// (under tests/) and external code can import types from this crate.
//
// The binary entry point lives in main.rs and re-uses these modules
// through `use manga_translator_gtk::…`.
//
// The toolkit-independent parts (`config`, `ipc_bridge`) live in the
// shared `manga-translator-core` crate and are re-exported here so that
// existing `crate::config::…` / `crate::ipc_bridge::…` paths keep working.

pub mod i18n;
pub mod ui;

pub use manga_translator_core::{config, ipc_bridge};

/// Application ID.
///
/// Also the name of the desktop entry and of the installed icon, so the
/// packaging (`flatpak/`, `build-appimage.sh`) has to use the same string.
pub const APP_ID: &str = "com.mangatranslator.gui";
