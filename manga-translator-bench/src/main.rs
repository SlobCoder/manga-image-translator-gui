//! Benchmark harness: gdk-pixbuf vs glycin vs image-rs.
//!
//! Usage:
//!   cargo run --release -- gen     # build the corpus from preview/1.png
//!   cargo run --release -- decode  # decoder matrix
//!   cargo run --release -- thumb   # thumbnail decode + cache encoder matrix
//!   cargo run --release -- all

mod bench;
mod decode;
mod encode;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use bench::{Row, Stats};

const CORPUS: &str = "corpus";
const RESULTS: &str = "results";
const SOURCE: &str = "../preview/1.png";

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    match cmd.as_str() {
        "gen" => gen_corpus(),
        "decode" => {
            let md = run_decode();
            write_results("decoder-bench.md", &md);
        }
        "thumb" => {
            let md = run_thumb();
            write_results("thumbnail-bench.md", &md);
        }
        "all" => {
            gen_corpus();
            let d = run_decode();
            let t = run_thumb();
            write_results("decoder-bench.md", &d);
            write_results("thumbnail-bench.md", &t);
        }
        other => {
            eprintln!("unknown command: {other}");
            std::process::exit(2);
        }
    }
}

fn write_results(name: &str, md: &str) {
    std::fs::create_dir_all(RESULTS).ok();
    let path = Path::new(RESULTS).join(name);
    std::fs::write(&path, md).ok();
    println!("\nwritten to {}", path.display());
}

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------

fn magick() -> &'static str {
    if Command::new("magick").arg("-version").output().is_ok() {
        "magick"
    } else {
        "convert"
    }
}

fn gen_corpus() {
    let bin = magick();
    std::fs::create_dir_all(CORPUS).ok();
    println!("generating corpus with {bin} from {SOURCE}");

    let jobs: &[(&str, &[&str])] = &[
        ("full.png", &[]),
        ("full_q90.jpg", &["-quality", "90"]),
        ("full_q75.jpg", &["-quality", "75"]),
        ("full.webp", &["-quality", "85"]),
        ("full_lossless.webp", &["-define", "webp:lossless=true"]),
        ("full.tiff", &["-compress", "lzw"]),
        ("full.bmp", &[]),
        ("full.gif", &["-colors", "256"]),
        ("full.qoi", &[]),
        ("full.jxl", &["-quality", "90"]),
        ("full.jp2", &["-quality", "90"]),
        ("page.png", &["-crop", "1901x2051+0+0", "+repage"]),
        (
            "page_q90.jpg",
            &["-crop", "1901x2051+0+0", "+repage", "-quality", "90"],
        ),
        (
            "page.webp",
            &["-crop", "1901x2051+0+0", "+repage", "-quality", "85"],
        ),
        ("small.png", &["-resize", "1064x1510"]),
        ("small_q90.jpg", &["-resize", "1064x1510", "-quality", "90"]),
        ("small.webp", &["-resize", "1064x1510", "-quality", "85"]),
        ("crop.exr", &["-crop", "1200x900+0+0", "+repage"]),
    ];

    for (name, args) in jobs {
        let out = PathBuf::from(CORPUS).join(name);
        let mut c = Command::new(bin);
        c.arg(SOURCE).args(*args).arg(&out);
        match c.output() {
            Ok(o) if o.status.success() => {
                let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
                println!("  {:<20} {:>9} KiB", name, size / 1024);
            }
            Ok(o) => eprintln!(
                "  {:<20} FAILED: {}",
                name,
                String::from_utf8_lossy(&o.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ),
            Err(e) => eprintln!("  {name:<20} FAILED: {e}"),
        }
    }
}

fn corpus_files() -> Vec<(String, PathBuf, u64)> {
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir(CORPUS) else {
        return out;
    };
    for entry in dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push((name, path, size));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn ext(name: &str) -> &str {
    name.rsplit_once('.').map(|(_, e)| e).unwrap_or("")
}

fn megapixels(w: u32, h: u32) -> f64 {
    w as f64 * h as f64 / 1e6
}

// ---------------------------------------------------------------------------
// Decode benchmark
// ---------------------------------------------------------------------------

/// Run `f` once to obtain the result, then measure it. `weigh` turns the
/// result into a number so the optimiser cannot drop the work.
fn timed<T>(mut f: impl FnMut() -> Option<T>, weigh: impl Fn(&T) -> usize) -> Option<(Stats, T)> {
    let first = f()?;
    let stats = bench::measure(|| f().map(|v| weigh(&v)).unwrap_or(0));
    Some((stats, first))
}

fn header(title: &str, intro: &str) -> String {
    let rustc = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    format!(
        "# {title}\n\n{}\n\n| | |\n|---|---|\n| CPU | {} |\n| {} | — |\n| gdk-pixbuf | {} |\n| glycin | crate 4.0.5, system loaders 2.2.5 ({}) |\n| image-rs | 0.25 |\n\n",
        intro,
        bench::cpu_model(),
        rustc,
        pkg_version("gdk-pixbuf-2.0"),
        sandbox_note(),
    )
}

fn run_decode() -> String {
    let files = corpus_files();
    let mut out = header(
        "Decode benchmark — gdk-pixbuf vs glycin vs image-rs",
        "Times are wall-clock medians; input comes from disk with a warm page cache.\n\
         `out` is the produced pixel buffer: `bgra8-premult` is texture-ready for GTK/Cairo,\n\
         `rgba8`/`native` are the decoders' own output formats.\n\
         `vs gdk` compares against `gdk-pixbuf → BGRA premult` for the same file.",
    );

    // Cold start: the first glycin decode spawns the sandboxed loader process.
    if let Some((_, path, _)) = files.iter().find(|(n, _, _)| n == "full_q90.jpg") {
        let t = Instant::now();
        let ok = decode::glycin_full(path).is_some();
        let cold = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        let ok2 = decode::gdk_app_bgra_premult(path).is_some();
        let cold_gdk = t.elapsed().as_secs_f64() * 1e3;
        out.push_str(&format!(
            "**Cold start** (first decode in a fresh process, `{}`): glycin **{cold:.0} ms** (ok={ok}), \
             gdk-pixbuf **{cold_gdk:.0} ms** (ok={ok2}).\n\n",
            path.file_name().unwrap().to_string_lossy()
        ));
    }

    let mut rows: Vec<Row> = Vec::new();

    for (name, path, size) in &files {
        let e = ext(name);
        let mut gdk_ms: Option<f64> = None;

        // 1) gdk-pixbuf, current app path (baseline)
        if let Some((stats, dim)) = timed(|| decode::gdk_app_bgra_premult(path), |v| v.2) {
            gdk_ms = Some(stats.median_ms);
            rows.push(Row {
                codec: "gdk-pixbuf → BGRA premult (current)",
                file: name.clone(),
                megapixels: megapixels(dim.0, dim.1),
                input_bytes: *size,
                output_note: "bgra8-premult",
                stats,
                rel_to_gdk: Some(1.0),
            });
        }

        // 2) gdk-pixbuf, decode only
        if let Some((stats, dim)) = timed(|| decode::gdk_raw(path), |_| 0) {
            rows.push(Row {
                codec: "gdk-pixbuf (decode only)",
                file: name.clone(),
                megapixels: megapixels(dim.0, dim.1),
                input_bytes: *size,
                output_note: "pixbuf",
                stats,
                rel_to_gdk: gdk_ms.map(|b| stats.median_ms / b),
            });
        }

        // 3) image-rs
        if let Some((stats, dim)) = timed(|| decode::image_rs_rgba(path), |v| v.2) {
            rows.push(Row {
                codec: "image-rs",
                file: name.clone(),
                megapixels: megapixels(dim.0, dim.1),
                input_bytes: *size,
                output_note: "rgba8",
                stats,
                rel_to_gdk: gdk_ms.map(|b| stats.median_ms / b),
            });
        }

        // 4) image-rs + the same conversion the app does today
        if let Some((stats, dim)) = timed(|| decode::image_rs_app_bgra_premult(path), |v| v.2) {
            rows.push(Row {
                codec: "image-rs → BGRA premult",
                file: name.clone(),
                megapixels: megapixels(dim.0, dim.1),
                input_bytes: *size,
                output_note: "bgra8-premult",
                stats,
                rel_to_gdk: gdk_ms.map(|b| stats.median_ms / b),
            });
        }

        // 5) glycin (sandboxed; premultiplied BGRA straight from the loader)
        if let Some((stats, dim)) = timed(|| decode::glycin_full(path), |v| v.2) {
            rows.push(Row {
                codec: "glycin (sandboxed)",
                file: name.clone(),
                megapixels: megapixels(dim.0, dim.1),
                input_bytes: *size,
                output_note: "bgra8-premult",
                stats,
                rel_to_gdk: gdk_ms.map(|b| stats.median_ms / b),
            });
        }

        // 6) glycin with the format the loader decodes natively (no conversion)
        if let Some((stats, dim)) = timed(|| decode::glycin_full_native(path), |v| v.2) {
            rows.push(Row {
                codec: "glycin (native format)",
                file: name.clone(),
                megapixels: megapixels(dim.0, dim.1),
                input_bytes: *size,
                output_note: "native",
                stats,
                rel_to_gdk: gdk_ms.map(|b| stats.median_ms / b),
            });
        }

        // 7) the format-specific crate from the glycin loader set
        let specific: Option<(&'static str, fn(&Path) -> Option<(u32, u32, usize)>)> = match e {
            "png" => Some(("png crate", decode::png_crate as _)),
            "jpg" | "jpeg" => Some(("zune-jpeg", decode::zune_jpeg as _)),
            "webp" => Some(("image-webp", decode::image_webp as _)),
            "gif" => Some(("gif crate", decode::gif_crate as _)),
            "tiff" => Some(("tiff crate", decode::tiff_crate as _)),
            "bmp" => Some(("bmp crate", decode::bmp_crate as _)),
            "qoi" => Some(("qoi crate", decode::qoi_crate as _)),
            "exr" => Some(("exr crate", decode::exr_crate as _)),
            "jp2" => Some(("hayro-jpeg2000", decode::jp2_crate as _)),
            _ => None,
        };
        if let Some((codec, f)) = specific {
            if let Some((stats, dim)) = timed(|| f(path), |v| v.2) {
                rows.push(Row {
                    codec,
                    file: name.clone(),
                    megapixels: megapixels(dim.0, dim.1),
                    input_bytes: *size,
                    output_note: "native",
                    stats,
                    rel_to_gdk: gdk_ms.map(|b| stats.median_ms / b),
                });
            }
        }
    }

    out.push_str(&bench::print_decode_table("Decode", &rows));
    out
}

fn pkg_version(pkg: &str) -> String {
    Command::new("pkg-config")
        .args(["--modversion", pkg])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "?".into())
}

fn sandbox_note() -> String {
    if Path::new("/usr/bin/bwrap").exists() {
        "bwrap present".into()
    } else {
        "bwrap MISSING".into()
    }
}

// ---------------------------------------------------------------------------
// Thumbnail + encoder benchmark
// ---------------------------------------------------------------------------

fn run_thumb() -> String {
    let mut out = header(
        "Thumbnail decode + cache encoding",
        "Thumbnails are 112 px (the size the file browser requests) decoded from the full-size page.",
    );

    let candidates = [
        "full.png",
        "full_q90.jpg",
        "full.webp",
        "page_q90.jpg",
        "small_q90.jpg",
    ];

    let mut rows: Vec<Row> = Vec::new();
    for name in candidates {
        let path = Path::new(CORPUS).join(name);
        if !path.exists() {
            continue;
        }
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        let mut gdk_ms = None;
        if let Some((stats, first)) = timed(|| decode::gdk_thumb(&path, 112), |v| v.2) {
            gdk_ms = Some(stats.median_ms);
            rows.push(Row {
                codec: "gdk-pixbuf from_file_at_size",
                file: name.into(),
                megapixels: megapixels(first.0, first.1),
                input_bytes: size,
                output_note: "png bytes",
                stats,
                rel_to_gdk: Some(1.0),
            });
        }
        if let Some((stats, first)) = timed(|| decode::gdk_thumb_pixels(&path, 112), |v| v.2) {
            rows.push(Row {
                codec: "gdk-pixbuf from_file_at_size (no PNG)",
                file: name.into(),
                megapixels: megapixels(first.0, first.1),
                input_bytes: size,
                output_note: "pixbuf",
                stats,
                rel_to_gdk: gdk_ms.map(|b| stats.median_ms / b),
            });
        }
        if let Some((stats, first)) = timed(|| decode::image_rs_thumb(&path, 112), |v| v.2) {
            rows.push(Row {
                codec: "image-rs thumbnail",
                file: name.into(),
                megapixels: megapixels(first.0, first.1),
                input_bytes: size,
                output_note: "rgba8",
                stats,
                rel_to_gdk: gdk_ms.map(|b| stats.median_ms / b),
            });
        }
        if let Some((stats, first)) = timed(|| decode::glycin_thumb(&path, 112), |v| v.2) {
            rows.push(Row {
                codec: "glycin scale(112) → BGRA premult",
                file: name.into(),
                megapixels: megapixels(first.0, first.1),
                input_bytes: size,
                output_note: "bgra8-premult",
                stats,
                rel_to_gdk: gdk_ms.map(|b| stats.median_ms / b),
            });
        }
        if let Some((stats, first)) = timed(|| decode::glycin_thumb_native(&path, 112), |v| v.2) {
            rows.push(Row {
                codec: "glycin scale(112) native",
                file: name.into(),
                megapixels: megapixels(first.0, first.1),
                input_bytes: size,
                output_note: "native",
                stats,
                rel_to_gdk: gdk_ms.map(|b| stats.median_ms / b),
            });
        }
    }
    out.push_str(&bench::print_decode_table(
        "Thumbnail decode (full page → 112 px)",
        &rows,
    ));

    // --- encoders ---------------------------------------------------------
    let src = Path::new(CORPUS).join("small.png");
    let img = image::ImageReader::open(&src)
        .ok()
        .and_then(|r| r.with_guessed_format().ok())
        .and_then(|r| r.decode().ok());

    let Some(img) = img else {
        out.push_str("\n(corpus/small.png missing — encoder section skipped)\n");
        return out;
    };

    let thumb = img.thumbnail(112, 112).to_rgba8();
    let page = img.to_rgba8();

    out.push_str("\nThumbnail cache encoders, 112×112 RGBA:\n");
    out.push_str(&encode_section(
        thumb.as_raw(),
        thumb.width(),
        thumb.height(),
    ));
    out.push_str("\nPage encoders, 1064×1510 RGBA:\n");
    out.push_str(&encode_section(page.as_raw(), page.width(), page.height()));

    out
}

fn encode_section(rgba: &[u8], w: u32, h: u32) -> String {
    let mut rows = Vec::new();
    for (name, f) in encode::all() {
        let Some(first) = f(rgba, w, h) else {
            continue;
        };
        let stats = bench::measure(|| f(rgba, w, h).map(|v| v.len()).unwrap_or(0));
        rows.push(bench::EncRow {
            encoder: name,
            output_bytes: first.len(),
            stats,
        });
    }
    bench::print_encode_table("", megapixels(w, h), &rows)
}
