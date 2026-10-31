// manga-translator-gtk/src/ui/preview.rs
//
// Image preview widget with comparison slider (original ↔ translated).
//
// Features:
//   - Gtk.Stack switching between placeholder animation and image view
//   - Original / Translated toggle buttons
//   - Comparison mode with draggable vertical slider
//   - Gtk.Overlay with clipped translated layer on top of original
//   - Gtk.DrawingArea placeholder with a 3D page-turn animation
//   - Zoom and fit modes via Gtk.Picture with ContentFit
//   - File info bar (name, dimensions, size)
//   - Smooth cross-fade transitions between images

use adw::prelude::*;
use gtk::gdk::Texture;
use gtk::glib;
use gtk::glib::clone;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use crate::i18n;

/// Names and msgids of the three preview modes, shared by the toggle group
/// and its retranslation hook.
const MODE_TOGGLES: &[(&str, &str)] = &[
    ("original", "Original"),
    ("translated", "Übersetzung"),
    ("compare", "Vergleich"),
];

// ---------------------------------------------------------------------------
// Off-thread page decoding
// ---------------------------------------------------------------------------

/// A page decoded on a worker thread, in the exact byte layout the main thread
/// needs: tightly packed BGRA, premultiplied.
///
/// That layout is valid both for a `gdk::MemoryTexture`
/// (`B8g8r8a8Premultiplied`) and for a Cairo `ARgb32` surface, so a single
/// buffer feeds the picture and the comparison overlay.
struct DecodedPage {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
}

/// Decode an image for display. Must run off the main thread.
///
/// gdk-pixbuf is thread-safe — the thumbnail loader already runs it on worker
/// threads — while GDK objects are not, so only the decoding moves.
///
/// This replaces the previous main-thread sequence
/// `Texture::from_bytes` → `texture.download()` → per-pixel R↔B swap, which
/// cost 4–10 ms per 1064×1510 page and scales with the page size.
fn decode_page(path: &std::path::Path) -> Option<DecodedPage> {
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
        other => {
            log::warn!("unsupported pixbuf channel count: {}", other);
            return None;
        }
    }

    Some(DecodedPage {
        width,
        height,
        pixels,
    })
}

/// Wrap decoded pixels in a GDK texture. Main thread only.
fn texture_from_page(page: DecodedPage) -> Texture {
    let DecodedPage {
        width,
        height,
        pixels,
    } = page;
    let bytes = glib::Bytes::from_owned(pixels);
    gtk::gdk::MemoryTexture::new(
        width,
        height,
        gtk::gdk::MemoryFormat::B8g8r8a8Premultiplied,
        &bytes,
        width as usize * 4,
    )
    .upcast()
}

/// Cairo surface for the comparison overlay. Main thread only.
fn surface_from_page(page: &DecodedPage) -> Option<cairo::ImageSurface> {
    cairo::ImageSurface::create_for_data(
        page.pixels.clone(),
        cairo::Format::ARgb32,
        page.width,
        page.height,
        page.width * 4,
    )
    .ok()
}

// ---------------------------------------------------------------------------
// Preview mode
// ---------------------------------------------------------------------------

/// Current display mode of the preview pane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreviewMode {
    /// Show only the original image.
    Original,
    /// Show only the translated image.
    Translated,
    /// Side-by-side comparison with a draggable slider.
    Compare,
}

impl Default for PreviewMode {
    fn default() -> Self {
        PreviewMode::Original
    }
}

impl PreviewMode {
    /// Stable name of the matching toggle in the segmented control.
    fn toggle_name(self) -> &'static str {
        match self {
            PreviewMode::Original => "original",
            PreviewMode::Translated => "translated",
            PreviewMode::Compare => "compare",
        }
    }
}

// ---------------------------------------------------------------------------
// Preview widget (GObject)
// ---------------------------------------------------------------------------

glib::wrapper! {
    pub struct Preview(ObjectSubclass<PreviewPrivate>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// Internal state for the preview widget.
pub struct PreviewPrivate {
    /// Current preview mode (original, translated, compare).
    mode: RefCell<PreviewMode>,
    /// Path to the currently displayed original image.
    original_path: RefCell<Option<PathBuf>>,
    /// Path to the currently displayed translated image.
    translated_path: RefCell<Option<PathBuf>>,
    /// Slider position for comparison mode (0.0..=1.0, left to right).
    slider_position: RefCell<f64>,
    /// Whether the slider is currently being dragged.
    #[allow(dead_code)]
    dragging_slider: RefCell<bool>,

    // Widget references
    container: RefCell<Option<gtk::Box>>,
    /// Stack switching between placeholder and image view.
    stack: RefCell<Option<gtk::Stack>>,
    /// Placeholder drawing area (shown when no image is loaded).
    placeholder: RefCell<Option<gtk::DrawingArea>>,
    /// Overlay container for comparison mode.
    overlay: RefCell<Option<gtk::Overlay>>,
    /// Gtk.Picture showing the original image.
    original_picture: RefCell<Option<gtk::Picture>>,
    /// DrawingArea overlay for translated image (Cairo-based clipping in compare mode).
    translated_overlay: RefCell<Option<gtk::DrawingArea>>,
    /// Cached translated texture (kept for reference).
    translated_texture: RefCell<Option<Texture>>,
    /// Cached Cairo surface for translated texture (avoids GPU→CPU download per frame).
    translated_surface: RefCell<Option<cairo::ImageSurface>>,
    /// Slider line widget (vertical bar in compare mode).
    slider_widget: RefCell<Option<gtk::DrawingArea>>,
    /// Adwaita segmented control for the preview modes.
    mode_group: RefCell<Option<adw::ToggleGroup>>,
    /// Info label (filename, dimensions).
    info_label: RefCell<Option<gtk::Label>>,
    /// Dimensions label.
    dimensions_label: RefCell<Option<gtk::Label>>,
}

impl Default for PreviewPrivate {
    fn default() -> Self {
        Self {
            mode: RefCell::new(PreviewMode::default()),
            original_path: RefCell::new(None),
            translated_path: RefCell::new(None),
            slider_position: RefCell::new(0.5),
            dragging_slider: RefCell::new(false),
            container: RefCell::new(None),
            stack: RefCell::new(None),
            placeholder: RefCell::new(None),
            overlay: RefCell::new(None),
            original_picture: RefCell::new(None),
            translated_overlay: RefCell::new(None),
            translated_texture: RefCell::new(None),
            translated_surface: RefCell::new(None),
            slider_widget: RefCell::new(None),
            mode_group: RefCell::new(None),
            info_label: RefCell::new(None),
            dimensions_label: RefCell::new(None),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for PreviewPrivate {
    const NAME: &'static str = "MangaPreview";
    type Type = Preview;
    type ParentType = gtk::Widget;
}

impl ObjectImpl for PreviewPrivate {
    fn constructed(&self) {
        self.parent_constructed();
        let obj = self.obj();
        // Preview subclasses gtk::Widget (a leaf), so we must provide
        // a layout manager so the internal vbox child gets size-allocated.
        obj.set_layout_manager(Some(gtk::BoxLayout::new(gtk::Orientation::Vertical)));
        self.build_ui(&obj);
    }

    fn dispose(&self) {
        if let Some(container) = self.container.borrow().as_ref() {
            container.unparent();
        }
    }
}

impl WidgetImpl for PreviewPrivate {}

/// Helper: return a zeroed-out `cairo::TextExtents`.
/// cairo-rs 0.22 does not provide `TextExtents::zeroed()` or `Default`.
fn text_extents_zeroed() -> cairo::TextExtents {
    cairo::TextExtents::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
}

/// Expose the comparison slider's position to assistive technologies.
fn update_slider_a11y(widget: &gtk::DrawingArea, pos: f64) {
    let pct = (pos * 100.0).round().clamp(0.0, 100.0);
    let text = format!("{:.0} %", pct);
    widget.update_property(&[
        gtk::accessible::Property::ValueMin(0.0),
        gtk::accessible::Property::ValueMax(100.0),
        gtk::accessible::Property::ValueNow(pct),
        gtk::accessible::Property::ValueText(&text),
    ]);
}

impl PreviewPrivate {
    /// Point the segmented control at *mode*.
    fn sync_mode_group(&self, mode: PreviewMode) {
        if let Some(group) = self.mode_group.borrow().as_ref() {
            if group.active_name().as_deref() != Some(mode.toggle_name()) {
                group.set_active_name(Some(mode.toggle_name()));
            }
        }
    }

    /// Move the comparison slider to *pos*, redraw both layers and update the
    /// assistive-technology value of the slider widget.
    fn set_slider_position(&self, pos: f64) {
        *self.slider_position.borrow_mut() = pos;
        if let Some(slider) = self.slider_widget.borrow().as_ref() {
            update_slider_a11y(slider, pos);
            slider.queue_draw();
        }
        if let Some(overlay) = self.translated_overlay.borrow().as_ref() {
            overlay.queue_draw();
        }
    }

    /// Build the preview UI.
    fn build_ui(&self, obj: &Preview) {
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        vbox.set_vexpand(true);
        vbox.set_hexpand(true);

        // -- Header row: title + mode toggles --
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.set_margin_start(12);
        header.set_margin_end(12);
        header.set_margin_top(8);
        header.set_margin_bottom(8);

        let title = gtk::Label::new(Some(&i18n::t("Vorschau")));
        title.add_css_class("title-2");
        i18n::register_label(&title, "Vorschau");
        header.append(&title);

        // Adwaita segmented control (Original | Translation | Compare).
        let mode_group = adw::ToggleGroup::new();
        mode_group.set_halign(gtk::Align::End);
        for &(name, msgid) in MODE_TOGGLES {
            let toggle = adw::Toggle::new();
            toggle.set_name(Some(name));
            toggle.set_label(Some(&i18n::t(msgid)));
            mode_group.add(toggle);
        }
        mode_group.set_active_name(Some("original"));
        i18n::register_toggle_group(&mode_group, MODE_TOGGLES);
        header.append(&mode_group);

        vbox.append(&header);

        // -- Preview stack: placeholder ↔ image overlay --
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.set_hexpand(true);
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(300);

        // --- Placeholder ---
        let placeholder = gtk::DrawingArea::new();
        placeholder.set_vexpand(true);
        placeholder.set_hexpand(true);
        placeholder.add_css_class("preview-placeholder");

        // Draw the comic book placeholder animation
        placeholder.set_draw_func(|_area, cr, width, height| {
            draw_placeholder(cr, width, height);
        });

        // Frame-rate-limited animation (~60fps).
        // The tick callback fires at monitor refresh rate (potentially 240Hz),
        // but the comic-book animation is smooth at 60fps. Skipping frames
        // reduces Cairo CPU rendering by ~75% at 240Hz.
        let last_render = Rc::new(Cell::new(0i64));
        placeholder.add_tick_callback(move |widget, clock| {
            let now = clock.frame_time(); // microseconds
            let elapsed = now - last_render.get();
            // 16,667 µs ≈ 60fps — sufficient for a page-turn animation
            if elapsed >= 16_667 {
                last_render.set(now);
                widget.queue_draw();
            }
            glib::ControlFlow::Continue
        });

        stack.add_named(&placeholder, Some("placeholder"));

        // --- Image overlay container ---
        let overlay = gtk::Overlay::new();
        overlay.add_css_class("comparison-container");
        overlay.set_vexpand(true);
        overlay.set_hexpand(true);

        // Original image (bottom layer)
        let original_picture = gtk::Picture::new();
        original_picture.set_content_fit(gtk::ContentFit::Contain);
        original_picture.set_can_shrink(true);
        original_picture.add_css_class("preview-image");
        original_picture.add_css_class("comparison-image");
        overlay.set_child(Some(&original_picture));

        // Translated image overlay (top layer — DrawingArea for Cairo-based clipping)
        let translated_overlay = gtk::DrawingArea::new();
        translated_overlay.set_hexpand(true);
        translated_overlay.set_vexpand(true);
        translated_overlay.set_can_target(false); // let pointer events pass to slider
        translated_overlay.add_css_class("translated-layer");
        translated_overlay.set_draw_func(clone!(
            #[weak]
            obj,
            move |_area, cr, width, height| {
                let this = obj.imp();
                let mode = *this.mode.borrow();

                // Only draw in Translated and Compare modes
                if matches!(mode, PreviewMode::Original) {
                    return;
                }

                let surface_opt = this.translated_surface.borrow();
                let Some(surface) = surface_opt.as_ref() else {
                    return;
                };

                let tex_w = surface.width() as f64;
                let tex_h = surface.height() as f64;
                if tex_w <= 0.0 || tex_h <= 0.0 {
                    return;
                }

                // Calculate "contain" fit (same logic as gtk::Picture ContentFit::Contain)
                let w = width as f64;
                let h = height as f64;
                let scale = (w / tex_w).min(h / tex_h);
                let draw_w = tex_w * scale;
                let draw_h = tex_h * scale;
                let offset_x = (w - draw_w) / 2.0;
                let offset_y = (h - draw_h) / 2.0;

                let _ = cr.save();

                if matches!(mode, PreviewMode::Compare) {
                    // Clip to the area right of the slider
                    let pos = *this.slider_position.borrow();
                    let slider_x = (w * pos).round();
                    let _ = cr.rectangle(slider_x, 0.0, w - slider_x, h);
                    cr.clip();
                }

                // Draw the cached Cairo surface with the same scaling as the original
                cr.translate(offset_x, offset_y);
                cr.scale(scale, scale);
                let _ = cr.set_source_surface(surface, 0.0, 0.0);
                let _ = cr.paint();

                let _ = cr.restore();
            }
        ));
        overlay.add_overlay(&translated_overlay);

        // Comparison slider line
        let slider_widget = gtk::DrawingArea::new();
        slider_widget.add_css_class("comparison-slider");
        slider_widget.set_hexpand(true);
        slider_widget.set_vexpand(true);
        slider_widget.set_cursor_from_name(Some("col-resize"));
        slider_widget.set_focusable(true);
        update_slider_a11y(&slider_widget, 0.5);
        slider_widget.set_draw_func(clone!(
            #[weak]
            obj,
            move |_area, cr, width, height| {
                let this = obj.imp();
                let pos = *this.slider_position.borrow();
                let x = (width as f64 * pos).round();

                // Draw the slider line — dark outline first, then white center
                // This ensures visibility on both light and dark images
                cr.set_line_width(4.5);
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.6);
                let _ = cr.move_to(x, 0.0);
                let _ = cr.line_to(x, height as f64);
                let _ = cr.stroke();

                cr.set_line_width(2.0);
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
                let _ = cr.move_to(x, 0.0);
                let _ = cr.line_to(x, height as f64);
                let _ = cr.stroke();

                // Draw handle circle — dark outline then white fill
                let cy = height as f64 / 2.0;
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.6);
                let _ = cr.arc(x, cy, 13.0, 0.0, 2.0 * std::f64::consts::PI);
                cr.fill().ok();

                cr.set_source_rgb(0.98, 0.98, 0.98);
                let _ = cr.arc(x, cy, 12.0, 0.0, 2.0 * std::f64::consts::PI);
                cr.fill().ok();

                // Draw arrows on the handle
                cr.set_source_rgb(0.2, 0.2, 0.2);
                cr.set_line_width(2.0);
                // Left arrow
                let _ = cr.move_to(x - 5.0, cy);
                let _ = cr.line_to(x - 9.0, cy - 4.0);
                let _ = cr.move_to(x - 5.0, cy);
                let _ = cr.line_to(x - 9.0, cy + 4.0);
                let _ = cr.stroke();
                // Right arrow
                let _ = cr.move_to(x + 5.0, cy);
                let _ = cr.line_to(x + 9.0, cy - 4.0);
                let _ = cr.move_to(x + 5.0, cy);
                let _ = cr.line_to(x + 9.0, cy + 4.0);
                let _ = cr.stroke();

                // Position label
                let pct = (pos * 100.0).round() as i32;
                cr.set_source_rgb(0.2, 0.2, 0.2);
                cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                cr.set_font_size(9.0);
                let text = format!("{}%", pct);
                let extents = cr
                    .text_extents(&text)
                    .unwrap_or_else(|_| text_extents_zeroed());
                let _ = cr.move_to(x - extents.width() / 2.0, cy + 24.0);
                let _ = cr.show_text(&text);
            }
        ));

        overlay.add_overlay(&slider_widget);

        // Slider gesture (drag)
        let gesture = gtk::GestureDrag::new();
        gesture.set_button(gtk::gdk::BUTTON_PRIMARY);

        gesture.connect_drag_begin(move |_gesture, _x, _y| {
            // Start dragging — nothing special needed
        });

        gesture.connect_drag_update(clone!(
            #[strong]
            slider_widget,
            #[weak]
            obj,
            move |_gesture, offset_x, _offset_y| {
                let this = obj.imp();
                let start_x = _gesture.start_point().map(|(x, _)| x).unwrap_or(0.0);
                let allocation = slider_widget.allocation();
                let width = allocation.width() as f64;

                if width > 0.0 {
                    let mut pos = (start_x + offset_x) / width;
                    pos = pos.clamp(0.02, 0.98);

                    this.set_slider_position(pos);
                }
            }
        ));

        gesture.connect_drag_end(clone!(
            #[weak]
            obj,
            move |_gesture, _offset_x, _offset_y| {
                let this = obj.imp();
                let final_pos = *this.slider_position.borrow();

                // Micro-bounce: overshoot by 1.5% then snap back
                let overshoot = if final_pos > 0.5 {
                    (final_pos + 0.015).min(0.98)
                } else {
                    (final_pos - 0.015).max(0.02)
                };

                // Step 1: Move to overshoot
                this.set_slider_position(overshoot);

                // Step 2: Snap back to final position after 80ms
                let obj2 = obj.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
                    let this = obj2.imp();
                    this.set_slider_position(final_pos);
                    glib::ControlFlow::Break
                });
            }
        ));

        slider_widget.add_controller(gesture);

        // Keyboard control: arrows move by 2 %, Home/End jump to the edges.
        let key_controller = gtk::EventControllerKey::new();
        let weak_obj = obj.downgrade();
        key_controller.connect_key_pressed(move |_controller, key, _code, _modifier| {
            let Some(obj) = weak_obj.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let this = obj.imp();
            let current = *this.slider_position.borrow();
            let next = match key {
                gtk::gdk::Key::Left => Some((current - 0.02).max(0.02)),
                gtk::gdk::Key::Right => Some((current + 0.02).min(0.98)),
                gtk::gdk::Key::Home => Some(0.02),
                gtk::gdk::Key::End => Some(0.98),
                _ => None,
            };
            match next {
                Some(next) => {
                    this.set_slider_position(next);
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        });
        slider_widget.add_controller(key_controller);

        // Re-apply clip when the slider widget is resized (handles initial allocation
        // race: Compare mode may be entered before the widget has its first allocation)
        slider_widget.connect_resize(clone!(
            #[weak]
            obj,
            move |_area, _width, _height| {
                let this = obj.imp();
                if matches!(*this.mode.borrow(), PreviewMode::Compare) {
                    if let Some(overlay) = this.translated_overlay.borrow().as_ref() {
                        overlay.queue_draw();
                    }
                }
            }
        ));

        // Initially hide translated layer (we're in Original mode)
        translated_overlay.set_visible(false);
        slider_widget.set_visible(false);

        stack.add_named(&overlay, Some("images"));
        stack.set_visible_child_name("placeholder");

        vbox.append(&stack);

        // -- Info bar --
        let info_bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        info_bar.set_margin_start(12);
        info_bar.set_margin_end(12);
        info_bar.set_margin_top(4);
        info_bar.set_margin_bottom(4);

        let info_label = gtk::Label::new(Some(&i18n::t("Keine Datei ausgewählt")));
        i18n::register_label(&info_label, "Keine Datei ausgewählt");
        info_label.add_css_class("caption");
        info_label.set_hexpand(true);
        info_label.set_xalign(0.0);
        info_label.set_ellipsize(pango::EllipsizeMode::Middle);
        info_bar.append(&info_label);

        let dimensions_label = gtk::Label::new(None);
        dimensions_label.add_css_class("caption");
        dimensions_label.add_css_class("dim-label");
        dimensions_label.set_halign(gtk::Align::End);
        info_bar.append(&dimensions_label);

        vbox.append(&info_bar);

        // -- Connect the mode toggle group --
        mode_group.connect_active_name_notify(clone!(
            #[weak(rename_to = this)]
            self,
            move |group| {
                let mode = match group.active_name().as_deref() {
                    Some("translated") => PreviewMode::Translated,
                    Some("compare") => PreviewMode::Compare,
                    _ => PreviewMode::Original,
                };
                this.set_mode(mode);
            }
        ));

        // Store references
        *self.container.borrow_mut() = Some(vbox.clone());
        *self.stack.borrow_mut() = Some(stack);
        *self.placeholder.borrow_mut() = Some(placeholder);
        *self.overlay.borrow_mut() = Some(overlay);
        *self.original_picture.borrow_mut() = Some(original_picture);
        *self.translated_overlay.borrow_mut() = Some(translated_overlay);
        *self.slider_widget.borrow_mut() = Some(slider_widget);
        *self.mode_group.borrow_mut() = Some(mode_group);
        *self.info_label.borrow_mut() = Some(info_label);
        *self.dimensions_label.borrow_mut() = Some(dimensions_label);

        vbox.set_parent(obj);
    }

    /// Set the preview mode and update visibility of layers.
    fn set_mode(&self, mode: PreviewMode) {
        let mut current_mode = self.mode.borrow_mut();
        *current_mode = mode;
        drop(current_mode);

        let original = self.original_picture.borrow();
        let translated = self.translated_overlay.borrow();
        let slider = self.slider_widget.borrow();

        if let (Some(orig_pic), Some(trans_overlay), Some(slider_w)) =
            (original.as_ref(), translated.as_ref(), slider.as_ref())
        {
            match mode {
                PreviewMode::Original => {
                    orig_pic.set_visible(true);
                    // Smooth fade-out of translated layer
                    trans_overlay.add_css_class("layer-hidden");
                    let overlay_clone = trans_overlay.clone();
                    glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
                        overlay_clone.set_visible(false);
                        overlay_clone.remove_css_class("layer-hidden");
                        glib::ControlFlow::Break
                    });
                    slider_w.set_visible(false);
                }
                PreviewMode::Translated => {
                    orig_pic.set_visible(true);
                    // Smooth fade-in of translated layer
                    trans_overlay.set_visible(true);
                    trans_overlay.add_css_class("layer-hidden");
                    trans_overlay.queue_draw();
                    let overlay_clone = trans_overlay.clone();
                    glib::idle_add_local_once(move || {
                        overlay_clone.remove_css_class("layer-hidden");
                    });
                    slider_w.set_visible(false);
                }
                PreviewMode::Compare => {
                    orig_pic.set_visible(true);
                    // Smooth fade-in of translated layer
                    trans_overlay.set_visible(true);
                    trans_overlay.add_css_class("layer-hidden");
                    trans_overlay.queue_draw();
                    let overlay_clone = trans_overlay.clone();
                    glib::idle_add_local_once(move || {
                        overlay_clone.remove_css_class("layer-hidden");
                    });
                    slider_w.set_visible(true);
                    slider_w.queue_draw();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Placeholder drawing
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Placeholder drawing — full port of Python ComicBookPlaceholder
// ---------------------------------------------------------------------------

/// Animation timing constants.
const STATIC_TIME: f64 = 2.6; // seconds the spread rests before the next turn
const FLIP_TIME: f64 = 1.5; // seconds one sheet takes to turn over
const CYCLE: f64 = STATIC_TIME + FLIP_TIME;

// --- Turn motion ------------------------------------------------------------
//
// The rotation follows the step response of a damped spring rather than an
// ease-in-out curve. An ease curve decelerates into the target and stops dead,
// which reads as a slide; a spring carries the sheet a little past the far side
// and rocks it back, which reads as paper.

/// Damping ratio. Below 1 on purpose: this is what produces the overshoot.
const TURN_DAMPING: f64 = 0.76;
/// E-folds of decay packed into `FLIP_TIME`. Together with `TURN_DAMPING` this
/// puts the overshoot at ~2.5 % of the arc, about seven tenths of the way in.
const TURN_SETTLE: f64 = 6.0;
/// Where the leftover tail starts being trimmed, as a fraction of `FLIP_TIME`.
/// A spring only approaches its target asymptotically; the trim makes the sheet
/// exactly flat when the flip ends, so the resting spread does not jump.
const TURN_TRIM_START: f64 = 0.78;

// --- Page turn geometry -----------------------------------------------------
//
// The turning sheet is projected as if a camera sat above the book plane: a
// point at distance `d` from the spine, rotated by `theta` about it, is at
// height `d * sin(theta)` and therefore projects with a scale of
// `cam / (cam - height)`. The sheet is drawn as thin vertical strips, each with
// its own affine transform, which yields the perspective without a 3D stack.

/// Camera height above the book plane, in page widths. Large = flat and calm;
/// the default keeps just enough perspective for the lift to read.
const CAM_HEIGHT: f64 = 9.5;
/// Camera offset from the spine, in page widths. A three-quarter view keeps the
/// sheet readable at 90 degrees instead of collapsing it into a line.
const CAM_OFFSET: f64 = -0.45;
/// Vertical strips used to project the turning sheet.
const TURN_STRIPS: usize = 40;
/// How far the free edge runs ahead of the hinge at full sweep speed, in
/// radians. This is the whole curl: the bow follows how fast the sheet is
/// actually moving, so it builds up after the lift starts and relaxes as the
/// sheet lands, instead of being a fixed function of the angle.
const PAPER_CURL: f64 = 0.34;
/// Peak of [`turn_motion`]'s rate. Dividing by it makes `PAPER_CURL` a plain
/// angle rather than a number that has to be re-tuned with the spring. Guarded
/// by `test_turn_rate_peak_is_current`.
const TURN_RATE_PEAK: f64 = 0.888;

/// Panel layouts per "page" — (x, y, w, h) as fractions of the page area.
const LAYOUTS: &[&[(f64, f64, f64, f64)]] = &[
    // Page 0: Big top panel, 3 small bottom panels
    &[
        (0.04, 0.03, 0.92, 0.54),
        (0.04, 0.61, 0.29, 0.36),
        (0.36, 0.61, 0.29, 0.36),
        (0.68, 0.61, 0.29, 0.36),
    ],
    // Page 1: 2x2 grid
    &[
        (0.04, 0.03, 0.45, 0.47),
        (0.53, 0.03, 0.45, 0.47),
        (0.04, 0.53, 0.45, 0.44),
        (0.53, 0.53, 0.45, 0.44),
    ],
    // Page 2: Three vertical strips
    &[
        (0.04, 0.03, 0.28, 0.94),
        (0.37, 0.03, 0.28, 0.94),
        (0.68, 0.03, 0.28, 0.94),
    ],
    // Page 3: Big left panel, two stacked right
    &[
        (0.04, 0.03, 0.58, 0.94),
        (0.66, 0.03, 0.32, 0.45),
        (0.66, 0.52, 0.32, 0.45),
    ],
    // Page 4: Full-bleed splash with small inset overlay
    &[(0.0, 0.0, 1.0, 1.0), (0.60, 0.05, 0.35, 0.25)],
    // Page 5: Asymmetric L-shape
    &[
        (0.04, 0.03, 0.45, 0.94),
        (0.52, 0.03, 0.46, 0.45),
        (0.52, 0.52, 0.22, 0.45),
        (0.77, 0.52, 0.21, 0.45),
    ],
    // Page 6: Wide horizontal strips
    &[
        (0.04, 0.03, 0.92, 0.28),
        (0.04, 0.35, 0.92, 0.30),
        (0.04, 0.69, 0.92, 0.28),
    ],
    // Page 7: Overlapping panels (last drawn on top)
    &[(0.04, 0.03, 0.60, 0.60), (0.35, 0.40, 0.62, 0.55)],
];

/// Bubble content types.
const BUBBLE_TYPES: &[&str] = &["dots", "japanese", "lines", "exclaim"];

/// Bubble position offsets (x, y) relative to panel center.
const BUBBLE_OFFSETS: &[(f64, f64)] = &[
    (0.50, 0.35),
    (0.55, 0.40),
    (0.45, 0.30),
    (0.50, 0.38),
    (0.60, 0.35),
    (0.40, 0.42),
];

/// Draw a rounded-rectangle path.
fn rrect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let _ = cr.move_to(x + r, y);
    let _ = cr.line_to(x + w - r, y);
    let _ = cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    let _ = cr.line_to(x + w, y + h - r);
    let _ = cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    let _ = cr.line_to(x + r, y + h);
    let _ = cr.arc(
        x + r,
        y + h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    let _ = cr.line_to(x, y + r);
    let _ = cr.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    cr.close_path();
}

/// Centered book rectangle with manga tankobon aspect ratio (two pages side-by-side).
fn book_rect(w: f64, h: f64) -> (f64, f64, f64, f64) {
    let mx = w * 0.06;
    let my = h * 0.07;
    let aw = w - mx * 2.0;
    let ah = h - my * 2.0;
    let ratio = 1.3; // Two pages side-by-side
    let (bw, bh) = if aw / ah > ratio {
        (ah * ratio, ah)
    } else {
        (aw, aw / ratio)
    };
    ((w - bw) / 2.0, (h - bh) / 2.0, bw, bh)
}

/// Draw static book structure: shadow, stacked pages, spine.
fn draw_book(cr: &cairo::Context, bx: f64, by: f64, bw: f64, bh: f64, sw: f64, is_dark: bool) {
    // Drop shadow
    let _ = cr.save();
    cr.set_source_rgba(0.0, 0.0, 0.0, if is_dark { 0.3 } else { 0.13 });
    rrect(cr, bx + 5.0, by + 7.0, bw, bh, 4.0);
    cr.fill().ok();
    let _ = cr.restore();

    // Stacked pages visible at edges
    for i in (1..=5).rev() {
        let s = if is_dark {
            0.18 - i as f64 * 0.015
        } else {
            0.91 - i as f64 * 0.025
        };
        let _ = cr.save();
        cr.set_source_rgb(s, s, s);
        rrect(cr, bx + i as f64 * 1.5, by + i as f64 * 1.5, bw, bh, 3.0);
        cr.fill().ok();
        let _ = cr.restore();
    }

    // Main page surface
    let _ = cr.save();
    if is_dark {
        cr.set_source_rgb(0.15, 0.15, 0.15);
    } else {
        cr.set_source_rgb(0.98, 0.98, 0.98);
    }
    rrect(cr, bx, by, bw, bh, 3.0);
    cr.fill().ok();
    let _ = cr.restore();

    // Spine with gradient down the middle
    let _ = cr.save();
    let spine_x = bx + bw / 2.0 - sw / 2.0;
    let grad = cairo::LinearGradient::new(spine_x, by, spine_x + sw, by);
    if is_dark {
        grad.add_color_stop_rgb(0.0, 0.15, 0.15, 0.15);
        grad.add_color_stop_rgb(0.5, 0.05, 0.05, 0.05);
        grad.add_color_stop_rgb(1.0, 0.15, 0.15, 0.15);
    } else {
        grad.add_color_stop_rgb(0.0, 0.9, 0.9, 0.9);
        grad.add_color_stop_rgb(0.5, 0.7, 0.7, 0.7);
        grad.add_color_stop_rgb(1.0, 0.9, 0.9, 0.9);
    }
    cr.set_source(&grad).ok();
    let _ = cr.rectangle(spine_x, by, sw, bh);
    cr.fill().ok();

    // Spine crease
    cr.set_source_rgba(0.0, 0.0, 0.0, if is_dark { 0.5 } else { 0.2 });
    cr.set_line_width(1.0);
    let _ = cr.move_to(bx + bw / 2.0, by);
    let _ = cr.line_to(bx + bw / 2.0, by + bh);
    let _ = cr.stroke();

    // Spine edge shadows for depth illusion
    let shadow_w = (sw * 1.5).min(12.0);

    // Left page: shadow fading rightward from spine
    let sl = cairo::LinearGradient::new(spine_x, by, spine_x + shadow_w, by);
    sl.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, if is_dark { 0.25 } else { 0.14 });
    sl.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    cr.set_source(&sl).ok();
    let _ = cr.rectangle(spine_x, by, shadow_w, bh);
    cr.fill().ok();

    // Right page: shadow fading leftward from spine
    let sr = cairo::LinearGradient::new(spine_x + sw, by, spine_x + sw - shadow_w, by);
    sr.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, if is_dark { 0.25 } else { 0.14 });
    sr.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    cr.set_source(&sr).ok();
    let _ = cr.rectangle(spine_x + sw - shadow_w, by, shadow_w, bh);
    cr.fill().ok();

    let _ = cr.restore();
}

/// Draw subtle paper grain texture on a page area.
fn draw_paper_texture(cr: &cairo::Context, px: f64, py: f64, pw: f64, ph: f64, is_dark: bool) {
    let _ = cr.save();
    let _ = cr.rectangle(px, py, pw, ph);
    cr.clip();
    let grain_alpha = if is_dark { 0.03 } else { 0.025 };
    if is_dark {
        cr.set_source_rgba(1.0, 1.0, 1.0, grain_alpha);
    } else {
        cr.set_source_rgba(0.0, 0.0, 0.0, grain_alpha);
    }
    let step = 5.0_f64;
    let mut y = py;
    let mut row = 0i32;
    while y < py + ph {
        let mut x = px + if row % 2 != 0 { step } else { 0.0 };
        while x < px + pw {
            // Deterministic pseudo-random grain (~20% of positions)
            if ((x as i32 * 73 + y as i32 * 137) % 97) < 19 {
                let _ = cr.rectangle(x, y, 1.5, 1.5);
                cr.fill().ok();
            }
            x += step;
        }
        y += step;
        row += 1;
    }
    let _ = cr.restore();
}

/// Draw a manga screentone pattern inside a panel.
/// pattern_type: 1=dots, 2=diagonal lines, 3=crosshatch
fn draw_screentone(
    cr: &cairo::Context,
    rx: f64,
    ry: f64,
    rw: f64,
    rh: f64,
    pattern_type: i32,
    is_dark: bool,
) {
    let _ = cr.save();
    let _ = cr.rectangle(rx + 1.0, ry + 1.0, rw - 2.0, rh - 2.0);
    cr.clip();

    if is_dark {
        cr.set_source_rgba(0.55, 0.55, 0.55, 0.45);
    } else {
        cr.set_source_rgba(0.25, 0.25, 0.25, 0.35);
    }

    let pi2 = std::f64::consts::PI * 2.0;

    match pattern_type {
        1 => {
            // Dot screentone (halftone)
            let spacing = 7.0_f64;
            let dot_r = 1.2_f64;
            let mut y = ry + spacing / 2.0;
            let mut row = 0i32;
            while y < ry + rh {
                let offset = if row % 2 != 0 { spacing / 2.0 } else { 0.0 };
                let mut x = rx + spacing / 2.0 + offset;
                while x < rx + rw {
                    let _ = cr.arc(x, y, dot_r, 0.0, pi2);
                    cr.fill().ok();
                    x += spacing;
                }
                y += spacing;
                row += 1;
            }
        }
        2 => {
            // Diagonal lines (hatching)
            let spacing = 6.0_f64;
            cr.set_line_width(0.7);
            let total = (rw + rh) as i32;
            for i in (-total..total).step_by(spacing as usize) {
                let _ = cr.move_to(rx + i as f64, ry);
                let _ = cr.line_to(rx + i as f64 + rh, ry + rh);
                let _ = cr.stroke();
            }
        }
        3 => {
            // Crosshatch
            let spacing = 9.0_f64;
            cr.set_line_width(0.5);
            let total = (rw + rh) as i32;
            for i in (-total..total).step_by(spacing as usize) {
                let _ = cr.move_to(rx + i as f64, ry);
                let _ = cr.line_to(rx + i as f64 + rh, ry + rh);
                let _ = cr.stroke();
            }
            for i in (-total..total).step_by(spacing as usize) {
                let _ = cr.move_to(rx + i as f64 + rw, ry);
                let _ = cr.line_to(rx + i as f64 + rw - rh, ry + rh);
                let _ = cr.stroke();
            }
        }
        _ => {}
    }
    let _ = cr.restore();
}

/// Draw a speech bubble with tail and varied content.
fn draw_bubble(cr: &cairo::Context, cx: f64, cy: f64, r: f64, is_dark: bool, bubble_type: &str) {
    if r < 6.0 {
        return;
    }
    let r = r.min(36.0);
    let pi2 = std::f64::consts::PI * 2.0;

    let (ink_r, ink_g, ink_b) = if is_dark {
        (0.15, 0.15, 0.15)
    } else {
        (1.0, 1.0, 1.0)
    };
    let (border_r, border_g, border_b) = if is_dark {
        (0.8, 0.8, 0.8)
    } else {
        (0.05, 0.05, 0.05)
    };
    let (text_r, text_g, text_b) = if is_dark {
        (0.85, 0.85, 0.85)
    } else {
        (0.10, 0.10, 0.10)
    };

    let _ = cr.save();
    cr.set_source_rgb(ink_r, ink_g, ink_b);
    let _ = cr.arc(cx, cy, r, 0.0, pi2);
    cr.fill().ok();
    cr.set_source_rgb(border_r, border_g, border_b);
    cr.set_line_width(1.5);
    let _ = cr.arc(cx, cy, r, 0.0, pi2);
    let _ = cr.stroke();

    // Tail
    cr.set_source_rgb(ink_r, ink_g, ink_b);
    let _ = cr.move_to(cx - r * 0.2, cy + r * 0.78);
    let _ = cr.line_to(cx - r * 0.65, cy + r * 1.45);
    let _ = cr.line_to(cx + r * 0.15, cy + r * 0.88);
    cr.close_path();
    cr.fill_preserve().ok();
    cr.set_source_rgb(border_r, border_g, border_b);
    let _ = cr.stroke();

    // Content varies by type
    cr.set_source_rgb(text_r, text_g, text_b);

    match bubble_type {
        "dots" => {
            // Three horizontal lines (classic placeholder text)
            cr.set_line_width(1.4);
            for (i, yoff) in [-0.28_f64, 0.0, 0.24].iter().enumerate() {
                let lw = r * if i < 2 { 0.72 } else { 0.38 };
                let _ = cr.move_to(cx - lw / 2.0, cy + r * yoff);
                let _ = cr.line_to(cx + lw / 2.0, cy + r * yoff);
                let _ = cr.stroke();
            }
        }
        "japanese" => {
            // Japanese placeholder characters
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            let fs = (r * 0.65).max(8.0);
            cr.set_font_size(fs);
            let chars = "ああ…";
            if let Ok(ext) = cr.text_extents(chars) {
                let _ = cr.move_to(cx - ext.width() / 2.0, cy + ext.height() / 2.0);
                let _ = cr.show_text(chars);
            }
        }
        "lines" => {
            // Empty text block rectangle
            cr.set_line_width(1.2);
            let bw = r * 0.70;
            let bh = r * 0.50;
            let _ = cr.rectangle(cx - bw / 2.0, cy - bh / 2.0, bw, bh);
            let _ = cr.stroke();
        }
        "exclaim" => {
            // Exclamation marks
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            let fs = (r * 0.8).max(9.0);
            cr.set_font_size(fs);
            let text = "!!";
            if let Ok(ext) = cr.text_extents(text) {
                let _ = cr.move_to(cx - ext.width() / 2.0, cy + ext.height() / 2.0);
                let _ = cr.show_text(text);
            }
        }
        _ => {
            // Fallback: dots
            cr.set_line_width(1.4);
            for (i, yoff) in [-0.28_f64, 0.0, 0.24].iter().enumerate() {
                let lw = r * if i < 2 { 0.72 } else { 0.38 };
                let _ = cr.move_to(cx - lw / 2.0, cy + r * yoff);
                let _ = cr.line_to(cx + lw / 2.0, cy + r * yoff);
                let _ = cr.stroke();
            }
        }
    }
    let _ = cr.restore();
}

/// Draw a small red hanko (seal stamp) — authentic manga accent.
fn draw_hanko(cr: &cairo::Context, cx: f64, cy: f64, r: f64, _is_dark: bool) {
    if r < 5.0 {
        return;
    }
    let r = r.min(20.0);
    let _ = cr.save();
    // Red circle
    cr.set_source_rgb(0.80, 0.12, 0.10);
    let _ = cr.arc(cx, cy, r, 0.0, std::f64::consts::PI * 2.0);
    cr.fill().ok();
    // White character inside
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(r * 1.1);
    let text = "漫";
    if let Ok(ext) = cr.text_extents(text) {
        let _ = cr.move_to(
            cx - ext.width() / 2.0 - ext.x_bearing(),
            cy - ext.height() / 2.0 - ext.y_bearing(),
        );
        let _ = cr.show_text(text);
    }
    let _ = cr.restore();
}

/// Draw manga panels for a given layout index.
fn draw_panels(
    cr: &cairo::Context,
    px: f64,
    py: f64,
    pw: f64,
    ph: f64,
    layout_idx: usize,
    is_dark: bool,
    cycle: usize,
) {
    let layout = LAYOUTS[layout_idx % LAYOUTS.len()];

    for (pidx, panel) in layout.iter().enumerate() {
        let rx = px + panel.0 * pw;
        let ry = py + panel.1 * ph;
        let rw = panel.2 * pw;
        let rh = panel.3 * ph;

        // Panel base fill
        let _ = cr.save();
        if is_dark {
            cr.set_source_rgb(0.20, 0.20, 0.20);
        } else {
            cr.set_source_rgb(0.88, 0.88, 0.88);
        }
        let _ = cr.rectangle(rx, ry, rw, rh);
        cr.fill().ok();
        let _ = cr.restore();

        // Screentone texture (rotate pattern with cycle for variation)
        let tone = (layout_idx + pidx + cycle) % 4;
        if tone > 0 && rw > 20.0 && rh > 20.0 {
            draw_screentone(cr, rx, ry, rw, rh, tone as i32, is_dark);
        }

        // Panel border (thicker for clearer structure)
        let _ = cr.save();
        if is_dark {
            cr.set_source_rgb(0.72, 0.72, 0.72);
        } else {
            cr.set_source_rgb(0.05, 0.05, 0.05);
        }
        cr.set_line_width(4.0);
        let _ = cr.rectangle(rx, ry, rw, rh);
        let _ = cr.stroke();
        let _ = cr.restore();

        // Dynamic speech bubble (varies per panel + cycle)
        if rw > 45.0 && rh > 45.0 {
            let btype_idx = (layout_idx + pidx + cycle) % BUBBLE_TYPES.len();
            let boff_idx = (pidx + cycle) % BUBBLE_OFFSETS.len();
            let boff = BUBBLE_OFFSETS[boff_idx];
            draw_bubble(
                cr,
                rx + rw * boff.0,
                ry + rh * boff.1,
                rw.min(rh) * 0.28,
                is_dark,
                BUBBLE_TYPES[btype_idx],
            );
        }

        // Hanko stamp (red seal) — first panel of every 3rd layout
        if pidx == 0 && layout_idx % 3 == 0 && rw > 60.0 && rh > 60.0 {
            draw_hanko(
                cr,
                rx + rw * 0.88,
                ry + rh * 0.88,
                rw.min(rh) * 0.14,
                is_dark,
            );
        }
    }
}

/// State of the sheet while it is mid-turn.
#[derive(Clone, Copy)]
struct Turn {
    /// Rotation progress: 0 on the right spread, 1 flat on the left one. Climbs
    /// a couple of percent past 1 before it settles.
    progress: f64,
    /// Angle the free edge runs ahead of the hinge by, in radians. Positive
    /// while the sheet is being swept, negative for the last moment as the
    /// sheet comes to rest and the edge swings through.
    lag: f64,
}

/// Spring response at `ft` seconds of spring time: rotation progress, plus the
/// rate at which it changes normalised so an even turn would read 1.
fn turn_spring(ft: f64) -> (f64, f64) {
    let zeta = TURN_DAMPING;
    let omega = TURN_SETTLE / (zeta * FLIP_TIME);
    let omega_d = omega * (1.0 - zeta * zeta).sqrt();
    let decay = (-zeta * omega * ft).exp();
    let phase = omega_d * ft;

    // Step response of a second-order system: 0 at rest, 1 at the target.
    let progress = 1.0 - decay * (phase.cos() + (zeta * omega / omega_d) * phase.sin());

    // Its derivative. Writing the decay out of the bracket collapses the cosine
    // terms, leaving a single sine.
    let speed = decay * phase.sin() * omega * omega / omega_d * FLIP_TIME / std::f64::consts::PI;
    (progress, speed)
}

/// The spring runs on an eased clock. A step response tears off the target with
/// zero speed but full acceleration, and at a turn this slow that reads as a
/// jerk — the sheet jumps from lying flat to halfway over. Easing the clock
/// gives the lift a gentle start. `p^1.5` rather than a smoothstep, because a
/// smoothstep also flattens the *end* of the clock, and the last stretch is
/// where the sheet settles: flattening it there just parks the sheet early.
fn turn_clock(p: f64) -> f64 {
    p * p.sqrt()
}

/// `turn_clock` differentiated, for turning a rate per unit of spring time into
/// a rate per unit of the flip.
fn turn_clock_gain(p: f64) -> f64 {
    1.5 * p.sqrt()
}

/// Rotation progress and its real rate at a fraction `p` of the flip.
fn turn_motion(p: f64) -> (f64, f64) {
    let (progress, speed) = turn_spring(turn_clock(p) * FLIP_TIME);
    (progress, speed * turn_clock_gain(p))
}

/// Motion of the sheet, or `None` while the spread rests.
fn turn_phase(t: f64) -> Option<Turn> {
    if t < STATIC_TIME {
        return None;
    }
    let ft = t - STATIC_TIME;
    if ft >= FLIP_TIME {
        return None;
    }
    let p = ft / FLIP_TIME;

    let (mut progress, rate) = turn_motion(p);

    // Trim the asymptotic remainder over the tail. Without it the sheet would
    // still be a fraction of a degree off flat when the flip ends, and the
    // resting spread is drawn from scratch, so that residue showed up as a pop.
    let (end, _) = turn_spring(FLIP_TIME);
    let q = ((p - TURN_TRIM_START) / (1.0 - TURN_TRIM_START)).clamp(0.0, 1.0);
    progress += (1.0 - end) * (q * q * (3.0 - 2.0 * q));

    Some(Turn {
        progress,
        // The edge only bends once the sheet is moving, and it unbends as the
        // sheet comes to rest. Late in the flip the rate goes negative and the
        // sign flips, which leaves the edge a fraction of a degree up off the
        // page for the last moment before it settles.
        lag: PAPER_CURL * rate / TURN_RATE_PEAK,
    })
}

/// Rotation of the sheet at distance `d` from the spine.
///
/// The free edge runs ahead of the hinge: the offset grows towards the free
/// side as a cubic, so the sheet bows smoothly instead of hinging like a plank.
/// `lag` is that lead, in radians, for this instant.
fn turn_theta_at(d: f64, pw: f64, theta: f64, lag: f64) -> f64 {
    let w = (d / pw).clamp(0.0, 1.0).powi(3);
    theta + lag * w
}

/// Project a point of the turning page: screen x plus the perspective scale
/// that applies to the vertical axis.
fn project_page(d: f64, theta: f64, hinge: f64, cam: f64, cam_x: f64) -> (f64, f64) {
    // Never let the sheet sink below the plane it lies on. Past 180 degrees the
    // sine goes negative, and feeding that into the perspective would shrink the
    // whole sheet vertically for as long as the overshoot lasts.
    let height = (d * theta.sin()).max(0.0);
    let scale = cam / (cam - height).max(cam * 0.2);
    let x = cam_x + scale * (hinge + d * theta.cos() - cam_x);
    (x, scale)
}

type PageKey = (usize, bool, i32, i32);
type SpreadKey = (usize, usize, bool, i32, i32, i32);
type GrainKey = (i32, i32, bool, i32);

thread_local! {
    /// Pages already rendered for the turning sheet. Redrawing the panels into a
    /// fresh surface on every frame dominated the CPU cost of the placeholder,
    /// and the same page is on screen for a whole turn.
    static PAGE_CACHE: std::cell::RefCell<Vec<(PageKey, cairo::ImageSurface)>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Panels and gutter for one spread. The spread only changes when the page
    /// index changes — three times per cycle at most — but it was being redrawn
    /// from scratch on every frame, screentone included.
    static SPREAD_CACHE: std::cell::RefCell<Vec<(SpreadKey, cairo::ImageSurface)>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Paper grain. The dots are laid out deterministically from their position,
    /// so the pattern never changes for a given size; building ~3.600 one-pixel
    /// fills per frame was pure waste.
    static GRAIN_CACHE: std::cell::RefCell<Vec<(GrainKey, cairo::ImageSurface)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Ceiling on the cached spreads. A spread surface is `pw * 2` by `ph` at four
/// bytes a pixel, which is a few megabytes at a normal window size but grows
/// with the window; past the ceiling a spread is simply drawn directly, which is
/// what used to happen every frame anyway.
const SPREAD_CACHE_BUDGET: usize = 24 << 20;
/// Entries kept per cache. Three spreads cover one cycle: the resting spread,
/// the pair mid-turn, and the pair it lands on.
const CACHE_ENTRIES: usize = 3;

fn surface_bytes(surface: &cairo::ImageSurface) -> usize {
    (surface.width().max(0) as usize) * (surface.height().max(0) as usize) * 4
}

/// Panels plus gutter for the spread `left_idx` | `right_idx`, rendered once and
/// reused until the page pair changes.
///
/// `pixel` is the width of one device pixel in user units: the surface is
/// allocated at device resolution and the context scaled to match, so blitting
/// it back at a pixel-aligned origin is a 1:1 copy. Rendering it at user
/// resolution instead resamples every panel border on the way out, which shows
/// as a softness the direct drawing did not have.
fn spread_surface(
    left_idx: usize,
    right_idx: usize,
    is_dark: bool,
    pw: f64,
    ph: f64,
    pixel: f64,
) -> Option<cairo::ImageSurface> {
    let scale = 1.0 / pixel;
    let w_px = (pw * 2.0 * scale).round().max(1.0) as i32;
    let h_px = (ph * scale).round().max(1.0) as i32;
    let key = (
        left_idx,
        right_idx,
        is_dark,
        w_px,
        h_px,
        (pixel * 1000.0) as i32,
    );

    if let Some(cached) = SPREAD_CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, surface)| surface.clone())
    }) {
        return Some(cached);
    }

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w_px, h_px).ok()?;
    if surface_bytes(&surface) > SPREAD_CACHE_BUDGET {
        return None;
    }

    {
        let ctx = cairo::Context::new(&surface).ok()?;
        ctx.scale(scale, scale);
        // Left page at the surface origin, right page one page width along — the
        // same offset the two halves have on screen.
        draw_panels(&ctx, 0.0, 0.0, pw, ph, left_idx, is_dark, 0);
        draw_panels(&ctx, pw, 0.0, pw, ph, right_idx, is_dark, 0);
        draw_spine_overlay(&ctx, pw, 0.0, ph, is_dark);
    }

    SPREAD_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        while !cache.is_empty()
            && (cache.len() >= CACHE_ENTRIES
                || cache.iter().map(|(_, s)| surface_bytes(s)).sum::<usize>()
                    + surface_bytes(&surface)
                    > SPREAD_CACHE_BUDGET)
        {
            cache.remove(0);
        }
        cache.push((key, surface.clone()));
    });

    Some(surface)
}

/// Paper grain for a spread, rendered once into its own surface.
fn grain_surface(pw: f64, ph: f64, is_dark: bool, pixel: f64) -> Option<cairo::ImageSurface> {
    let scale = 1.0 / pixel;
    let w_px = (pw * scale).round().max(1.0) as i32;
    let h_px = (ph * scale).round().max(1.0) as i32;
    let key = (w_px, h_px, is_dark, (pixel * 1000.0) as i32);

    if let Some(cached) = GRAIN_CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, surface)| surface.clone())
    }) {
        return Some(cached);
    }

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w_px, h_px).ok()?;
    {
        let ctx = cairo::Context::new(&surface).ok()?;
        ctx.scale(scale, scale);
        draw_paper_texture(&ctx, 0.0, 0.0, pw, ph, is_dark);
    }

    GRAIN_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        while cache.len() >= CACHE_ENTRIES {
            cache.remove(0);
        }
        cache.push((key, surface.clone()));
    });

    Some(surface)
}

/// Render one page (paper, tone, panels) into its own surface so the turn can
/// sample it strip by strip instead of redrawing the panels per strip.
fn page_surface(
    layout_idx: usize,
    is_dark: bool,
    pw: i32,
    ph: i32,
    base: (f64, f64, f64),
) -> Option<cairo::ImageSurface> {
    let key = (layout_idx, is_dark, pw, ph);

    if let Some(cached) = PAGE_CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, surface)| surface.clone())
    }) {
        return Some(cached);
    }

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, pw.max(1), ph.max(1)).ok()?;
    {
        let ctx = cairo::Context::new(&surface).ok()?;
        ctx.set_operator(cairo::Operator::Source);
        ctx.set_source_rgb(base.0, base.1, base.2);
        ctx.paint().ok();
        ctx.set_operator(cairo::Operator::Over);

        // No paper grain here: the grain is applied once over the finished
        // spread instead, so the sheet matches the page it lands on exactly.
        draw_panels(&ctx, 0.0, 0.0, pw as f64, ph as f64, layout_idx, is_dark, 0);
    }

    PAGE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= 6 {
            cache.remove(0);
        }
        cache.push((key, surface.clone()));
    });

    Some(surface)
}

/// Soft shadow the lifted sheet throws onto the spread it uncovers.
#[allow(clippy::too_many_arguments)]
fn draw_turn_shadow(
    cr: &cairo::Context,
    theta: f64,
    lag: f64,
    hinge: f64,
    py: f64,
    ph: f64,
    pw: f64,
    is_dark: bool,
) {
    let cam = CAM_HEIGHT * pw;
    let cam_x = hinge + CAM_OFFSET * pw;
    let (lead_x, _) = project_page(pw, turn_theta_at(pw, pw, theta, lag), hinge, cam, cam_x);

    let alpha = theta.sin().max(0.0) * if is_dark { 0.62 } else { 0.42 };
    if alpha < 0.01 {
        return;
    }

    // The shadow falls on the side the sheet has moved away from.
    let dir = if theta.cos() >= 0.0 { 1.0 } else { -1.0 };
    let band = (pw * 0.45).max(16.0);
    let (x0, x1) = if dir > 0.0 {
        (lead_x, lead_x + band)
    } else {
        (lead_x - band, lead_x)
    };

    let _ = cr.save();

    // Wide, soft part: the sheet is a hand's width above the spread.
    let outer = cairo::LinearGradient::new(lead_x, 0.0, lead_x + dir * band, 0.0);
    outer.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, alpha);
    outer.add_color_stop_rgba(0.35, 0.0, 0.0, 0.0, alpha * 0.45);
    outer.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    cr.set_source(&outer).ok();
    let _ = cr.rectangle(x0, py, x1 - x0, ph);
    cr.fill().ok();

    // Tight, dark core right at the edge, so the sheet visibly leaves the page.
    let core = (band * 0.18).max(3.0);
    let (c0, c1) = if dir > 0.0 {
        (lead_x, lead_x + core)
    } else {
        (lead_x - core, lead_x)
    };
    let tight = cairo::LinearGradient::new(lead_x, 0.0, lead_x + dir * core, 0.0);
    tight.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, (alpha * 1.5).min(0.85));
    tight.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    cr.set_source(&tight).ok();
    let _ = cr.rectangle(c0, py, c1 - c0, ph);
    cr.fill().ok();

    let _ = cr.restore();
}

/// Draw the sheet that is mid-turn, one vertical strip at a time.
///
/// `mirrored` selects the back of the sheet (once it passes 90 degrees), which
/// carries the page it is turning into: the left half of the next spread.
#[allow(clippy::too_many_arguments)]
fn draw_turning_page(
    cr: &cairo::Context,
    surface: &cairo::ImageSurface,
    mirrored: bool,
    theta: f64,
    lag: f64,
    hinge: f64,
    cy: f64,
    py: f64,
    ph: f64,
    pw: f64,
    is_dark: bool,
) {
    let cam = CAM_HEIGHT * pw;
    let cam_x = hinge + CAM_OFFSET * pw;
    let lift = theta.sin().max(0.0);
    let step = pw / TURN_STRIPS as f64;

    // Width of one device pixel in the coordinates the strips are laid out in.
    // Snapping against this rather than a plain integer keeps the clip edges on
    // pixel boundaries even at a fractional UI scale (1.25, 1.5 on Wayland),
    // where whole user-space units land between pixels.
    let pixel = cr
        .device_to_user_distance(1.0, 1.0)
        .map(|(dx, _)| dx)
        .ok()
        .filter(|dx| *dx > 0.0)
        .unwrap_or(1.0);

    // Projected boundary along the sheet: (x, y top, y bottom).
    let mut edge: Vec<(f64, f64, f64)> = Vec::with_capacity(TURN_STRIPS + 1);
    for i in 0..=TURN_STRIPS {
        let d = i as f64 * step;
        let (x, s) = project_page(d, turn_theta_at(d, pw, theta, lag), hinge, cam, cam_x);
        edge.push((x, cy + s * (py - cy), cy + s * (py + ph - cy)));
    }

    // Pass 1 — the printed surface. Neighbouring strips share their boundary
    // exactly and are drawn without overlap, so nothing blends twice.
    for i in 0..TURN_STRIPS {
        let d0 = i as f64 * step;
        let d1 = d0 + step;
        let (x0, top0, bot0) = edge[i];
        let (x1, top1, bot1) = edge[i + 1];
        if !(x0.is_finite() && x1.is_finite()) {
            continue;
        }

        // Front face: the surface is measured from the spine (x = d).
        // Back face: the same sheet seen from behind, so x = pw - d.
        let (src0, src1) = if mirrored {
            (pw - d0, pw - d1)
        } else {
            (d0, d1)
        };
        let src_dx = src1 - src0;
        if src_dx.abs() < 1e-6 {
            continue;
        }

        let sy = 0.5 * ((top0 - cy) + (top1 - cy)) / (py - cy);
        let top_mid = cy + sy * (py - cy);
        let sx = (x1 - x0) / src_dx;

        let _ = cr.save();
        // Neighbouring strips share this boundary exactly, but the clip is
        // antialiased, so at the shared column each one only reached partial
        // coverage and the page underneath bled through as a hairline — bright
        // over the panels, with the red seal of the spread below visible inside
        // it. Snapping the two vertical clip edges to whole pixels puts them on
        // pixel boundaries, where coverage is already all-or-nothing, so the
        // strips tile without a gap while the top and bottom edges — the ones
        // that actually need smoothing — keep their antialiasing. Overlapping the
        // strips instead makes it worse: two partial coverages under `Over` blend
        // two different resamplings of the page into a visible line.
        let cx0 = (x0 / pixel).round() * pixel;
        let cx1 = (x1 / pixel).round() * pixel;
        let _ = cr.move_to(cx0, top0);
        let _ = cr.line_to(cx1, top1);
        let _ = cr.line_to(cx1, bot1);
        let _ = cr.line_to(cx0, bot0);
        cr.close_path();
        cr.clip();

        // The surface starts at (0, 0), so the page's top edge is `top_mid` —
        // not `top_mid - py`, which shifted the whole sheet up by the page offset.
        cr.transform(cairo::Matrix::new(
            sx,
            0.0,
            0.0,
            sy,
            x0 - src0 * sx,
            top_mid,
        ));
        // `set_source_surface` locks the pattern to the user space in effect
        // here; creating a bare SurfacePattern does not, and painted nothing.
        cr.set_source_surface(surface, 0.0, 0.0).ok();
        // These strips are only a few pixels wide, so the slower resampling path
        // of the default filter costs far more than it shows.
        cr.source().set_filter(cairo::Filter::Fast);
        cr.paint().ok();
        let _ = cr.restore();
    }

    // Pass 2 — one gradient across the whole silhouette. A sheet lifted towards
    // the light catches it: brightest while it stands up, dimmer towards the
    // free edge where the paper curls away. On a light theme the same shape has
    // to read as a shadow instead.
    let _ = cr.save();
    let _ = cr.move_to(edge[0].0, edge[0].1);
    for p in &edge[1..] {
        let _ = cr.line_to(p.0, p.1);
    }
    for p in edge.iter().rev() {
        let _ = cr.line_to(p.0, p.2);
    }
    cr.close_path();
    cr.clip();

    let spine_x = edge[0].0;
    let lead_x = edge[TURN_STRIPS].0;
    let dir = if lead_x >= spine_x { 1.0 } else { -1.0 };

    let (tint, strength) = if is_dark {
        ((1.0, 1.0, 1.0), 1.0)
    } else {
        ((0.0, 0.0, 0.0), 0.55)
    };
    let sheen = lift * if mirrored { 0.12 } else { 0.22 } * strength;
    if sheen > 0.004 {
        // Never fade to zero across the sheet: the sheet has to read as one
        // surface, with the light falling off only towards the curled edge.
        let g = cairo::LinearGradient::new(spine_x, 0.0, lead_x, 0.0);
        g.add_color_stop_rgba(0.0, tint.0, tint.1, tint.2, sheen * 0.55);
        g.add_color_stop_rgba(0.30, tint.0, tint.1, tint.2, sheen);
        g.add_color_stop_rgba(0.80, tint.0, tint.1, tint.2, sheen * 0.75);
        g.add_color_stop_rgba(1.0, tint.0, tint.1, tint.2, sheen * 0.5);
        cr.set_source(&g).ok();
        cr.paint().ok();
    }

    // The curled free edge turns away from the light.
    let curl = lift * 0.34;
    if curl > 0.004 {
        let band = ((lead_x - spine_x).abs() * 0.22).max(2.0);
        let g = cairo::LinearGradient::new(lead_x, 0.0, lead_x - dir * band, 0.0);
        g.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, curl.min(0.34));
        g.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
        cr.set_source(&g).ok();
        cr.paint().ok();
    }
    // Crisp outline of the sheet, so its edges stay readable independent of how
    // much light happens to fall on them.
    let outline = lift * if is_dark { 0.28 } else { 0.35 };
    if outline > 0.004 {
        cr.set_source_rgba(1.0, 1.0, 1.0, outline);
        cr.set_line_width(1.0);
        let _ = cr.move_to(edge[0].0, edge[0].1);
        for p in &edge[1..] {
            let _ = cr.line_to(p.0, p.1);
        }
        for p in edge.iter().rev() {
            let _ = cr.line_to(p.0, p.2);
        }
        // Left open on purpose: the hinge edge sits in the gutter and should not
        // get a bright line of its own.
        let _ = cr.stroke();
    }
    let _ = cr.restore();
    // Leading edge of the free side: a light rim so the sheet reads as paper
    // with a thickness.
    let (edge_x, edge_s) = project_page(pw, turn_theta_at(pw, pw, theta, lag), hinge, cam, cam_x);
    let edge_top = cy + edge_s * (py - cy);
    let edge_bot = cy + edge_s * (py + ph - cy);

    let _ = cr.save();
    cr.set_source_rgba(1.0, 1.0, 1.0, if is_dark { 0.34 } else { 0.65 } * lift);
    cr.set_line_width((pw * 0.008).clamp(0.8, 1.8));
    let _ = cr.move_to(edge_x, edge_top);
    let _ = cr.line_to(edge_x, edge_bot);
    let _ = cr.stroke();
    let _ = cr.restore();
}
/// Gutter crease and inner shadow, drawn after the page content so the panels
/// do not paint over it.
fn draw_spine_overlay(cr: &cairo::Context, cx: f64, py: f64, ph: f64, is_dark: bool) {
    let width = (ph * 0.035).clamp(6.0, 16.0);
    let strength = if is_dark { 0.38 } else { 0.18 };

    let _ = cr.save();
    let left = cairo::LinearGradient::new(cx - width, 0.0, cx, 0.0);
    left.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
    left.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, strength);
    cr.set_source(&left).ok();
    let _ = cr.rectangle(cx - width, py, width, ph);
    cr.fill().ok();

    let right = cairo::LinearGradient::new(cx, 0.0, cx + width, 0.0);
    right.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, strength);
    right.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    cr.set_source(&right).ok();
    let _ = cr.rectangle(cx, py, width, ph);
    cr.fill().ok();

    cr.set_source_rgba(0.0, 0.0, 0.0, if is_dark { 0.45 } else { 0.22 });
    cr.set_line_width(1.0);
    let _ = cr.move_to(cx, py);
    let _ = cr.line_to(cx, py + ph);
    let _ = cr.stroke();
    let _ = cr.restore();
}

/// Draw "Kein Bild geladen" loading text with hollow-to-fill progress bar effect and glow.
fn draw_loading_text(
    cr: &cairo::Context,
    bx: f64,
    by: f64,
    bw: f64,
    bh: f64,
    is_dark: bool,
    _w: f64,
    h: f64,
    t: f64,
) {
    let text = i18n::t("Kein Bild geladen");
    let font_size = (bw * 0.042).min(20.0).max(8.0);

    let _ = cr.save();
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(font_size);

    // Text metrics
    let extents = cr
        .text_extents(&text)
        .unwrap_or_else(|_| text_extents_zeroed());
    let text_left = bx + (bw - extents.width()) / 2.0;
    let tx = text_left - extents.x_bearing();
    let ty = by + bh + font_size * 1.4;

    // Abort if not enough space below the book
    if ty + extents.height() + 4.0 > h {
        let _ = cr.restore();
        return;
    }

    // Colours: the unfilled part stays readable instead of turning into a
    // black outline, which on a dark placeholder looked like broken text.
    let (base_r, base_g, base_b) = if is_dark {
        (0.34, 0.34, 0.37)
    } else {
        (0.60, 0.60, 0.64)
    };
    let (fill_r, fill_g, fill_b) = if is_dark {
        (0.93, 0.93, 0.95)
    } else {
        (0.10, 0.10, 0.12)
    };

    // Unfilled text plus a soft pulse, so it never looks stale.
    cr.set_source_rgb(base_r, base_g, base_b);
    let _ = cr.move_to(tx, ty);
    let _ = cr.show_text(&text);

    // Fill left-to-right, tied to the cycle.
    let progress = (t / CYCLE).clamp(0.0, 1.0);
    if progress > 0.005 {
        let clip_w = extents.width() * progress;
        let _ = cr.save();
        let _ = cr.rectangle(
            text_left - 1.0,
            ty + extents.y_bearing() - 3.0,
            clip_w + 2.0,
            extents.height() + 6.0,
        );
        cr.clip();

        // A small glow behind the filled part reads as a highlight running
        // across the label rather than a hard wipe.
        let pulse = 0.5 + 0.5 * (t * std::f64::consts::PI).sin();
        cr.set_source_rgba(
            1.0,
            1.0,
            1.0,
            if is_dark { 0.10 + pulse * 0.10 } else { 0.0 },
        );
        cr.set_line_width((font_size * 0.35).max(4.0));
        let _ = cr.move_to(tx, ty);
        let _ = cr.text_path(&text);
        let _ = cr.stroke();

        cr.set_source_rgb(fill_r, fill_g, fill_b);
        let _ = cr.move_to(tx, ty);
        let _ = cr.show_text(&text);
        let _ = cr.restore();
    }

    let _ = cr.restore();
}

/// Main draw function for the comic book placeholder animation.
///
/// Ported from Python ComicBookPlaceholder — draws a two-page manga book
/// with panels, speech bubbles, screentone textures, hanko stamps,
/// and a page that lifts off the spine and turns over, with perspective,
/// paper curl, curvature shading and a cast shadow.
fn draw_placeholder(cr: &cairo::Context, width: i32, height: i32) {
    // Time-based animation state (stateless — computed from system clock)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs_f64();

    let cycle_number = (total_secs / CYCLE) as usize;
    let t = total_secs % CYCLE;

    draw_placeholder_at(
        cr,
        width,
        height,
        t,
        cycle_number,
        adw::StyleManager::default().is_dark(),
    );
}

/// Draw the placeholder for an explicit point in the animation. Split out from
/// [`draw_placeholder`] so the frames can be rendered deterministically.
fn draw_placeholder_at(
    cr: &cairo::Context,
    width: i32,
    height: i32,
    t: f64,
    cycle_number: usize,
    is_dark: bool,
) {
    let w = width as f64;
    let h = height as f64;

    if w < 20.0 || h < 20.0 {
        return;
    }

    let page = (cycle_number * 2) % LAYOUTS.len();

    // Background fill
    if is_dark {
        cr.set_source_rgb(0.05, 0.05, 0.07);
    } else {
        cr.set_source_rgb(0.92, 0.92, 0.94);
    }
    cr.paint().ok();

    // Book rectangle
    let (bx, by, bw, bh) = book_rect(w, h);
    let sw = bw * 0.02;

    // Book structure (shadow, stacked pages, spine)
    draw_book(cr, bx, by, bw, bh, sw, is_dark);

    // Page geometry. Both halves share the hinge at the spine, which is what
    // lets the turning sheet land exactly on the left half at 180 degrees.
    //
    // Snapped to whole device pixels: the spread and the grain come from cached
    // surfaces, and a fractional origin would force cairo to resample them on the
    // way out, softening every panel border. Snapping also makes the borders
    // themselves land on the pixel grid.
    let pixel = cr
        .device_to_user_distance(1.0, 1.0)
        .map(|(dx, _)| dx)
        .ok()
        .filter(|dx| *dx > 0.0)
        .unwrap_or(1.0);
    let snap = |v: f64| (v / pixel).round() * pixel;

    let inset = 3.0;
    let ph = snap(bh - inset * 2.0);
    let pw = snap((bw - sw) / 2.0 - inset);
    let cx = snap(bx + bw / 2.0);
    let cy = by + bh / 2.0;
    let py = snap(by + inset);
    let left_px = cx - pw;
    let right_px = cx;

    let page_base = if is_dark {
        (0.15, 0.15, 0.15)
    } else {
        (0.98, 0.98, 0.98)
    };

    let left_idx = page;
    let right_idx = (page + 1) % LAYOUTS.len();
    let next_left = (page + 2) % LAYOUTS.len();
    let next_right = (page + 3) % LAYOUTS.len();

    let turn = turn_phase(t);

    // Near either end the sheet lies flat on a page it matches, so that page is
    // drawn underneath as well: the hairlines between strips then blend into
    // identical content instead of revealing the old page. Three pairs cover the
    // whole cycle, which is what makes the spread cacheable at all.
    let (under_left, under_right) = match turn {
        Some(turn) if turn.progress > 0.97 => (next_left, next_right),
        Some(turn) if turn.progress < 0.03 => (left_idx, right_idx),
        Some(_) => (left_idx, next_right),
        None => (left_idx, right_idx),
    };

    match spread_surface(under_left, under_right, is_dark, pw, ph, pixel) {
        Some(surface) => {
            cr.set_source_surface(&surface, left_px, py).ok();
            cr.paint().ok();
        }
        // Over the cache ceiling, or the surface could not be created.
        None => {
            draw_panels(cr, left_px, py, pw, ph, under_left, is_dark, 0);
            draw_panels(cr, right_px, py, pw, ph, under_right, is_dark, 0);
            draw_spine_overlay(cr, cx, py, ph, is_dark);
        }
    }

    // Only the sheet itself still has to be drawn per frame.
    if let Some(turn) = turn {
        let theta = std::f64::consts::PI * turn.progress;
        let mirrored = theta > std::f64::consts::FRAC_PI_2;

        draw_turn_shadow(cr, theta, turn.lag, cx, py, ph, pw, is_dark);

        // The front of the sheet is the page being turned away, its back is the
        // page it becomes on the left half of the next spread.
        let turning_idx = if mirrored { next_left } else { right_idx };
        let (pw_px, ph_px) = (pw.round() as i32, ph.round() as i32);
        if let Some(surface) = page_surface(turning_idx, is_dark, pw_px, ph_px, page_base) {
            draw_turning_page(
                cr, &surface, mirrored, theta, turn.lag, cx, cy, py, ph, pw, is_dark,
            );
        }
    }

    // Paper grain over the finished spread, applied once: the turning sheet comes
    // from a cached surface, so a second grain pass at a different phase used to
    // make the texture flicker the moment the turn ended.
    match grain_surface(pw * 2.0, ph, is_dark, pixel) {
        Some(surface) => {
            cr.set_source_surface(&surface, left_px, py).ok();
            cr.paint().ok();
        }
        None => draw_paper_texture(cr, left_px, py, pw * 2.0, ph, is_dark),
    }

    // Loading text with progress fill and glow
    draw_loading_text(cr, bx, by, bw, bh, is_dark, w, h, t);
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

impl Preview {
    /// Create a new preview widget.
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Load an original image from a file path.
    ///
    /// Reads the file on a background thread to avoid blocking the UI.
    /// The texture is created on the main thread once bytes arrive.
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn load_original(&self, path: &std::path::Path) {
        let priv_ = self.imp();

        // Update UI state immediately (path info, not image-dependent)
        *priv_.original_path.borrow_mut() = Some(path.to_path_buf());
        if let Some(label) = priv_.info_label.borrow().as_ref() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            label.set_label(name);
        }
        priv_.sync_mode_group(PreviewMode::Original);

        // Decode on a background thread; only texture creation stays on the main thread.
        let path_buf = path.to_path_buf();
        let (tx, rx) = async_channel::bounded::<Option<DecodedPage>>(1);
        std::thread::spawn(move || {
            let page = decode_page(&path_buf);
            let _ = tx.send_blocking(page);
        });

        // Main thread: wrap the decoded pixels and update the UI
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Ok(Some(page)) = rx.recv().await else {
                return;
            };
            let Some(obj) = weak.upgrade() else { return };
            let priv_ = obj.imp();

            let (width, height) = (page.width, page.height);
            let texture = texture_from_page(page);
            if let Some(pic) = priv_.original_picture.borrow().as_ref() {
                pic.set_paintable(Some(&texture));
            }
            if let Some(label) = priv_.dimensions_label.borrow().as_ref() {
                label.set_label(&format!("{}×{}", width, height));
            }
            if let Some(stack) = priv_.stack.borrow().as_ref() {
                stack.set_visible_child_name("images");
            }
        });
    }

    /// Load a translated image from a file path.
    ///
    /// Does not switch the view mode — the user can toggle to see it.
    /// Reads the file on a background thread to avoid blocking the UI.
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn load_translated(&self, path: &std::path::Path) {
        let priv_ = self.imp();

        *priv_.translated_path.borrow_mut() = Some(path.to_path_buf());

        // Decode on a background thread; only texture creation stays on the main thread.
        let path_buf = path.to_path_buf();
        let (tx, rx) = async_channel::bounded::<Option<DecodedPage>>(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(decode_page(&path_buf));
        });

        // Main thread: build texture + Cairo surface from the decoded buffer
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Ok(Some(page)) = rx.recv().await else {
                return;
            };
            let Some(obj) = weak.upgrade() else { return };
            let priv_ = obj.imp();

            if let Some(surface) = surface_from_page(&page) {
                *priv_.translated_surface.borrow_mut() = Some(surface);
            }
            *priv_.translated_texture.borrow_mut() = Some(texture_from_page(page));

            // Redraw overlay
            if let Some(overlay) = priv_.translated_overlay.borrow().as_ref() {
                overlay.queue_draw();
            }

            // If there's no original yet, show the translated image
            if priv_.original_path.borrow().is_none() {
                if let Some(stack) = priv_.stack.borrow().as_ref() {
                    stack.set_visible_child_name("images");
                }
                priv_.sync_mode_group(PreviewMode::Translated);
            }
        });
    }

    /// Load both original and translated images at once.
    ///
    /// Automatically switches to comparison mode if both are available.
    /// Both images are loaded in parallel on background threads.
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub fn load_pair(
        &self,
        original: Option<&std::path::Path>,
        translated: Option<&std::path::Path>,
    ) {
        let priv_ = self.imp();

        // Update path info immediately (not image-dependent)
        if let Some(path) = original {
            *priv_.original_path.borrow_mut() = Some(path.to_path_buf());
            if let Some(label) = priv_.info_label.borrow().as_ref() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                label.set_label(name);
            }
        } else {
            *priv_.original_path.borrow_mut() = None;
        }
        match translated {
            Some(path) => *priv_.translated_path.borrow_mut() = Some(path.to_path_buf()),
            None => {
                // Do not leave the previous page's translation on screen.
                *priv_.translated_path.borrow_mut() = None;
                *priv_.translated_texture.borrow_mut() = None;
                *priv_.translated_surface.borrow_mut() = None;
                if let Some(overlay) = priv_.translated_overlay.borrow().as_ref() {
                    overlay.queue_draw();
                }
            }
        }

        // Capture for mode switching after both loads complete
        let has_original = original.is_some();
        let has_translated = translated.is_some();

        // Spawn background threads for both images — they run in parallel
        let orig_rx = original.map(|path| {
            let path_buf = path.to_path_buf();
            let (tx, rx) = async_channel::bounded::<Option<DecodedPage>>(1);
            std::thread::spawn(move || {
                let _ = tx.send_blocking(decode_page(&path_buf));
            });
            rx
        });

        let trans_rx = translated.map(|path| {
            let path_buf = path.to_path_buf();
            let (tx, rx) = async_channel::bounded::<Option<DecodedPage>>(1);
            std::thread::spawn(move || {
                let _ = tx.send_blocking(decode_page(&path_buf));
            });
            rx
        });

        // Main thread: create textures from bytes and update UI
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(obj) = weak.upgrade() else { return };
            let priv_ = obj.imp();

            // Load original texture
            if let Some(rx) = orig_rx {
                if let Ok(Some(page)) = rx.recv().await {
                    let (width, height) = (page.width, page.height);
                    let texture = texture_from_page(page);
                    if let Some(pic) = priv_.original_picture.borrow().as_ref() {
                        pic.set_paintable(Some(&texture));
                    }
                    if let Some(label) = priv_.dimensions_label.borrow().as_ref() {
                        label.set_label(&format!("{}×{}", width, height));
                    }
                }
            }

            // Load translated texture → cache Cairo surface
            if let Some(rx) = trans_rx {
                if let Ok(Some(page)) = rx.recv().await {
                    if let Some(surface) = surface_from_page(&page) {
                        *priv_.translated_surface.borrow_mut() = Some(surface);
                    }
                    *priv_.translated_texture.borrow_mut() = Some(texture_from_page(page));
                    if let Some(overlay) = priv_.translated_overlay.borrow().as_ref() {
                        overlay.queue_draw();
                    }
                }
            }

            // Switch to image view
            if let Some(stack) = priv_.stack.borrow().as_ref() {
                stack.set_visible_child_name("images");
            }

            // Auto-switch to compare mode if both images are available
            if has_original && has_translated {
                priv_.sync_mode_group(PreviewMode::Compare);
            } else if has_original {
                priv_.sync_mode_group(PreviewMode::Original);
            } else if has_translated {
                priv_.sync_mode_group(PreviewMode::Translated);
            }
        });
    }

    /// Clear the preview and show the placeholder.
    pub fn clear(&self) {
        let priv_ = self.imp();

        if let Some(pic) = priv_.original_picture.borrow().as_ref() {
            pic.set_paintable(None::<&gtk::gdk::Paintable>);
        }
        *priv_.translated_texture.borrow_mut() = None;
        *priv_.translated_surface.borrow_mut() = None;
        if let Some(overlay) = priv_.translated_overlay.borrow().as_ref() {
            overlay.queue_draw();
        }

        *priv_.original_path.borrow_mut() = None;
        *priv_.translated_path.borrow_mut() = None;

        if let Some(stack) = priv_.stack.borrow().as_ref() {
            stack.set_visible_child_name("placeholder");
        }
        if let Some(label) = priv_.info_label.borrow().as_ref() {
            label.set_label(&i18n::t("Keine Datei ausgewählt"));
        }
        if let Some(label) = priv_.dimensions_label.borrow().as_ref() {
            label.set_label("");
        }

        // Reset to original mode
        priv_.sync_mode_group(PreviewMode::Original);
    }

    /// Set the preview mode programmatically.
    pub fn set_mode(&self, mode: PreviewMode) {
        let priv_ = self.imp();
        priv_.set_mode(mode);

        // Keep the segmented control in sync without re-entering set_mode.
        priv_.sync_mode_group(mode);
    }

    /// Get the current preview mode.
    pub fn mode(&self) -> PreviewMode {
        *self.imp().mode.borrow()
    }

    /// Get the path of the currently loaded original image.
    pub fn original_path(&self) -> Option<PathBuf> {
        self.imp().original_path.borrow().clone()
    }

    /// Get the path of the currently loaded translated image.
    pub fn translated_path(&self) -> Option<PathBuf> {
        self.imp().translated_path.borrow().clone()
    }

    /// Check if an original image is loaded.
    pub fn has_original(&self) -> bool {
        self.imp().original_path.borrow().is_some()
    }

    /// Check if a translated image is loaded.
    pub fn has_translated(&self) -> bool {
        self.imp().translated_path.borrow().is_some()
    }

    /// Set the comparison slider position (0.0 = far left, 1.0 = far right).
    pub fn set_slider_position(&self, pos: f64) {
        let pos = pos.clamp(0.02, 0.98);
        let priv_ = self.imp();
        *priv_.slider_position.borrow_mut() = pos;

        // Redraw slider and translated overlay
        if let Some(slider) = priv_.slider_widget.borrow().as_ref() {
            slider.queue_draw();
        }
        if let Some(overlay) = priv_.translated_overlay.borrow().as_ref() {
            overlay.queue_draw();
        }
    }

    /// Get the current slider position.
    pub fn slider_position(&self) -> f64 {
        *self.imp().slider_position.borrow()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_mode_default() {
        assert_eq!(PreviewMode::default(), PreviewMode::Original);
    }

    #[test]
    fn test_preview_mode_equality() {
        assert_eq!(PreviewMode::Original, PreviewMode::Original);
        assert_ne!(PreviewMode::Original, PreviewMode::Translated);
        assert_ne!(PreviewMode::Original, PreviewMode::Compare);
    }

    #[test]
    fn test_slider_position_clamp() {
        let priv_ = PreviewPrivate::default();
        // Default position
        assert_eq!(*priv_.slider_position.borrow(), 0.5);
    }

    #[test]
    fn test_paths_default_none() {
        let priv_ = PreviewPrivate::default();
        assert!(priv_.original_path.borrow().is_none());
        assert!(priv_.translated_path.borrow().is_none());
    }

    #[test]
    fn test_dragging_default_false() {
        let priv_ = PreviewPrivate::default();
        assert!(!*priv_.dragging_slider.borrow());
    }

    /// The turn has to reach the far spread exactly: the resting frame that
    /// follows is drawn from scratch, so any leftover angle shows up as a pop.
    #[test]
    fn test_turn_lands_exactly_flat() {
        let last = turn_phase(STATIC_TIME + FLIP_TIME - 1e-9).expect("still turning");
        assert!(
            (last.progress - 1.0).abs() < 1e-6,
            "progress {}",
            last.progress
        );
        assert!(turn_phase(STATIC_TIME + FLIP_TIME + 1e-6).is_none());
        assert!(turn_phase(STATIC_TIME - 1e-9).is_none());
    }

    /// A spring that does not overshoot would be an ease curve again. The rock
    /// back onto the far page is the whole reason for using one.
    #[test]
    fn test_turn_overshoots_then_settles() {
        let mut peak: f64 = 0.0;
        let mut peak_at = 0.0;
        for i in 0..=300 {
            let ft = FLIP_TIME * i as f64 / 300.0;
            let Some(turn) = turn_phase(STATIC_TIME + ft) else {
                continue;
            };
            if turn.progress > peak {
                peak = turn.progress;
                peak_at = ft / FLIP_TIME;
            }
        }
        assert!((1.005..=1.05).contains(&peak), "overshoot {peak}");
        // About seven tenths of the way in, so the sheet has most of its travel
        // behind it but still time to rock back before the flip ends.
        assert!((0.6..0.85).contains(&peak_at), "peak at {peak_at}");
    }

    /// The bow is driven by how fast the sheet is moving, not by its angle, so
    /// it has to be widest somewhere in the middle of the sweep and gone by the
    /// time the sheet comes to rest.
    #[test]
    fn test_bow_follows_the_sweep() {
        let mut widest: f64 = 0.0;
        let mut widest_at = 0.0;
        for i in 0..=400 {
            let p = i as f64 / 400.0;
            let Some(turn) = turn_phase(STATIC_TIME + p * FLIP_TIME) else {
                continue;
            };
            if turn.lag.abs() > widest.abs() {
                widest = turn.lag;
                widest_at = p;
            }
        }
        assert!(widest > 0.5 * PAPER_CURL, "bow too shallow: {widest}");
        assert!(
            widest <= PAPER_CURL * 1.02,
            "bow overshoots its gain: {widest}"
        );
        // A quarter to a half of the way in, i.e. while the sheet is upright.
        assert!((0.2..0.5).contains(&widest_at), "widest at {widest_at}");

        // Both ends of the flip are still, so neither may carry a bow.
        let start = turn_phase(STATIC_TIME + 1e-6).expect("turning");
        assert!(start.lag.abs() < 1e-3, "bow at start {}", start.lag);
        let end = turn_phase(STATIC_TIME + FLIP_TIME - 1e-6).expect("turning");
        assert!(end.lag.abs() < 1e-2, "bow at end {}", end.lag);
    }

    /// `TURN_RATE_PEAK` only makes `PAPER_DRAG` readable if it really is the
    /// peak of the rate; if the spring is retuned it has to be updated.
    #[test]
    fn test_turn_rate_peak_is_current() {
        let mut peak: f64 = 0.0;
        for i in 0..=4000 {
            let p = i as f64 / 4000.0;
            peak = peak.max(turn_motion(p).1.abs());
        }
        assert!(
            (peak - TURN_RATE_PEAK).abs() < 0.02 * TURN_RATE_PEAK,
            "measured {peak}, constant {TURN_RATE_PEAK}"
        );
    }

    /// Time one placeholder frame for a resting and a turning spread when
    /// `PLACEHOLDER_BENCH` is set. The placeholder is drawn with cairo, so this
    /// is CPU time, not GPU; it is here to keep the caching honest.
    ///
    ///     PLACEHOLDER_BENCH=1 cargo test --release bench_placeholder_frames -- --nocapture
    #[test]
    fn bench_placeholder_frames() {
        if std::env::var_os("PLACEHOLDER_BENCH").is_none() {
            return;
        }

        let (w, h) = (420, 720);
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();

        let run = |label: &str, t_at: &dyn Fn(usize) -> f64| {
            for i in 0..20 {
                draw_placeholder_at(&cr, w, h, t_at(i), 0, true);
            }
            let n = 300;
            let t0 = std::time::Instant::now();
            for i in 0..n {
                draw_placeholder_at(&cr, w, h, t_at(i), 0, true);
            }
            let per = t0.elapsed().as_secs_f64() / n as f64;
            eprintln!(
                "{label:6} {:.2} ms -> at 60 fps {:.1}% of one core",
                per * 1000.0,
                per * 60.0 * 100.0
            );
        };

        run("rest", &|i| 0.1 + i as f64 * 0.007);
        run("turn", &|i| 2.65 + i as f64 * 0.004);
    }

    /// Render the placeholder animation to PNGs when `PLACEHOLDER_DUMP` names a
    /// directory. Lets the page turn be reviewed frame by frame without a
    /// compositor or a running window; a plain `cargo test` writes nothing.
    #[test]
    fn dump_placeholder_frames() {
        let Some(dir) = std::env::var_os("PLACEHOLDER_DUMP") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();

        let (w, h) = (420, 720);
        // Sample by absolute time so the cycle boundary is covered too: that is
        // where the turned sheet has to hand over to the next resting spread.
        let steps = (CYCLE * 2.0 / 0.05) as usize;

        for i in 0..=steps {
            let total = i as f64 * 0.05;
            let cycle_number = (total / CYCLE) as usize;
            let t = total % CYCLE;

            let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).unwrap();
            {
                let cr = cairo::Context::new(&surface).unwrap();
                draw_placeholder_at(&cr, w, h, t, cycle_number, true);
            }
            let path = dir.join(format!("ph_{total:06.2}.png"));
            let mut file = std::fs::File::create(path).unwrap();
            surface.write_to_png(&mut file).unwrap();
        }
    }
}
