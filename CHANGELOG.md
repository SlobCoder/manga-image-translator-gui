# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- **Responsive sidebars** — the file browser, the settings pane and the preview
  now live in two nested `AdwOverlaySplitView`s. Above 1000 px window width the
  browser is pinned next to the rest; below, it collapses and slides in as an
  overlay (header button, F8, or a swipe). The settings sidebar collapses at
  900 px (header button, F9, or a swipe). Both draggable dividers are gone —
  the browser is 280–420 px wide, the settings 420–560 px.
- **Search in the OpenRouter popup** — a search field above the sort bar
  filters the model list by id or display name while typing; the filter is
  cleared when the popup closes.
- **Model details in the OpenRouter popup** — a sidebar next to the list shows
  the input price, output price (both per 1M tokens) and the Artificial
  Analysis intelligence index of the model under the pointer. Free models and
  unscored models are labelled accordingly.
- **Current Gemini models** — the Gemini dropdown used a hardcoded list that
  stopped at the 3.1 previews. A new "Gemini-Modelle aktualisieren" row loads
  the list from the Gemini API when the configured key is accepted, and falls
  back to OpenRouter's public catalogue otherwise (the same ids without the
  `google/` prefix). The result is cached in `gemini_models`, so it survives
  restarts; image generation, TTS and embedding models are filtered out.
- **Sortable OpenRouter model list** — the model popup carries a bar with
  "Preis ↑/↓" and "Intelligenz ↑/↓". Price sorts on the prompt price reported
  by the API, intelligence on the Artificial Analysis index; models without a
  value stay at the end in both directions. The bar sits above the scrolled
  list, so it stays visible while the list scrolls, and the active order is
  highlighted. Model metadata and the chosen order are cached in the settings,
  so both survive a restart.
- **Scrolling model names in the OpenRouter popup** — model ids rarely fit the
  popup. The list rows use a `MarqueeLabel`: ids that do not fit scroll back
  and forth instead of being ellipsized, and the check mark stays pinned to the
  right edge.

### Changed
- **Keyboard-accessible comparison slider** — the divider now shows a
  col-resize cursor, can be focused and moved with Left/Right (2 % steps) and
  Home/End, and reports its position to assistive technologies.
- **Adwaita alignment** — the preview mode selector is now an `AdwToggleGroup`
  instead of three grouped `GtkToggleButton`s, and the progress spinners are
  `AdwSpinner`, which animates whenever it is visible.

### Fixed
- **Comparison did not work after a restart** — selecting a page in an already
  translated chapter only loaded the original, because the GTK frontend never
  looked for the translated counterpart. It now searches the current session's
  outputs, the sibling `<folder>-translated` directory and the configured output
  directory, loads both images and opens the comparison, matching the GPUI
  frontend.
- **Invalid CSS property in the scroll indicators** — `pointer-events: none`
  is not a GTK4 CSS property and produced a `Gtk-WARNING` on every start. The
  scroll indicators are now made non-targetable in code instead.
- **VLM settings did not appear after switching the mode** — starting in any
  other mode and then selecting "VLM" revealed an empty `GtkRevealer`:
  `set_config()` had hidden the `AdwPreferencesGroup` itself, while
  `on_mode_changed()` only toggled the revealer. The revealer is now the only
  visibility control, so the group is never hidden.
- **OpenRouter model list stayed stale** — "Modellliste aktualisieren" imported
  `manga_translator.vlm.mtpe.fetch_openrouter_models`, a module that only
  exists in newer Python checkouts. The import failed, the fetch returned an
  empty list and the GUI kept showing its cached models. The backend now
  queries `/api/v1/models` itself with the standard library and keeps the
  models that accept image input.

## [0.2.3] — 2026-10-26

### Changed
- **Preview placeholder** — the manga spread now turns a real page instead of
  wiping across it. The sheet lifts off the spine, foreshortens in perspective,
  bends at the free edge and throws a shadow on the page it uncovers; both
  themes are handled, and the page lands exactly on the next spread so the loop
  has no seam. The "Kein Bild geladen" label stays readable while it fills.
- **Preview page turn motion** — the rotation follows the step response of a
  damped spring instead of an ease-in-out curve, so the sheet carries a little
  past the far side and rocks back onto it rather than decelerating and stopping
  dead. The spring runs on an eased clock, which keeps the lift from jumping off
  the spread in the first frame. The free edge is no longer bowed by a fixed
  function of the angle: its bend follows the actual sweep speed, so it builds up
  after the lift starts and relaxes as the sheet lands.
- **Preview placeholder drawing is cached** — the spread (panels, screentone,
  gutter) and the paper grain are drawn into cached surfaces instead of being
  rebuilt on every frame; only the turning sheet still changes. A resting frame
  drops from 2.48 ms to 0.32 ms and a turning one from 2.87 ms to 0.66 ms at
  420×720, i.e. from ~15 % to ~2 % of a core at 60 fps. The surfaces are
  allocated at device resolution and the page geometry is snapped to whole
  device pixels, so the cached blit is a 1:1 copy — rendering them at user
  resolution resampled every panel border and showed up as a softness the direct
  drawing did not have. Two redundant grain passes that were painted before the
  panels and then covered by them are gone with it; they only survived in the
  gutters, where they doubled the grain density.

### Fixed
- **Vertical hairlines across the turning page** — the sheet is projected as 40
  vertical strips and neighbouring strips share a boundary exactly, but with an
  antialiased clip each one only reached partial coverage on that shared column.
  The result was a vertical hairline every few pixels, bright across the panels,
  with the red seal of the spread underneath visible inside it. The strip
  boundaries are now snapped to whole device pixels, where coverage is already
  all-or-nothing, so they tile without a gap while the sheet's own silhouette
  keeps its antialiasing. Snapping in device pixels rather than whole user units
  also covers fractional UI scales (1.25, 1.5 on Wayland).

## [0.2.2] — 2026-10-25

GTK frontend release. The repository now also contains the experimental GPUI
frontend (`manga-translator-gpui/`), which is not part of this release.

### Added
- **Spring-animated splitter** — F9 collapses/expands the settings panel with
  `AdwSpringAnimation` instead of snapping; double-clicking a divider resets it
  to its default width
- libadwaita feature level raised to `v1_9`, unlocking `AdwToggleGroup`,
  `AdwWrapBox`, `AdwSidebar`, `AdwViewSwitcherSidebar`
- README: troubleshooting entry for light mode on dark-only system themes

### Changed
- **Dependencies updated** — gtk4 0.11.5, libadwaita 0.9.2, glib/gio 0.22.10,
  gettext-rs 0.8, dirs 7, base64 0.23, hotpath 0.29. Building from source now
  needs GTK 4.22+ and libadwaita 1.9+ (flatpak, AppImage and Windows bundles
  ship their own libraries)
- **Faster start** — the window is presented before the page listing is built
  (~70–80 ms), and the backend availability check is a cheap package probe
  (1.8 s → 0.3 ms) with the expensive import moved to a background warm-up
- Preview images decode off the main thread; thumbnails decode in parallel on
  all cores and use an on-disk cache

### Fixed
- **Light color scheme** only brightened parts of the window when a third-party
  theme pins a dark palette in `~/.config/gtk-4.0/gtk.css` (e.g. Matugen)
- Panel widths are no longer overwritten by GTK's layout clamping during the
  splitter animation or while the settings panel is collapsed
- Non-JSON output from the Python backend no longer corrupts the IPC stream

### Build
- The Flatpak is compiled inside the GNOME 50 SDK (`org.freedesktop.Sdk.Extension.rust-stable`)
  instead of on the build host, so it is built against the GTK 4.22 /
  libadwaita 1.9 that the runtime actually ships. Host images only had 1.5.
- Windows builds use the MSYS2 **UCRT64** environment; MSYS2 retired the
  msvcrt-based `mingw64` environment in March 2026 and no longer publishes
  gtk4/libadwaita for it.
- `appstream-compose` is disabled in the Flatpak manifest. `appstreamcli` in the
  current GNOME 50 SDK loads icons through glycin, which refuses to start inside
  a flatpak-builder sandbox (it spawns its loader through `flatpak-spawn`, and
  `/.flatpak-info` of a *build* sandbox has no `[Application]` group). Metainfo
  and icons are still installed; only the pre-composed catalogue in
  `share/app-info` is skipped. Re-enable once the runtime ships a fix.
- Release assets are named consistently (`MangaTranslator-<version>-<arch>.<ext>`);
  the 4-part Windows version now only appears inside the MSIX manifest.

## [0.2.1] — 2026-05-15

### Added
- **Flatpak build** — GNOME 50 SDK, rustup-based CI, NVIDIA GL extension + ROCm support
  - `--device=all` for CUDA/ROCm GPU compute
  - `org.freedesktop.Platform.GL` extension for NVIDIA driver access
  - Matugen color theme support (`xdg-config/gtk-4.0`, `xdg-data/themes`)
  - App-ID: `com.mangatranslator.gui`
- **AppImage build** — openSUSE Tumbleweed container, manual lib bundling, type2 runtime
- **Windows build** — MSYS2/mingw64, MSIX + ZIP dual packaging, self-signed code signing
- **Hotpath profiler** — 16 instrumented functions, zero-cost when disabled (`--features hotpath`)
- Windows: `#![windows_subsystem = "windows"]` (no CMD flash)
- Windows: `bind_textdomain_codeset("UTF-8")` (fixes gettext crash)
- Windows: `setup_windows_panic_hook()` shows MessageBox on crash

### Changed
- Removed `build-linux.yml` (superseded by `build-appimage.yml` and `build-flatpak.yml`)

## [0.2.0] — 2026-04-15

### Added
- Live UI retranslation for dropdowns and menus on language change
- Italian translation (9 languages total: DE, EN, ES, FR, IT, JA, KO, PT-BR, ZH-CN)

### Changed
- Per-chapter `_Text/` directories for translation storage (replaces global cache)
- VLM: `verbose=False` fix — Pass 2 and MTPE now find `_translations.txt`
- New translation mode: **"Text extrahieren"** (extract) — OCR + translate, no rendering
- "Cache-Modus" renamed to **"Text einfügen"**
- UI sensitivity fixes: file browser and BBox row disabled during translation

### Fixed
- `python_bin` derivation: 3x `.parent()` (was 2x, resolved to `venv/lib/` instead of `venv/`)
- VLM/Cache revealer collapsed on startup even with saved mode

## [0.1.0] — 2026-04-15

### Added
- GTK4/libadwaita GUI with folder-based manga browsing
- Standard, VLM, extract, and insert translation modes
- IPC subprocess backend (Python 3.10+ with any venv)
- Live preview with original/translated comparison slider
- 8 languages: DE, EN, ES, FR, JA, KO, PT-BR, ZH-CN
- Configurable accent colors and theme support
- API key management for translation services
