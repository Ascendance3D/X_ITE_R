//! Port of src/x_ite/Parser/SVGParser.js (pinned X_ITE 16.5.0): an `<svg>` document becomes an
//! OrthoViewpoint, a Background and a `ViewBox` Transform holding the shapes.
//!
//! What X_ITE supports is ported, and nothing else: `use`, `g`, `switch`, `a`, `rect`, `circle`,
//! `ellipse`, `image`, `polyline`, `polygon` and `path` (`text` is an empty function there, and
//! `line`, `defs`, `svg` (nested) and `pattern` are not elements it knows). Fills are
//! triangulated with libtess (`x3d_text::libtess`, called as `SVGParser.triangulatePolygon` does);
//! gradients are painted into a canvas texture (`svg_canvas.rs`).
//!
//! The JavaScript quirks that change the output are kept (see the comments at each): the shared
//! `this .input` of the lexer, `null` attributes read as the text "null", a `stop` outside 0..1
//! that leaks its style, `vectorEffect` never being read from the style, and so on.

use crate::color::try_convert_color;
use crate::dom::{Element, XNode};
use crate::formats::{create, push_child, set, set_node, set_numbers};
use crate::svg_canvas::{self, GradientKind};
use crate::{LoadError, ParseError};
use std::collections::HashMap;
use x3d_fields::math::{Color3, Matrix3, Matrix4, Rotation4, Vector2, Vector3, Vector4};
use x3d_fields::{FieldValue, NodeId, jsmath};
use x3d_scene::*;

type R<T> = Result<T, LoadError>;

fn error<T>(message: impl Into<String>) -> R<T> {
    Err(LoadError::Parse(ParseError {
        message: message.into(),
        line: 0,
    }))
}

// Constants.
const MM: f64 = 0.001; // One mm in meters.
const CM: f64 = 0.01; // One cm in meters.
const INCH: f64 = 0.0254; // One inch in meters.
const POINT: f64 = INCH / 72.0; // One point in meters.
const PICA: f64 = INCH / 6.0; // One pica in meters.
const PIXEL: f64 = INCH / 90.0; // One pixel in meters.
const EM: f64 = 16.0; // One em in pixels.
const SPREAD: f64 = 16.0; // Spread factor, Integer.

/// Maximum nesting of `use`/`g` elements. X_ITE has none; it throws a RangeError at the engine's
/// stack limit (some thousand frames), which this stands in for.
const MAX_DEPTH: usize = 400;

/// BrowserOptions PrimitiveQuality and TextureQuality, as `SVGParser` reads them in its constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
}

thread_local! {
    static QUALITY: std::cell::Cell<(Quality, Quality)> = const { std::cell::Cell::new((Quality::Medium, Quality::Medium)) };
}

/// The browser options the SVG parser reads: (PrimitiveQuality, TextureQuality).
pub fn set_quality(primitive: Quality, texture: Quality) {
    QUALITY.with(|q| q.set((primitive, texture)));
}

// --- the lexer ----------------------------------------------------------------------------------

/// The parser's own string cursor (`parseValue`, the sticky `Grammar` expressions, `this .value`,
/// `this .result`).
struct Lex {
    s: String,
    p: usize,
    value: f64,
    /// `this .result [0]` and `this .result [1]` of the last successful expression.
    r0: String,
    r1: String,
}

fn is_digit(b: u8) -> bool {
    b.is_ascii_digit()
}

/// JS parseFloat of a token that matched the `double` expression.
fn parse_float_token(t: &str) -> f64 {
    let t = t.strip_prefix('+').unwrap_or(t);

    if let Ok(v) = t.parse::<f64>() {
        return v;
    }

    // "5." and "5.e3": Rust wants a digit after the point only in some forms.
    let fixed = t.replacen(".e", ".0e", 1).replacen(".E", ".0E", 1);
    let fixed = if fixed.ends_with('.') {
        format!("{fixed}0")
    } else {
        fixed
    };

    fixed.parse::<f64>().unwrap_or(f64::NAN)
}

impl Lex {
    fn new() -> Lex {
        Lex {
            s: String::new(),
            p: 0,
            value: f64::NAN,
            r0: String::new(),
            r1: String::new(),
        }
    }

    /// `parseValue (value)`: `null` is read as the text "null" by every expression.
    fn parse_value(&mut self, value: Option<&str>) {
        self.s = value.unwrap_or("null").to_string();
        self.p = 0;
        self.value = f64::NAN;
    }

    fn bytes(&self) -> &[u8] {
        self.s.as_bytes()
    }

    /// whitespaces: /[\x20\n\t\r]+/y
    fn whitespaces(&mut self) -> bool {
        let b = self.bytes();
        let mut i = self.p;

        while i < b.len() && matches!(b[i], b' ' | b'\n' | b'\t' | b'\r') {
            i += 1;
        }

        let ok = i > self.p;
        self.p = i;
        ok
    }

    fn comma_token(&mut self) -> bool {
        if self.bytes().get(self.p) == Some(&b',') {
            self.p += 1;
            true
        } else {
            false
        }
    }

    /// `!! (this .whitespaces () | Grammar .comma .parse (this))`: both parts always run.
    fn comma(&mut self) -> bool {
        let a = self.whitespaces();
        let b = self.comma_token();

        a | b
    }

    /// A literal (case-sensitive sticky expression such as /matrix/y).
    fn literal(&mut self, text: &str) -> bool {
        if self.s[self.p..].starts_with(text) {
            self.p += text.len();
            true
        } else {
            false
        }
    }

    /// The `double` expression: /[+-]?(?:(?:\d*\.\d+)|(?:\d+(?:\.)?))(?:[eE][+-]?\d+)?/y
    fn scan_double(&self) -> Option<usize> {
        let b = self.bytes();
        let mut i = self.p;

        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }

        let digits_start = i;

        while i < b.len() && is_digit(b[i]) {
            i += 1;
        }

        let mut end;

        if i < b.len() && b[i] == b'.' && i + 1 < b.len() && is_digit(b[i + 1]) {
            end = i + 1;

            while end < b.len() && is_digit(b[end]) {
                end += 1;
            }
        } else if i > digits_start {
            end = if i < b.len() && b[i] == b'.' {
                i + 1
            } else {
                i
            };
        } else {
            return None;
        }

        if end < b.len() && (b[end] == b'e' || b[end] == b'E') {
            let mut k = end + 1;

            if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
                k += 1;
            }

            let digits = k;

            while k < b.len() && is_digit(b[k]) {
                k += 1;
            }

            if k > digits {
                end = k;
            }
        }

        Some(end)
    }

    /// `constants`: /([+-])((?:NAN|INF|INFINITY))/iy. The alternation takes INF before INFINITY.
    fn scan_constant(&self) -> Option<(usize, f64)> {
        let b = self.bytes();
        let i = self.p;

        if i >= b.len() || (b[i] != b'+' && b[i] != b'-') {
            return None;
        }

        let rest = &self.s[i + 1..];
        let upper: String = rest
            .chars()
            .take(3)
            .collect::<String>()
            .to_ascii_uppercase();

        let value = if upper == "NAN" {
            f64::NAN
        } else if upper == "INF" {
            f64::INFINITY
        } else {
            return None;
        };

        Some((i + 4, if b[i] == b'-' { -value } else { value }))
    }

    /// `this .double ()`
    fn double(&mut self) -> bool {
        self.whitespaces();

        if let Some(end) = self.scan_double() {
            self.r0 = self.s[self.p..end].to_string();
            self.p = end;
            self.value = parse_float_token(&self.r0);
            return true;
        }

        if let Some((end, value)) = self.scan_constant() {
            self.r0 = self.s[self.p..end].to_string();
            self.p = end;
            self.value = value;
            return true;
        }

        false
    }

    /// `this .int32 ()`: /(?:0[xX][\da-fA-F]+)|(?:[+-]?\d+)/y and parseInt.
    fn int32(&mut self) -> bool {
        self.whitespaces();

        let b = self.bytes();
        let mut i = self.p;

        if b.get(i) == Some(&b'0') && matches!(b.get(i + 1), Some(b'x') | Some(b'X')) {
            let mut k = i + 2;

            while k < b.len() && b[k].is_ascii_hexdigit() {
                k += 1;
            }

            if k > i + 2 {
                self.value = u64::from_str_radix(&self.s[i + 2..k], 16)
                    .map(|v| v as f64)
                    .unwrap_or(f64::INFINITY);
                self.r0 = self.s[i..k].to_string();
                self.p = k;
                return true;
            }
        }

        let start = i;

        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }

        let d = i;

        while i < b.len() && is_digit(b[i]) {
            i += 1;
        }

        if i == d {
            return false;
        }

        self.r0 = self.s[start..i].to_string();
        self.value = self
            .r0
            .trim_start_matches('+')
            .parse::<f64>()
            .unwrap_or(f64::NAN);
        self.p = i;
        true
    }

    /// `length`: /em|ex|px|in|cm|mm|pt|pc|%/y
    fn unit(&mut self) -> Option<&'static str> {
        for u in ["em", "ex", "px", "in", "cm", "mm", "pt", "pc", "%"] {
            if self.s[self.p..].starts_with(u) {
                self.p += u.len();
                return Some(u);
            }
        }

        None
    }

    /// `path`: /[mMlLhHvVqQtTcCsSaAzZ]/y
    fn path_command(&mut self) -> Option<char> {
        let c = *self.bytes().get(self.p)?;

        if b"mMlLhHvVqQtTcCsSaAzZ".contains(&c) {
            self.p += 1;
            Some(c as char)
        } else {
            None
        }
    }

    /// `color`: /#[\da-fA-F]+|[a-zA-Z]+\([^)]*\)|[a-zA-Z]+/y
    fn color_token(&mut self) -> Option<String> {
        let b = self.bytes();
        let i = self.p;

        if b.get(i) == Some(&b'#') {
            let mut k = i + 1;

            while k < b.len() && b[k].is_ascii_hexdigit() {
                k += 1;
            }

            if k > i + 1 {
                self.p = k;
                return Some(self.s[i..k].to_string());
            }
        }

        let mut k = i;

        while k < b.len() && b[k].is_ascii_alphabetic() {
            k += 1;
        }

        if k == i {
            return None;
        }

        if b.get(k) == Some(&b'(') {
            if let Some(close) = self.s[k + 1..].find(')') {
                let end = k + 1 + close + 1;

                self.p = end;
                return Some(self.s[i..end].to_string());
            }
        }

        self.p = k;
        Some(self.s[i..k].to_string())
    }

    /// `url`: /url\("?(.*?)"?\)/y. `.` does not match line terminators. Sets `r1`.
    fn url_value(&mut self) -> bool {
        if !self.s[self.p..].starts_with("url(") {
            return false;
        }

        let start = self.p + 4;
        let rest = &self.s[start..];
        let quoted = rest.starts_with('"');
        let attempts: &[usize] = if quoted { &[1, 0] } else { &[0] };

        for skip in attempts {
            let body = &self.s[start + skip..];
            let mut k = 0;

            // `.*?` grows until `"?\)` matches.
            loop {
                let tail = &body[k..];

                if tail.starts_with("\")") {
                    self.r1 = body[..k].to_string();
                    self.p = start + skip + k + 2;
                    return true;
                }

                if tail.starts_with(')') {
                    self.r1 = body[..k].to_string();
                    self.p = start + skip + k + 1;
                    return true;
                }

                let Some(c) = tail.chars().next() else { break };

                if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                    break;
                }

                k += c.len_utf8();
            }
        }

        false
    }
}

fn is_js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// Number to string and back, as `lengthAttribute (this .value, ...)` does with a number: the
/// text of the number goes through the `double` expression again.
fn length_of_number(value: f64, default: f64) -> f64 {
    // `if (!attribute) return defaultValue`
    if value == 0.0 || value.is_nan() {
        return default;
    }

    let mut lex = Lex::new();

    lex.s = jsmath::number_to_string(value);

    if lex.double() {
        value_with_unit(&mut lex, "", None)
    } else {
        default
    }
}

/// The unit conversion of `lengthAttribute` after the number has been read.
fn value_with_unit(lex: &mut Lex, percent: &str, view_box: Option<[f64; 4]>) -> f64 {
    let mut value = lex.value;

    if let Some(unit) = lex.unit() {
        match unit {
            "em" => value *= EM,
            "ex" => {} // TODO
            "px" => {} // We are pixels :)
            "in" => value *= INCH / PIXEL,
            "cm" => value *= CM / PIXEL,
            "mm" => value *= MM / PIXEL,
            "pt" => value *= POINT / PIXEL,
            "pc" => value *= PICA / PIXEL,
            _ => {
                let vb = view_box.unwrap_or([0.0, 0.0, 100.0, 100.0]);

                match percent {
                    "width" => value *= vb[2] / 100.0,
                    "height" => value *= vb[3] / 100.0,
                    _ => value *= jsmath::hypot(&[vb[2], vb[3]]) / 100.0,
                }
            }
        }
    }

    value
}

// --- numbers, boxes and curves -------------------------------------------------------------------

type P3 = [f64; 3];

/// A subpath: `points` with `closed`, `index` (offset into the Coordinate) set later.
struct Contour {
    pts: Vec<P3>,
    closed: bool,
    index: usize,
}

fn matrix3(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Matrix3 {
    Matrix3([a, b, 0.0, c, d, 0.0, e, f, 1.0])
}

fn skew_x(m: &mut Matrix3, angle: f64) {
    let a = jsmath::tan(angle);

    m.0[3] += m.0[0] * a;
    m.0[4] += m.0[1] * a;
    m.0[5] += m.0[2] * a;
}

fn skew_y(m: &mut Matrix3, angle: f64) {
    let a = jsmath::tan(angle);

    m.0[0] += m.0[3] * a;
    m.0[1] += m.0[4] * a;
    m.0[2] += m.0[5] * a;
}

fn matrix4_from_matrix3(m: &Matrix3) -> Matrix4 {
    let m = &m.0;

    Matrix4([
        m[0], m[1], 0.0, 0.0, m[3], m[4], 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, m[6], m[7], 0.0, 1.0,
    ])
}

/// standard/Math/Geometry/Box2.js (only what the parser uses).
#[derive(Clone)]
struct Box2 {
    matrix: Matrix3,
}

impl Box2 {
    fn empty() -> Box2 {
        Box2 {
            matrix: Matrix3([0.0; 9]),
        }
    }

    fn new(size: [f64; 2], center: [f64; 2]) -> Box2 {
        Box2 {
            matrix: Matrix3([
                size[0] / 2.0,
                0.0,
                0.0,
                0.0,
                size[1] / 2.0,
                0.0,
                center[0],
                center[1],
                1.0,
            ]),
        }
    }

    fn set_extents(&mut self, min: [f64; 2], max: [f64; 2]) {
        let sx = (max[0] - min[0]) / 2.0;
        let sy = (max[1] - min[1]) / 2.0;
        let cx = (max[0] + min[0]) / 2.0;
        let cy = (max[1] + min[1]) / 2.0;

        self.matrix = Matrix3([sx, 0.0, 0.0, 0.0, sy, 0.0, cx, cy, 1.0]);
    }

    fn is_empty(&self) -> bool {
        self.matrix.0[8] == 0.0
    }

    fn get_extents(&self) -> ([f64; 2], [f64; 2]) {
        let m = &self.matrix.0;
        let (x, y) = ([m[0], m[1]], [m[3], m[4]]);
        let p1 = [x[0] + y[0], x[1] + y[1]];
        let p2 = [y[0] - x[0], y[1] - x[1]];
        let n1 = [-p1[0], -p1[1]];
        let n2 = [-p2[0], -p2[1]];
        let mut min = [0.0; 2];
        let mut max = [0.0; 2];

        for i in 0..2 {
            min[i] = jsmath::js_min(jsmath::js_min(jsmath::js_min(p1[i], p2[i]), n1[i]), n2[i]);
            max[i] = jsmath::js_max(jsmath::js_max(jsmath::js_max(p1[i], p2[i]), n1[i]), n2[i]);
        }

        // getExtents: min .add (this .center)
        (
            ([min[0] + m[6], min[1] + m[7]]),
            ([max[0] + m[6], max[1] + m[7]]),
        )
    }

    fn add(&mut self, other: &Box2) {
        if self.is_empty() {
            self.matrix = other.matrix;
            return;
        }

        if other.is_empty() {
            return;
        }

        let (lmin, lmax) = self.get_extents();
        let (rmin, rmax) = other.get_extents();

        self.set_extents(
            [
                jsmath::js_min(lmin[0], rmin[0]),
                jsmath::js_min(lmin[1], rmin[1]),
            ],
            [
                jsmath::js_max(lmax[0], rmax[0]),
                jsmath::js_max(lmax[1], rmax[1]),
            ],
        );
    }

    fn from_points(points: &[P3]) -> Box2 {
        let mut b = Box2::empty();

        if points.is_empty() {
            return b;
        }

        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];

        for p in points {
            for i in 0..2 {
                min[i] = jsmath::js_min(min[i], p[i]);
                max[i] = jsmath::js_max(max[i], p[i]);
            }
        }

        b.set_extents(min, max);
        b
    }
}

/// Bezier.quadric
fn bezier_quadric(p0: P3, p1: P3, p2: P3, steps: usize, points: &mut Vec<P3>) {
    let mut p = Matrix3([
        p0[0], p0[1], p0[2], p1[0], p1[1], p1[2], p2[0], p2[1], p2[2],
    ]);

    p.mult_left(&Matrix3([1.0, 0.0, 0.0, -2.0, 2.0, 0.0, 1.0, -2.0, 1.0]));

    let d = steps as f64 - 1.0;

    for i in 0..steps {
        let t = i as f64 / d;
        let mut v = Vector3::new(1.0, t, t * t);

        p.mult_vec_matrix3(&mut v);
        points.push([v.x, v.y, v.z]);
    }
}

/// Bezier.cubic
fn bezier_cubic(p0: P3, p1: P3, p2: P3, p3: P3, steps: usize, points: &mut Vec<P3>) {
    let mut p = Matrix4([
        p0[0], p0[1], p0[2], 0.0, p1[0], p1[1], p1[2], 0.0, p2[0], p2[1], p2[2], 0.0, p3[0], p3[1],
        p3[2], 0.0,
    ]);

    p.mult_left(&Matrix4([
        1.0, 0.0, 0.0, 0.0, -3.0, 3.0, 0.0, 0.0, 3.0, -6.0, 3.0, 0.0, -1.0, 3.0, -3.0, 1.0,
    ]));

    let d = steps as f64 - 1.0;

    for i in 0..steps {
        let t = i as f64 / d;
        let mut v = Vector4::new(1.0, t, t * t, t * t * t);

        p.mult_vec_matrix4(&mut v);
        points.push([v.x, v.y, v.z]);
    }
}

/// Bezier.arc
#[allow(clippy::too_many_arguments)]
fn bezier_arc(
    ax: f64,
    ay: f64,
    rx: f64,
    ry: f64,
    x_axis_rotation: f64,
    large_arc: f64,
    sweep: f64,
    x: f64,
    y: f64,
    steps: usize,
    points: &mut Vec<P3>,
) {
    use std::f64::consts::PI;

    // If the endpoints are identical, then this is equivalent to omitting the elliptical arc segment entirely.
    if ax == x && ay == y {
        points.push([x, y, 0.0]);
        return;
    }

    let mut rx = rx.abs();
    let mut ry = ry.abs();

    // If rx = 0 or ry = 0 then this arc is treated as a straight line segment joining the endpoints.
    if rx == 0.0 || ry == 0.0 {
        points.push([ax, ay, 0.0]);
        points.push([x, y, 0.0]);
        return;
    }

    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let x_axis_rotation = jsmath::interval(x_axis_rotation, 0.0, 2.0 * PI);
    let sin_rotation = jsmath::sin(x_axis_rotation);
    let cos_rotation = jsmath::cos(x_axis_rotation);

    // Step #1: Compute transformedPoint
    let d = [(ax - x) / 2.0, (ay - y) / 2.0];
    let tp = [
        cos_rotation * d[0] + sin_rotation * d[1],
        -sin_rotation * d[0] + cos_rotation * d[1],
    ];
    let tp2 = [tp[0] * tp[0], tp[1] * tp[1]];

    // Ensure radii are large enough
    let radii_check = tp2[0] / rx2 + tp2[1] / ry2;

    if radii_check > 1.0 {
        rx = radii_check.sqrt() * rx;
        ry = radii_check.sqrt() * ry;
    }

    // Step #2: Compute transformedCenter
    let c_square_numerator = rx2 * ry2 - rx2 * tp2[1] - ry2 * tp2[0];
    let c_square_root_denom = rx2 * tp2[1] + ry2 * tp2[0];
    let mut c_radicand = c_square_numerator / c_square_root_denom;

    // Make sure this never drops below zero because of precision
    c_radicand = jsmath::js_max(0.0, c_radicand);

    let c_coef = (if large_arc != sweep { 1.0 } else { -1.0 }) * c_radicand.sqrt();
    let tc = [c_coef * rx * tp[1] / ry, -c_coef * ry * tp[0] / rx];

    // Step #3: Compute center
    let center = [
        cos_rotation * tc[0] - sin_rotation * tc[1] + ((ax + x) / 2.0),
        sin_rotation * tc[0] + cos_rotation * tc[1] + ((ay + y) / 2.0),
    ];

    // Step #4: Compute start/sweep angles
    let start_vector = [(tp[0] - tc[0]) / rx, (tp[1] - tc[1]) / ry];
    let end_vector = [(-tp[0] - tc[0]) / rx, (-tp[1] - tc[1]) / ry];
    let get_angle = |x: f64| if x > 0.0 { x } else { 2.0 * PI + x };
    let start_angle = get_angle(jsmath::atan2(start_vector[1], start_vector[0]));
    let end_angle = get_angle(jsmath::atan2(end_vector[1], end_vector[0]));
    let mut sweep_angle = end_angle - start_angle;

    if large_arc != 0.0 && !large_arc.is_nan() {
        // sweepAngle must be positive
        if sweep_angle < 0.0 {
            sweep_angle += 2.0 * PI;
        }
    } else if sweep_angle > 0.0 {
        // sweepAngle must be negative
        sweep_angle -= 2.0 * PI;
    }

    let sweep_flag = sweep != 0.0 && !sweep.is_nan();

    if sweep_flag && sweep_angle < 0.0 {
        sweep_angle += 2.0 * PI;
    } else if !sweep_flag && sweep_angle > 0.0 {
        sweep_angle -= 2.0 * PI;
    }

    // Interpolate:
    let bezier_steps = jsmath::js_max(4.0, sweep_angle.abs() * steps as f64 / (2.0 * PI));
    let bezier_steps_1 = bezier_steps - 1.0;

    points.push([ax, ay, 0.0]);

    let mut i = 1.0;

    while i < bezier_steps_1 {
        let t = i / bezier_steps_1;

        // From http://www.w3.org/TR/SVG/implnote.html#ArcParameterizationAlternatives
        let angle = start_angle + (sweep_angle * t);
        let px = rx * jsmath::cos(angle);
        let py = ry * jsmath::sin(angle);

        points.push([
            cos_rotation * px - sin_rotation * py + center[0],
            sin_rotation * px + cos_rotation * py + center[1],
            0.0,
        ]);
        i += 1.0;
    }

    points.push([x, y, 0.0]);
}

// --- styles --------------------------------------------------------------------------------------

const BLACK: [f64; 4] = [0.0, 0.0, 0.0, 1.0];

#[derive(Clone)]
struct Style {
    display: String,
    fill_type: String,
    fill_color: [f64; 4],
    fill_url: String,
    fill_opacity: f64,
    fill_rule: String,
    stroke_type: String,
    stroke_color: [f64; 4],
    stroke_url: String,
    stroke_opacity: f64,
    stroke_width: f64,
    opacity: f64,
    stop_color: [f64; 4],
    stop_opacity: f64,
    vector_effect: String,
}

impl Style {
    fn initial() -> Style {
        Style {
            display: "inline".into(),
            fill_type: "COLOR".into(),
            fill_color: BLACK,
            fill_url: String::new(),
            fill_opacity: 1.0,
            fill_rule: "nonzero".into(),
            stroke_type: "none".into(),
            stroke_color: BLACK,
            stroke_url: String::new(),
            stroke_opacity: 1.0,
            stroke_width: 1.0,
            opacity: 1.0,
            stop_color: BLACK,
            stop_opacity: 1.0,
            vector_effect: "none".into(),
        }
    }
}

/// `this .cssColor (c)` handed to the CSS parser as the wrapper color, and back through
/// `getComputedStyle`: each channel rounded to 8 bits.
fn wrapper_color(c: [f64; 4]) -> [f64; 4] {
    let q = |v: f64| (v * 255.0).clamp(0.0, 255.0).round() / 255.0;

    [q(c[0]), q(c[1]), q(c[2]), c[3]]
}

// --- gradients -----------------------------------------------------------------------------------

#[derive(Clone)]
struct Gradient {
    stops: Vec<(f64, [f64; 4], f64)>,
    x1: Option<f64>,
    y1: Option<f64>,
    x2: Option<f64>,
    y2: Option<f64>,
    cx: Option<f64>,
    cy: Option<f64>,
    r: Option<f64>,
    fx: Option<f64>,
    fy: Option<f64>,
    fr: Option<f64>,
    units: String,
    spread_method: Option<String>,
    transform: Matrix3,
    spread_matrix: Matrix3,
}

impl Gradient {
    fn new() -> Gradient {
        Gradient {
            stops: vec![],
            x1: None,
            y1: None,
            x2: None,
            y2: None,
            cx: None,
            cy: None,
            r: None,
            fx: None,
            fy: None,
            fr: None,
            units: String::new(),
            spread_method: None,
            transform: Matrix3::IDENTITY,
            spread_matrix: Matrix3::IDENTITY,
        }
    }
}

/// `value || default`
fn or(value: Option<f64>, default: f64) -> f64 {
    match value {
        Some(v) if v != 0.0 && !v.is_nan() => v,
        _ => default,
    }
}

// --- the parser ----------------------------------------------------------------------------------

struct Svg<'a> {
    scene: Scene,
    world_url: String,
    document: &'a Element,
    lex: Lex,
    styles: Vec<Style>,
    view_box: [f64; 4],
    model: Vec<Matrix4>,
    fill_geometries: HashMap<usize, NodeId>,
    stroke_geometries: HashMap<usize, NodeId>,
    line_properties: HashMap<u64, NodeId>,
    group_nodes: Vec<NodeId>,
    texture_properties: NodeId,
    num_switch_nodes: usize,
    bezier_steps: usize,
    circle_steps: usize,
    gradient_size: usize,
    depth: usize,
    /// The inline `color` of the parser's helper `div` (X3DParser.convertColor reuses one element
    /// for every call): a value the CSS parser rejects leaves the previous one in place.
    div_color: Option<DivColor>,
}

#[derive(Clone, Copy)]
enum DivColor {
    /// `currentcolor`: computes to the wrapper's color of the call.
    Current,
    Color([f64; 4]),
}

fn key(el: &Element) -> usize {
    el as *const Element as usize
}

/// A JS Map key for a number (SameValueZero).
fn number_key(v: f64) -> u64 {
    if v.is_nan() {
        f64::NAN.to_bits()
    } else if v == 0.0 {
        0
    } else {
        v.to_bits()
    }
}

fn percent_encode_fragment(s: &str) -> String {
    let mut out = String::new();

    for b in s.bytes() {
        if b <= 0x20 || b >= 0x7f || matches!(b, b'"' | b'<' | b'>' | b'`') {
            out.push_str(&format!("%{b:02X}"));
        } else {
            out.push(b as char);
        }
    }

    out
}

/// `new URL (attribute, base) .hash .slice (1)`.
fn url_hash(attribute: &str, base: &str) -> Result<String, String> {
    let trimmed = attribute.trim_matches(|c: char| c <= ' ');
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let has_scheme = {
        let mut chars = cleaned.chars();
        let first = chars.next().is_some_and(|c| c.is_ascii_alphabetic());

        first
            && cleaned.find(':').is_some_and(|i| {
                cleaned[..i]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            })
    };

    // A relative reference needs a base that can be one: a data: URL cannot.
    if !has_scheme && base.starts_with("data:") && !cleaned.starts_with('#') {
        return Err("Failed to construct 'URL': Invalid URL".into());
    }

    Ok(match cleaned.find('#') {
        Some(i) => percent_encode_fragment(&cleaned[i + 1..]),
        None => String::new(),
    })
}

fn find_by_id<'a>(el: &'a Element, id: &str) -> Option<&'a Element> {
    if el.attr("id") == Some(id) {
        return Some(el);
    }

    for c in &el.children {
        if let XNode::Element(e) = c {
            if let Some(found) = find_by_id(e, id) {
                return Some(found);
            }
        }
    }

    None
}

/// `xmlElement .getAttribute ("href") || xmlElement .getAttribute ("xlink:href")`
fn href_of(el: &Element) -> Option<String> {
    el.attr("href")
        .filter(|v| !v.is_empty())
        .or_else(|| el.attr("xlink:href"))
        .map(String::from)
}

impl<'a> Svg<'a> {
    fn style(&self) -> &Style {
        self.styles.last().unwrap()
    }

    fn style_mut(&mut self) -> &mut Style {
        self.styles.last_mut().unwrap()
    }

    // -- attributes ---------------------------------------------------------------------------

    /// `lengthAttribute (attribute, defaultValue, percent)`
    fn length_attribute(&mut self, attribute: Option<&str>, default: f64, percent: &str) -> f64 {
        let Some(attribute) = attribute.filter(|a| !a.is_empty()) else {
            return default;
        };

        self.lex.parse_value(Some(attribute));

        if self.lex.double() {
            let view_box = Some(self.view_box);

            return value_with_unit(&mut self.lex, percent, view_box);
        }

        default
    }

    /// `percentAttribute (attribute, defaultValue)`
    fn percent_attribute(&mut self, attribute: Option<&str>, default: f64) -> f64 {
        self.lex.parse_value(attribute);

        if self.lex.double() {
            let mut value = self.lex.value;

            if self.lex.literal("%") {
                value /= 100.0;
            }

            return jsmath::clamp(value, 0.0, 1.0);
        }

        default
    }

    /// `viewBoxAttribute`: a new box, or `None` for the default.
    fn view_box_attribute(&mut self, attribute: Option<&str>) -> Option<[f64; 4]> {
        let attribute = attribute.filter(|a| !a.is_empty())?;

        self.lex.parse_value(Some(attribute));

        let mut v = [0.0; 4];

        for item in &mut v {
            if !self.lex.double() {
                return None;
            }

            *item = self.lex.value;
        }

        Some(v)
    }

    /// `hrefAttribute`
    fn href_attribute(&self, attribute: Option<&str>) -> R<Option<&'a Element>> {
        let Some(attribute) = attribute.filter(|a| !a.is_empty()) else {
            return Ok(None);
        };

        let hash = match url_hash(attribute, &self.world_url) {
            Ok(h) => h,
            Err(e) => return error(e),
        };

        if hash.is_empty() {
            return Ok(None);
        }

        Ok(find_by_id(self.document, &hash))
    }

    /// `pointsAttribute`
    fn points_attribute(&mut self, attribute: Option<&str>, points: &mut Vec<P3>) -> bool {
        let Some(attribute) = attribute.filter(|a| !a.is_empty()) else {
            return false;
        };

        self.lex.parse_value(Some(attribute));

        loop {
            if self.lex.double() {
                let x = self.lex.value;

                if self.lex.comma() && self.lex.double() {
                    let y = self.lex.value;

                    points.push([x, y, 0.0]);

                    if self.lex.comma() {
                        continue;
                    }
                }
            }

            break;
        }

        !points.is_empty()
    }

    /// Reads `n` numbers separated by `comma ()`; `ints` marks the positions read with `int32`.
    fn numbers(&mut self, kinds: &[bool]) -> Option<Vec<f64>> {
        let mut out = Vec::with_capacity(kinds.len());

        for (i, int) in kinds.iter().enumerate() {
            if i > 0 {
                self.lex.comma();
            }

            let ok = if *int {
                self.lex.int32()
            } else {
                self.lex.double()
            };

            if !ok {
                return None;
            }

            out.push(self.lex.value);
        }

        Some(out)
    }

    /// `dAttribute`
    fn d_attribute(&mut self, attribute: Option<&str>, contours: &mut Vec<Contour>) -> bool {
        let Some(attribute) = attribute.filter(|a| !a.is_empty()) else {
            return false;
        };

        self.lex.parse_value(Some(attribute));

        let mut points: Vec<P3> = vec![];
        let mut command = '\0';
        let (mut ax, mut ay, mut px, mut py) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);

        loop {
            self.lex.whitespaces();

            let Some(c) = self.lex.path_command() else {
                break;
            };
            let previous = command;

            command = c;

            let relative = c.is_ascii_lowercase();

            match c.to_ascii_lowercase() {
                'm' => {
                    // moveto
                    if points.len() > 2 {
                        contours.push(Contour {
                            pts: std::mem::take(&mut points),
                            closed: false,
                            index: 0,
                        });
                    }

                    points = vec![];

                    while let Some(v) = self.numbers(&[false, false]) {
                        let (mut x, mut y) = (v[0], v[1]);

                        if relative {
                            x += ax;
                            y += ay;
                        }

                        points.push([x, y, 0.0]);
                        ax = x;
                        ay = y;
                        self.lex.comma();
                    }
                }
                'l' => {
                    // lineto
                    while let Some(v) = self.numbers(&[false, false]) {
                        let (mut x, mut y) = (v[0], v[1]);

                        if relative {
                            x += ax;
                            y += ay;
                        }

                        points.push([x, y, 0.0]);
                        ax = x;
                        ay = y;
                        self.lex.comma();
                    }
                }
                'h' => {
                    // horizontal lineto
                    while let Some(v) = self.numbers(&[false]) {
                        let mut x = v[0];

                        if relative {
                            x += ax;
                        }

                        points.push([x, ay, 0.0]);
                        ax = x;
                        self.lex.comma();
                    }
                }
                'v' => {
                    // vertical lineto
                    while let Some(v) = self.numbers(&[false]) {
                        let mut y = v[0];

                        if relative {
                            y += ay;
                        }

                        points.push([ax, y, 0.0]);
                        ay = y;
                        self.lex.comma();
                    }
                }
                'q' => {
                    // quadratic Bezier curveto
                    while let Some(v) = self.numbers(&[false; 4]) {
                        let (mut x1, mut y1, mut x, mut y) = (v[0], v[1], v[2], v[3]);

                        if relative {
                            x1 += ax;
                            y1 += ay;
                            x += ax;
                            y += ay;
                        }

                        bezier_quadric(
                            [ax, ay, 0.0],
                            [x1, y1, 0.0],
                            [x, y, 0.0],
                            self.bezier_steps,
                            &mut points,
                        );

                        ax = x;
                        ay = y;
                        px = x1;
                        py = y1;
                        self.lex.comma();
                    }
                }
                't' => {
                    // Shorthand/smooth quadratic Bezier curveto
                    while let Some(v) = self.numbers(&[false, false]) {
                        let (mut x, mut y) = (v[0], v[1]);

                        if relative {
                            x += ax;
                            y += ay;
                        }

                        let (x1, y1) = match previous {
                            'Q' | 'q' | 'T' | 't' => (ax + (ax - px), ay + (ay - py)),
                            _ => (ax, ay),
                        };

                        bezier_quadric(
                            [ax, ay, 0.0],
                            [x1, y1, 0.0],
                            [x, y, 0.0],
                            self.bezier_steps,
                            &mut points,
                        );

                        ax = x;
                        ay = y;
                        self.lex.comma();
                    }
                }
                'c' => {
                    // curveto, cubic Bezier curve
                    while let Some(v) = self.numbers(&[false; 6]) {
                        let (mut x1, mut y1, mut x2, mut y2, mut x, mut y) =
                            (v[0], v[1], v[2], v[3], v[4], v[5]);

                        if relative {
                            x1 += ax;
                            y1 += ay;
                            x2 += ax;
                            y2 += ay;
                            x += ax;
                            y += ay;
                        }

                        bezier_cubic(
                            [ax, ay, 0.0],
                            [x1, y1, 0.0],
                            [x2, y2, 0.0],
                            [x, y, 0.0],
                            self.bezier_steps,
                            &mut points,
                        );

                        ax = x;
                        ay = y;
                        px = x2;
                        py = y2;
                        self.lex.comma();
                    }
                }
                's' => {
                    // shorthand/smooth curveto, cubic Bezier curve
                    while let Some(v) = self.numbers(&[false; 4]) {
                        let (mut x2, mut y2, mut x, mut y) = (v[0], v[1], v[2], v[3]);

                        if relative {
                            x2 += ax;
                            y2 += ay;
                            x += ax;
                            y += ay;
                        }

                        let (x1, y1) = match previous {
                            'C' | 'c' | 'S' | 's' => (ax + (ax - px), ay + (ay - py)),
                            _ => (ax, ay),
                        };

                        bezier_cubic(
                            [ax, ay, 0.0],
                            [x1, y1, 0.0],
                            [x2, y2, 0.0],
                            [x, y, 0.0],
                            self.bezier_steps,
                            &mut points,
                        );

                        ax = x;
                        ay = y;
                        px = x2;
                        py = y2;
                        self.lex.comma();
                    }
                }
                'a' => {
                    // elliptical arc
                    while let Some(v) =
                        self.numbers(&[false, false, false, true, true, false, false])
                    {
                        let (rx, ry) = (v[0], v[1]);
                        let x_axis_rotation = v[2] * (std::f64::consts::PI / 180.0);
                        let (large_arc_flag, sweep_flag) = (v[3], v[4]);
                        let (mut x, mut y) = (v[5], v[6]);

                        if relative {
                            x += ax;
                            y += ay;
                        }

                        bezier_arc(
                            ax,
                            ay,
                            rx,
                            ry,
                            x_axis_rotation,
                            large_arc_flag,
                            sweep_flag,
                            x,
                            y,
                            self.circle_steps,
                            &mut points,
                        );

                        ax = x;
                        ay = y;
                        self.lex.comma();
                    }
                }
                _ => {
                    // closepath
                    if points.len() > 2 {
                        ax = points[0][0];
                        ay = points[0][1];

                        contours.push(Contour {
                            pts: std::mem::take(&mut points),
                            closed: true,
                            index: 0,
                        });
                    }

                    points = vec![];
                    self.lex.comma();
                }
            }
        }

        if points.len() > 2 {
            contours.push(Contour {
                pts: points,
                closed: false,
                index: 0,
            });
        }

        !contours.is_empty()
    }

    /// `transformAttribute`
    fn transform_attribute(&mut self, attribute: Option<&str>) -> Matrix3 {
        let mut matrix = Matrix3::IDENTITY;
        let Some(attribute) = attribute.filter(|a| !a.is_empty()) else {
            return matrix;
        };

        self.lex.parse_value(Some(attribute));

        loop {
            self.lex.comma();
            self.lex.whitespaces();

            if self.lex.literal("matrix") {
                self.lex.whitespaces();

                if self.lex.literal("(") {
                    // Six numbers, a comma (or white space) after all but the last.
                    let mut v = [0.0; 6];
                    let mut ok = true;

                    for (i, item) in v.iter_mut().enumerate() {
                        if i > 0 && !self.lex.comma() {
                            ok = false;
                            break;
                        }

                        if !self.lex.double() {
                            ok = false;
                            break;
                        }

                        *item = self.lex.value;
                    }

                    if ok {
                        self.lex.whitespaces();

                        if self.lex.literal(")") {
                            matrix.mult_left(&matrix3(v[0], v[1], v[2], v[3], v[4], v[5]));
                            continue;
                        }
                    }
                }
            } else if self.lex.literal("translate") {
                self.lex.whitespaces();

                if self.lex.literal("(") && self.lex.double() {
                    let tx = self.lex.value;
                    // `new Vector2 (tx, undefined)` gives y = x.
                    let ty: Option<f64>;

                    if self.lex.comma() {
                        ty = if self.lex.double() {
                            Some(self.lex.value)
                        } else {
                            None
                        };
                    } else {
                        ty = Some(0.0);
                    }

                    self.lex.whitespaces();

                    if self.lex.literal(")") {
                        matrix.translate(&Vector2::new(tx, ty.unwrap_or(tx)));
                        continue;
                    }
                }
            } else if self.lex.literal("rotate") {
                self.lex.whitespaces();

                if self.lex.literal("(") && self.lex.double() {
                    let angle = self.lex.value;

                    self.lex.whitespaces();

                    if self.lex.literal(")") {
                        matrix.rotate(angle * (std::f64::consts::PI / 180.0));
                        continue;
                    } else if self.lex.comma() && self.lex.double() {
                        let cx = self.lex.value;

                        if self.lex.comma() && self.lex.double() {
                            let cy = self.lex.value;

                            self.lex.whitespaces();

                            if self.lex.literal(")") {
                                matrix.translate(&Vector2::new(cx, cy));
                                matrix.rotate(angle * (std::f64::consts::PI / 180.0));
                                matrix.translate(&Vector2::new(-cx, -cy));
                                continue;
                            }
                        }
                    }
                }
            } else if self.lex.literal("scale") {
                self.lex.whitespaces();

                if self.lex.literal("(") && self.lex.double() {
                    let sx = self.lex.value;
                    let sy: Option<f64>;

                    if self.lex.comma() {
                        sy = if self.lex.double() {
                            Some(self.lex.value)
                        } else {
                            None
                        };
                    } else {
                        sy = Some(sx);
                    }

                    self.lex.whitespaces();

                    if self.lex.literal(")") {
                        matrix.scale(&Vector2::new(sx, sy.unwrap_or(sx)));
                        continue;
                    }
                }
            } else if self.lex.literal("skewX") {
                self.lex.whitespaces();

                if self.lex.literal("(") && self.lex.double() {
                    let angle = self.lex.value;

                    self.lex.whitespaces();

                    if self.lex.literal(")") {
                        skew_x(&mut matrix, angle * (std::f64::consts::PI / 180.0));
                        continue;
                    }
                }
            } else if self.lex.literal("skewY") {
                self.lex.whitespaces();

                if self.lex.literal("(") && self.lex.double() {
                    let angle = self.lex.value;

                    self.lex.whitespaces();

                    if self.lex.literal(")") {
                        skew_y(&mut matrix, angle * (std::f64::consts::PI / 180.0));
                        continue;
                    }
                }
            }

            break;
        }

        matrix
    }

    // -- styles -------------------------------------------------------------------------------

    /// `styleAttributes`: false when the parent style hides the element.
    fn style_attributes(&mut self, el: &Element) -> bool {
        let style = self.style().clone();

        if self.style().display == "none" {
            return false;
        }

        self.styles.push(style);

        for (name, value) in &el.attrs {
            self.parse_style(name, value);
        }

        // Style attribute has higher precedence.
        self.style_attribute(el.attr("style"));

        true
    }

    fn style_attribute(&mut self, attribute: Option<&str>) {
        let Some(attribute) = attribute.filter(|a| !a.is_empty()) else {
            return;
        };

        for value in attribute.split(';') {
            let pair: Vec<&str> = value.split(':').collect();

            if pair.len() != 2 {
                continue;
            }

            self.parse_style(
                pair[0].trim_matches(is_js_space),
                pair[1].trim_matches(is_js_space),
            );
        }
    }

    fn parse_style(&mut self, style: &str, value: &str) {
        if value == "inherit" || value == "unset" {
            return;
        }

        self.lex.parse_value(Some(value));

        match style {
            "display" => {
                self.style_mut().display = if value == "default" {
                    "inline".into()
                } else {
                    value.into()
                };
            }
            "fill" => self.fill_style(value),
            "fill-opacity" => {
                let initial = self.styles[0].fill_opacity;

                if value == "default" {
                    self.style_mut().fill_opacity = initial;
                } else if value == "transparent" {
                    self.style_mut().fill_opacity = 0.0;
                } else if self.lex.double() {
                    self.style_mut().fill_opacity = jsmath::clamp(self.lex.value, 0.0, 1.0);
                }
            }
            "fill-rule" => {
                let initial = self.styles[0].fill_rule.clone();

                self.style_mut().fill_rule = if value == "default" {
                    initial
                } else {
                    value.into()
                };
            }
            "stroke" => self.stroke_style(value),
            "stroke-opacity" => {
                let initial = self.styles[0].stroke_opacity;

                if value == "default" {
                    self.style_mut().stroke_opacity = initial;
                } else if value == "transparent" {
                    self.style_mut().stroke_opacity = 0.0;
                } else if self.lex.double() {
                    self.style_mut().stroke_opacity = jsmath::clamp(self.lex.value, 0.0, 1.0);
                }
            }
            "stroke-width" => {
                let initial = self.styles[0].stroke_width;

                if value == "default" {
                    self.style_mut().stroke_width = initial;
                } else if value == "none" {
                    self.style_mut().stroke_width = 0.0;
                } else if self.lex.double() {
                    // `this .lengthAttribute (this .value, 1)`
                    self.style_mut().stroke_width = length_of_number(self.lex.value, 1.0);
                }
            }
            "opacity" => {
                let initial = self.styles[0].opacity;

                if value == "default" {
                    self.style_mut().opacity = initial;
                } else if value == "transparent" {
                    self.style_mut().opacity = 0.0;
                } else if self.lex.double() {
                    let parent = self.style().opacity;

                    self.style_mut().opacity = jsmath::clamp(self.lex.value, 0.0, 1.0) * parent;
                }
            }
            "stop-color" => {
                let initial = self.styles[0].stop_color;

                if value == "default" {
                    self.style_mut().stop_color = initial;
                } else if let Some(c) = self.color_value(BLACK) {
                    self.style_mut().stop_color = c;
                }
            }
            "stop-opacity" => {
                let initial = self.styles[0].stop_opacity;

                if value == "default" {
                    self.style_mut().stop_opacity = initial;
                } else if value == "transparent" {
                    self.style_mut().stop_opacity = 0.0;
                } else if self.lex.double() {
                    self.style_mut().stop_opacity = jsmath::clamp(self.lex.value, 0.0, 1.0);
                }
            }
            "vector-effect" => {
                let initial = self.styles[0].vector_effect.clone();

                self.style_mut().vector_effect = if value == "default" {
                    initial
                } else {
                    value.into()
                };
            }
            _ => {}
        }
    }

    /// `colorValue (c)`: the token, converted by the CSS parser with `c` as the inherited color.
    fn color_value(&mut self, default: [f64; 4]) -> Option<[f64; 4]> {
        let token = self.lex.color_token()?;

        // `div .style .color = value` keeps the previous value when the CSS parser rejects it.
        let parsed = if token.eq_ignore_ascii_case("currentcolor") {
            Some(DivColor::Current)
        } else {
            try_convert_color(&token).map(DivColor::Color)
        };

        if parsed.is_some() {
            self.div_color = parsed;
        }

        // The computed color: the div's own, or the wrapper's (`defaultColor`) when it has none
        // or `currentcolor`.
        Some(match self.div_color {
            Some(DivColor::Color(c)) => c,
            _ => wrapper_color(default),
        })
    }

    fn fill_style(&mut self, value: &str) {
        if value == "default" {
            let initial = self.styles[0].clone();
            let s = self.style_mut();

            s.fill_type = initial.fill_type;
            s.fill_color = initial.fill_color;
            s.fill_url = initial.fill_url;
            return;
        }

        if value == "transparent" || value == "none" {
            self.style_mut().fill_type = "none".into();
            return;
        }

        if self.lex.url_value() {
            let url = self.lex.r1.trim_matches(is_js_space).to_string();
            let s = self.style_mut();

            s.fill_type = "URL".into();
            s.fill_url = url;
            return;
        }

        let current = self.style().fill_color;

        if let Some(c) = self.color_value(current) {
            let s = self.style_mut();

            s.fill_type = "COLOR".into();
            s.fill_color = c;
        }
    }

    fn stroke_style(&mut self, value: &str) {
        if value == "default" {
            let initial = self.styles[0].clone();
            let s = self.style_mut();

            s.stroke_type = initial.stroke_type;
            s.stroke_color = initial.stroke_color;
            s.stroke_url = initial.stroke_url;
            return;
        }

        if value == "transparent" || value == "none" {
            self.style_mut().stroke_type = "none".into();
            return;
        }

        if self.lex.url_value() {
            let url = self.lex.r1.trim_matches(is_js_space).to_string();
            let s = self.style_mut();

            s.stroke_type = "URL".into();
            s.stroke_url = url;
            return;
        }

        let current = self.style().stroke_color;

        if let Some(c) = self.color_value(current) {
            let s = self.style_mut();

            s.stroke_type = "COLOR".into();
            s.stroke_color = c;
        }
    }

    // -- scene helpers ------------------------------------------------------------------------

    fn node(&mut self, type_name: &str) -> R<NodeId> {
        let node = create(&mut self.scene, type_name)?;

        // `createNode (name)` sets the node up as it is made: the runtime does so in this order
        // (an ImageTexture asks for its url then).
        self.scene.setup_order.push(node);
        Ok(node)
    }

    /// `idAttribute (attribute, suffix, node, { named, exported })`
    fn id_attribute(
        &mut self,
        attribute: Option<&str>,
        suffix: &str,
        node: NodeId,
        named: bool,
        exported: bool,
    ) {
        let Some(attribute) = attribute.filter(|a| !a.is_empty()) else {
            return;
        };
        let name = crate::formats::sanitize_name(attribute);

        if name.is_empty() {
            return;
        }

        if named {
            let unique = self.scene.get_unique_name(ROOT, &format!("{name}{suffix}"));

            self.scene.update_named_node(ROOT, &unique, node);
        }

        if exported {
            let unique = self.scene.get_unique_export_name(&name);

            let _ = self.scene.update_exported_node(ROOT, &unique, node, "");
        }
    }

    /// `createTransform (xmlElement, t, s)`: pushes the model matrix and adds the node to the
    /// current group.
    fn create_transform(&mut self, el: &Element, t: [f64; 2], s: [f64; 2]) -> R<NodeId> {
        let mut m = self.transform_attribute(el.attr("transform"));

        // modelMatrix .push (); modelMatrix .multLeft (Matrix4 .fromMatrix3 (m))
        let mut top = *self.model.last().unwrap();

        top.mult_left(&matrix4_from_matrix3(&m));
        self.model.push(top);

        m.translate(&Vector2::new(t[0], t[1]));
        m.scale(&Vector2::new(s[0], s[1]));

        let transform = self.node("Transform")?;
        let matrix = matrix4_from_matrix3(&m);
        let mut translation = Vector3::new(0.0, 0.0, 0.0);
        let mut rotation = Rotation4::identity();
        let mut scale = Vector3::new(1.0, 1.0, 1.0);
        let mut scale_orientation = Rotation4::identity();

        matrix.get_transform(
            Some(&mut translation),
            Some(&mut rotation),
            Some(&mut scale),
            Some(&mut scale_orientation),
            None,
        );

        set(
            &mut self.scene,
            transform,
            "translation",
            FieldValue::SFVec3f(translation),
        );
        set(
            &mut self.scene,
            transform,
            "rotation",
            FieldValue::SFRotation(rotation),
        );
        set(
            &mut self.scene,
            transform,
            "scale",
            FieldValue::SFVec3f(scale),
        );
        set(
            &mut self.scene,
            transform,
            "scaleOrientation",
            FieldValue::SFRotation(scale_orientation),
        );

        self.id_attribute(el.attr("id"), "", transform, true, true);

        let group = *self.group_nodes.last().unwrap();

        push_child(&mut self.scene, group, "children", transform);
        Ok(transform)
    }

    fn pop_all(&mut self) {
        self.group_nodes.pop();
        self.model.pop();
        self.styles.pop();
    }

    fn create_fill_appearance(&mut self, bbox: &Box2) -> R<Option<NodeId>> {
        let appearance = self.node("Appearance")?;

        match self.style().fill_type.as_str() {
            "none" => return Ok(None),
            "COLOR" => {
                let material = self.node("UnlitMaterial")?;
                let (c, fill_opacity, opacity) = (
                    self.style().fill_color,
                    self.style().fill_opacity,
                    self.style().opacity,
                );

                set_node(&mut self.scene, appearance, "material", Some(material));
                set(
                    &mut self.scene,
                    material,
                    "emissiveColor",
                    FieldValue::SFColor(Color3::new(c[0], c[1], c[2])),
                );
                set(
                    &mut self.scene,
                    material,
                    "transparency",
                    FieldValue::SFFloat(1.0 - fill_opacity * opacity),
                );
            }
            "URL" => {
                // Gradient
                let texture = self.node("ImageTexture")?;
                let fill_url = self.style().fill_url.clone();
                let url = self.get_fill_url(&fill_url, bbox)?;

                // Get image from url.
                let Some(url) = url else { return Ok(None) };

                set(
                    &mut self.scene,
                    texture,
                    "url",
                    FieldValue::MFString(vec![url]),
                );
                set_node(
                    &mut self.scene,
                    texture,
                    "textureProperties",
                    Some(self.texture_properties),
                );
                set_node(&mut self.scene, appearance, "texture", Some(texture));
            }
            _ => {}
        }

        Ok(Some(appearance))
    }

    fn get_fill_url(&mut self, fill_url: &str, bbox: &Box2) -> R<Option<String>> {
        let Some(el) = self.href_attribute(Some(fill_url))? else {
            return Ok(None);
        };

        match el.name.as_str() {
            "linearGradient" => {
                let mut g = Gradient::new();

                self.linear_gradient_element(el, bbox, &mut g)?;

                let kind = GradientKind::Linear {
                    x1: g.x1.unwrap_or(f64::NAN),
                    y1: g.y1.unwrap_or(f64::NAN),
                    x2: g.x2.unwrap_or(f64::NAN),
                    y2: g.y2.unwrap_or(f64::NAN),
                };

                self.draw_gradient(kind, &g, bbox)
            }
            "radialGradient" => {
                let mut g = Gradient::new();

                self.radial_gradient_element(el, bbox, &mut g)?;

                let kind = GradientKind::Radial {
                    fx: g.fx.unwrap_or(f64::NAN),
                    fy: g.fy.unwrap_or(f64::NAN),
                    fr: g.fr.unwrap_or(f64::NAN),
                    cx: g.cx.unwrap_or(f64::NAN),
                    cy: g.cy.unwrap_or(f64::NAN),
                    r: g.r.unwrap_or(f64::NAN),
                };

                self.draw_gradient(kind, &g, bbox)
            }
            // patternUrl () is empty.
            _ => Ok(None),
        }
    }

    fn create_stroke_appearance(&mut self) -> R<NodeId> {
        let appearance = self.node("Appearance")?;
        let material = self.node("UnlitMaterial")?;
        let (c, stroke_opacity, opacity) = (
            self.style().stroke_color,
            self.style().stroke_opacity,
            self.style().opacity,
        );

        set_node(&mut self.scene, appearance, "material", Some(material));
        set(
            &mut self.scene,
            material,
            "emissiveColor",
            FieldValue::SFColor(Color3::new(c[0], c[1], c[2])),
        );
        set(
            &mut self.scene,
            material,
            "transparency",
            FieldValue::SFFloat(1.0 - stroke_opacity * opacity),
        );

        // `this .vectorEffect` is never set (the value lives in `this .style`), so this is always
        // getStokeWidth ().
        let stroke_width = self.get_stoke_width();

        if stroke_width > 1.0 {
            let properties = self.get_line_properties(stroke_width)?;

            set_node(
                &mut self.scene,
                appearance,
                "lineProperties",
                Some(properties),
            );
        }

        Ok(appearance)
    }

    fn get_stoke_width(&self) -> f64 {
        let w = self.style().stroke_width;
        let mut v = Vector3::new(w, w, 0.0);

        self.model.last().unwrap().mult_dir_matrix(&mut v);

        (v.x + v.y) / 2.0
    }

    fn get_line_properties(&mut self, stroke_width: f64) -> R<NodeId> {
        if let Some(n) = self.line_properties.get(&number_key(stroke_width)) {
            return Ok(*n);
        }

        let node = self.node("LineProperties")?;

        set(
            &mut self.scene,
            node,
            "linewidthScaleFactor",
            FieldValue::SFFloat(stroke_width),
        );
        self.line_properties.insert(number_key(stroke_width), node);
        Ok(node)
    }

    fn create_texture_properties(&mut self) -> R<NodeId> {
        let node = self.node("TextureProperties")?;

        set(
            &mut self.scene,
            node,
            "generateMipMaps",
            FieldValue::SFBool(true),
        );
        set(
            &mut self.scene,
            node,
            "minificationFilter",
            FieldValue::SFString("NICEST".into()),
        );
        set(
            &mut self.scene,
            node,
            "magnificationFilter",
            FieldValue::SFString("NICEST".into()),
        );
        set(
            &mut self.scene,
            node,
            "boundaryModeS",
            FieldValue::SFString("CLAMP_TO_EDGE".into()),
        );
        set(
            &mut self.scene,
            node,
            "boundaryModeT",
            FieldValue::SFString("CLAMP_TO_EDGE".into()),
        );
        set(
            &mut self.scene,
            node,
            "boundaryModeR",
            FieldValue::SFString("CLAMP_TO_EDGE".into()),
        );
        set(
            &mut self.scene,
            node,
            "textureCompression",
            FieldValue::SFString("DEFAULT".into()),
        );
        Ok(node)
    }

    // -- gradients ----------------------------------------------------------------------------

    fn linear_gradient_element(&mut self, el: &'a Element, bbox: &Box2, g: &mut Gradient) -> R<()> {
        if el.name != "linearGradient" {
            return Ok(());
        }

        // Attribute xlink:href
        let reference = href_of(el);
        let ref_element = self.href_attribute(reference.as_deref())?;

        if let Some(r) = ref_element {
            self.gradient_element(r, bbox, g)?;
        }

        // Attributes
        g.x1 = Some(self.length_attribute(el.attr("x1"), or(g.x1, 0.0), "width"));
        g.y1 = Some(self.length_attribute(el.attr("y1"), or(g.y1, 0.0), "height"));
        g.x2 = Some(self.length_attribute(el.attr("x2"), or(g.x2, 1.0), "width"));
        g.y2 = Some(self.length_attribute(el.attr("y2"), or(g.y2, 0.0), "height"));
        g.units = el
            .attr("gradientUnits")
            .filter(|u| !u.is_empty())
            .unwrap_or("objectBoundingBox")
            .to_string();
        g.transform = self.transform_attribute(el.attr("gradientTransform"));

        // Spread matrix
        let c = Vector2::new(g.x1.unwrap(), g.y1.unwrap());
        let mut s = Matrix3::IDENTITY;

        s.translate(&c);
        s.scale(&Vector2::new(SPREAD, SPREAD));
        s.translate(&Vector2::new(-c.x, -c.y));

        g.spread_matrix = s;

        // Stops
        for child in &el.children {
            if let XNode::Element(e) = child {
                self.gradient_child(e, g)?;
            }
        }

        Ok(())
    }

    fn radial_gradient_element(&mut self, el: &'a Element, bbox: &Box2, g: &mut Gradient) -> R<()> {
        // Attribute xlink:href
        let reference = href_of(el);
        let ref_element = self.href_attribute(reference.as_deref())?;

        if let Some(r) = ref_element {
            self.gradient_element(r, bbox, g)?;
        }

        // Attributes
        g.cx = Some(self.length_attribute(el.attr("cx"), or(g.cx, 0.5), "width"));
        g.cy = Some(self.length_attribute(el.attr("cy"), or(g.cy, 0.5), "height"));
        g.r = Some(self.length_attribute(el.attr("r"), or(g.r, 0.5), ""));
        g.fx = Some(self.length_attribute(el.attr("fx"), or(g.fx, g.cx.unwrap()), "width"));
        g.fy = Some(self.length_attribute(el.attr("fy"), or(g.fy, g.cy.unwrap()), "height"));
        g.fr = Some(self.length_attribute(el.attr("fr"), or(g.fr, 0.0), ""));
        g.units = el
            .attr("gradientUnits")
            .filter(|u| !u.is_empty())
            .unwrap_or("objectBoundingBox")
            .to_string();
        g.spread_method = el.attr("spreadMethod").map(String::from);
        g.transform = self.transform_attribute(el.attr("gradientTransform"));

        // Spread matrix
        let c = Vector2::new(g.fx.unwrap(), g.fy.unwrap());
        let mut s = Matrix3::IDENTITY;

        s.translate(&c);
        // `new Vector2 (SPREAD)` is (SPREAD, SPREAD).
        s.scale(&Vector2::new(SPREAD, SPREAD));
        s.translate(&Vector2::new(-c.x, -c.y));

        g.spread_matrix = s;

        // Stops
        for child in &el.children {
            if let XNode::Element(e) = child {
                self.gradient_child(e, g)?;
            }
        }

        Ok(())
    }

    fn gradient_element(&mut self, el: &'a Element, bbox: &Box2, g: &mut Gradient) -> R<()> {
        match el.name.as_str() {
            "linearGradient" => self.linear_gradient_element(el, bbox, g),
            "radialGradient" => self.radial_gradient_element(el, bbox, g),
            _ => Ok(()),
        }
    }

    fn gradient_child(&mut self, el: &'a Element, g: &mut Gradient) -> R<()> {
        if el.name == "stop" {
            self.stop_element(el, g);
        }

        Ok(())
    }

    fn stop_element(&mut self, el: &Element, g: &mut Gradient) {
        if !self.style_attributes(el) {
            return;
        }

        let offset = self.percent_attribute(el.attr("offset"), 0.0);

        // A stop outside 0..1 returns without popping its style.
        if !(0.0..=1.0).contains(&offset) {
            return;
        }

        let (stop_color, stop_opacity) = (self.style().stop_color, self.style().stop_opacity);

        g.stops.push((offset, stop_color, stop_opacity));
        self.styles.pop();
    }

    /// `drawGradient (gradient, g, bbox)`: the stops as the canvas gets them, the matrix, the PNG.
    fn draw_gradient(
        &mut self,
        kind: GradientKind,
        g: &Gradient,
        bbox: &Box2,
    ) -> R<Option<String>> {
        let mut stops: Vec<(f64, [f64; 4])> = vec![];
        let css =
            |c: &[f64; 4], a: f64| -> [f64; 4] { [c[0] * 255.0, c[1] * 255.0, c[2] * 255.0, a] };
        let mut spread_matrix = g.spread_matrix;

        match g.spread_method.as_deref() {
            Some("repeat") => {
                for i in 0..SPREAD as usize {
                    let s = i as f64 / SPREAD;

                    for (o, c, a) in &g.stops {
                        stops.push((s + o / SPREAD, css(c, *a)));
                    }
                }
            }
            Some("reflect") => {
                for i in 0..SPREAD as usize {
                    let s = i as f64 / SPREAD;

                    for (o, c, a) in &g.stops {
                        stops.push((
                            s + (if i % 2 == 1 { 1.0 - o } else { *o }) / SPREAD,
                            css(c, *a),
                        ));
                    }
                }
            }
            _ => {
                // pad
                spread_matrix = Matrix3::IDENTITY;

                for (o, c, a) in &g.stops {
                    stops.push((*o, css(c, *a)));
                }
            }
        }

        // Create Matrix.
        let size = self.gradient_size as f64;
        let mut m = Matrix3::IDENTITY;

        m.scale(&Vector2::new(size / 2.0, size / 2.0));
        m.translate(&Vector2::new(1.0, 1.0));
        m.scale(&Vector2::new(1.0, -1.0));

        if g.units == "userSpaceOnUse" {
            let mut inverse = bbox.matrix;

            inverse.inverse();
            m.mult_left(&inverse);
        } else {
            m.mult_left(&Matrix3([2.0, 0.0, 0.0, 0.0, 2.0, 0.0, -1.0, -1.0, 1.0]));
        }

        m.mult_left(&g.transform);
        m.mult_left(&spread_matrix);

        // Paint.
        match svg_canvas::paint(
            self.gradient_size,
            &kind,
            &stops,
            [m.0[0], m.0[1], m.0[3], m.0[4], m.0[6], m.0[7]],
        ) {
            Ok(rgba) => Ok(Some(svg_canvas::png_data_url(self.gradient_size, &rgba))),
            Err(message) => error(message),
        }
    }

    // -- elements -----------------------------------------------------------------------------

    fn elements(&mut self, el: &'a Element) -> R<()> {
        for child in &el.children {
            if let XNode::Element(e) = child {
                self.element(e)?;
            }
        }

        Ok(())
    }

    fn element(&mut self, el: &'a Element) -> R<()> {
        if self.depth >= MAX_DEPTH {
            return error("Maximum call stack size exceeded");
        }

        self.depth += 1;

        let result = match el.name.as_str() {
            "use" => self.use_element(el),
            "g" => self.g_element(el),
            "switch" => self.switch_element(el),
            "a" => self.a_element(el),
            "rect" => self.rect_element(el),
            "circle" => self.circle_element(el),
            "ellipse" => self.ellipse_element(el),
            "text" => Ok(()), // textElement () is empty.
            "image" => self.image_element(el),
            "polyline" => self.polyline_element(el),
            "polygon" => self.polygon_element(el),
            "path" => self.path_element(el),
            _ => Ok(()),
        };

        self.depth -= 1;
        result
    }

    fn use_element(&mut self, el: &'a Element) -> R<()> {
        // Get href.
        let reference = href_of(el);
        let Some(used) = self.href_attribute(reference.as_deref())? else {
            return Ok(());
        };

        // Determine style.
        if !self.style_attributes(el) {
            return Ok(());
        }

        // Create Transform node.
        let x = self.length_attribute(el.attr("x"), 0.0, "width");
        let y = self.length_attribute(el.attr("y"), 0.0, "height");
        let transform = self.create_transform(el, [x, y], [1.0, 1.0])?;

        self.group_nodes.push(transform);

        let result = self.element(used);

        self.pop_all();
        result
    }

    fn g_element(&mut self, el: &'a Element) -> R<()> {
        // Determine style.
        if !self.style_attributes(el) {
            return Ok(());
        }

        // Create Transform node.
        let transform = self.create_transform(el, [0.0, 0.0], [1.0, 1.0])?;

        // Get child elements.
        self.group_nodes.push(transform);

        let result = self.elements(el);

        self.pop_all();
        result
    }

    fn switch_element(&mut self, el: &'a Element) -> R<()> {
        // Determine style.
        if !self.style_attributes(el) {
            return Ok(());
        }

        // Create Transform node.
        let transform = self.create_transform(el, [0.0, 0.0], [1.0, 1.0])?;
        let switch = self.node("Switch")?;

        push_child(&mut self.scene, transform, "children", switch);
        set(
            &mut self.scene,
            switch,
            "whichChoice",
            FieldValue::SFInt32(0),
        );

        self.num_switch_nodes += 1;

        let unique = self
            .scene
            .get_unique_export_name(&format!("Switch{}", self.num_switch_nodes));

        let _ = self.scene.update_exported_node(ROOT, &unique, switch, "");

        // Get child elements.
        self.group_nodes.push(switch);

        let result = self.elements(el);

        self.pop_all();
        result
    }

    fn a_element(&mut self, el: &'a Element) -> R<()> {
        // Determine style.
        if !self.style_attributes(el) {
            return Ok(());
        }

        // Get attributes.
        let href = href_of(el);
        let title = el
            .attr("title")
            .filter(|t| !t.is_empty())
            .or_else(|| el.attr("xlink:title"));
        let target = el.attr("target");

        // Create Transform node.
        let transform = self.create_transform(el, [0.0, 0.0], [1.0, 1.0])?;
        let anchor = self.node("Anchor")?;

        push_child(&mut self.scene, transform, "children", anchor);

        // `anchorNode .description = title` with title `null` is the text "null".
        set(
            &mut self.scene,
            anchor,
            "description",
            FieldValue::SFString(title.unwrap_or("null").to_string()),
        );
        set(
            &mut self.scene,
            anchor,
            "url",
            FieldValue::MFString(vec![href.unwrap_or_else(|| "null".into())]),
        );

        if let Some(target) = target.filter(|t| !t.is_empty()) {
            set(
                &mut self.scene,
                anchor,
                "parameter",
                FieldValue::MFString(vec![format!("target={target}")]),
            );
        }

        // Get child elements.
        self.group_nodes.push(anchor);

        let result = self.elements(el);

        self.pop_all();
        result
    }

    fn rect_element(&mut self, el: &'a Element) -> R<()> {
        // Create Transform node.
        let x = self.length_attribute(el.attr("x"), 0.0, "width");
        let y = self.length_attribute(el.attr("y"), 0.0, "height");
        let width = self.length_attribute(el.attr("width"), 0.0, "width");
        let height = self.length_attribute(el.attr("height"), 0.0, "height");
        let mut rx = jsmath::js_max(0.0, self.length_attribute(el.attr("rx"), 0.0, "width"));
        let mut ry = jsmath::js_max(0.0, self.length_attribute(el.attr("ry"), 0.0, "height"));

        if rx == 0.0 && ry == 0.0 {
            // Determine style.
            if !self.style_attributes(el) {
                return Ok(());
            }

            // Create Transform node.
            let size = [width, height];
            let center = [x + width / 2.0, y + height / 2.0];
            let bbox = Box2::new(size, center);
            let transform = self.create_transform(el, center, [1.0, 1.0])?;

            self.group_nodes.push(transform);

            // Create nodes.
            let result = self.rect_shapes(el, transform, &bbox, size);

            self.pop_all();
            return result;
        }

        // Create points.
        if rx != 0.0 && ry == 0.0 {
            ry = rx;
        }

        if ry != 0.0 && rx == 0.0 {
            rx = ry;
        }

        rx = jsmath::js_min(rx, width / 2.0);
        ry = jsmath::js_min(ry, height / 2.0);

        let x_offsets = [x + width - rx, x + rx, x + rx, x + width - rx];
        let y_offsets = [y + height - ry, y + height - ry, y + ry, y + ry];
        let mut points: Vec<P3> = vec![];
        let n = (self.circle_steps as f64 / 4.0) as usize;

        for c in 0..4 {
            let s = c as f64 * std::f64::consts::PI / 2.0;
            let mut i = 0;

            while i < n {
                let angle = s + std::f64::consts::PI / 2.0 * i as f64 / (n as f64 - 1.0);
                let (real, imag) = (1.0 * jsmath::cos(angle), 1.0 * jsmath::sin(angle));

                points.push([x_offsets[c] + real * rx, y_offsets[c] + imag * ry, 0.0]);
                i += 1;
            }
        }

        points.pop();

        // Create nodes.
        self.path_like_element(
            el,
            vec![Contour {
                pts: points,
                closed: true,
                index: 0,
            }],
        )
    }

    fn rect_shapes(
        &mut self,
        el: &Element,
        transform: NodeId,
        bbox: &Box2,
        size: [f64; 2],
    ) -> R<()> {
        if self.style().fill_type != "none" {
            let shape = self.node("Shape")?;
            let rectangle = self.fill_geometries.get(&key(el)).copied();

            push_child(&mut self.scene, transform, "children", shape);

            let appearance = self.create_fill_appearance(bbox)?;

            set_node(&mut self.scene, shape, "appearance", appearance);

            if let Some(rectangle) = rectangle {
                set_node(&mut self.scene, shape, "geometry", Some(rectangle));
            } else {
                let rectangle = self.node("Rectangle2D")?;

                self.fill_geometries.insert(key(el), rectangle);

                set_node(&mut self.scene, shape, "geometry", Some(rectangle));
                set(
                    &mut self.scene,
                    rectangle,
                    "solid",
                    FieldValue::SFBool(false),
                );
                set(
                    &mut self.scene,
                    rectangle,
                    "size",
                    FieldValue::SFVec2f(Vector2::new(size[0], size[1])),
                );
            }
        }

        if self.style().stroke_type != "none" {
            let shape = self.node("Shape")?;
            let polyline = self.stroke_geometries.get(&key(el)).copied();

            push_child(&mut self.scene, transform, "children", shape);

            let appearance = self.create_stroke_appearance()?;

            set_node(&mut self.scene, shape, "appearance", Some(appearance));

            if let Some(polyline) = polyline {
                set_node(&mut self.scene, shape, "geometry", Some(polyline));
            } else {
                let polyline = self.node("Polyline2D")?;
                let (w, h) = (size[0] / 2.0, size[1] / 2.0);

                self.stroke_geometries.insert(key(el), polyline);

                set_node(&mut self.scene, shape, "geometry", Some(polyline));
                set_numbers(
                    &mut self.scene,
                    polyline,
                    "lineSegments",
                    &[w, h, -w, h, -w, -h, w, -h, w, h],
                );
            }
        }

        Ok(())
    }

    fn circle_element(&mut self, el: &'a Element) -> R<()> {
        // Determine style.
        if !self.style_attributes(el) {
            return Ok(());
        }

        // Create Transform node.
        let cx = self.length_attribute(el.attr("cx"), 0.0, "width");
        let cy = self.length_attribute(el.attr("cy"), 0.0, "height");
        let r = self.length_attribute(el.attr("r"), 0.0, "");
        let bbox = Box2::new([r * 2.0, r * 2.0], [cx, cy]);
        let transform = self.create_transform(el, [cx, cy], [1.0, 1.0])?;

        self.group_nodes.push(transform);

        let result = self.round_shapes(el, transform, &bbox, "Circle2D", "radius", r, r);

        self.pop_all();
        result
    }

    fn ellipse_element(&mut self, el: &'a Element) -> R<()> {
        // Determine style.
        if !self.style_attributes(el) {
            return Ok(());
        }

        // Create Transform node.
        let cx = self.length_attribute(el.attr("cx"), 0.0, "width");
        let cy = self.length_attribute(el.attr("cy"), 0.0, "height");
        let rx = self.length_attribute(el.attr("rx"), 0.0, "width");
        let ry = self.length_attribute(el.attr("ry"), 0.0, "height");
        let r_min = jsmath::js_min(rx, ry);
        let bbox = Box2::new([rx * 2.0, ry * 2.0], [cx, cy]);
        let transform = self.create_transform(el, [cx, cy], [rx / r_min, ry / r_min])?;

        self.group_nodes.push(transform);

        let result = self.round_shapes(el, transform, &bbox, "Circle2D", "radius", r_min, r_min);

        self.pop_all();
        result
    }

    /// The Disk2D fill and Circle2D stroke of `circle` and `ellipse`.
    #[allow(clippy::too_many_arguments)]
    fn round_shapes(
        &mut self,
        el: &Element,
        transform: NodeId,
        bbox: &Box2,
        stroke_type: &str,
        stroke_field: &str,
        outer_radius: f64,
        radius: f64,
    ) -> R<()> {
        if self.style().fill_type != "none" {
            let shape = self.node("Shape")?;
            let disk = self.fill_geometries.get(&key(el)).copied();

            push_child(&mut self.scene, transform, "children", shape);

            let appearance = self.create_fill_appearance(bbox)?;

            set_node(&mut self.scene, shape, "appearance", appearance);

            if let Some(disk) = disk {
                set_node(&mut self.scene, shape, "geometry", Some(disk));
            } else {
                let disk = self.node("Disk2D")?;

                self.fill_geometries.insert(key(el), disk);

                set_node(&mut self.scene, shape, "geometry", Some(disk));
                set(&mut self.scene, disk, "solid", FieldValue::SFBool(false));
                set(
                    &mut self.scene,
                    disk,
                    "outerRadius",
                    FieldValue::SFFloat(outer_radius),
                );
            }
        }

        if self.style().stroke_type != "none" {
            let shape = self.node("Shape")?;
            let circle = self.stroke_geometries.get(&key(el)).copied();

            push_child(&mut self.scene, transform, "children", shape);

            let appearance = self.create_stroke_appearance()?;

            set_node(&mut self.scene, shape, "appearance", Some(appearance));

            if let Some(circle) = circle {
                set_node(&mut self.scene, shape, "geometry", Some(circle));
            } else {
                let circle = self.node(stroke_type)?;

                self.stroke_geometries.insert(key(el), circle);

                set_node(&mut self.scene, shape, "geometry", Some(circle));
                set(
                    &mut self.scene,
                    circle,
                    stroke_field,
                    FieldValue::SFFloat(radius),
                );
            }
        }

        Ok(())
    }

    fn image_element(&mut self, el: &'a Element) -> R<()> {
        if let Some(transform) = self.fill_geometries.get(&key(el)).copied() {
            let group = *self.group_nodes.last().unwrap();

            push_child(&mut self.scene, group, "children", transform);
            return Ok(());
        }

        // Determine style.
        if !self.style_attributes(el) {
            return Ok(());
        }

        // Create Transform node.
        let x = self.length_attribute(el.attr("x"), 0.0, "width");
        let y = self.length_attribute(el.attr("y"), 0.0, "height");
        let width = self.length_attribute(el.attr("width"), 0.0, "width");
        let height = self.length_attribute(el.attr("height"), 0.0, "height");
        let href = href_of(el);
        let transform =
            self.create_transform(el, [x + width / 2.0, y + height / 2.0], [1.0, -1.0])?;

        self.fill_geometries.insert(key(el), transform);
        self.group_nodes.push(transform);

        // Create nodes.
        let shape = self.node("Shape")?;
        let appearance = self.node("Appearance")?;
        let texture = self.node("ImageTexture")?;
        let rectangle = self.node("Rectangle2D")?;

        set_node(&mut self.scene, shape, "appearance", Some(appearance));
        set_node(&mut self.scene, shape, "geometry", Some(rectangle));
        set_node(&mut self.scene, appearance, "texture", Some(texture));
        set(
            &mut self.scene,
            texture,
            "url",
            FieldValue::MFString(vec![href.unwrap_or_else(|| "null".into())]),
        );
        set_node(
            &mut self.scene,
            texture,
            "textureProperties",
            Some(self.texture_properties),
        );
        set(
            &mut self.scene,
            rectangle,
            "solid",
            FieldValue::SFBool(false),
        );
        set(
            &mut self.scene,
            rectangle,
            "size",
            FieldValue::SFVec2f(Vector2::new(width, height)),
        );

        push_child(&mut self.scene, transform, "children", shape);

        self.pop_all();
        Ok(())
    }

    fn polyline_element(&mut self, el: &'a Element) -> R<()> {
        // Get points.
        let mut points = vec![];

        if !self.points_attribute(el.attr("points"), &mut points) {
            return Ok(());
        }

        // Create nodes.
        self.path_like_element(
            el,
            vec![Contour {
                pts: points,
                closed: false,
                index: 0,
            }],
        )
    }

    fn polygon_element(&mut self, el: &'a Element) -> R<()> {
        // Get points.
        let mut points = vec![];

        if !self.points_attribute(el.attr("points"), &mut points) {
            return Ok(());
        }

        // Create nodes.
        self.path_like_element(
            el,
            vec![Contour {
                pts: points,
                closed: true,
                index: 0,
            }],
        )
    }

    fn path_element(&mut self, el: &'a Element) -> R<()> {
        // Get path points.
        let mut contours = vec![];

        if !self.d_attribute(el.attr("d"), &mut contours) {
            return Ok(());
        }

        // Create nodes.
        self.path_like_element(el, contours)
    }

    fn path_like_element(&mut self, el: &Element, contours: Vec<Contour>) -> R<()> {
        // Determine style.
        if !self.style_attributes(el) {
            return Ok(());
        }

        // Filter consecutive equal points.
        const EPSILON: f64 = 1e-9; // Min point distance.

        let distance = |a: &P3, b: &P3| jsmath::hypot(&[a[0] - b[0], a[1] - b[1], a[2] - b[2]]);

        let mut contours: Vec<Contour> = contours
            .into_iter()
            .map(|c| {
                let n = c.pts.len();

                if c.closed {
                    let pts = (0..n)
                        .filter(|i| distance(&c.pts[*i], &c.pts[(i + 1) % n]) > EPSILON)
                        .map(|i| c.pts[i])
                        .collect();

                    Contour {
                        pts,
                        closed: true,
                        index: 0,
                    }
                } else {
                    let pts = (0..n)
                        .filter(|i| *i == 0 || distance(&c.pts[*i], &c.pts[i - 1]) > EPSILON)
                        .map(|i| c.pts[i])
                        .collect();

                    Contour {
                        pts,
                        closed: false,
                        index: 0,
                    }
                }
            })
            .filter(|c| c.pts.len() > 2)
            .collect();

        // Add index property to points.
        let mut next = 0;

        for c in &mut contours {
            c.index = next;
            next += c.pts.len();
        }

        // Create Transform node.
        let transform = self.create_transform(el, [0.0, 0.0], [1.0, 1.0])?;
        let mut bbox = Box2::empty();

        for c in &contours {
            bbox.add(&Box2::from_points(&c.pts));
        }

        self.group_nodes.push(transform);

        let result = self.path_shapes(el, transform, &contours, &bbox);

        self.pop_all();
        result
    }

    fn path_shapes(
        &mut self,
        el: &Element,
        transform: NodeId,
        contours: &[Contour],
        bbox: &Box2,
    ) -> R<()> {
        // Create nodes.
        let coordinate = self.node("Coordinate")?;
        let mut vertices: Vec<f64> = vec![];

        for c in contours {
            for p in &c.pts {
                vertices.extend_from_slice(p);
            }
        }

        if self.style().fill_type != "none" {
            let shape = self.node("Shape")?;
            let existing = self.fill_geometries.get(&key(el)).copied();

            push_child(&mut self.scene, transform, "children", shape);

            let appearance = self.create_fill_appearance(bbox)?;

            set_node(&mut self.scene, shape, "appearance", appearance);

            if let Some(geometry) = existing {
                set_node(&mut self.scene, shape, "geometry", Some(geometry));
            } else {
                let geometry = self.node("IndexedTriangleSet")?;

                self.id_attribute(el.attr("id"), "-Fill", geometry, true, false);
                self.fill_geometries.insert(key(el), geometry);

                set_node(&mut self.scene, shape, "geometry", Some(geometry));
                set(
                    &mut self.scene,
                    geometry,
                    "solid",
                    FieldValue::SFBool(false),
                );

                let index = self.triangulate_polygon(contours, &mut vertices);

                set_numbers(&mut self.scene, geometry, "index", &index);

                // createTextureCoordinate (coordinateNode, bbox, shapeNode .appearance)
                let tex_coord = match appearance {
                    Some(a) if node_has_texture(&self.scene, a) => {
                        let tex_coord = self.node("TextureCoordinate")?;
                        let mut inverse = bbox.matrix;
                        let mut points = vec![];

                        inverse.inverse();

                        for p in vertices.chunks_exact(3) {
                            // The Coordinate stores 32-bit floats.
                            let mut v = Vector2::new(p[0] as f32 as f64, p[1] as f32 as f64);

                            inverse.mult_vec_matrix2(&mut v);
                            v.add(&Vector2::new(1.0, 1.0));
                            v.divide(2.0);
                            points.push(v.x);
                            points.push(v.y);
                        }

                        set_numbers(&mut self.scene, tex_coord, "point", &points);
                        Some(tex_coord)
                    }
                    _ => None,
                };

                set_node(&mut self.scene, geometry, "texCoord", tex_coord);
                set_node(&mut self.scene, geometry, "coord", Some(coordinate));
            }
        }

        if self.style().stroke_type != "none" {
            let shape = self.node("Shape")?;
            let existing = self.stroke_geometries.get(&key(el)).copied();

            push_child(&mut self.scene, transform, "children", shape);

            let appearance = self.create_stroke_appearance()?;

            set_node(&mut self.scene, shape, "appearance", Some(appearance));

            if let Some(geometry) = existing {
                set_node(&mut self.scene, shape, "geometry", Some(geometry));
            } else {
                let geometry = self.node("IndexedLineSet")?;

                self.id_attribute(el.attr("id"), "-Stroke", geometry, true, false);
                self.stroke_geometries.insert(key(el), geometry);

                set_node(&mut self.scene, shape, "geometry", Some(geometry));
                set_node(&mut self.scene, geometry, "coord", Some(coordinate));

                // Create contour indices.
                let mut indices: Vec<f64> = vec![];

                for c in contours {
                    for i in 0..c.pts.len() {
                        indices.push((c.index + i) as f64);
                    }

                    if c.closed {
                        indices.push(c.index as f64);
                    }

                    indices.push(-1.0);
                }

                set_numbers(&mut self.scene, geometry, "coordIndex", &indices);
            }
        }

        // The Coordinate holds the contour points and the vertices libtess added.
        set_numbers(&mut self.scene, coordinate, "point", &vertices);
        Ok(())
    }

    /// `triangulatePolygon (contours, coordinateNode)`; libtess appends vertices to `vertices`.
    fn triangulate_polygon(&mut self, contours: &[Contour], vertices: &mut Vec<f64>) -> Vec<f64> {
        let winding_odd = self.style().fill_rule == "evenodd";
        let input: Vec<Vec<([f64; 2], usize)>> = contours
            .iter()
            .map(|c| {
                c.pts
                    .iter()
                    .enumerate()
                    .map(|(i, p)| ([p[0], p[1]], c.index + i))
                    .collect()
            })
            .collect();
        let (triangles, added) =
            x3d_text::libtess::triangulate_svg(&input, winding_odd, vertices.len() / 3);

        for v in added {
            vertices.extend_from_slice(&v);
        }

        triangles.into_iter().map(|i| i as f64).collect()
    }
}

fn node_has_texture(scene: &Scene, appearance: NodeId) -> bool {
    matches!(
        crate::formats::node_field(scene, appearance, "texture"),
        FieldValue::SFNode(Some(_))
    )
}

// --- entry ---------------------------------------------------------------------------------------

/// SVGParser.isValid: the document element is `svg`.
pub(crate) fn is_valid(root: &Element) -> bool {
    root.name == "svg"
}

/// SVGParser.parseIntoScene.
pub(crate) fn parse_svg(root: &Element, world_url: &str) -> R<Scene> {
    let (primitive, texture) = QUALITY.with(|q| q.get());
    let (bezier_steps, circle_steps) = match primitive {
        Quality::Low => (9, 20),
        Quality::High => (15, 64),
        Quality::Medium => (12, 32),
    };
    let gradient_size = match texture {
        Quality::Low => 128,
        Quality::High => 512,
        Quality::Medium => 256,
    };

    let mut scene = Scene::new(world_url);

    scene.encoding = "SVG".into();
    scene.profile = Some("Interchange".into());
    scene.update_component("Geometry2D", 2);

    let mut svg = Svg {
        scene,
        world_url: world_url.to_string(),
        document: root,
        lex: Lex::new(),
        styles: vec![Style::initial()],
        view_box: [0.0, 0.0, 100.0, 100.0],
        model: vec![Matrix4::IDENTITY],
        fill_geometries: HashMap::new(),
        stroke_geometries: HashMap::new(),
        line_properties: HashMap::new(),
        group_nodes: vec![],
        texture_properties: 0,
        num_switch_nodes: 0,
        bezier_steps,
        circle_steps,
        gradient_size,
        depth: 0,
        div_color: None,
    };

    svg.svg_element(root)?;

    let mut scene = svg.scene;

    // X_ITE sets each node up as it is created; the port does the same side effects after the parse, like the XML parser.
    scene.setup_nodes();

    crate::optimizer::Optimizer {
        remove_empty_groups: true,
        combine_grouping_nodes: true,
        ..Default::default()
    }
    .optimize_scene_graph(&mut scene, ROOT);
    Ok(scene)
}

impl<'a> Svg<'a> {
    fn svg_element(&mut self, el: &'a Element) -> R<()> {
        // Init nodes.
        let root_transform = self.node("Transform")?;

        self.group_nodes = vec![root_transform];
        self.texture_properties = self.create_texture_properties()?;

        // Create background.
        let background = self.node("Background")?;

        set_numbers(&mut self.scene, background, "skyColor", &[1.0, 1.0, 1.0]);
        self.scene.ctx_mut(ROOT).root_nodes.push(Some(background));

        // Create navigation info.
        let navigation_info = self.node("NavigationInfo")?;

        set(
            &mut self.scene,
            navigation_info,
            "type",
            FieldValue::MFString(
                ["PLANE", "PLANE_create3000.github.io", "EXAMINE", "ANY"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
        );

        // Get attributes of svg element.
        let default_width = self.length_attribute(el.attr("width"), 300.0, "width");
        let default_height = self.length_attribute(el.attr("height"), 150.0, "height");

        // `this .viewBox .set (0, 0, defaultWidth, defaultHeight)` is also the default view box object.
        self.view_box = [0.0, 0.0, default_width, default_height];

        let attribute_box = self.view_box_attribute(el.attr("viewBox"));
        let mut view_box = attribute_box.unwrap_or(self.view_box);
        let width = self.length_attribute(el.attr("width"), view_box[2], "width");
        let height = self.length_attribute(el.attr("height"), view_box[3], "height");

        {
            // preserveAspectRatio = "xMidYMid meet"
            let r = width / height;
            let rv = view_box[2] / view_box[3];

            if rv > r {
                view_box[3] += view_box[2] / r - view_box[3];
            } else {
                view_box[2] += view_box[3] * r - view_box[2];
            }

            // Without a viewBox attribute the box is `this .viewBox` itself.
            if attribute_box.is_none() {
                self.view_box = view_box;
            }
        }

        // Create viewpoint.
        let viewpoint = self.node("OrthoViewpoint")?;
        let x = (view_box[0] + width / 2.0) * PIXEL;
        let y = -(view_box[1] + height / 2.0) * PIXEL;

        set_node(
            &mut self.scene,
            viewpoint,
            "navigationInfo",
            Some(navigation_info),
        );
        set(
            &mut self.scene,
            viewpoint,
            "position",
            FieldValue::SFVec3f(Vector3::new(x, y, 10.0)),
        );
        set(
            &mut self.scene,
            viewpoint,
            "centerOfRotation",
            FieldValue::SFVec3f(Vector3::new(x, y, 0.0)),
        );
        // `viewpoint .fieldOfView = new Vector4 (...)`: an SFVec4f in an X3D 4.1 scene (the SVG parser's), an MFFloat in a scene up to
        // 4.0 (OrthoViewpoint's legacy changeField).
        let fov = [
            -width / 2.0 * PIXEL,
            -height / 2.0 * PIXEL,
            width / 2.0 * PIXEL,
            height / 2.0 * PIXEL,
        ];
        let i = self
            .scene
            .get_predefined_field(viewpoint, "fieldOfView")
            .unwrap();
        let value = match self.scene.node(viewpoint).fields[i].value {
            FieldValue::SFVec4f(_) => FieldValue::SFVec4f(x3d_fields::math::Vector4::new(
                fov[0], fov[1], fov[2], fov[3],
            )),
            _ => FieldValue::MFFloat(fov.iter().map(|v| *v as f32).collect()),
        };

        set(&mut self.scene, viewpoint, "fieldOfView", value);

        self.scene.ctx_mut(ROOT).root_nodes.push(Some(viewpoint));

        // Create view matrix.
        let scale = Vector3::new(
            width * PIXEL / view_box[2],
            -height * PIXEL / view_box[3],
            1.0,
        );

        set(
            &mut self.scene,
            root_transform,
            "scale",
            FieldValue::SFVec3f(scale),
        );

        // Parse elements.
        self.elements(el)?;

        // Add root Transform node.
        let unique = self.scene.get_unique_name(ROOT, "ViewBox");

        self.scene.update_named_node(ROOT, &unique, root_transform);

        let exported = self.scene.get_unique_export_name("ViewBox");

        let _ = self
            .scene
            .update_exported_node(ROOT, &exported, root_transform, "");
        self.scene
            .ctx_mut(ROOT)
            .root_nodes
            .push(Some(root_transform));

        Ok(())
    }
}
