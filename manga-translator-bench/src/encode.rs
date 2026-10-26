//! Encoders for the thumbnail disk cache (and a raw-copy baseline).

pub type EncFn = fn(&[u8], u32, u32) -> Option<Vec<u8>>;
pub type Encoders = Vec<(&'static str, EncFn)>;

use image::ImageEncoder as _;

pub fn all() -> Encoders {
    vec![
        ("png Fast (fdeflate)", png_fast),
        ("png Balanced", png_balanced),
        ("png High", png_high),
        ("png NoCompression", png_none),
        ("image PNG", image_png),
        ("image JPEG q85 (RGB)", image_jpeg),
        ("image-webp lossless", webp_lossless),
        ("gif (speed 30)", gif_enc),
        ("qoi", qoi_enc),
        ("tiff (deflate)", tiff_enc),
        ("memcpy (no cache encoding)", raw_copy),
    ]
}

fn png_with(rgba: &[u8], w: u32, h: u32, compression: png::Compression) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(compression);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

fn png_fast(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    png_with(rgba, w, h, png::Compression::Fast)
}

fn png_balanced(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    png_with(rgba, w, h, png::Compression::Balanced)
}

fn png_high(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    png_with(rgba, w, h, png::Compression::High)
}

fn png_none(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    png_with(rgba, w, h, png::Compression::NoCompression)
}

fn image_png(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(rgba, w, h, image::ExtendedColorType::Rgba8)
        .ok()?;
    Some(out)
}

fn image_jpeg(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let rgb = rgb_from_rgba(rgba);
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
        .write_image(&rgb, w, h, image::ExtendedColorType::Rgb8)
        .ok()?;
    Some(out)
}

fn rgb_from_rgba(rgba: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for px in rgba.chunks_exact(4) {
        rgb.extend_from_slice(&px[..3]);
    }
    rgb
}

fn webp_lossless(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    image_webp::WebPEncoder::new(&mut out)
        .encode(rgba, w, h, image_webp::ColorType::Rgba8)
        .ok()?;
    Some(out)
}

fn gif_enc(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut out, w as u16, h as u16, &[]).ok()?;
        let mut pixels = rgba.to_vec();
        let frame = gif::Frame::from_rgba_speed(w as u16, h as u16, &mut pixels, 30);
        encoder.write_frame(&frame).ok()?;
    }
    Some(out)
}

fn qoi_enc(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    qoi::encode_to_vec(rgba, w, h).ok()
}

fn tiff_enc(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut encoder = tiff::encoder::TiffEncoder::new(&mut cursor)
            .ok()?
            .with_compression(tiff::encoder::Compression::Deflate(
                tiff::encoder::DeflateLevel::Balanced,
            ));
        encoder
            .write_image::<tiff::encoder::colortype::RGBA8>(w, h, rgba)
            .ok()?;
    }
    Some(cursor.into_inner())
}

fn raw_copy(rgba: &[u8], _w: u32, _h: u32) -> Option<Vec<u8>> {
    Some(rgba.to_vec())
}
