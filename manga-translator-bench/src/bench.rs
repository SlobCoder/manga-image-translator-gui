//! Timing harness and Markdown table output.

use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub min_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
}

/// Warm up three times, then take samples until ~1.2 s have elapsed
/// (at least 7, at most 40 samples).
pub fn measure<T>(mut f: impl FnMut() -> T) -> Stats {
    for _ in 0..3 {
        std::hint::black_box(f());
    }

    let mut times = Vec::with_capacity(40);
    let start = Instant::now();
    while times.len() < 40 && (times.len() < 7 || start.elapsed().as_secs_f64() < 1.2) {
        let t = Instant::now();
        let out = f();
        times.push(t.elapsed().as_secs_f64() * 1e3);
        std::hint::black_box(out);
        if start.elapsed().as_secs_f64() > 20.0 {
            break;
        }
    }

    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = times.len();
    let p95 = times[((n as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(n - 1)];
    Stats {
        min_ms: times[0],
        median_ms: times[n / 2],
        p95_ms: p95,
    }
}

/// One measured (decoder, file) pair.
pub struct Row {
    pub codec: &'static str,
    pub file: String,
    pub megapixels: f64,
    pub input_bytes: u64,
    pub output_note: &'static str,
    pub stats: Stats,
    /// Median time relative to `gdk-pixbuf → BGRA premult` for the same file.
    pub rel_to_gdk: Option<f64>,
}

fn mpix_s(row: &Row) -> f64 {
    row.megapixels / (row.stats.median_ms / 1e3)
}

fn mb_s(row: &Row) -> f64 {
    row.input_bytes as f64 / 1e6 / (row.stats.median_ms / 1e3)
}

pub fn print_decode_table(title: &str, rows: &[Row]) -> String {
    let mut s = String::new();
    s.push_str(&format!("\n### {title}\n\n"));
    s.push_str(
        "| Decoder | File | Input | out | med ms | min ms | p95 ms | MPix/s | in MB/s | vs gdk |\n",
    );
    s.push_str("|---|---|---:|---|---:|---:|---:|---:|---:|---:|\n");

    for r in rows {
        let rel = match r.rel_to_gdk {
            Some(x) => format!("{:+.0} %", (x - 1.0) * 100.0),
            None => "—".to_string(),
        };
        s.push_str(&format!(
            "| {} | {} | {:.2} MB | {} | {:.2} | {:.2} | {:.2} | {:.1} | {:.1} | {} |\n",
            r.codec,
            r.file,
            r.input_bytes as f64 / 1e6,
            r.output_note,
            r.stats.median_ms,
            r.stats.min_ms,
            r.stats.p95_ms,
            mpix_s(r),
            mb_s(r),
            rel
        ));
    }
    s
}

pub struct EncRow {
    pub encoder: &'static str,
    pub output_bytes: usize,
    pub stats: Stats,
}

pub fn print_encode_table(title: &str, pixels: f64, rows: &[EncRow]) -> String {
    let mut s = String::new();
    s.push_str(&format!("\n### {title}\n\n"));
    s.push_str("| Encoder | size | med ms | MPix/s | vs png Fast |\n");
    s.push_str("|---|---:|---:|---:|---:|\n");
    // baseline: first row (png Fast), the realistic cache format
    let fastest = rows.first().map(|r| r.stats.median_ms).unwrap_or(1.0);
    for r in rows {
        s.push_str(&format!(
            "| {} | {:.1} KiB | {:.3} | {:.1} | {:.1}x |\n",
            r.encoder,
            r.output_bytes as f64 / 1024.0,
            r.stats.median_ms,
            pixels / (r.stats.median_ms / 1e3),
            r.stats.median_ms / fastest
        ));
    }
    s
}

pub fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
        })
        .unwrap_or_else(|| "unknown CPU".into())
}
