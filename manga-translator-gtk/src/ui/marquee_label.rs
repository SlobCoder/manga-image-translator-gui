// manga-translator-gtk/src/ui/marquee_label.rs
//
// A single-line label that scrolls its text horizontally when it is wider
// than the space it is given.
//
// Used by the OpenRouter model popup: `AdwComboRow`'s default factory caps
// labels at 20 characters, which cuts every model id short. This widget
// reports a capped natural width so the popover stays compact, and animates
// `scroll-offset` with a libadwaita timed animation whenever the text does
// not fit. The animation stops as soon as the row is unmapped (popup
// closed), so nothing keeps running in the background.

use adw::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;
use pango::EllipsizeMode;
use std::cell::{Cell, RefCell};

/// Upper bound for the width the widget asks for. Longer text scrolls
/// instead of widening the popover.
const MAX_NATURAL_WIDTH: i32 = 300;

/// Reading speed of the marquee, in pixels per second.
const SCROLL_SPEED_PX_PER_S: f64 = 60.0;

/// Extra room after the last glyph. Without it the final character sits exactly
/// on the right edge and is clipped by the overflow mask.
const TAIL_PADDING: i32 = 12;

/// Bounds for the animation duration, so short overflows do not race by and
/// very long ids do not crawl.
const MIN_DURATION_MS: u32 = 700;
const MAX_DURATION_MS: u32 = 24_000;

/// Property animated by the timed animation.
const SCROLL_OFFSET: &str = "scroll-offset";

glib::wrapper! {
    pub struct MarqueeLabel(ObjectSubclass<imp::MarqueeLabelPrivate>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for MarqueeLabel {
    fn default() -> Self {
        Self::new()
    }
}

impl MarqueeLabel {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Set the text to display and restart the scroll if it no longer fits.
    pub fn set_text(&self, text: &str) {
        let imp = self.imp();
        if let Some(label) = imp.label.borrow().as_ref() {
            label.set_label(text);
        }
        self.set_tooltip_text(Some(text));
        self.queue_resize();
        imp.schedule_update();
    }

    /// Stop a running marquee and reset it to the start of the text.
    pub fn stop_animation(&self) {
        self.imp().stop_animation();
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct MarqueeLabelPrivate {
        pub label: RefCell<Option<gtk::Label>>,
        offset: Cell<f64>,
        animation: RefCell<Option<adw::TimedAnimation>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MarqueeLabelPrivate {
        const NAME: &'static str = "MtgMarqueeLabel";
        type Type = super::MarqueeLabel;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for MarqueeLabelPrivate {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: std::sync::OnceLock<Vec<glib::ParamSpec>> =
                std::sync::OnceLock::new();
            PROPERTIES.get_or_init(|| vec![glib::ParamSpecDouble::builder(SCROLL_OFFSET).build()])
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                SCROLL_OFFSET => {
                    let offset = value.get::<f64>().unwrap_or(0.0);
                    self.set_offset(offset);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                SCROLL_OFFSET => self.offset.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            // The label is allocated at its full natural width, so anything
            // beyond the widget's own bounds has to be clipped away.
            obj.set_overflow(gtk::Overflow::Hidden);

            let label = gtk::Label::new(None);
            label.set_xalign(0.0);
            label.set_ellipsize(EllipsizeMode::None);
            label.set_single_line_mode(true);
            label.set_parent(&*obj);
            self.label.replace(Some(label));
        }

        fn dispose(&self) {
            if let Some(label) = self.label.borrow_mut().take() {
                label.unparent();
            }
        }
    }

    impl WidgetImpl for MarqueeLabelPrivate {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let label_ref = self.label.borrow();
            let Some(label) = label_ref.as_ref() else {
                return (0, 0, -1, -1);
            };

            if orientation == gtk::Orientation::Horizontal {
                let (_, natural, _, _) = label.measure(gtk::Orientation::Horizontal, -1);
                // Minimum 0 lets the popover shrink; the natural width is
                // capped so long ids scroll instead of widening the list.
                (0, natural.min(MAX_NATURAL_WIDTH), -1, -1)
            } else {
                label.measure(gtk::Orientation::Vertical, for_size)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            {
                let label_ref = self.label.borrow();
                let Some(label) = label_ref.as_ref() else {
                    return;
                };

                let (_, natural_width, _, _) = label.measure(gtk::Orientation::Horizontal, -1);
                let (_, natural_height, _, _) =
                    label.measure(gtk::Orientation::Vertical, natural_width);
                let y = ((height - natural_height) / 2).max(0);

                label.size_allocate(
                    &gtk::Allocation::new(
                        0,
                        y,
                        natural_width.max(width) + TAIL_PADDING,
                        natural_height,
                    ),
                    -1,
                );
            }

            // Starting an animation inside the layout cycle is not allowed,
            // so defer to an idle callback.
            self.schedule_update();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let label_ref = self.label.borrow();
            let Some(label) = label_ref.as_ref() else {
                return;
            };

            let offset = self.offset.get() as f32;
            if offset.abs() < 0.5 {
                self.obj().snapshot_child(label, snapshot);
                return;
            }

            snapshot.save();
            snapshot.translate(&gtk::graphene::Point::new(-offset, 0.0));
            self.obj().snapshot_child(label, snapshot);
            snapshot.restore();
        }

        fn map(&self) {
            self.parent_map();
            self.schedule_update();
        }

        fn unmap(&self) {
            self.stop_animation();
            self.parent_unmap();
        }
    }

    impl MarqueeLabelPrivate {
        /// Restart the marquee for the current text and allocation.
        pub fn schedule_update(&self) {
            let obj = self.obj().clone();
            glib::idle_add_local_once(move || {
                obj.imp().update_animation();
            });
        }

        pub fn update_animation(&self) {
            self.stop_animation();

            let obj = self.obj();
            if !obj.is_mapped() {
                return;
            }

            let overflow = {
                let label_ref = self.label.borrow();
                let Some(label) = label_ref.as_ref() else {
                    return;
                };
                let (_, natural_width, _, _) = label.measure(gtk::Orientation::Horizontal, -1);
                (natural_width + TAIL_PADDING - obj.width()) as f64
            };

            if overflow <= 1.0 {
                return;
            }

            let duration = ((overflow / SCROLL_SPEED_PX_PER_S) * 1000.0)
                .clamp(MIN_DURATION_MS as f64, MAX_DURATION_MS as f64)
                as u32;

            let target = adw::PropertyAnimationTarget::new(&*obj, SCROLL_OFFSET);
            let animation = adw::TimedAnimation::new(&*obj, 0.0, overflow, duration, target);
            // Ease in and out symmetrically: with the default ease-out the
            // turnaround at the end feels like a hard bounce.
            animation.set_easing(adw::Easing::EaseInOutCubic);
            animation.set_alternate(true);
            // 0 repeats endlessly; `alternate` scrolls back and forth.
            animation.set_repeat_count(0);
            animation.play();

            *self.animation.borrow_mut() = Some(animation);
        }

        pub fn stop_animation(&self) {
            if let Some(animation) = self.animation.borrow_mut().take() {
                animation.pause();
            }
            self.set_offset(0.0);
        }

        fn set_offset(&self, offset: f64) {
            if (self.offset.get() - offset).abs() < f64::EPSILON {
                return;
            }
            self.offset.set(offset);
            self.obj().queue_draw();
        }
    }
}
