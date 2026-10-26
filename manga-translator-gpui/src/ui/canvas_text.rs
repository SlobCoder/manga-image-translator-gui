// manga-translator-gpui/src/ui/canvas_text.rs
//
// Text inside `canvas` paint passes.
//
// gpui has no direct "draw this string at x, y" call while painting: the text
// is shaped into a `ShapedLine` first, which is also what carries the font and
// colour. This is the thin wrapper both the placeholder and the comparison
// slider use.

use gpui::{App, Hsla, Pixels, Point, TextAlign, TextRun, Window, font, px};

/// Draw one horizontally and vertically centred line of text.
pub(crate) fn paint_text(
    window: &mut Window,
    app: &mut App,
    text: &str,
    center: Point<Pixels>,
    font_size: Pixels,
    color: Hsla,
    bold: bool,
) {
    let size = font_size.as_f32();
    if size < 4.0 || text.is_empty() {
        return;
    }

    let family = if bold {
        font(".SystemUIFont").bold()
    } else {
        font(".SystemUIFont")
    };
    let run = TextRun {
        len: text.len(),
        font: family,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = {
        let text_system = window.text_system();
        text_system.shape_line(text.to_string().into(), font_size, &[run], None)
    };

    let width = shaped.width().as_f32();
    let line_height = size * 1.15;
    let origin = Point::new(
        px(center.x.as_f32() - width / 2.0),
        px(center.y.as_f32() - line_height / 2.0),
    );
    let _ = shaped.paint(origin, px(line_height), TextAlign::Left, None, window, app);
}
