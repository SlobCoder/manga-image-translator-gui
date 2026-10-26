//! Breakdown of glycin's per-image cost: metadata load, frame transfer,
//! requested memory format, sandbox on/off.
use std::path::PathBuf;
use std::time::Instant;

fn run(
    path: &str,
    formats: glycin::MemoryFormatSelection,
    sandboxed: bool,
) -> Option<(f64, f64, String)> {
    let mut loader = glycin::Loader::new(gio::File::for_path(path));
    loader.accepted_memory_formats(formats);
    if !sandboxed {
        loader.sandbox_selector(glycin::SandboxSelector::NotSandboxed);
    }

    let t0 = Instant::now();
    let mut image = futures_lite::future::block_on(loader.load()).ok()?;
    let load_ms = t0.elapsed().as_secs_f64() * 1e3;

    let t1 = Instant::now();
    let frame = futures_lite::future::block_on(image.next_frame()).ok()?;
    let frame_ms = t1.elapsed().as_secs_f64() * 1e3;

    Some((
        load_ms,
        frame_ms,
        format!(
            "{:?} {}x{}",
            frame.memory_format(),
            frame.width(),
            frame.height()
        ),
    ))
}

fn bench(label: &str, path: &str, formats: glycin::MemoryFormatSelection, sandboxed: bool) {
    // warm up
    let _ = run(path, formats, sandboxed);
    let _ = run(path, formats, sandboxed);

    let mut loads = Vec::new();
    let mut frames = Vec::new();
    let mut info = String::new();
    for _ in 0..7 {
        if let Some((l, f, i)) = run(path, formats, sandboxed) {
            loads.push(l);
            frames.push(f);
            info = i;
        }
    }
    loads.sort_by(|a, b| a.partial_cmp(b).unwrap());
    frames.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "{:<28} load {:>6.2} ms  frame {:>6.2} ms  total {:>6.2} ms  [{}]",
        label,
        loads[loads.len() / 2],
        frames[frames.len() / 2],
        loads[loads.len() / 2] + frames[frames.len() / 2],
        info
    );
}

fn main() {
    for file in ["corpus/full_q90.jpg", "corpus/full.png", "corpus/full.webp"] {
        if !PathBuf::from(file).exists() {
            println!("missing {file}");
            continue;
        }
        println!("\n== {file} ==");
        use glycin::MemoryFormatSelection as F;
        bench("RGBA8", file, F::R8g8b8a8, true);
        bench("BGRA8 premult", file, F::B8g8r8a8Premultiplied, true);
        bench(
            "BGRA8 premult, no sandbox",
            file,
            F::B8g8r8a8Premultiplied,
            false,
        );
        bench("any format", file, F::all(), true);
    }
}
