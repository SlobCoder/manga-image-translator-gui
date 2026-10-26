# manga-translator-bench

Decode/encode benchmark for the question: should the GTK frontend replace
`gdk-pixbuf` with [`glycin`](https://crates.io/crates/glycin)?

Everything is measured in one process, single-threaded (`taskset -c 4`), with a
warm page cache and 3 warm-up + 7–40 timed iterations per case. Input images are
real manga pages from `../preview/1.png` (3802×2051).

## Run

```sh
cargo run --release -- gen      # build corpus/ from preview/1.png (ImageMagick)
cargo run --release -- decode   # decoder matrix      -> results/decoder-bench.md
cargo run --release -- thumb    # thumbnails+encoders -> results/thumbnail-bench.md
cargo run --release -- all

cargo run --release --bin profile   # glycin cost breakdown (format/sandbox)
```

`corpus/` and `target/` are not tracked.

## Environment of the recorded run

| | |
|---|---|
| CPU | AMD Ryzen 9 7950X3D (16C/32T), pinned to one core |
| rustc | 1.99.0 |
| gdk-pixbuf | 2.44.8 (system) |
| glycin | crate 4.0.5, system loaders 2.2.5 (external, bwrap) |
| image-rs | 0.25.10 |

The loaders are the distribution's 2.2.5 binaries: glycin 4.0.5 asks for
protocol `2+`, which they implement. (The 4.x loader binaries are not packaged
on Arch/CachyOS yet.) A cross-check against `glycin-image-rs` built from
current git produced the same numbers, so the findings are not a 2.2.5 artifact.

## Key results — full spread, 3802×2051 (7.8 MPix)

| Path | JPEG q90 | PNG | WebP q85 |
|---|---:|---:|---:|
| **gdk-pixbuf → BGRA premult** (current) | **35.5 ms** | **54.3 ms** | **120.6 ms** |
| gdk-pixbuf, decode only | 28.4 | 46.8 | 111.7 |
| image-rs → BGRA premult | 26.8 | 36.5 | 117.9 |
| glycin, native format | 29.2 | 47.9 | 112.9 |
| glycin, native + own conversion | ~36 | ~55 | ~118 |
| glycin, asking for `B8g8r8a8Premultiplied` | 153.6 | 182.7 | 248.4 |
| zune-jpeg / png crate / image-webp | 15.8 | 29.2 | 112.3 |

## Key results — single page, 1064×1510 (1.6 MPix)

| Path | JPEG q90 | PNG |
|---|---:|---:|
| gdk-pixbuf → BGRA premult (current) | 3.8 ms | 5.2 ms |
| image-rs → BGRA premult | 2.4 ms | 2.8 ms |
| glycin, native | 3.2 ms | 4.7 ms |
| glycin, asking for BGRA premult | 13.0 ms | 15.3 ms |

Thumbnails (full page → 112 px):

| Path | JPEG 3802² | PNG 3802² | WebP 3802² |
|---|---:|---:|---:|
| gdk-pixbuf `from_file_at_size` | 54.7 ms | 65.4 ms | 131.0 ms |
| image-rs `thumbnail` | 22.0 ms | 35.7 ms | 118.6 ms |
| glycin `FrameRequest::scale(112)` native | 28.7 ms | 47.2 ms | 111.8 ms |
| glycin `scale(112)` → BGRA premult | 160.7 ms | 181.8 ms | 247.1 ms |

## Encoders for the thumbnail cache (112×112 RGBA)

| Encoder | size | median | vs png Fast |
|---|---:|---:|---:|
| png `Compression::Fast` (fdeflate) | 12.6 KiB | 0.023 ms | 1.0× |
| png `Balanced` | 9.0 KiB | 0.593 ms | 25.6× |
| png `High` | 9.0 KiB | 1.146 ms | 49.5× |
| image PNG | 12.6 KiB | 0.024 ms | 1.0× |
| image JPEG q85 | 3.5 KiB | 0.108 ms | 4.7× |
| image-webp lossless | 7.2 KiB | 0.041 ms | 1.8× |
| gif (speed 30) | 5.5 KiB | 0.234 ms | 10.1× |
| qoi | 9.3 KiB | 0.010 ms | 0.4× |
| tiff (deflate) | 8.9 KiB | 0.395 ms | 17.1× |
| memcpy (raw BGRA, no encoding) | 26.2 KiB | ~0.000 ms | — |

## Findings

1. **glycin's format conversion is the bottleneck, not the sandbox.** Letting
   the loader return its native format costs the same as `image-rs`; asking for
   an alpha format (`R8G8B8A8`, `B8G8R8A8Premultiplied`) costs ~16 ns/pixel —
   more than the whole JPEG decode. Sandbox on/off differs by <1 %.
   → Always request the native format and convert in-process (as `preview.rs`
   already does today, ~6 ms per 7.8 MPix).
2. **Sandboxing and IPC are cheap.** Warm `load()` is 0.4–1.9 ms; the cold start
   (spawn loader + bwrap) is ~13 ms once per process, 166 ms for the very first
   image in a fresh process.
3. **gdk-pixbuf is not the slow part.** `image-rs` beats it by 25–45 %
   end-to-end; `glycin` (used correctly) is on par with it. The 60-line BGRA
   premultiply loop in `preview.rs` is ~19 % of the total and would disappear
   with a decoder that returns premultiplied BGRA natively — glycin does that
   only at a 4× penalty.
4. **Thumbnails: glycin's `scale()` and image-rs' `thumbnail()` beat
   `from_file_at_size`** for large pages (29 vs 54 ms for a 7.8 MPix JPEG,
   47 vs 65 ms for PNG).
5. **The cache encoder matters more than the decoder.**
   `png::Compression::Fast` is 25× faster than `Balanced` for 40 % larger files;
   writing raw BGRA is another 20× faster and 2× larger.

## glycin features

The benchmark uses `glycin = "4.0.5"` with its default features, i.e. only
`async-io` (`cargo tree -e features -i glycin`). Neither `gdk4` nor `tokio` was
enabled. Re-running the same decode with those features changes nothing:

| glycin features | JPEG native | PNG native | JPEG RGBA8 | JPEG BGRA premult |
|---|---:|---:|---:|---:|
| `async-io` (default, used here) | 27.95 ms | 46.02 ms | 152.04 ms | 155.07 ms |
| `+ gdk4`, `Frame::texture()` | 28.00 | 45.43 | 153.24 | 154.59 |
| `tokio` instead of `async-io` | 27.66 | 45.58 | 151.67 | 154.56 |

`Frame::texture()` only wraps the existing buffer in a `gdk::MemoryTexture`
(with a colour state, sRGB fallback); it does not convert pixels. It adds
`gdk4` 0.11.5 to the dependency graph (matching the GTK frontend's version, but
requiring GTK >= 4.16 at build time). With the `tokio` feature, glycin must be
called from inside a tokio runtime — `Loader::new()` outside of one panics with
"there is no reactor running".

## GDK's own texture loaders

`gdk::Texture::from_filename()` (GTK >= 4.6) bypasses gdk-pixbuf for the formats
GDK has native loaders for (PNG, JPEG, TIFF). It returns a
`B8g8r8a8Premultiplied` texture directly, needs no `gtk::init()` and is
documented as thread-safe. Same pages, one core:

| page | `Texture::from_filename` | `Pixbuf` + `Texture::for_pixbuf` | current: pixbuf + premultiply + `MemoryTexture` |
|---|---:|---:|---:|
| JPEG q90 | **15.3 ms** | 27.8 ms | 36.8 ms |
| PNG | **47.7 ms** | 46.7 ms | 63.7 ms |
| TIFF | **24.0 ms** | 82.2 ms | 98.9 ms |
| WebP | 112.9 ms | 112.1 ms | 129.8 ms |
| BMP | 58.1 ms | 40.5 ms | 57.6 ms |

WebP and BMP are not in GDK's native loader set, so they fall back to
gdk-pixbuf (for BMP with an extra copy, i.e. slower than calling gdk-pixbuf
directly). GDK's own documentation warns that `gdk_texture_new_from_filename()`
"should not be used with untrusted data" and points at glycin instead.
`Texture::for_pixbuf()` is deprecated since GTK 4.20.

## rayon

`image` 0.25 has a `rayon` feature and it is **on by default**
(`default = ["rayon", "default-formats"]`). It provides
`ImageBuffer::{par_pixels, par_pixels_mut, par_enumerate_pixels,
par_enumerate_pixels_mut}` and enables threading for AVIF encoding (`ravif`) and
OpenEXR (`exr/rayon`). It does **not** parallelise JPEG/PNG/WebP decoding
(zune-jpeg, png and image-webp are single-threaded; image 0.24's `jpeg_rayon`
feature is gone) and `imageops::resize`/`thumbnail` have no parallel variant.

The GPUI crate already pulls rayon in through `image`'s default features.
Measured on 3802x2051 RGBA (the BGRA-premultiply step in `preview.rs`):

| rayon threads | sequential | rayon | speedup |
|---:|---:|---:|---:|
| 32 | 6.99 ms | 1.98 ms | 3.5x |
| 8 | 6.97 ms | 2.97 ms | 2.4x |
| 1 (`taskset -c 4`) | 7.13 ms | 16.28 ms | 0.44x |

At 1064x574 (2.4 MB) rayon is a wash (0.53 -> 0.57 ms). The step is
memory-bandwidth bound, and rayon's pool follows the CPU affinity mask, so
under `taskset -c 4` it reports one thread and loses to the plain loop.
glycin's loader uses rayon only for JXL and builds `image` without its `rayon`
feature, so sandboxed JPEG/PNG/WebP decoding stays single-threaded.

## Not measured

| Crate | Why |
|---|---|
| `librsvg` | SVG only affects app icons (GTK renders those), not page images; the crate exposes no stable public render API. |
| `libopenraw` | System library not installed and no RAW sample exists — irrelevant for scanned manga. |
| `jxl` direct | `jxl` 0.8 is a streaming decoder without a high-level entry point; glycin's JXL loader uses the same `jxl-rs`, so the `full.jxl` glycin row covers it (273 ms native, 406 ms premultiplied). |
| AVIF/HEIC | No loader installed; not part of the app's file-extension list. |
