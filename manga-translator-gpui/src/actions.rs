// manga-translator-gpui/src/actions.rs
//
// Application actions. They are bound to keys in `main` and handled in
// `MangaApp::render`; the shortcuts dialog lists exactly these.

use gpui::actions;

actions!(
    manga,
    [
        Reload,
        OpenDirectory,
        GoUp,
        Translate,
        CancelTranslation,
        ToggleSettings,
        FocusSearch,
        SelectAllFiles,
        DeselectAllFiles,
        ShowShortcuts,
        ShowLog,
        ShowAbout,
        ShowApiKeys,
    ]
);
