//! Decoders under test.
//!
//! Every function returns `(width, height)` (or the produced buffer, where the
//! buffer size is the interesting part) so the caller can `black_box` it and
//! nothing is optimised away. All of them read from disk; the page cache is
//! warm after the warm-up iterations.

use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;

pub type Dim = (u32, u32);

// ---------------------------------------------------------------------------
// gdk-pixbuf (current implementation)
// ---------------------------------------------------------------------------

pub fn gdk_raw(path: &Path) -> Option<Dim> {
    let pixbuf = gdk_pixbuf::Pixbuf::from_file(path).ok()?;
    Some((pixbuf.width() as u32, pixbuf.height() as u32))
}

/// The exact conversion `preview.rs::decode_page()` does today: premultiplied
/// BGRA, tightly packed.
pub fn gdk_app_bgra_premult(path: &Path) -> Option<(u32, u32, usize)> {
    let pixbuf = gdk_pixbuf::Pixbuf::from_file(path).ok()?;

    let width = pixbuf.width();
    let height = pixbuf.height();
    let channels = pixbuf.n_channels() as usize;
    let rowstride = pixbuf.rowstride() as usize;
    let row_bytes = width as usize * 4;

    let storage = pixbuf.read_pixel_bytes();
    let source: &[u8] = storage.as_ref();

    let mut pixels = vec![0u8; row_bytes * height as usize];
    match channels {
        4 => {
            for (out_row, row) in pixels
                .chunks_exact_mut(row_bytes)
                .zip(source.chunks_exact(rowstride))
            {
                for (out, px) in out_row
                    .chunks_exact_mut(4)
                    .zip(row[..row_bytes].chunks_exact(4))
                {
                    let alpha = px[3];
                    if alpha == 255 {
                        out[0] = px[2];
                        out[1] = px[1];
                        out[2] = px[0];
                        out[3] = 255;
                    } else {
                        let scale = |c: u8| ((c as u32 * alpha as u32 + 127) / 255) as u8;
                        out[0] = scale(px[2]);
                        out[1] = scale(px[1]);
                        out[2] = scale(px[0]);
                        out[3] = alpha;
                    }
                }
            }
        }
        3 => {
            for (out_row, row) in pixels
                .chunks_exact_mut(row_bytes)
                .zip(source.chunks_exact(rowstride))
            {
                for (out, px) in out_row
                    .chunks_exact_mut(4)
                    .zip(row.chunks_exact(3).take(width as usize))
                {
                    out[0] = px[2];
                    out[1] = px[1];
                    out[2] = px[0];
                    out[3] = 255;
                }
            }
        }
        _ => return None,
    }

    std::hint::black_box(&pixels);
    Some((width as u32, height as u32, pixels.len()))
}

/// `Pixbuf::from_file_at_size()` without the PNG encode the cache does.
pub fn gdk_thumb_pixels(path: &Path, edge: i32) -> Option<(u32, u32, usize)> {
    let pixbuf = gdk_pixbuf::Pixbuf::from_file_at_size(path, edge, edge).ok()?;
    let bytes = pixbuf.read_pixel_bytes();
    std::hint::black_box(&bytes);
    Some((pixbuf.width() as u32, pixbuf.height() as u32, bytes.len()))
}

/// `Pixbuf::from_file_at_size()` — scaled decode, as used for thumbnails.
pub fn gdk_thumb(path: &Path, edge: i32) -> Option<(u32, u32, usize)> {
    let pixbuf = gdk_pixbuf::Pixbuf::from_file_at_size(path, edge, edge).ok()?;
    let png = pixbuf.save_to_bufferv("png", &[]).ok()?;
    Some((pixbuf.width() as u32, pixbuf.height() as u32, png.len()))
}

// ---------------------------------------------------------------------------
// image-rs (the decoder inside glycin's image-rs loader)
// ---------------------------------------------------------------------------

pub fn image_rs_rgba(path: &Path) -> Option<(u32, u32, usize)> {
    let img = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let rgba = img.to_rgba8();
    std::hint::black_box(&rgba);
    Some((rgba.width(), rgba.height(), rgba.as_raw().len()))
}

pub fn image_rs_app_bgra_premult(path: &Path) -> Option<(u32, u32, usize)> {
    let img = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let rgba = img.to_rgba8();
    let bgra = rgba_to_bgra_premult(rgba.as_raw());
    std::hint::black_box(&bgra);
    Some((rgba.width(), rgba.height(), bgra.len()))
}

pub fn image_rs_thumb(path: &Path, edge: u32) -> Option<(u32, u32, usize)> {
    let img = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let thumb = img.thumbnail(edge, edge);
    std::hint::black_box(&thumb);
    Some((thumb.width(), thumb.height(), thumb.as_bytes().len()))
}

/// Same per-pixel work the GTK frontend does today, but on RGBA input.
pub fn rgba_to_bgra_premult(rgba: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; rgba.len()];
    for (dst, src) in out.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        let a = src[3];
        if a == 255 {
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = 255;
        } else {
            let scale = |c: u8| ((c as u32 * a as u32 + 127) / 255) as u8;
            dst[0] = scale(src[2]);
            dst[1] = scale(src[1]);
            dst[2] = scale(src[0]);
            dst[3] = a;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// glycin (sandboxed, external loaders)
// ---------------------------------------------------------------------------

fn glycin_loader(path: &Path) -> glycin::Loader {
    let mut loader = glycin::Loader::new(gio::File::for_path(path));
    loader.accepted_memory_formats(glycin::MemoryFormatSelection::B8g8r8a8Premultiplied);
    loader
}

/// Let the loader return whatever it decodes natively (RGB8 for JPEG, RGBA8 for PNG).
fn glycin_loader_native(path: &Path) -> glycin::Loader {
    let mut loader = glycin::Loader::new(gio::File::for_path(path));
    loader.accepted_memory_formats(glycin::MemoryFormatSelection::all());
    loader
}

pub fn glycin_full(path: &Path) -> Option<(u32, u32, usize)> {
    let mut image = futures_lite::future::block_on(glycin_loader(path).load()).ok()?;
    let frame = futures_lite::future::block_on(image.next_frame()).ok()?;
    Some((frame.width(), frame.height(), frame.buf_bytes().len()))
}

pub fn glycin_full_native(path: &Path) -> Option<(u32, u32, usize)> {
    let mut image = futures_lite::future::block_on(glycin_loader_native(path).load()).ok()?;
    let frame = futures_lite::future::block_on(image.next_frame()).ok()?;
    let bytes = frame.buf_bytes();
    std::hint::black_box(&bytes);
    Some((frame.width(), frame.height(), bytes.len()))
}

pub fn glycin_thumb_native(path: &Path, edge: u32) -> Option<(u32, u32, usize)> {
    let mut image = futures_lite::future::block_on(glycin_loader_native(path).load()).ok()?;
    let request = glycin::FrameRequest::new().scale(edge, edge);
    let frame = futures_lite::future::block_on(image.specific_frame(request)).ok()?;
    let bytes = frame.buf_bytes();
    std::hint::black_box(&bytes);
    Some((frame.width(), frame.height(), bytes.len()))
}

pub fn glycin_thumb(path: &Path, edge: u32) -> Option<(u32, u32, usize)> {
    let mut image = futures_lite::future::block_on(glycin_loader(path).load()).ok()?;
    let request = glycin::FrameRequest::new().scale(edge, edge);
    let frame = futures_lite::future::block_on(image.specific_frame(request)).ok()?;
    Some((frame.width(), frame.height(), frame.buf_bytes().len()))
}

// ---------------------------------------------------------------------------
// Format-specific crates from the glycin loader set
// ---------------------------------------------------------------------------

pub fn png_crate(path: &Path) -> Option<(u32, u32, usize)> {
    let decoder = png::Decoder::new(BufReader::new(File::open(path).ok()?));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    Some((info.width, info.height, info.buffer_size()))
}

pub fn zune_jpeg(path: &Path) -> Option<(u32, u32, usize)> {
    let bytes = std::fs::read(path).ok()?;
    let mut decoder = zune_jpeg::JpegDecoder::new(Cursor::new(bytes));
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    Some((info.width as u32, info.height as u32, pixels.len()))
}

pub fn image_webp(path: &Path) -> Option<(u32, u32, usize)> {
    let mut decoder = image_webp::WebPDecoder::new(BufReader::new(File::open(path).ok()?)).ok()?;
    let (w, h) = decoder.dimensions();
    let mut buf = vec![0; decoder.output_buffer_size()?];
    decoder.read_image(&mut buf).ok()?;
    Some((w as u32, h as u32, buf.len()))
}

pub fn gif_crate(path: &Path) -> Option<(u32, u32, usize)> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options
        .read_info(BufReader::new(File::open(path).ok()?))
        .ok()?;
    let (w, h) = (decoder.width() as u32, decoder.height() as u32);
    let frame = decoder.read_next_frame().ok()??;
    Some((w, h, frame.buffer.len()))
}

pub fn tiff_crate(path: &Path) -> Option<(u32, u32, usize)> {
    let mut decoder = tiff::decoder::Decoder::new(BufReader::new(File::open(path).ok()?)).ok()?;
    let result = decoder.read_image().ok()?;
    let (w, h) = decoder.dimensions().ok()?;
    let len = match result {
        tiff::decoder::DecodingResult::U8(buf) => buf.len(),
        tiff::decoder::DecodingResult::U16(buf) => buf.len() * 2,
        _ => 0,
    };
    Some((w, h, len))
}

pub fn bmp_crate(path: &Path) -> Option<(u32, u32, usize)> {
    let img = bmp::open(path).ok()?;
    let (w, h) = (img.get_width(), img.get_height());
    Some((w, h, (w * h * 3) as usize))
}

pub fn qoi_crate(path: &Path) -> Option<(u32, u32, usize)> {
    let bytes = std::fs::read(path).ok()?;
    let (header, pixels) = qoi::decode_to_vec(&bytes).ok()?;
    Some((header.width, header.height, pixels.len()))
}

pub fn exr_crate(path: &Path) -> Option<(u32, u32, usize)> {
    let image = exr::prelude::read_first_flat_layer_from_file(path).ok()?;
    let size = image.layer_data.size;
    let len = image
        .layer_data
        .channel_data
        .list
        .iter()
        .map(|c| c.sample_data.len())
        .sum();
    Some((size.x() as u32, size.y() as u32, len))
}

pub fn jp2_crate(path: &Path) -> Option<(u32, u32, usize)> {
    let bytes = std::fs::read(path).ok()?;
    let image =
        hayro_jpeg2000::Image::new(&bytes, &hayro_jpeg2000::DecodeSettings::default()).ok()?;
    let (w, h) = (image.width(), image.height());
    let mut context = hayro_jpeg2000::DecoderContext::default();
    image.decode(&mut context).ok()?;
    Some((w, h, (w as usize) * (h as usize) * 3))
}
