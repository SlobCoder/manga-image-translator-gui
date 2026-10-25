// manga-translator-gtk/src/ui/spring_splitter.rs
//
// Spring-animated `GtkPaned` dividers, driven by libadwaita's `AdwSpringAnimation`.
//
// `GtkPaned:position` is an integer property, so `AdwPropertyAnimationTarget`
// (which only animates doubles) does not apply here. `AdwCallbackAnimationTarget`
// gives the same result and lets us round to whole pixels, which avoids
// sub-pixel shimmer on the panel text while a panel moves.
//
// Only *programmatic* moves are animated. Dragging has to stay 1:1 with the
// pointer, so the divider keeps GTK's own drag handling; a drag cancels a
// running spring (see `cancel_if_external`) instead of fighting it.

use adw::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Damping ratio below 1 gives the short overshoot that makes the movement read
/// as physical rather than eased. Stiffness and mass are tuned so a full panel
/// travels in about a third of a second.
///
/// These values predict an overshoot of `exp(-pi*z/sqrt(1-z^2))` of ~3.8 %,
/// which measured as 401 px on a 386 px target.
const DAMPING_RATIO: f64 = 0.72;
const MASS: f64 = 1.0;
const STIFFNESS: f64 = 260.0;

/// Samples older than this are not used to carry velocity into a new spring.
const VELOCITY_MAX_AGE_US: i64 = 50_000;

/// Window after a spring settles in which position changes are still ignored
/// by the caller's persistence.
///
/// Restoring the paned's minimum-width constraints re-runs the layout, and GTK
/// then clamps the divider once more - usually within a frame, but always after
/// the animation reported completion.
const SETTLE_GRACE_US: i64 = 150_000;

/// Last two applied values with their timestamps, in microseconds.
///
/// A retarget can then start with the velocity the divider currently has, so
/// rapid repeated toggles flow into each other instead of restarting from a
/// standstill.
#[derive(Default)]
struct SampleHistory {
    previous: (f64, i64),
    last: (f64, i64),
}

impl SampleHistory {
    fn push(&mut self, value: f64, time: i64) {
        self.previous = self.last;
        self.last = (value, time);
    }

    /// Pixels per second, or 0 when the samples are stale or degenerate.
    fn velocity(&self) -> f64 {
        let dt = (self.last.1 - self.previous.1) as f64 / 1_000_000.0;
        let age = glib::monotonic_time() - self.last.1;
        if dt <= 0.0 || age > VELOCITY_MAX_AGE_US {
            return 0.0;
        }
        (self.last.0 - self.previous.0) / dt
    }
}

/// Callback run once when a spring ends or is cancelled.
type SettleCallback = Rc<dyn Fn()>;

/// Wraps a `GtkPaned` and animates its divider with spring physics.
pub struct SpringPaned {
    paned: gtk::Paned,
    animation: RefCell<Option<adw::SpringAnimation>>,
    animating: Rc<Cell<bool>>,
    samples: Rc<RefCell<SampleHistory>>,
    /// Callback of the running spring, run once when it ends or is cancelled.
    pending: Rc<RefCell<Option<SettleCallback>>>,
    /// When the last spring ended, in microseconds.
    settled_at: Rc<Cell<i64>>,
}

impl SpringPaned {
    pub fn new(paned: &gtk::Paned) -> Rc<Self> {
        Rc::new(Self {
            paned: paned.clone(),
            animation: RefCell::new(None),
            animating: Rc::new(Cell::new(false)),
            samples: Rc::new(RefCell::new(SampleHistory::default())),
            pending: Rc::new(RefCell::new(None)),
            settled_at: Rc::new(Cell::new(0)),
        })
    }

    /// Whether a spring is currently moving the divider.
    ///
    /// The window persists every position change; intermediate values must be
    /// skipped, otherwise a collapse animation would store width 0.
    pub fn is_animating(&self) -> bool {
        self.animating.get()
    }

    /// Whether a spring settled so recently that the layout is still catching up.
    pub fn is_settling(&self) -> bool {
        glib::monotonic_time() - self.settled_at.get() < SETTLE_GRACE_US
    }

    /// Enter the persistence guard without starting a spring yet.
    ///
    /// Preparing a transition can move the divider synchronously (hiding or
    /// showing a paned child re-clamps its position), and that value must not
    /// reach the settings file either.
    pub fn begin_transition(&self) {
        self.animating.set(true);
    }

    /// Spring the divider to *target* and run *settled* once it ends.
    ///
    /// *settled* runs while [`is_animating`](Self::is_animating) is still true,
    /// so position changes caused by it (hiding a panel, for example) are not
    /// persisted. Callers should store the target width before animating: a
    /// spring keeps running briefly after it looks at rest.
    ///
    /// With `overshoot`, the value may pass the target; GTK clamps the actual
    /// position to the window, so this only shows up as extra life.
    pub fn animate_to(&self, target: i32, overshoot: bool, settled: impl Fn() + 'static) {
        // Cancelling first also runs the previous callback, so caller
        // bookkeeping (relaxed minimum widths, for example) is not lost. Only do
        // it when something is actually running: `finish` clears the guard that
        // `begin_transition` set for the preparation phase.
        if self.animation.borrow().is_some() || self.pending.borrow().is_some() {
            self.finish();
        }

        let settled: Rc<dyn Fn()> = Rc::new(settled);
        *self.pending.borrow_mut() = Some(settled);

        let from = self.paned.position() as f64;
        let to = target as f64;

        // Nothing to animate, or no frame clock to animate on.
        if (from - to).abs() < 0.5 || !self.paned.is_mapped() {
            self.paned.set_position(target);
            self.finish();
            return;
        }

        let initial_velocity = self.samples.borrow().velocity();

        let paned = self.paned.clone();
        let samples = self.samples.clone();
        let target_obj = adw::CallbackAnimationTarget::new(move |value| {
            paned.set_position(value.round() as i32);
            samples.borrow_mut().push(value, glib::monotonic_time());
        });

        let animation = adw::SpringAnimation::new(
            &self.paned,
            from,
            to,
            adw::SpringParams::new(DAMPING_RATIO, MASS, STIFFNESS),
            target_obj,
        );
        animation.set_clamp(!overshoot);
        animation.set_initial_velocity(initial_velocity);

        self.animating.set(true);
        {
            let pending = self.pending.clone();
            let animating = self.animating.clone();
            let settled_at = self.settled_at.clone();
            animation.connect_done(move |_| {
                if let Some(callback) = pending.borrow_mut().take() {
                    callback();
                }
                animating.set(false);
                settled_at.set(glib::monotonic_time());
            });
        }
        animation.play();
        *self.animation.borrow_mut() = Some(animation);
    }

    /// Stop a running spring because something else moved the divider.
    ///
    /// The animation writes `position` every frame, so a drag that starts while
    /// it runs would otherwise be overwritten until the spring comes to rest.
    pub fn cancel_if_external(&self, position: i32) {
        if !self.animating.get() {
            return;
        }
        let last_written = self.samples.borrow().last.0.round() as i32;
        if (position - last_written).abs() <= 1 {
            return; // our own frame
        }
        self.finish();
    }

    /// Stop the current spring and run its pending callback.
    ///
    /// Call this *before* preparing a new transition: the callback may undo
    /// layout changes (re-showing a collapsed panel, for instance) that the
    /// new transition is about to make.
    pub fn cancel(&self) {
        self.finish();
    }

    /// Drop the animation and run the pending callback exactly once.
    fn finish(&self) {
        if let Some(animation) = self.animation.borrow_mut().take() {
            animation.pause();
        }
        let pending = self.pending.borrow_mut().take();
        if let Some(pending) = pending {
            pending();
        }
        self.animating.set(false);
        self.settled_at.set(glib::monotonic_time());
    }

    /// Double-clicking the divider springs back to *default_position*, then
    /// runs *on_reset* with that width so the caller can persist it.
    pub fn enable_double_click_reset(
        self: &Rc<Self>,
        default_position: i32,
        on_reset: impl Fn(i32) + 'static,
    ) {
        let Some(handle) = divider(&self.paned) else {
            log::warn!("spring splitter: no GtkPanedHandle found, double-click reset disabled");
            return;
        };

        let gesture = gtk::GestureClick::new();
        gesture.set_button(gtk::gdk::BUTTON_PRIMARY);
        let split = Rc::downgrade(self);
        let on_reset = Rc::new(on_reset);
        gesture.connect_pressed(move |_, n_press, _, _| {
            if n_press != 2 {
                return;
            }
            if let Some(split) = split.upgrade() {
                let on_reset = on_reset.clone();
                split.animate_to(default_position, true, move || on_reset(default_position));
            }
        });
        handle.add_controller(gesture);
    }
}

/// The divider between the two children of a paned.
fn divider(paned: &gtk::Paned) -> Option<gtk::Widget> {
    let mut child = paned.first_child();
    while let Some(widget) = child {
        if widget.type_().name() == "GtkPanedHandle" {
            return Some(widget);
        }
        child = widget.next_sibling();
    }
    None
}
