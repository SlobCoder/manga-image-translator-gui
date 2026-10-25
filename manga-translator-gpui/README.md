# manga-translator-gpui

GPU-accelerated frontend for the manga-image-translator backend, built with
[GPUI](https://gpui.rs) — the UI framework behind the Zed editor.

It is a sibling of the GTK4 frontend in `manga-translator-gtk/`. Both talk to the
same Python backend (`manga-translator-gtk/backend/server.py`) and both read the
same configuration file, so you can switch between them at will.

## Running

```bash
cargo run --release                       # last used folder
cargo run --release -- ~/Manga/Chapter\ 1 # explicit folder
```

Linux needs a working Vulkan (or OpenGL via `blade`) driver as well as Wayland
or X11. The binary is otherwise self-contained: no GTK, no libadwaita.

## Layout

```
┌───────────────────────────────────────────────────────────────┐
│ toolbar   path · ↑ parent · ⌂ manga · ↻ reload · Übersetzen   │
┌───────────────┬───────────────────────────────┬───────────────┐
│ folders       │                               │ translation   │
│ pages         │   page preview / comparison   │ detection     │
│  + filter     │   original · translated ·     │ inpainting    │
│               │   draggable slider            │ VLM / render  │
├───────────────┴───────────────────────────────┴───────────────┤
│ backend pill · status · page n/m · progress bar · %           │
└───────────────────────────────────────────────────────────────┘
```

## Threading model

GPUI's `Context::spawn` only carries non-blocking work, so all heavy work runs on
two dedicated std threads that report back through one `async_channel`:

| Worker | Responsibilities |
|---|---|
| `mt-render` | directory scan, thumbnail decoding (all cores, memory-budgeted) |
| `mt-preview` | full-page decoding and downscaling for the preview pane |
| `mt-bridge` | `IpcBridge::ensure_initialized` and `IpcBridge::translate` |

`Generation` counters tag every folder scan so stale results are dropped when
the user navigates faster than the decoder can keep up — the render worker also
stops decoding a cancelled batch mid-way. Previews have their own thread so a
page opens immediately instead of queueing behind 200 thumbnails; their results
are matched by path.

Thumbnails are decoded in batches on `std::thread::scope` and reported in index
order, so the list still fills top to bottom. All cores are used; the only limit
is a 512 MB budget for in-flight decodes, derived from the first page's header
dimensions, so folders of large webtoon strips don't exhaust memory. Override
with `MT_THUMB_THREADS` when tuning.

Before decoding, each page is looked up in the **thumbnail disk cache** that is
shared with the GTK frontend. A 217-page chapter therefore takes 6 ms when warm
versus 220 ms when cold (32 cores, measured).

## Notable implementation details

- **Shared thumbnail cache** — `~/.cache/manga-translator/thumbnails/<hash>.png`,
  using the GTK frontend's exact key (hash of absolute path + mtime in seconds)
  and exact geometry (fit into 112x112). A chapter thumbnailed by either
  frontend is instant in the other. Writes go through a per-process temp file
  plus rename, so a concurrently running GTK instance never reads a truncated
  PNG.
- **Resizable panels** — the two dividers are draggable (double click resets
  them). No layout library is needed for this: GPUI already lays out with Taffy
  internally, and a divider only changes a `width` value in the entity state.
  The drag reuses the window-level mouse listeners described below, so it keeps
  working while the pointer moves quickly. Widths are stored in the same
  `paned_position` / `right_paned_position` config keys the GTK frontend uses,
  so a layout chosen in one frontend is honoured by the other.
- **Comparison slider** — a `canvas` element paints both pages into the same
  fitted rectangle and clips the translated one with a content mask, so the
  split is pixel-exact and survives resizes and different source resolutions.
  Dragging is handled by window-level mouse listeners, which keeps working when
  the pointer leaves the image. The right-hand page is discovered from the
  session's translation results or from the backend's `<folder>-translated`
  sibling directory, so earlier runs can be compared too.
- **Text input** — GPUI has none, so `ui/text_input.rs` adapts the official
  `examples/input.rs`: full `EntityInputHandler` (IME, dead keys, marked text),
  grapheme-aware cursor movement, selection and clipboard.
- **Selects** — implemented as inline expanding lists rather than floating
  popups, which avoids anchoring/clipping problems inside the scrollable
  settings panel and keeps keyboard focus predictable.
- **Thumbnails are decoded in Rust**, not via the Python backend, so browsing
  works while the backend is still starting and folder changes stay instant.
  Formats are sniffed from the content: the backend stores PNG data in files
  that keep the source's `.jpg` name.
- **Settings persist per keystroke** where a text field is concerned (the JSON
  file is tiny) and immediately for every other control.
- **`vendor/xattr-shim`** exists because `gpui` 0.2.2 → `zed-async-tar` →
  `xattr 0.2.3` references `libc::ENOATTR`, which current `libc` releases no
  longer define. The shim re-exports `xattr 1.x` under the `0.2.3` version that
  the dependency graph requires. Delete it once gpui ships a newer
  `zed-async-tar`.

## Not implemented yet

- Grid view / keyboard navigation between pages
- Log viewer, colour picker and shortcuts dialogs
- Fetching the OpenRouter model list from the API (the current list comes from
  the cached config value)
