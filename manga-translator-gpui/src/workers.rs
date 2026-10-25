// manga-translator-gpui/src/workers.rs
//
// Two background workers keep the GPUI thread free:
//
//   * `render`  — filesystem scanning plus image decoding (thumbnails,
//                 previews). Pure Rust, independent of the Python backend.
//   * `bridge`  — the IPC bridge. `IpcBridge::translate` blocks until the
//                 backend is done, so it must never run on the UI thread.
//
// Both feed a single result channel.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use async_channel::Sender;
use manga_translator_core::config::ConfigManager;
use manga_translator_core::ipc_bridge::{IpcBridge, TranslationParams};

use crate::theme::sizes;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp", "tiff", "tif", "webp", "gif"];

#[derive(Debug, Clone)]
pub struct FolderItem {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ImageItem {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
}

pub struct Job {
    pub files: Vec<PathBuf>,
    pub params: TranslationParams,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

pub enum RenderCmd {
    LoadFolder {
        generation: u64,
        path: PathBuf,
        sort_method: u32,
    },
}

pub enum BridgeCmd {
    /// Start the Python backend without translating (warm-up / status check).
    EnsureReady,
    Translate(Box<Job>),
}

pub enum Out {
    Folder {
        generation: u64,
        path: PathBuf,
        folders: Vec<FolderItem>,
        images: Vec<ImageItem>,
    },
    Thumb {
        generation: u64,
        path: PathBuf,
        png: Vec<u8>,
    },
    Preview {
        path: PathBuf,
        png: Vec<u8>,
    },
    Progress {
        fraction: f64,
        message: String,
    },
    Backend {
        ok: bool,
        message: String,
    },
    Finished {
        ok: bool,
        message: String,
        outputs: Vec<PathBuf>,
    },
}

// ---------------------------------------------------------------------------
// Render worker
// ---------------------------------------------------------------------------

pub fn spawn_render_worker(out: Sender<Out>, generation: Arc<Generation>) -> Sender<RenderCmd> {
    let (tx, rx) = async_channel::unbounded::<RenderCmd>();

    std::thread::Builder::new()
        .name("mt-render".into())
        .spawn(move || {
            while let Ok(cmd) = rx.recv_blocking() {
                match cmd {
                    RenderCmd::LoadFolder {
                        generation: gen_id,
                        path,
                        sort_method,
                    } => {
                        let started = Instant::now();
                        let (folders, images) = scan_folder(&path, sort_method);
                        if out
                            .send_blocking(Out::Folder {
                                generation: gen_id,
                                path: path.clone(),
                                folders,
                                images: images.clone(),
                            })
                            .is_err()
                        {
                            return;
                        }

                        let batch_size =
                            thumbnail_parallelism(images.first().map(|image| image.path.as_path()));
                        let mut sent = 0usize;
                        let mut cached = 0usize;
                        let mut start = 0usize;

                        while start < images.len() {
                            // Stop decoding as soon as the user navigated away.
                            if generation.current() != gen_id {
                                break;
                            }

                            let end = (start + batch_size).min(images.len());
                            let batch = &images[start..end];

                            // Decode the batch on all cores, then report it in
                            // index order so the list still fills top to bottom.
                            let mut results = Vec::with_capacity(batch.len());
                            std::thread::scope(|scope| {
                                let handles: Vec<_> = batch
                                    .iter()
                                    .map(|image| {
                                        let path = image.path.as_path();
                                        scope.spawn(move || thumbnail_for_cache(path))
                                    })
                                    .collect();

                                for handle in handles {
                                    results.push(handle.join().unwrap_or(None));
                                }
                            });

                            for (image, result) in batch.iter().zip(results) {
                                match result {
                                    Some((png, from_cache)) => {
                                        if from_cache {
                                            cached += 1;
                                        }
                                        if out
                                            .send_blocking(Out::Thumb {
                                                generation: gen_id,
                                                path: image.path.clone(),
                                                png,
                                            })
                                            .is_err()
                                        {
                                            return;
                                        }
                                        sent += 1;
                                    }
                                    None => {
                                        log::debug!("thumbnail failed: {}", image.path.display())
                                    }
                                }
                            }

                            start = end;
                        }

                        log::info!(
                            "thumbnails: {}/{} pages in {:?} ({} threads, {} from cache)",
                            sent,
                            images.len(),
                            started.elapsed(),
                            batch_size,
                            cached
                        );
                    }
                }
            }
        })
        .expect("failed to spawn render worker");

    tx
}

/// How many pages are decoded at the same time.
///
/// Uses every available core, limited only by a memory budget: each in-flight
/// decode holds the full-size image (6 MB for a 1064x1510 page, ~64 MB for a
/// long webtoon strip), so a fixed "all cores" rule would spike RAM on large
/// pages. The size of the first page is read from its header to estimate the
/// per-page cost — parsing headers is microseconds and needs no decode.
///
/// Override with `MT_THUMB_THREADS` for tuning.
fn thumbnail_parallelism(sample: Option<&Path>) -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    if let Some(override_value) = std::env::var("MT_THUMB_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        return override_value.clamp(1, cores);
    }

    const BUDGET: usize = 512 * 1024 * 1024;
    let per_page = sample.map(decoded_bytes).unwrap_or(8 * 1024 * 1024);
    (BUDGET / per_page.max(1)).clamp(1, cores)
}

/// Estimate the decoded size of an image from its header, without decoding it.
fn decoded_bytes(path: &Path) -> usize {
    pixel_dimensions(path)
        .map(|(width, height)| width as usize * height as usize * 4)
        .unwrap_or(8 * 1024 * 1024)
}

fn pixel_dimensions(path: &Path) -> Option<(u32, u32)> {
    let file = std::fs::File::open(path).ok()?;
    image::ImageReader::new(std::io::BufReader::new(file))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

// ---------------------------------------------------------------------------
// Preview worker
// ---------------------------------------------------------------------------

/// Dedicated thread for full-size previews.
///
/// Previews must not queue behind the thumbnail batch of a 200-page folder, so
/// they get their own thread. Requests are matched by path in the UI, which
/// makes stale results harmless.
pub fn spawn_preview_worker(out: Sender<Out>) -> Sender<PathBuf> {
    let (tx, rx) = async_channel::unbounded::<PathBuf>();

    std::thread::Builder::new()
        .name("mt-preview".into())
        .spawn(move || {
            while let Ok(path) = rx.recv_blocking() {
                if let Some(png) = make_preview(&path) {
                    if out.send_blocking(Out::Preview { path, png }).is_err() {
                        return;
                    }
                }
            }
        })
        .expect("failed to spawn preview worker");

    tx
}

/// Incrementing counter used to discard stale render results.
pub struct Generation(AtomicU64);

impl Generation {
    pub fn new() -> Self {
        Self(AtomicU64::new(0))
    }
    /// Allocate a new generation number.
    pub fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }
    /// The generation number of the most recent request.
    pub fn current(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

fn scan_folder(path: &Path, sort_method: u32) -> (Vec<FolderItem>, Vec<ImageItem>) {
    let mut folders = Vec::new();
    let mut images = Vec::new();

    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("cannot read {}: {}", path.display(), e);
            return (folders, images);
        }
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if name.starts_with('.') {
            continue;
        }

        match entry.file_type() {
            Ok(t) if t.is_dir() => folders.push(FolderItem {
                path: entry_path,
                name,
            }),
            Ok(t) if t.is_file() => {
                let ext = entry_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    images.push(ImageItem {
                        path: entry_path,
                        name,
                        size,
                    });
                }
            }
            _ => {}
        }
    }

    folders.sort_by(|a, b| alphanumeric_sort::compare_str(&a.name, &b.name));
    sort_images(&mut images, sort_method);

    (folders, images)
}

fn sort_images(images: &mut [ImageItem], sort_method: u32) {
    match sort_method {
        2 => images.sort_by(|a, b| {
            let ma = std::fs::metadata(&a.path).and_then(|m| m.modified()).ok();
            let mb = std::fs::metadata(&b.path).and_then(|m| m.modified()).ok();
            mb.cmp(&ma)
        }),
        0 => images.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        _ => images.sort_by(|a, b| alphanumeric_sort::compare_str(&a.name, &b.name)),
    }
}

/// Decode an image by sniffing its content rather than trusting the extension.
///
/// The Python backend writes PNG data into files that keep the source name
/// (e.g. `0001.jpg`), so extension-based detection would fail for every
/// translated page.
fn load_image(path: &Path) -> Option<image::DynamicImage> {
    let file = std::fs::File::open(path).ok()?;
    let reader = image::ImageReader::new(std::io::BufReader::new(file))
        .with_guessed_format()
        .ok()?;
    reader.decode().ok()
}

fn encode_png(image: &image::DynamicImage) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    image.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
    Some(buffer)
}

/// Decode a thumbnail, reusing the on-disk cache when possible.
///
/// Returns the PNG bytes and whether they came from the cache. The cache is
/// shared with the GTK frontend: same directory, same key (path + mtime),
/// same 112x112 fit — so a chapter thumbnailed by either frontend is instant
/// in the other one.
fn thumbnail_for_cache(path: &Path) -> Option<(Vec<u8>, bool)> {
    if let Some(cached) = load_cached_thumbnail(path) {
        return Some((cached, true));
    }

    let image = load_image(path)?;
    let thumb = image.thumbnail(sizes::THUMB_FIT, sizes::THUMB_FIT);
    let png = encode_png(&thumb)?;
    save_cached_thumbnail(path, &png);
    Some((png, false))
}

// ---------------------------------------------------------------------------
// Thumbnail disk cache (shared with the GTK frontend)
// ---------------------------------------------------------------------------

/// `~/.cache/manga-translator/thumbnails`, identical to the GTK frontend.
fn thumbnail_cache_dir() -> PathBuf {
    ConfigManager::cache_dir().join("thumbnails")
}

/// Cache file for a source image: hash of absolute path + mtime (seconds).
///
/// Deliberately byte-for-byte the same key the GTK frontend computes, so both
/// frontends hit each other's entries. A modified source file gets a new
/// mtime and therefore a new key.
fn thumbnail_cache_path(source: &Path) -> Option<PathBuf> {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let mtime = std::fs::metadata(source)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())?;

    let mut hasher = DefaultHasher::new();
    source.to_string_lossy().hash(&mut hasher);
    mtime.hash(&mut hasher);

    Some(thumbnail_cache_dir().join(format!("{:016x}.png", hasher.finish())))
}

fn load_cached_thumbnail(source: &Path) -> Option<Vec<u8>> {
    let cache_path = thumbnail_cache_path(source)?;
    std::fs::read(&cache_path)
        .ok()
        .filter(|bytes| !bytes.is_empty())
}

fn save_cached_thumbnail(source: &Path, png: &[u8]) {
    let Some(cache_path) = thumbnail_cache_path(source) else {
        return;
    };
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Write through a unique temporary file: the cache is shared with another
    // process, which must never read a half-written PNG.
    let temp = cache_path.with_extension(format!("png.{}.tmp", std::process::id()));
    if std::fs::write(&temp, png).is_ok() {
        let _ = std::fs::rename(&temp, &cache_path);
    } else {
        let _ = std::fs::remove_file(&temp);
    }
}

fn make_preview(path: &Path) -> Option<Vec<u8>> {
    let image = load_image(path)?;
    let max = sizes::PREVIEW_MAX;
    let preview = if image.width() > max || image.height() > max {
        image.resize(max, max, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    encode_png(&preview)
}

// ---------------------------------------------------------------------------
// Bridge worker
// ---------------------------------------------------------------------------

pub fn spawn_bridge_worker(
    bridge: Arc<IpcBridge>,
    cancel: Arc<AtomicBool>,
    out: Sender<Out>,
) -> Sender<BridgeCmd> {
    let (tx, rx) = async_channel::unbounded::<BridgeCmd>();

    std::thread::Builder::new()
        .name("mt-bridge".into())
        .spawn(move || {
            while let Ok(cmd) = rx.recv_blocking() {
                match cmd {
                    BridgeCmd::EnsureReady => {
                        let message = match bridge.ensure_initialized() {
                            // The backend answers this without importing the
                            // model stack, so it stays cheap.
                            Ok(()) => Out::Backend {
                                ok: bridge.is_backend_available(),
                                message: String::new(),
                            },
                            Err(e) => Out::Backend {
                                ok: false,
                                message: format!("{}", e),
                            },
                        };
                        if out.send_blocking(message).is_err() {
                            return;
                        }
                    }
                    BridgeCmd::Translate(job) => {
                        cancel.store(false, Ordering::SeqCst);

                        let progress_out = out.clone();
                        let result = bridge.translate(
                            &job.files,
                            &job.params,
                            &cancel,
                            Box::new(move |fraction, message| {
                                let _ = progress_out.send_blocking(Out::Progress {
                                    fraction,
                                    message: message.to_string(),
                                });
                            }),
                        );

                        let msg = match result {
                            Ok(outputs) => Out::Finished {
                                ok: true,
                                message: format!("{} Seite(n) übersetzt", outputs.len()),
                                outputs,
                            },
                            Err(e) => Out::Finished {
                                ok: false,
                                message: format!("{}", e),
                                outputs: Vec::new(),
                            },
                        };
                        if out.send_blocking(msg).is_err() {
                            return;
                        }
                    }
                }
            }
        })
        .expect("failed to spawn bridge worker");

    tx
}
