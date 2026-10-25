// manga-translator-core/src/i18n.rs
//
// Framework-agnostic internationalization using gettext-rs.
//
// The GTK frontend has its own richer wrapper (widget re-translation
// registry) in `manga-translator-gtk/src/i18n.rs`; this module only
// provides the parts that both frontends need: domain setup, language
// switching and the `t()` lookup used by `config.rs`.

use gettextrs::{
    LocaleCategory, bind_textdomain_codeset, bindtextdomain, gettext, setlocale, textdomain,
};
use std::path::PathBuf;

/// Supported UI languages with their display names.
/// The first entry ("", "Automatisch") represents auto-detection.
pub const SUPPORTED_LANGUAGES: &[(&str, &str)] = &[
    ("", "Automatisch"),
    ("de", "Deutsch"),
    ("en", "English"),
    ("es", "Español"),
    ("fr", "Français"),
    ("it", "Italiano"),
    ("pt_BR", "Português (Brasil)"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("zh_CN", "简体中文"),
];

const TEXT_DOMAIN: &str = "manga-translator";

/// Initialize gettext with the locale directory.
pub fn init() {
    // SAFETY: `setlocale` mutates process-global state and is not reentrant;
    // it runs on the main thread during startup.
    unsafe {
        setlocale(LocaleCategory::LcAll, "");
    }

    if let Some(locale_dir) = find_locale_dir() {
        bindtextdomain(TEXT_DOMAIN, &locale_dir).ok();
    }

    // On Windows gettext returns the system codepage unless told otherwise.
    bind_textdomain_codeset(TEXT_DOMAIN, "UTF-8").ok();
    textdomain(TEXT_DOMAIN).ok();

    log::info!("i18n initialized (domain: {})", TEXT_DOMAIN);
}

/// Set the UI language explicitly via the `LANGUAGE` environment variable.
///
/// Unlike `setlocale("en.UTF-8")` this works even when the locale is not
/// installed on the system.
pub fn set_language(lang: &str) {
    // SAFETY: single-threaded startup operation that only affects this
    // process' own gettext lookups.
    unsafe {
        std::env::set_var("LANGUAGE", lang);
    }

    // SAFETY: `setlocale` mutates process-global state and is not reentrant.
    // It is only called from the main thread, as part of language setup.
    unsafe {
        setlocale(LocaleCategory::LcAll, "");
    }

    if let Some(locale_dir) = find_locale_dir() {
        let _ = bindtextdomain(TEXT_DOMAIN, &locale_dir);
    }
    let _ = textdomain(TEXT_DOMAIN);

    log::info!("Language set to: '{}'", lang);
}

/// Translate a string using the current gettext domain.
pub fn t(msgid: &str) -> String {
    gettext(msgid)
}

/// Get the display name for a language code.
pub fn get_language_display_name(code: &str) -> String {
    SUPPORTED_LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| code.to_string())
}

/// Attempt to locate the locale directory.
fn find_locale_dir() -> Option<String> {
    // 1. Compile-time path: the locale/ directory at the project root.
    {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_locale = manifest_dir.join("..").join("locale");
        if project_locale.is_dir() {
            if let Ok(canonical) = project_locale.canonicalize() {
                return Some(canonical.to_string_lossy().into_owned());
            }
        }
    }

    // 2. Relative to the executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidates = [
                exe_dir.join("locale"),
                exe_dir.join("..").join("share").join("locale"),
                exe_dir.join("..").join("..").join("locale"),
            ];
            for candidate in candidates {
                if candidate.is_dir() {
                    if let Ok(canonical) = candidate.canonicalize() {
                        log::info!("Locale dir found: {}", canonical.display());
                        return Some(canonical.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }

    // 3. Current working directory
    for candidate in &["locale", "po"] {
        let path = PathBuf::from(candidate);
        if path.is_dir() {
            return Some(path.to_string_lossy().into_owned());
        }
    }

    // 4. XDG data directory
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        let xdg_locale = PathBuf::from(data_home).join("locale");
        if xdg_locale.is_dir() {
            return Some(xdg_locale.to_string_lossy().into_owned());
        }
    }

    log::warn!("No locale directory found — translations will not be available");
    None
}
