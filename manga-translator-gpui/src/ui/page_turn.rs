// manga-translator-gpui/src/ui/page_turn.rs
//
// Animated manga-book placeholder for the preview pane.
//
// Port of `manga-translator-gtk/src/ui/preview.rs` (ComicBookPlaceholder):
// a two-page spread with panels, screentones, speech bubbles and a hanko,
// plus a sheet that lifts off the spine and turns over with perspective,
// paper curl and a cast shadow.
//
// The GTK version samples a cached Cairo surface strip by strip. GPUI has no
// arbitrary texture transform, so the turning sheet is built from projected
// geometry instead: every panel, bubble and stamp is mapped through the same
// `project_page` function, one vertical strip at a time. The motion — spring,
// eased clock, curl — is a line-for-line port and stays testable.

use std::f64::consts::{PI, TAU};

use gpui::{
    App, Bounds, ContentMask, Context, Corners, Hsla, IntoElement, PathBuilder, Pixels, Point,
    Render, Rgba, Size, TextAlign, TextRun, WeakEntity, Window, canvas, fill, font,
    linear_color_stop, linear_gradient, point, prelude::*, px, size,
};

// ---------------------------------------------------------------------------
// Timing and motion
// ---------------------------------------------------------------------------

/// Seconds the spread rests before the next turn.
pub(crate) const STATIC_TIME: f64 = 2.6;
/// Seconds one sheet takes to turn over.
pub(crate) const FLIP_TIME: f64 = 1.5;
pub(crate) const CYCLE: f64 = STATIC_TIME + FLIP_TIME;

/// Damping ratio below 1 on purpose: this is what produces the overshoot.
const TURN_DAMPING: f64 = 0.76;
/// E-folds of decay packed into `FLIP_TIME`.
const TURN_SETTLE: f64 = 6.0;
/// Where the leftover tail starts being trimmed, as a fraction of `FLIP_TIME`.
const TURN_TRIM_START: f64 = 0.78;

/// Camera height above the book plane, in page widths.
const CAM_HEIGHT: f64 = 9.5;
/// Camera offset from the spine, in page widths.
const CAM_OFFSET: f64 = -0.45;
/// Vertical strips used to project the turning sheet.
const TURN_STRIPS: usize = 40;
/// How far the free edge runs ahead of the hinge at full sweep speed, in radians.
const PAPER_CURL: f64 = 0.34;
/// Peak of [`turn_motion`]'s rate, guarded by a test.
const TURN_RATE_PEAK: f64 = 0.888;

/// Panel layouts per page — (x, y, w, h) as fractions of the page area.
const LAYOUTS: &[&[(f64, f64, f64, f64)]] = &[
    // Page 0: big top panel, three small ones below.
    &[
        (0.04, 0.03, 0.92, 0.54),
        (0.04, 0.61, 0.29, 0.36),
        (0.36, 0.61, 0.29, 0.36),
        (0.68, 0.61, 0.29, 0.36),
    ],
    // Page 1: 2x2 grid.
    &[
        (0.04, 0.03, 0.45, 0.47),
        (0.53, 0.03, 0.45, 0.47),
        (0.04, 0.53, 0.45, 0.44),
        (0.53, 0.53, 0.45, 0.44),
    ],
    // Page 2: three vertical strips.
    &[
        (0.04, 0.03, 0.28, 0.94),
        (0.37, 0.03, 0.28, 0.94),
        (0.68, 0.03, 0.28, 0.94),
    ],
    // Page 3: big left panel, two stacked right.
    &[
        (0.04, 0.03, 0.58, 0.94),
        (0.66, 0.03, 0.32, 0.45),
        (0.66, 0.52, 0.32, 0.45),
    ],
    // Page 4: full-bleed splash with a small inset.
    &[(0.0, 0.0, 1.0, 1.0), (0.60, 0.05, 0.35, 0.25)],
    // Page 5: asymmetric L-shape.
    &[
        (0.04, 0.03, 0.45, 0.94),
        (0.52, 0.03, 0.46, 0.45),
        (0.52, 0.52, 0.22, 0.45),
        (0.77, 0.52, 0.21, 0.45),
    ],
    // Page 6: wide horizontal strips.
    &[
        (0.04, 0.03, 0.92, 0.28),
        (0.04, 0.35, 0.92, 0.30),
        (0.04, 0.69, 0.92, 0.28),
    ],
    // Page 7: overlapping panels.
    &[(0.04, 0.03, 0.60, 0.60), (0.35, 0.40, 0.62, 0.55)],
];

const BUBBLE_TYPES: &[&str] = &["dots", "japanese", "lines", "exclaim"];

const BUBBLE_OFFSETS: &[(f64, f64)] = &[
    (0.50, 0.35),
    (0.55, 0.40),
    (0.45, 0.30),
    (0.50, 0.38),
    (0.60, 0.35),
    (0.40, 0.42),
];

/// State of the sheet while it is mid-turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Turn {
    /// Rotation progress: 0 on the right spread, 1 flat on the left one.
    pub(crate) progress: f64,
    /// Angle the free edge runs ahead of the hinge by, in radians.
    pub(crate) lag: f64,
}

/// Spring response at `ft` seconds of spring time.
fn turn_spring(ft: f64) -> (f64, f64) {
    let zeta = TURN_DAMPING;
    let omega = TURN_SETTLE / (zeta * FLIP_TIME);
    let omega_d = omega * (1.0 - zeta * zeta).sqrt();
    let decay = (-zeta * omega * ft).exp();
    let phase = omega_d * ft;

    let progress = 1.0 - decay * (phase.cos() + (zeta * omega / omega_d) * phase.sin());
    let speed = decay * phase.sin() * omega * omega / omega_d * FLIP_TIME / PI;
    (progress, speed)
}

/// The spring runs on an eased clock so the lift starts gently.
fn turn_clock(p: f64) -> f64 {
    p * p.sqrt()
}

/// `turn_clock` differentiated.
fn turn_clock_gain(p: f64) -> f64 {
    1.5 * p.sqrt()
}

/// Rotation progress and its real rate at a fraction `p` of the flip.
fn turn_motion(p: f64) -> (f64, f64) {
    let (progress, speed) = turn_spring(turn_clock(p) * FLIP_TIME);
    (progress, speed * turn_clock_gain(p))
}

/// Motion of the sheet, or `None` while the spread rests.
pub(crate) fn turn_phase(t: f64) -> Option<Turn> {
    if t < STATIC_TIME {
        return None;
    }
    let ft = t - STATIC_TIME;
    if ft >= FLIP_TIME {
        return None;
    }
    let p = ft / FLIP_TIME;

    let (mut progress, rate) = turn_motion(p);

    // Trim the asymptotic remainder so the sheet is exactly flat when the
    // flip ends; otherwise the resting spread pops.
    let (end, _) = turn_spring(FLIP_TIME);
    let q = ((p - TURN_TRIM_START) / (1.0 - TURN_TRIM_START)).clamp(0.0, 1.0);
    progress += (1.0 - end) * (q * q * (3.0 - 2.0 * q));

    Some(Turn {
        progress,
        lag: PAPER_CURL * rate / TURN_RATE_PEAK,
    })
}

/// Rotation of the sheet at distance `d` from the spine.
fn turn_theta_at(d: f64, pw: f64, theta: f64, lag: f64) -> f64 {
    let w = (d / pw).clamp(0.0, 1.0).powi(3);
    theta + lag * w
}

/// Project a point of the turning page: screen x plus the vertical scale.
fn project_page(d: f64, theta: f64, hinge: f64, cam: f64, cam_x: f64) -> (f64, f64) {
    let height = (d * theta.sin()).max(0.0);
    let scale = cam / (cam - height).max(cam * 0.2);
    let x = cam_x + scale * (hinge + d * theta.cos() - cam_x);
    (x, scale)
}

/// Centered book rectangle with tankobon aspect ratio (two pages side by side).
fn book_rect(w: f64, h: f64) -> (f64, f64, f64, f64) {
    let mx = w * 0.06;
    let my = h * 0.07;
    let aw = w - mx * 2.0;
    let ah = h - my * 2.0;
    let ratio = 1.3;
    let (bw, bh) = if aw / ah > ratio {
        (ah * ratio, ah)
    } else {
        (aw, aw / ratio)
    };
    ((w - bw) / 2.0, (h - bh) / 2.0, bw, bh)
}

// ---------------------------------------------------------------------------
// Painting primitives
// ---------------------------------------------------------------------------

fn color(r: f64, g: f64, b: f64, a: f64) -> Hsla {
    Hsla::from(Rgba {
        r: r as f32,
        g: g as f32,
        b: b as f32,
        a: a as f32,
    })
}

fn gray(v: f64, a: f64) -> Hsla {
    color(v, v, v, a)
}

fn p(x: f64, y: f64) -> Point<Pixels> {
    point(px(x as f32), px(y as f32))
}

fn s(w: f64, h: f64) -> Size<Pixels> {
    size(px(w as f32), px(h as f32))
}

fn rect(window: &mut Window, x: f64, y: f64, w: f64, h: f64, r: f64, fill_color: Hsla) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    window.paint_quad(
        fill(Bounds::new(p(x, y), s(w, h)), fill_color)
            .corner_radii(Corners::all(px(r.max(0.0) as f32))),
    );
}

/// Horizontal multi-stop gradient drawn as bands: gpui's `Background` only
/// holds two stops, and the book needs dark–light–dark across the spine.
fn h_gradient(window: &mut Window, x: f64, y: f64, w: f64, h: f64, stops: &[(f64, Hsla)]) {
    if stops.is_empty() || w <= 0.0 || h <= 0.0 {
        return;
    }
    const BANDS: usize = 48;
    for i in 0..BANDS {
        let t0 = i as f64 / BANDS as f64;
        let t1 = (i + 1) as f64 / BANDS as f64;
        rect(
            window,
            x + w * t0,
            y,
            w * (t1 - t0) + 0.6,
            h,
            0.0,
            sample_stops(stops, (t0 + t1) * 0.5),
        );
    }
}

fn sample_stops(stops: &[(f64, Hsla)], t: f64) -> Hsla {
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for pair in stops.windows(2) {
        let (a, ca) = pair[0];
        let (b, cb) = pair[1];
        if t <= b {
            let span = (b - a).max(1e-6);
            let f = ((t - a) / span) as f32;
            return Hsla {
                h: ca.h + (cb.h - ca.h) * f,
                s: ca.s + (cb.s - ca.s) * f,
                l: ca.l + (cb.l - ca.l) * f,
                a: ca.a + (cb.a - ca.a) * f,
            };
        }
    }
    stops[stops.len() - 1].1
}

fn add_circle(builder: &mut PathBuilder, cx: f64, cy: f64, r: f64, segments: usize) {
    let mut points = Vec::with_capacity(segments);
    for i in 0..segments {
        let a = i as f64 / segments as f64 * TAU;
        points.push(p(cx + r * a.cos(), cy + r * a.sin()));
    }
    builder.add_polygon(&points, true);
}

fn add_polygon(builder: &mut PathBuilder, points: &[(f64, f64)]) {
    let mapped: Vec<Point<Pixels>> = points.iter().map(|(x, y)| p(*x, *y)).collect();
    builder.add_polygon(&mapped, true);
}

fn paint_fill(window: &mut Window, builder: PathBuilder, fill_color: Hsla) {
    if let Ok(path) = builder.build() {
        window.paint_path(path, fill_color);
    }
}

fn paint_stroke(window: &mut Window, builder: PathBuilder, stroke_color: Hsla) {
    if let Ok(path) = builder.build() {
        window.paint_path(path, stroke_color);
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Paint the placeholder for `t` seconds since the animation started.
pub(crate) fn paint(
    window: &mut Window,
    app: &mut App,
    bounds: Bounds<Pixels>,
    dark: bool,
    t: f64,
) {
    let w = bounds.size.width.as_f32() as f64;
    let h = bounds.size.height.as_f32() as f64;
    if w < 20.0 || h < 20.0 {
        return;
    }
    let ox = bounds.origin.x.as_f32() as f64;
    let oy = bounds.origin.y.as_f32() as f64;

    rect(
        window,
        ox,
        oy,
        w,
        h,
        0.0,
        if dark {
            color(0.05, 0.05, 0.07, 1.0)
        } else {
            color(0.92, 0.92, 0.94, 1.0)
        },
    );

    let cycle_number = (t / CYCLE) as usize;
    let cycle_t = t % CYCLE;
    let page = (cycle_number * 2) % LAYOUTS.len();

    let (bx, by, bw, bh) = book_rect(w, h);
    let bx = bx + ox;
    let by = by + oy;
    let sw = bw * 0.02;

    paint_book(window, bx, by, bw, bh, sw, dark);

    let inset = 3.0;
    let ph = bh - inset * 2.0;
    let pw = (bw - sw) / 2.0 - inset;
    let hinge = bx + bw / 2.0;
    let cy = by + bh / 2.0;
    let py = by + inset;
    let left_px = hinge - pw;
    let right_px = hinge;

    let page_base = if dark {
        (0.15, 0.15, 0.15)
    } else {
        (0.98, 0.98, 0.98)
    };

    let left_idx = page;
    let right_idx = (page + 1) % LAYOUTS.len();
    let next_left = (page + 2) % LAYOUTS.len();
    let next_right = (page + 3) % LAYOUTS.len();

    let turn = turn_phase(cycle_t);

    // Near either end the sheet lies flat on a page it matches, so that page is
    // drawn underneath as well: the hairlines between strips then blend into
    // identical content instead of revealing the old page.
    let (under_left, under_right) = match turn {
        Some(turn) if turn.progress > 0.97 => (next_left, next_right),
        Some(turn) if turn.progress < 0.03 => (left_idx, right_idx),
        Some(_) => (left_idx, next_right),
        None => (left_idx, right_idx),
    };

    paint_page(window, app, left_px, py, pw, ph, under_left, dark);
    paint_page(window, app, right_px, py, pw, ph, under_right, dark);
    paint_spine_overlay(window, hinge, py, ph, dark);

    if let Some(turn) = turn {
        let theta = PI * turn.progress;
        let mirrored = theta > std::f64::consts::FRAC_PI_2;

        paint_turn_shadow(window, theta, turn.lag, hinge, py, ph, pw, dark);

        let turning_idx = if mirrored { next_left } else { right_idx };
        paint_turning_page(
            window,
            app,
            mirrored,
            turning_idx,
            theta,
            turn.lag,
            hinge,
            cy,
            py,
            ph,
            pw,
            dark,
            page_base,
        );
    }

    paint_grain(window, left_px, py, pw * 2.0, ph, dark);
}

// ---------------------------------------------------------------------------
// Book
// ---------------------------------------------------------------------------

fn paint_book(window: &mut Window, bx: f64, by: f64, bw: f64, bh: f64, sw: f64, dark: bool) {
    rect(
        window,
        bx + 5.0,
        by + 7.0,
        bw,
        bh,
        4.0,
        color(0.0, 0.0, 0.0, if dark { 0.3 } else { 0.13 }),
    );

    for i in (1..=5).rev() {
        let v = if dark {
            0.18 - i as f64 * 0.015
        } else {
            0.91 - i as f64 * 0.025
        };
        rect(
            window,
            bx + i as f64 * 1.5,
            by + i as f64 * 1.5,
            bw,
            bh,
            3.0,
            gray(v, 1.0),
        );
    }

    let base = if dark { 0.15 } else { 0.98 };
    rect(window, bx, by, bw, bh, 3.0, gray(base, 1.0));

    // Spine with a dark core.
    let spine_x = bx + bw / 2.0 - sw / 2.0;
    let (edge, core) = if dark { (0.15, 0.05) } else { (0.9, 0.7) };
    h_gradient(
        window,
        spine_x,
        by,
        sw,
        bh,
        &[
            (0.0, gray(edge, 1.0)),
            (0.5, gray(core, 1.0)),
            (1.0, gray(edge, 1.0)),
        ],
    );

    // Spine crease.
    paint_stroke(
        window,
        {
            let mut b = PathBuilder::stroke(px(1.0));
            b.move_to(p(bx + bw / 2.0, by));
            b.line_to(p(bx + bw / 2.0, by + bh));
            b
        },
        color(0.0, 0.0, 0.0, if dark { 0.5 } else { 0.2 }),
    );

    // Inner shadows either side of the gutter.
    let shadow_w = (sw * 1.5).min(12.0);
    let strength = if dark { 0.25 } else { 0.14 };
    h_gradient(
        window,
        spine_x,
        by,
        shadow_w,
        bh,
        &[
            (0.0, color(0.0, 0.0, 0.0, strength)),
            (1.0, color(0.0, 0.0, 0.0, 0.0)),
        ],
    );
    h_gradient(
        window,
        spine_x + sw - shadow_w,
        by,
        shadow_w,
        bh,
        &[
            (0.0, color(0.0, 0.0, 0.0, 0.0)),
            (1.0, color(0.0, 0.0, 0.0, strength)),
        ],
    );
}

fn paint_spine_overlay(window: &mut Window, cx: f64, py: f64, ph: f64, dark: bool) {
    let width = (ph * 0.035).clamp(6.0, 16.0);
    let strength = if dark { 0.38 } else { 0.18 };

    h_gradient(
        window,
        cx - width,
        py,
        width,
        ph,
        &[
            (0.0, color(0.0, 0.0, 0.0, 0.0)),
            (1.0, color(0.0, 0.0, 0.0, strength)),
        ],
    );
    h_gradient(
        window,
        cx,
        py,
        width,
        ph,
        &[
            (0.0, color(0.0, 0.0, 0.0, strength)),
            (1.0, color(0.0, 0.0, 0.0, 0.0)),
        ],
    );

    paint_stroke(
        window,
        {
            let mut b = PathBuilder::stroke(px(1.0));
            b.move_to(p(cx, py));
            b.line_to(p(cx, py + ph));
            b
        },
        color(0.0, 0.0, 0.0, if dark { 0.45 } else { 0.22 }),
    );
}

// ---------------------------------------------------------------------------
// Page content
// ---------------------------------------------------------------------------

fn paint_page(
    window: &mut Window,
    app: &mut App,
    page_x: f64,
    page_y: f64,
    page_w: f64,
    page_h: f64,
    layout_idx: usize,
    dark: bool,
) {
    let layout = LAYOUTS[layout_idx % LAYOUTS.len()];

    for (pidx, panel) in layout.iter().enumerate() {
        let rx = page_x + panel.0 * page_w;
        let ry = page_y + panel.1 * page_h;
        let rw = panel.2 * page_w;
        let rh = panel.3 * page_h;

        rect(
            window,
            rx,
            ry,
            rw,
            rh,
            0.0,
            if dark {
                gray(0.20, 1.0)
            } else {
                gray(0.88, 1.0)
            },
        );

        let tone = (layout_idx + pidx) % 4;
        if tone > 0 && rw > 20.0 && rh > 20.0 {
            paint_screentone(window, rx, ry, rw, rh, tone, dark);
        }

        paint_stroke(
            window,
            {
                let mut b = PathBuilder::stroke(px(4.0));
                b.add_polygon(
                    &[
                        p(rx, ry),
                        p(rx + rw, ry),
                        p(rx + rw, ry + rh),
                        p(rx, ry + rh),
                    ],
                    true,
                );
                b
            },
            if dark {
                gray(0.72, 1.0)
            } else {
                gray(0.05, 1.0)
            },
        );

        if rw > 45.0 && rh > 45.0 {
            let btype = BUBBLE_TYPES[(layout_idx + pidx) % BUBBLE_TYPES.len()];
            let boff = BUBBLE_OFFSETS[pidx % BUBBLE_OFFSETS.len()];
            paint_bubble(
                window,
                app,
                rx + rw * boff.0,
                ry + rh * boff.1,
                rw.min(rh) * 0.28,
                dark,
                btype,
            );
        }

        if pidx == 0 && layout_idx % 3 == 0 && rw > 60.0 && rh > 60.0 {
            paint_hanko(
                window,
                app,
                rx + rw * 0.88,
                ry + rh * 0.88,
                rw.min(rh) * 0.14,
            );
        }
    }
}

/// 1 = dot halftone, 2 = diagonal hatching, 3 = crosshatch.
fn paint_screentone(
    window: &mut Window,
    rx: f64,
    ry: f64,
    rw: f64,
    rh: f64,
    pattern: usize,
    dark: bool,
) {
    let tone = if dark {
        color(0.55, 0.55, 0.55, 0.45)
    } else {
        color(0.25, 0.25, 0.25, 0.35)
    };

    // Cairo clipped the pattern to the panel; a path has no such notion, and
    // the hatch lines run a full panel diagonal past every edge.
    let mask = ContentMask {
        bounds: Bounds::new(p(rx, ry), s(rw, rh)),
    };
    window.with_content_mask(Some(mask), |window| {
        paint_screentone_masked(window, rx, ry, rw, rh, pattern, tone);
    });
}

fn paint_screentone_masked(
    window: &mut Window,
    rx: f64,
    ry: f64,
    rw: f64,
    rh: f64,
    pattern: usize,
    tone: Hsla,
) {
    match pattern {
        1 => {
            // Coarser than Cairo's 7 px: the dots go into one path, but a
            // dense grid is still a lot of vertices to rebuild every frame.
            let spacing = 9.0;
            let dot_r = 1.5;
            let mut b = PathBuilder::fill();
            let mut y = ry + spacing / 2.0;
            let mut row = 0;
            while y < ry + rh - dot_r {
                let offset = if row % 2 != 0 { spacing / 2.0 } else { 0.0 };
                let mut x = rx + spacing / 2.0 + offset;
                while x < rx + rw - dot_r {
                    add_circle(&mut b, x, y, dot_r, 8);
                    x += spacing;
                }
                y += spacing;
                row += 1;
            }
            paint_fill(window, b, tone);
        }
        2 => {
            let spacing = 6.0;
            let mut b = PathBuilder::stroke(px(0.7));
            let total = (rw + rh) as i32;
            let mut i = -total;
            while i < total {
                b.move_to(p(rx + i as f64, ry));
                b.line_to(p(rx + i as f64 + rh, ry + rh));
                i += spacing as i32;
            }
            paint_stroke(window, b, tone);
        }
        3 => {
            let spacing = 9.0;
            let mut b = PathBuilder::stroke(px(0.5));
            let total = (rw + rh) as i32;
            let mut i = -total;
            while i < total {
                b.move_to(p(rx + i as f64, ry));
                b.line_to(p(rx + i as f64 + rh, ry + rh));
                i += spacing as i32;
            }
            let mut i = -total;
            while i < total {
                b.move_to(p(rx + i as f64 + rw, ry));
                b.line_to(p(rx + i as f64 + rw - rh, ry + rh));
                i += spacing as i32;
            }
            paint_stroke(window, b, tone);
        }
        _ => {}
    }
}

fn paint_bubble(
    window: &mut Window,
    app: &mut App,
    bx: f64,
    cy: f64,
    r: f64,
    dark: bool,
    kind: &str,
) {
    if r < 6.0 {
        return;
    }
    let r = r.min(36.0);

    let ink = if dark {
        gray(0.15, 1.0)
    } else {
        gray(1.0, 1.0)
    };
    let border = if dark {
        gray(0.8, 1.0)
    } else {
        gray(0.05, 1.0)
    };
    let text = if dark {
        gray(0.85, 1.0)
    } else {
        gray(0.10, 1.0)
    };

    paint_fill(
        window,
        {
            let mut b = PathBuilder::fill();
            add_circle(&mut b, bx, cy, r, 28);
            b
        },
        ink,
    );
    paint_stroke(
        window,
        {
            let mut b = PathBuilder::stroke(px(1.5));
            add_circle(&mut b, bx, cy, r, 28);
            b
        },
        border,
    );

    // Tail.
    let tail = [
        (bx - r * 0.2, cy + r * 0.78),
        (bx - r * 0.65, cy + r * 1.45),
        (bx + r * 0.15, cy + r * 0.88),
    ];
    paint_fill(
        window,
        {
            let mut b = PathBuilder::fill();
            add_polygon(&mut b, &tail);
            b
        },
        ink,
    );
    paint_stroke(
        window,
        {
            let mut b = PathBuilder::stroke(px(1.5));
            add_polygon(&mut b, &tail);
            b
        },
        border,
    );

    match kind {
        "japanese" | "exclaim" => {
            if let Some((content, size)) = bubble_text(kind, r) {
                paint_text(window, app, content, bx, cy, size, text, true);
            }
        }
        "lines" => {
            let bw = r * 0.70;
            let bh = r * 0.50;
            paint_stroke(
                window,
                {
                    let mut b = PathBuilder::stroke(px(1.2));
                    b.add_polygon(
                        &[
                            p(bx - bw / 2.0, cy - bh / 2.0),
                            p(bx + bw / 2.0, cy - bh / 2.0),
                            p(bx + bw / 2.0, cy + bh / 2.0),
                            p(bx - bw / 2.0, cy + bh / 2.0),
                        ],
                        true,
                    );
                    b
                },
                text,
            );
        }
        _ => {
            // Three ruled lines, the classic placeholder text.
            let mut b = PathBuilder::stroke(px(1.4));
            for (i, yoff) in [-0.28_f64, 0.0, 0.24].iter().enumerate() {
                let lw = r * if i < 2 { 0.72 } else { 0.38 };
                b.move_to(p(bx - lw / 2.0, cy + r * yoff));
                b.line_to(p(bx + lw / 2.0, cy + r * yoff));
            }
            paint_stroke(window, b, text);
        }
    }
}

/// Draw one centred line of text inside a canvas.
///
/// gpui has no direct "draw this string here" call in a paint pass: the text is
/// shaped into a `ShapedLine` first, which is also what gives the run its font
/// and colour.
#[allow(clippy::too_many_arguments)]
fn paint_text(
    window: &mut Window,
    app: &mut App,
    text: &str,
    center_x: f64,
    center_y: f64,
    size: f64,
    color: Hsla,
    bold: bool,
) {
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
    let font_size = px(size as f32);
    let shaped = {
        let text_system = window.text_system();
        text_system.shape_line(text.to_string().into(), font_size, &[run], None)
    };
    let width = shaped.width().as_f32() as f64;
    let line_height = size * 1.15;
    let origin = p(center_x - width / 2.0, center_y - line_height / 2.0);
    let _ = shaped.paint(
        origin,
        px(line_height as f32),
        TextAlign::Left,
        None,
        window,
        app,
    );
}

fn add_circle_path(window: &mut Window, cx: f64, cy: f64, r: f64, color: Hsla) {
    paint_fill(
        window,
        {
            let mut b = PathBuilder::fill();
            add_circle(&mut b, cx, cy, r, 10);
            b
        },
        color,
    );
}

fn paint_hanko(window: &mut Window, app: &mut App, bx: f64, cy: f64, r: f64) {
    if r < 5.0 {
        return;
    }
    let r = r.min(20.0);
    add_circle_path(window, bx, cy, r, color(0.80, 0.12, 0.10, 1.0));
    paint_text(window, app, "漫", bx, cy, r * 1.1, gray(1.0, 1.0), true);
}

fn paint_grain(
    window: &mut Window,
    grain_x: f64,
    grain_y: f64,
    grain_w: f64,
    grain_h: f64,
    dark: bool,
) {
    let alpha = if dark { 0.03 } else { 0.025 };
    let tone = if dark {
        color(1.0, 1.0, 1.0, alpha)
    } else {
        color(0.0, 0.0, 0.0, alpha)
    };
    let step = 6.0;
    let mut b = PathBuilder::fill();
    let mut y = grain_y;
    let mut row = 0i32;
    while y < grain_y + grain_h {
        let mut x = grain_x + if row % 2 != 0 { step } else { 0.0 };
        while x < grain_x + grain_w {
            if ((x as i32 * 73 + y as i32 * 137) % 97) < 19 {
                add_polygon(
                    &mut b,
                    &[(x, y), (x + 1.5, y), (x + 1.5, y + 1.5), (x, y + 1.5)],
                );
            }
            x += step;
        }
        y += step;
        row += 1;
    }
    paint_fill(window, b, tone);
}

// ---------------------------------------------------------------------------
// The turning sheet
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn paint_turn_shadow(
    window: &mut Window,
    theta: f64,
    lag: f64,
    hinge: f64,
    py: f64,
    ph: f64,
    pw: f64,
    dark: bool,
) {
    let cam = CAM_HEIGHT * pw;
    let cam_x = hinge + CAM_OFFSET * pw;
    let (lead_x, _) = project_page(pw, turn_theta_at(pw, pw, theta, lag), hinge, cam, cam_x);

    let alpha = theta.sin().max(0.0) * if dark { 0.62 } else { 0.42 };
    if alpha < 0.01 {
        return;
    }

    let dir = if theta.cos() >= 0.0 { 1.0 } else { -1.0 };
    let band = (pw * 0.45).max(16.0);
    let (x0, x1) = if dir > 0.0 {
        (lead_x, lead_x + band)
    } else {
        (lead_x - band, lead_x)
    };

    h_gradient(
        window,
        x0.min(x1),
        py,
        (x1 - x0).abs(),
        ph,
        &[
            (0.0, color(0.0, 0.0, 0.0, alpha)),
            (0.35, color(0.0, 0.0, 0.0, alpha * 0.45)),
            (1.0, color(0.0, 0.0, 0.0, 0.0)),
        ],
    );

    let core = (band * 0.18).max(3.0);
    let (c0, c1) = if dir > 0.0 {
        (lead_x, lead_x + core)
    } else {
        (lead_x - core, lead_x)
    };
    h_gradient(
        window,
        c0.min(c1),
        py,
        (c1 - c0).abs(),
        ph,
        &[
            (0.0, color(0.0, 0.0, 0.0, (alpha * 1.5).min(0.85))),
            (1.0, color(0.0, 0.0, 0.0, 0.0)),
        ],
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_turning_page(
    window: &mut Window,
    app: &mut App,
    mirrored: bool,
    layout_idx: usize,
    theta: f64,
    lag: f64,
    hinge: f64,
    cy: f64,
    py: f64,
    ph: f64,
    pw: f64,
    dark: bool,
    page_base: (f64, f64, f64),
) {
    let cam = CAM_HEIGHT * pw;
    let cam_x = hinge + CAM_OFFSET * pw;
    let step = pw / TURN_STRIPS as f64;

    // Projected boundary along the sheet: (x, y top, y bottom).
    let mut edge: Vec<(f64, f64, f64)> = Vec::with_capacity(TURN_STRIPS + 1);
    for i in 0..=TURN_STRIPS {
        let d = i as f64 * step;
        let (x, sc) = project_page(d, turn_theta_at(d, pw, theta, lag), hinge, cam, cam_x);
        edge.push((x, cy + sc * (py - cy), cy + sc * (py + ph - cy)));
    }

    let project = |d: f64, y: f64| -> (f64, f64) {
        let (x, sc) = project_page(d, turn_theta_at(d, pw, theta, lag), hinge, cam, cam_x);
        (x, cy + sc * (py + y - cy))
    };

    // Paper.
    paint_fill(
        window,
        {
            let mut b = PathBuilder::fill();
            let mut points: Vec<(f64, f64)> = edge.iter().map(|(x, t, _)| (*x, *t)).collect();
            for (x, _, bottom) in edge.iter().rev() {
                points.push((*x, *bottom));
            }
            add_polygon(&mut b, &points);
            b
        },
        color(page_base.0, page_base.1, page_base.2, 1.0),
    );

    let layout = LAYOUTS[layout_idx % LAYOUTS.len()];
    let panel_fill = if dark {
        gray(0.20, 1.0)
    } else {
        gray(0.88, 1.0)
    };
    let panel_border = if dark {
        gray(0.72, 1.0)
    } else {
        gray(0.05, 1.0)
    };

    for (pidx, panel) in layout.iter().enumerate() {
        let rx = panel.0 * pw;
        let ry = panel.1 * ph;
        let rw = panel.2 * pw;
        let rh = panel.3 * ph;

        // The back of the sheet shows the next left page mirrored: content at
        // page x sits at `pw - x` from the hinge.
        let (d_lo, d_hi) = if mirrored {
            ((pw - (rx + rw)).max(0.0), (pw - rx).min(pw))
        } else {
            (rx.max(0.0), (rx + rw).min(pw))
        };
        if d_hi <= d_lo {
            continue;
        }

        // Fill one strip at a time so the curl bends the panel with the sheet.
        let first = (d_lo / step).floor() as usize;
        let last = (d_hi / step).ceil() as usize;
        for i in first..last.min(TURN_STRIPS) {
            let a = (i as f64 * step).max(d_lo);
            let b = ((i + 1) as f64 * step).min(d_hi);
            if b <= a {
                continue;
            }
            let (xa, ta) = project(a, ry);
            let (_, ba) = project(a, ry + rh);
            let (xb, tb) = project(b, ry);
            let (_, bb) = project(b, ry + rh);
            // Neighbouring strips share an edge, and each is antialiased, so
            // without a small overlap the shared column shows as a hairline.
            // The fill is one flat colour, so overlapping costs nothing.
            let (x_lo, x_hi, top_lo, bot_lo, top_hi, bot_hi) = if xa <= xb {
                (xa - 0.5, xb + 0.5, ta, ba, tb, bb)
            } else {
                (xb - 0.5, xa + 0.5, tb, bb, ta, ba)
            };
            paint_fill(
                window,
                {
                    let mut path = PathBuilder::fill();
                    add_polygon(
                        &mut path,
                        &[
                            (x_lo, top_lo),
                            (x_hi, top_hi),
                            (x_hi, bot_hi),
                            (x_lo, bot_lo),
                        ],
                    );
                    path
                },
                panel_fill,
            );
        }

        // Tones follow the sheet as projected geometry. gpui has no texture
        // transform, so the pattern is generated in page space and mapped
        // through the same projection as the panels.
        let tone = (layout_idx + pidx) % 4;
        if tone > 0 && rw > 20.0 && rh > 20.0 {
            paint_projected_screentone(window, rx, ry, rw, rh, tone, dark, mirrored, pw, &project);
        }

        // Border: sample the outline so it follows the projection too.
        let mut border = PathBuilder::stroke(px(3.5));
        let samples = 20;
        let mut first_point = true;
        for i in 0..=samples {
            let d = d_lo + (d_hi - d_lo) * i as f64 / samples as f64;
            let (x, y) = project(d, ry);
            if first_point {
                border.move_to(p(x, y));
                first_point = false;
            } else {
                border.line_to(p(x, y));
            }
        }
        for i in (0..=samples).rev() {
            let d = d_lo + (d_hi - d_lo) * i as f64 / samples as f64;
            let (x, y) = project(d, ry + rh);
            border.line_to(p(x, y));
        }
        border.close();
        paint_stroke(window, border, panel_border);

        // Bubbles and the seal, projected point by point.
        if rw > 45.0 && rh > 45.0 {
            let boff = BUBBLE_OFFSETS[pidx % BUBBLE_OFFSETS.len()];
            let bx = rx + rw * boff.0;
            let by = ry + rh * boff.1;
            let r = rw.min(rh) * 0.28;
            if r >= 6.0 {
                paint_projected_bubble(
                    window,
                    app,
                    bx,
                    by,
                    r.min(36.0),
                    mirrored,
                    pw,
                    dark,
                    BUBBLE_TYPES[(layout_idx + pidx) % BUBBLE_TYPES.len()],
                    &project,
                );
            }
        }
        if pidx == 0 && layout_idx % 3 == 0 && rw > 60.0 && rh > 60.0 {
            let hx = rx + rw * 0.88;
            let hy = ry + rh * 0.88;
            let r = (rw.min(rh) * 0.14).clamp(5.0, 20.0);
            paint_projected_circle(
                window,
                hx,
                hy,
                r,
                mirrored,
                pw,
                color(0.80, 0.12, 0.10, 1.0),
                &project,
            );
        }
    }

    // Sheen across the sheet, then the curled edge and the rim.
    let spine_x = edge[0].0;
    let lead_x = edge[TURN_STRIPS].0;
    let dir = if lead_x >= spine_x { 1.0 } else { -1.0 };
    let (tint, strength) = if dark {
        (color(1.0, 1.0, 1.0, 1.0), 1.0)
    } else {
        (color(0.0, 0.0, 0.0, 1.0), 0.55)
    };
    let sheen = theta.sin().max(0.0) * if mirrored { 0.12 } else { 0.22 } * strength;

    if sheen > 0.004 {
        let angle = if dir > 0.0 { 90.0 } else { 270.0 };
        let path = {
            let mut b = PathBuilder::fill();
            let mut points: Vec<(f64, f64)> = edge.iter().map(|(x, t, _)| (*x, *t)).collect();
            for (x, _, bottom) in edge.iter().rev() {
                points.push((*x, *bottom));
            }
            add_polygon(&mut b, &points);
            b
        };
        if let Ok(path) = path.build() {
            window.paint_path(
                path,
                linear_gradient(
                    angle,
                    linear_color_stop(tint.opacity((sheen * 0.55) as f32), 0.0),
                    linear_color_stop(tint.opacity(sheen as f32), 1.0),
                ),
            );
        }
    }

    let curl = theta.sin().max(0.0) * 0.34;
    if curl > 0.004 {
        let band = ((lead_x - spine_x).abs() * 0.22).max(2.0);
        let x = if dir > 0.0 { lead_x - band } else { lead_x };
        h_gradient(
            window,
            x,
            py,
            band,
            ph,
            &[
                (0.0, color(0.0, 0.0, 0.0, 0.0)),
                (1.0, color(0.0, 0.0, 0.0, curl.min(0.34))),
            ],
        );
    }

    let (edge_x, edge_s) = project_page(pw, turn_theta_at(pw, pw, theta, lag), hinge, cam, cam_x);
    let (_, edge_top) = project(pw, 0.0);
    let (_, edge_bot) = project(pw, ph);
    let _ = edge_s;

    paint_stroke(
        window,
        {
            let mut b = PathBuilder::stroke(px((pw * 0.008).clamp(0.8, 1.8) as f32));
            b.move_to(p(edge_x, edge_top));
            b.line_to(p(edge_x, edge_bot));
            b
        },
        color(
            1.0,
            1.0,
            1.0,
            if dark { 0.34 } else { 0.65 } * theta.sin().max(0.0),
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_projected_bubble(
    window: &mut Window,
    app: &mut App,
    bx: f64,
    by: f64,
    r: f64,
    mirrored: bool,
    pw: f64,
    dark: bool,
    kind: &str,
    project: &impl Fn(f64, f64) -> (f64, f64),
) {
    let ink = if dark {
        gray(0.15, 1.0)
    } else {
        gray(1.0, 1.0)
    };
    paint_projected_circle(window, bx, by, r, mirrored, pw, ink, project);

    let border = if dark {
        gray(0.8, 1.0)
    } else {
        gray(0.05, 1.0)
    };
    let mut points = Vec::with_capacity(29);
    for i in 0..28 {
        let a = i as f64 / 28.0 * TAU;
        let x = bx + r * a.cos();
        let d = if mirrored { pw - x } else { x };
        points.push(project(d, by + r * a.sin()));
    }
    paint_stroke(
        window,
        {
            let mut b = PathBuilder::stroke(px(1.4));
            add_polygon(&mut b, &points);
            b
        },
        border,
    );

    // Text on the moving sheet: the sheet is squeezed horizontally, and gpui
    // cannot scale a shaped line, so the glyphs are only drawn while the face
    // is wide enough for them to read. The size follows the perspective.
    if let Some((content, size)) = bubble_text(kind, r) {
        let d_center = if mirrored { pw - bx } else { bx };
        let (tx, ty) = project(d_center, by);
        let (_, ty_below) = project(d_center, by + 1.0);
        let (tx_right, _) = project(d_center + 1.0, by);
        let scale_y = (ty_below - ty).abs();
        let width_factor = (tx_right - tx).abs();
        if width_factor > 0.45 {
            let text_color = if dark {
                gray(0.85, 1.0)
            } else {
                gray(0.10, 1.0)
            };
            paint_text(
                window,
                app,
                content,
                tx,
                ty,
                size * scale_y,
                text_color,
                true,
            );
        }
    }
}

/// The glyphs a bubble type shows, with the font size GTK uses.
fn bubble_text(kind: &str, r: f64) -> Option<(&'static str, f64)> {
    match kind {
        "japanese" => Some(("ああ…", (r * 0.65).max(8.0))),
        "exclaim" => Some(("!!", (r * 0.8).max(9.0))),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_projected_circle(
    window: &mut Window,
    cx: f64,
    cy: f64,
    r: f64,
    mirrored: bool,
    pw: f64,
    fill_color: Hsla,
    project: &impl Fn(f64, f64) -> (f64, f64),
) {
    let mut points = Vec::with_capacity(28);
    for i in 0..28 {
        let a = i as f64 / 28.0 * TAU;
        let x = cx + r * a.cos();
        let d = if mirrored { pw - x } else { x };
        points.push(project(d, cy + r * a.sin()));
    }
    paint_fill(
        window,
        {
            let mut b = PathBuilder::fill();
            add_polygon(&mut b, &points);
            b
        },
        fill_color,
    );
}

/// Tones on the turning sheet: the pattern is built in page space and every
/// point runs through `project`, so it bends with the paper.
#[allow(clippy::too_many_arguments)]
fn paint_projected_screentone(
    window: &mut Window,
    rx: f64,
    ry: f64,
    rw: f64,
    rh: f64,
    pattern: usize,
    dark: bool,
    mirrored: bool,
    pw: f64,
    project: &impl Fn(f64, f64) -> (f64, f64),
) {
    let tone = if dark {
        color(0.55, 0.55, 0.55, 0.45)
    } else {
        color(0.25, 0.25, 0.25, 0.35)
    };
    let map_x = |x: f64| if mirrored { pw - x } else { x };

    match pattern {
        1 => {
            // Coarser than the resting spread: every dot costs three
            // projections, and the sheet is in motion anyway.
            let spacing = 11.0;
            let dot_r = 1.5;
            let mut b = PathBuilder::fill();
            let mut y = ry + spacing / 2.0;
            let mut row = 0;
            while y < ry + rh - dot_r {
                let offset = if row % 2 != 0 { spacing / 2.0 } else { 0.0 };
                let mut x = rx + spacing / 2.0 + offset;
                while x < rx + rw - dot_r {
                    let d = map_x(x);
                    let (sx, sy) = project(d, y);
                    let (sx_next, _) = project(d + 0.5, y);
                    let (_, sy_next) = project(d, y + 0.5);
                    let rx_screen = (dot_r * (sx_next - sx).abs() * 2.0).max(0.35);
                    let ry_screen = (dot_r * (sy_next - sy).abs() * 2.0).max(0.35);
                    add_ellipse(&mut b, sx, sy, rx_screen, ry_screen, 6);
                    x += spacing;
                }
                y += spacing;
                row += 1;
            }
            paint_fill(window, b, tone);
        }
        2 | 3 => {
            let spacing = if pattern == 2 { 6.0 } else { 9.0 };
            let mut b = PathBuilder::stroke(px(if pattern == 2 { 0.7 } else { 0.5 }));
            let total = (rw + rh) as i32;
            let mut i = -total;
            while i < total {
                let x0 = rx + i as f64;
                if let Some((t0, t1)) = clip_segment(rx, ry, rw, rh, x0, ry, x0 + rh, ry + rh) {
                    add_projected_line(
                        &mut b,
                        x0,
                        ry,
                        x0 + rh,
                        ry + rh,
                        t0,
                        t1,
                        mirrored,
                        pw,
                        project,
                    );
                }
                i += spacing as i32;
            }
            if pattern == 3 {
                let mut i = -total;
                while i < total {
                    let x0 = rx + i as f64 + rw;
                    if let Some((t0, t1)) = clip_segment(rx, ry, rw, rh, x0, ry, x0 - rh, ry + rh) {
                        add_projected_line(
                            &mut b,
                            x0,
                            ry,
                            x0 - rh,
                            ry + rh,
                            t0,
                            t1,
                            mirrored,
                            pw,
                            project,
                        );
                    }
                    i += spacing as i32;
                }
            }
            paint_stroke(window, b, tone);
        }
        _ => {}
    }
}

/// Clip a page-space segment to the panel it belongs to, so a hatch line only
/// draws the part that Cairo's `clip()` would have kept.
fn clip_segment(
    rx: f64,
    ry: f64,
    rw: f64,
    rh: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> Option<(f64, f64)> {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let mut t0 = 0.0_f64;
    let mut t1 = 1.0_f64;
    for (p, q) in [
        (-dx, x0 - rx),
        (dx, rx + rw - x0),
        (-dy, y0 - ry),
        (dy, ry + rh - y0),
    ] {
        if p.abs() < 1e-9 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return None;
                }
                if r > t0 {
                    t0 = r;
                }
            } else {
                if r < t0 {
                    return None;
                }
                if r < t1 {
                    t1 = r;
                }
            }
        }
    }
    Some((t0, t1))
}

#[allow(clippy::too_many_arguments)]
fn add_projected_line(
    builder: &mut PathBuilder,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    t0: f64,
    t1: f64,
    mirrored: bool,
    pw: f64,
    project: &impl Fn(f64, f64) -> (f64, f64),
) {
    const SAMPLES: usize = 10;
    for k in 0..=SAMPLES {
        let t = t0 + (t1 - t0) * k as f64 / SAMPLES as f64;
        let x = x0 + (x1 - x0) * t;
        let y = y0 + (y1 - y0) * t;
        let d = if mirrored { pw - x } else { x };
        let (sx, sy) = project(d, y);
        if k == 0 {
            builder.move_to(p(sx, sy));
        } else {
            builder.line_to(p(sx, sy));
        }
    }
}

fn add_ellipse(builder: &mut PathBuilder, cx: f64, cy: f64, rx: f64, ry: f64, segments: usize) {
    let mut points = Vec::with_capacity(segments);
    for i in 0..segments {
        let a = i as f64 / segments as f64 * TAU;
        points.push(p(cx + rx * a.cos(), cy + ry * a.sin()));
    }
    builder.add_polygon(&points, true);
}

// ---------------------------------------------------------------------------
// Animation view
// ---------------------------------------------------------------------------

/// Owns the placeholder's frame loop so that a turn repaints this canvas only.
/// Re-rendering the whole `MangaApp` tree at 60 fps was the expensive part.
///
/// Frames are requested from inside `render`: when the view leaves the element
/// tree (a page is selected) nothing re-renders it, so the loop stops by itself.
pub(crate) struct PageTurnView {
    app: WeakEntity<crate::app::MangaApp>,
    /// When the last frame was actually painted. gpui rebuilds the whole
    /// window scene per frame, so a 60 fps decorative loop costs real CPU;
    /// 30 fps still reads smoothly for a turn that lasts 1.5 s.
    last_paint: std::time::Instant,
}

/// Minimum time between two repaints of the placeholder.
const FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(33);

impl PageTurnView {
    pub(crate) fn new(app: WeakEntity<crate::app::MangaApp>) -> Self {
        Self {
            app,
            last_paint: std::time::Instant::now(),
        }
    }

    /// Keep exactly one frame callback in flight. After a repaint the next one
    /// is armed by `render`; while waiting out `FRAME_INTERVAL` the callback
    /// re-arms itself so the loop does not stall between repaints.
    fn arm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.on_next_frame(window, |this, window, cx| {
            let now = std::time::Instant::now();
            if now.duration_since(this.last_paint) >= FRAME_INTERVAL {
                this.last_paint = now;
                cx.notify();
            } else {
                this.arm(window, cx);
            }
        });
    }
}

impl Render for PageTurnView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !cx.reduce_motion() {
            self.arm(window, cx);
        }

        let (dark, elapsed) = self
            .app
            .upgrade()
            .map(|app| {
                let app = app.read(cx);
                (app.theme.dark, app.page_turn_start.elapsed().as_secs_f64())
            })
            .unwrap_or((true, 0.0));

        canvas(
            move |_bounds, _window, _cx| (),
            move |bounds, _, window, cx| paint(window, cx, bounds, dark, elapsed),
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The turn has to reach the far spread exactly: the resting frame that
    /// follows is drawn from scratch, so any leftover angle shows up as a pop.
    #[test]
    fn turn_lands_exactly_flat() {
        let last = turn_phase(STATIC_TIME + FLIP_TIME - 1e-9).expect("still turning");
        assert!(
            (last.progress - 1.0).abs() < 1e-6,
            "progress {}",
            last.progress
        );
        assert!(turn_phase(STATIC_TIME + FLIP_TIME + 1e-6).is_none());
        assert!(turn_phase(STATIC_TIME - 1e-9).is_none());
    }

    /// A spring that does not overshoot would be an ease curve again.
    #[test]
    fn turn_overshoots_then_settles() {
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
        assert!((0.6..0.85).contains(&peak_at), "peak at {peak_at}");
    }

    /// The bow follows how fast the sheet moves, not its angle.
    #[test]
    fn bow_follows_the_sweep() {
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
        assert!((0.2..0.5).contains(&widest_at), "widest at {widest_at}");

        let start = turn_phase(STATIC_TIME + 1e-6).expect("turning");
        assert!(start.lag.abs() < 1e-3, "bow at start {}", start.lag);
        let end = turn_phase(STATIC_TIME + FLIP_TIME - 1e-6).expect("turning");
        assert!(end.lag.abs() < 1e-2, "bow at end {}", end.lag);
    }

    /// `TURN_RATE_PEAK` only makes `PAPER_CURL` readable if it really is the
    /// peak of the rate.
    #[test]
    fn turn_rate_peak_is_current() {
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

    /// A sheet lying flat projects without any perspective distortion.
    #[test]
    fn flat_sheet_projects_one_to_one() {
        let hinge = 100.0;
        let cam = CAM_HEIGHT * 50.0;
        let cam_x = hinge + CAM_OFFSET * 50.0;
        for d in [0.0, 12.5, 50.0] {
            let (x, scale) = project_page(d, 0.0, hinge, cam, cam_x);
            assert!((x - (hinge + d)).abs() < 1e-9, "x at {d}: {x}");
            assert!((scale - 1.0).abs() < 1e-9, "scale at {d}: {scale}");
        }
    }

    #[test]
    fn book_rect_keeps_the_spread_aspect() {
        let (_, _, bw, bh) = book_rect(800.0, 600.0);
        assert!((bw / bh - 1.3).abs() < 1e-9, "{}x{}", bw, bh);
        let (x, y, bw, bh) = book_rect(800.0, 600.0);
        assert!((x * 2.0 + bw - 800.0).abs() < 1e-9);
        assert!((y * 2.0 + bh - 600.0).abs() < 1e-9);
    }
}
