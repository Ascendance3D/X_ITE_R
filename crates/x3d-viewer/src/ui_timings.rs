//! The native Browser Timings panel: X_ITE's `Browser/Core/BrowserTimings.js` (pinned 16.5.0) drawn by the software
//! canvas of [`crate::ui`] at the bottom left of the world.
//!
//! What follows `BrowserTimings.js`: the 13 rows with their texts and order (`Frame rate`, `Speed` and, in the MORE
//! type, `Browser` .. `Number of Sensors`), the title `Browser Timings`, the More / Less button (`More Properties`,
//! `Less Properties`; the panel starts as LESS), the number formats (`Intl.NumberFormat` with 2 or 0 fraction digits and
//! grouping), the units (`fps`, `ms`, `m/s` below 15 m/s and then `km/h` = 3.6 × m/s), the frame rate of a `StopWatch`
//! that is read once more than a second has gone, the reset when the option turns on (after the next frame), and the
//! pointer rule of the CSS: the panel lets the pointer through to the world, except its button and the two cells with a
//! `title` (`pointer-events: all`).
//!
//! Differences from X_ITE (each is also a line of the PR that added this module):
//!
//! * The look is the plain light theme of the menu: no blur, no schedule icon, no fade, no hover colour of the button,
//!   and the `title` tooltips of the two count cells are not shown (the cells still take the pointer).
//! * Values the native viewer does not measure separately show zero, like the browser build: `Speed` (X_ITE's
//!   `currentSpeed`), `Pointer`, `Camera`, `Picking`, `Collision Detection` (the viewer's step runs these inside one
//!   call) and `Number of Sensors`. `Event Processing` is the time of the step (events, routes, scripts, navigation),
//!   `Rendering` is the frame's build plus its draw, `X3D total` is both, and `Browser` is the whole tick the host
//!   measured (X_ITE's system time).
//! * The primitive and shape counts come from the frame's draw items (opaque and transparent), counted the way
//!   `BrowserTimings.countPrimitives` counts a geometry node's vertices; X_ITE's `getCountPrimitives ()` of an
//!   execution context (prototype internals) is not modelled.
//! * The frame rate counts every interval between two updates (see `update`).
//! * The touch `touchend` of the button is not a native event.

use crate::ui_text::{Canvas, Color, Text};

const FONT: f64 = 12.0;
const LINE: f64 = FONT * 1.1;
const SPACING: f64 = 2.0;
const PAD: (f64, f64, f64, f64) = (8.0, 12.0, 10.0, 12.0);
const BUTTON_H: f64 = 14.0 * 4.0 / 3.0;
const BG: Color = [244, 244, 244, 238];
const BORDER: Color = [150, 150, 150, 255];
const TEXT: Color = [36, 36, 36, 255];
const TINT: Color = [220, 228, 240, 255];

/// The rows, as in the HTML of `BrowserTimings.initialize`: the label, the CSS class of the value cell and its `title`.
pub const ROWS: [(&str, &str, &str); 13] = [
    ("Frame rate", "frame-rate", ""),
    ("Speed", "speed", ""),
    ("Browser", "browser-time", ""),
    ("X3D total", "x3d-total", ""),
    ("Event Processing", "event-processing", ""),
    ("Pointer", "pointer", ""),
    ("Camera", "camera", ""),
    ("Picking", "picking", ""),
    ("Collision Detection", "collision-detection", ""),
    ("Rendering", "rendering", ""),
    (
        "Number of Primitives",
        "primitives pointer",
        "Points; Lines; Triangles",
    ),
    (
        "Number of Shapes",
        "shapes pointer",
        "Opaque Shapes + Transparent Shapes",
    ),
    ("Number of Sensors", "sensors", ""),
];

/// Row indices.
const FRAME_RATE: usize = 0;
const SPEED: usize = 1;
const BROWSER: usize = 2;
const X3D_TOTAL: usize = 3;
const EVENTS: usize = 4;
const POINTER: usize = 5;
const CAMERA: usize = 6;
const PICKING: usize = 7;
const COLLISION: usize = 8;
const RENDERING: usize = 9;
const PRIMITIVES: usize = 10;
const SHAPES: usize = 11;
const SENSORS: usize = 12;

/// `Intl.NumberFormat ("en-US", { minimumFractionDigits, maximumFractionDigits })` for a finite value, 0 otherwise.
fn format(value: f64, digits: usize) -> String {
    let value = if value.is_finite() { value } else { 0.0 };
    let text = format!("{:.*}", digits, value.abs());
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    let mut grouped = String::new();

    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(',');
        }

        grouped.push(c);
    }

    // `-0.00` is "-0.00" in Intl only for a value that rounds to zero from below; the times here are never negative.
    let negative = value < 0.0 && text.chars().any(|c| c.is_ascii_digit() && c != '0');
    let sign = if negative { "-" } else { "" };

    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{frac}")
    }
}

pub fn f0(value: f64) -> String {
    format(value, 0)
}

pub fn f2(value: f64) -> String {
    format(value, 2)
}

/// `getSpeed` and `getSpeedUnit`.
pub fn speed_text(speed: f64) -> String {
    if speed < 15.0 {
        format!("{} m/s", f2(speed))
    } else {
        format!("{} km/h", f2(speed * 3.6))
    }
}

/// A sum of times and the number of frames it covers (`StopWatch.averageTime`).
#[derive(Clone, Copy, Debug, Default)]
struct Average {
    sum: f64,
    cycles: u32,
}

impl Average {
    fn add(&mut self, ms: f64) {
        self.sum += ms;
        self.cycles += 1;
    }

    fn get(&self) -> f64 {
        if self.cycles > 0 {
            self.sum / self.cycles as f64
        } else {
            0.0
        }
    }
}

/// What one frame of the host measured, in milliseconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTimes {
    /// The whole tick of the host (X_ITE's system time).
    pub system: f64,
    /// `Viewer3d::step`: events, routes, scripts, navigation.
    pub step: f64,
    /// `build_frame` plus the draw.
    pub display: f64,
}

/// The counts of the last frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts {
    pub points: f64,
    pub lines: f64,
    pub triangles: f64,
    pub opaque: u32,
    pub transparent: u32,
}

/// The state of the panel.
#[derive(Clone, Debug)]
pub struct TimingsPanel {
    /// The panel shows (the `Timings` browser option, as the panel last saw it).
    pub visible: bool,
    /// The type is MORE (all rows and the `Less Properties` button).
    pub more: bool,
    // StopWatch `fps`, in milliseconds of the viewer's clock.
    start: f64,
    stop: f64,
    cycles: u32,
    elapsed: f64,
    /// BrowserTimings.reset: the stop watch restarts at the next frame.
    reset_pending: bool,
    system: Average,
    total: Average,
    display: Average,
    pub counts: Counts,
    /// The texts of the value cells, in [`ROWS`] order.
    pub values: Vec<String>,
    /// The button went down and not yet up.
    pub pressed: bool,
    /// Bumped by every change of the texts, the type or the visibility.
    pub revision: u64,
}

impl Default for TimingsPanel {
    fn default() -> TimingsPanel {
        TimingsPanel::new()
    }
}

impl TimingsPanel {
    /// `BrowserTimings.initialize`: the first type is LESS.
    pub fn new() -> TimingsPanel {
        let mut p = TimingsPanel {
            visible: false,
            // The type is flipped once before the first `set_type__`.
            more: true,
            start: 0.0,
            stop: 0.0,
            cycles: 0,
            elapsed: 0.0,
            reset_pending: false,
            system: Average::default(),
            total: Average::default(),
            display: Average::default(),
            counts: Counts::default(),
            values: vec![String::new(); ROWS.len()],
            pressed: false,
            revision: 0,
        };

        p.set_type();
        p
    }

    pub fn button_text(&self) -> &'static str {
        if self.more {
            "Less Properties"
        } else {
            "More Properties"
        }
    }

    /// The rows the table shows: `table.less tr.x_ite-private-more ~ tr { display: none }` hides all rows after `Speed`.
    pub fn visible_rows(&self) -> usize {
        if self.more { ROWS.len() } else { 2 }
    }

    /// `set_type__`: the button's click.
    pub fn set_type(&mut self) {
        self.more = !self.more;
        self.build();
    }

    /// `set_enabled__`: the `Timings` option changed to `on`.
    pub fn set_enabled(&mut self, on: bool) {
        if on == self.visible {
            return;
        }

        self.visible = on;
        self.revision += 1;

        if on {
            self.reset_pending = true;
        }
    }

    fn reset_watch(&mut self) {
        self.start = 0.0;
        self.stop = 0.0;
        self.cycles = 0;
        self.elapsed = 0.0;
    }

    /// `update`, at each frame (`prepareEvents`): `now` is the viewer's clock in seconds.
    pub fn update(&mut self, now: f64) {
        if !self.visible {
            return;
        }

        let now = now * 1000.0;

        // BrowserTimings.reset: after the next frame the stop watch starts again.
        if self.reset_pending {
            self.reset_pending = false;
            self.reset_watch();
            self.build();
            self.start = now;
            return;
        }

        // StopWatch.stop. X_ITE skips the cycle when `startTime <= stopTime`, which its millisecond `Date.now ()` gives
        // for an update shorter than a millisecond; the viewer's clock is the frame time, so every interval counts.
        if self.start > 0.0 {
            self.stop = now;
            self.cycles += 1;
            self.elapsed += self.stop - self.start;
        }

        if self.elapsed > 1000.0 {
            self.build();
            self.reset_watch();
        }

        self.start = now;
    }

    /// The times of the frame just drawn.
    pub fn record(&mut self, t: FrameTimes) {
        if !self.visible {
            return;
        }

        self.system.add(t.system.max(t.step + t.display));
        self.total.add(t.step + t.display);
        self.display.add(t.display);
    }

    /// The counts of the frame just built.
    pub fn set_counts(&mut self, c: Counts) {
        self.counts = c;
    }

    /// `build`.
    pub fn build(&mut self) {
        let mut v = vec![String::new(); ROWS.len()];

        if self.elapsed > 0.0 && self.cycles > 0 {
            let average = self.elapsed / self.cycles as f64;

            v[FRAME_RATE] = format!("{} fps", f2(1000.0 / average));
        } else {
            v[FRAME_RATE] = format!("{} fps", f2(0.0));
        }

        v[SPEED] = speed_text(0.0);

        if self.more {
            let display = self.display.get();
            let total = self.total.get();
            // routingTime: browserTime - (camera + collision + display), at least 0.
            let routing = (total - display).max(0.0);
            let ms = |x: f64| format!("{} ms", f2(x));

            v[BROWSER] = ms(self.system.get());
            v[X3D_TOTAL] = ms(total);
            v[EVENTS] = ms(routing);
            v[POINTER] = ms(0.0);
            v[CAMERA] = ms(0.0);
            v[PICKING] = ms(0.0);
            v[COLLISION] = ms(0.0);
            v[RENDERING] = ms(display);
            v[PRIMITIVES] = format!(
                "{}; {}; {}",
                f0(self.counts.points),
                f0(self.counts.lines),
                f0(self.counts.triangles)
            );
            v[SHAPES] = format!(
                "{} + {}",
                f0(self.counts.opaque as f64),
                f0(self.counts.transparent as f64)
            );
            v[SENSORS] = f0(0.0);

            self.system = Average::default();
            self.total = Average::default();
            self.display = Average::default();
        } else {
            // The rows of the LESS type are not built; the old texts stay.
            for (i, old) in self.values.iter().enumerate().skip(BROWSER) {
                v[i] = old.clone();
            }
        }

        if v != self.values {
            self.values = v;
            self.revision += 1;
        }
    }

    // --- layout --------------------------------------------------------------------------------------------------

    fn columns(&self, text: &Text) -> (f64, f64) {
        let rows = self.visible_rows();
        let label = ROWS[..rows]
            .iter()
            .map(|r| text.width(&format!("{}:", r.0), FONT))
            .fold(0.0, f64::max)
            + 10.0;
        let value = self.values[..rows]
            .iter()
            .map(|v| text.width(v, FONT))
            .fold(text.width("0.00 ms", FONT), f64::max);
        let head = 20.0 + text.width("Browser Timings", FONT);
        let button = text.width(self.button_text(), FONT) + 16.0;
        let table = (label + value).max(head).max(button);

        (label, table - label)
    }

    /// The panel's rectangle (x, y, w, h) in a canvas of `height` pixels, and the rectangles that take the pointer: the
    /// button, then the two cells with a title (when shown).
    pub fn layout(&self, text: &Text, height: f64) -> Layout {
        let (label, value) = self.columns(text);
        let table_w = label + value;
        let rows = self.visible_rows();
        let row_h = |i: usize| LINE + if i == SPEED { 5.0 } else { 2.0 } + SPACING;
        let head_h = LINE + 3.0 + 5.0 + SPACING;
        let rows_h: f64 = (0..rows).map(row_h).sum();
        let foot_h = 4.0 + 2.0 + BUTTON_H + SPACING;
        let w = PAD.1 + SPACING + table_w + SPACING + PAD.3;
        let h = PAD.0 + SPACING + head_h + rows_h + foot_h + SPACING + PAD.2;
        let y = height - h;
        let tx = PAD.3 + SPACING;
        let mut ry = y + PAD.0 + SPACING + head_h;
        let mut cells = vec![];

        for i in 0..rows {
            if i == PRIMITIVES || i == SHAPES {
                cells.push([tx + label, ry, value, LINE + 2.0]);
            }

            ry += row_h(i);
        }

        let button = [tx, ry + 4.0 + 2.0, table_w, BUTTON_H];

        Layout {
            rect: [0.0, y, w, h],
            button,
            cells,
            label,
            value,
            table_x: tx,
            top: y + PAD.0 + SPACING,
        }
    }

    /// Whether the point (canvas pixels, top left origin) is on a part of the panel that takes the pointer.
    pub fn takes_pointer(&self, text: &Text, canvas_h: f64, x: f64, y: f64) -> Option<Hit> {
        if !self.visible {
            return None;
        }

        let l = self.layout(text, canvas_h);
        let inside = |r: &[f64; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];

        if inside(&l.button) {
            Some(Hit::Button)
        } else if l.cells.iter().any(inside) {
            Some(Hit::Cell)
        } else {
            None
        }
    }

    pub fn paint(&self, canvas: &mut Canvas, text: &mut Text) {
        if !self.visible {
            return;
        }

        let l = self.layout(text, canvas.height as f64);
        let [x, y, w, h] = l.rect;

        canvas.fill_rect(x, y, w, h, [0.0, 12.0, 0.0, 0.0], BG);
        canvas.stroke_rect(x, y, w, h, BORDER);

        let ascent = text.ascent(FONT);
        let mut ry = l.top;

        text.draw(
            canvas,
            "Browser Timings",
            l.table_x + 20.0,
            ry + 3.0 + ascent,
            FONT,
            [0, 0, 0, 255],
        );

        ry += LINE + 3.0 + 5.0 + SPACING;

        let rows = self.visible_rows();

        for (i, row) in ROWS[..rows].iter().enumerate() {
            let base = ry + ascent;

            text.draw(canvas, &format!("{}:", row.0), l.table_x, base, FONT, TEXT);
            text.draw(
                canvas,
                &self.values[i],
                l.table_x + l.label,
                base,
                FONT,
                TEXT,
            );
            ry += LINE + if i == SPEED { 5.0 } else { 2.0 } + SPACING;
        }

        let [bx, by, bw, bh] = l.button;

        canvas.fill_rect(bx, by, bw, bh, [10.0; 4], TINT);
        canvas.stroke_rect(bx, by, bw, bh, BORDER);

        let label = self.button_text();
        let lw = text.width(label, FONT);

        text.draw(
            canvas,
            label,
            bx + (bw - lw) / 2.0,
            by + (bh - text.line_height(FONT)) / 2.0 + ascent,
            FONT,
            TEXT,
        );
    }
}

/// Where the parts of the panel are.
#[derive(Clone, Debug)]
pub struct Layout {
    pub rect: [f64; 4],
    pub button: [f64; 4],
    pub cells: Vec<[f64; 4]>,
    pub label: f64,
    pub value: f64,
    pub table_x: f64,
    pub top: f64,
}

/// What a point on the panel hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Button,
    /// A cell with a `title`: it takes the pointer and does nothing.
    Cell,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_formatted_like_intl() {
        assert_eq!(f2(0.0), "0.00");
        assert_eq!(f2(1234.5678), "1,234.57");
        assert_eq!(f0(1234567.0), "1,234,567");
        assert_eq!(f0(f64::NAN), "0");
        assert_eq!(f2(f64::INFINITY), "0.00");
        assert_eq!(speed_text(14.99), "14.99 m/s");
        assert_eq!(speed_text(15.0), "54.00 km/h");
    }

    #[test]
    fn the_panel_starts_as_less() {
        let p = TimingsPanel::new();

        assert!(!p.more);
        assert_eq!(p.button_text(), "More Properties");
        assert_eq!(p.visible_rows(), 2);
        assert_eq!(p.values[FRAME_RATE], "0.00 fps");
        assert_eq!(p.values[SPEED], "0.00 m/s");
    }
}
