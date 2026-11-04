//! CSS Color 5 relative color syntax: `<function>(from <color> c1 c2 c3 [/ alpha])` for `rgb()`,
//! `hsl()`, `hwb()`, `lab()`, `lch()`, `oklab()`, `oklch()` and `color()`.
//!
//! Chrome converts the origin color to the color space of the function, makes the channels
//! available as keywords (`r g b`, `h s l`, `l a b`, `x y z`, `alpha`), evaluates each channel
//! (a number, a percentage, an angle, `none`, a keyword, or `calc()` and the math functions) and
//! computes `rgb()`, `hsl()` and `hwb()` to `color(srgb ...)`. The other functions keep their
//! name. `X3DParser.convertColor` then misreads that string (see `unparsed_function`), so the
//! result here is already that misreading, like the other CSS Color 4 functions.
//!
//! Conversions between color spaces run through XYZ D50 in 64-bit floats. Chrome computes them in
//! 32-bit floats, so a result that goes through a different color space can differ from Chrome by
//! one unit of the sixth significant digit Chrome prints; see `tests/rea_css5_relative_color.rs`.

use super::{NAMED, hex, unparsed_function};
use std::cell::Cell;

thread_local! {
    /// What `currentcolor` means as an origin: the color of the wrapper element
    /// `X3DParser.convertColor` sets (white for a direct call, the inherited color in an SVG).
    static CURRENT: Cell<[f64; 4]> = const { Cell::new([1.0, 1.0, 1.0, 1.0]) };
}

/// Runs `f` with `currentcolor` resolving to `current` (`[r, g, b, a]` in [0, 1]).
pub(super) fn with_current<T>(current: [f64; 4], f: impl FnOnce() -> T) -> T {
    let before = CURRENT.with(|c| c.replace(current));
    let result = f();
    CURRENT.with(|c| c.set(before));
    result
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Space {
    Srgb,
    SrgbLinear,
    DisplayP3,
    A98,
    ProPhoto,
    Rec2020,
    XyzD50,
    XyzD65,
    Lab,
    Lch,
    Oklab,
    Oklch,
    Hsl,
    Hwb,
}

/// A color with every channel resolved (a missing channel is 0). RGB spaces are unit-range
/// floats, hue is in degrees, HSL and HWB percentages are fractions of 1.
#[derive(Clone, Copy, Debug)]
struct Col {
    space: Space,
    c: [f64; 3],
    alpha: f64,
}

/// The computed value of a relative color: channels and alpha, `None` for `none`.
struct Res {
    space: Space,
    c: [Option<f64>; 3],
    alpha: Option<f64>,
}

/// One evaluated component.
#[derive(Clone, Copy, Debug, PartialEq)]
enum V {
    None,
    Num(f64),
    Pct(f64),
    Angle(f64),
}

// ---- tokens -----------------------------------------------------------------------------------

/// The top-level tokens of the arguments: split at white space outside parentheses, `/` a token of
/// its own, a top-level comma is invalid.
fn tokenize(s: &str) -> Option<Vec<String>> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut depth = 0i32;

    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                cur.push(ch);
            }
            ',' if depth == 0 => return None,
            '/' if depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                out.push("/".to_string());
            }
            c if c.is_ascii_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }

    if depth != 0 {
        return None;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Some(out)
}

fn space_of(name: &str) -> Option<Space> {
    Some(match name {
        "srgb" => Space::Srgb,
        "srgb-linear" => Space::SrgbLinear,
        "display-p3" => Space::DisplayP3,
        "a98-rgb" => Space::A98,
        "prophoto-rgb" => Space::ProPhoto,
        "rec2020" => Space::Rec2020,
        "xyz" | "xyz-d65" => Space::XyzD65,
        "xyz-d50" => Space::XyzD50,
        _ => return None,
    })
}

// ---- calc() -----------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Tok<'a> {
    Num(f64, &'a str),
    Ident(&'a str),
    Op(char),
    Open,
    Close,
    Comma,
}

fn lex(s: &str) -> Option<Vec<Tok<'_>>> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut out = vec![];

    while i < b.len() {
        let c = b[i];

        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'(' {
            out.push(Tok::Open);
            i += 1;
        } else if c == b')' {
            out.push(Tok::Close);
            i += 1;
        } else if c == b',' {
            out.push(Tok::Comma);
            i += 1;
        } else if matches!(c, b'*' | b'/') {
            out.push(Tok::Op(c as char));
            i += 1;
        } else if c.is_ascii_digit()
            || c == b'.'
            || (matches!(c, b'+' | b'-')
                && b.get(i + 1)
                    .is_some_and(|n| n.is_ascii_digit() || *n == b'.'))
        {
            let start = i;
            if matches!(c, b'+' | b'-') {
                i += 1;
            }
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            // A fraction needs a digit after the point: `5.` is not a number.
            if i < b.len() && b[i] == b'.' {
                i += 1;
                if !b.get(i).is_some_and(u8::is_ascii_digit) {
                    return None;
                }
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            // An exponent: `e` followed by digits (a bare `e` is the constant or a unit).
            if i < b.len()
                && b[i] == b'e'
                && (b.get(i + 1).is_some_and(u8::is_ascii_digit)
                    || (matches!(b.get(i + 1), Some(b'+' | b'-'))
                        && b.get(i + 2).is_some_and(u8::is_ascii_digit)))
            {
                i += 2;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let num_end = i;
            while i < b.len() && (b[i].is_ascii_alphabetic() || b[i] == b'%') {
                i += 1;
                if b[i - 1] == b'%' {
                    break;
                }
            }
            let v: f64 = s[start..num_end].parse().ok()?;
            out.push(Tok::Num(v, &s[num_end..i]));
        } else if c.is_ascii_alphabetic()
            || c == b'_'
            || (c == b'-'
                && b.get(i + 1)
                    .is_some_and(|n| n.is_ascii_alphabetic() || matches!(n, b'_' | b'-')))
        {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'-') {
                i += 1;
            }
            out.push(Tok::Ident(&s[start..i]));
        } else if matches!(c, b'+' | b'-') {
            out.push(Tok::Op(c as char));
            i += 1;
        } else {
            return None;
        }
    }

    Some(out)
}

/// The value of a number with a unit.
fn with_unit(v: f64, unit: &str) -> Option<V> {
    Some(match unit {
        "" => V::Num(v),
        "%" => V::Pct(v),
        "deg" => V::Angle(v),
        "grad" => V::Angle(v * 0.9),
        "rad" => V::Angle(v * 180.0 / std::f64::consts::PI),
        "turn" => V::Angle(v * 360.0),
        _ => return None,
    })
}

struct Calc<'a, 'k> {
    toks: Vec<Tok<'a>>,
    at: usize,
    /// Channel keywords and their values.
    keywords: &'k [(&'k str, f64)],
}

impl<'a> Calc<'a, '_> {
    fn peek(&self) -> Option<Tok<'a>> {
        self.toks.get(self.at).copied()
    }

    fn next(&mut self) -> Option<Tok<'a>> {
        let t = self.peek();
        self.at += 1;
        t
    }

    fn sum(&mut self) -> Option<V> {
        let mut acc = self.product()?;
        while let Some(Tok::Op(op @ ('+' | '-'))) = self.peek() {
            self.at += 1;
            let rhs = self.product()?;
            acc = add(acc, rhs, op == '-')?;
        }
        Some(acc)
    }

    fn product(&mut self) -> Option<V> {
        let mut acc = self.factor()?;
        while let Some(Tok::Op(op @ ('*' | '/'))) = self.peek() {
            self.at += 1;
            let rhs = self.factor()?;
            acc = if op == '*' {
                mul(acc, rhs)?
            } else {
                div(acc, rhs)?
            };
        }
        Some(acc)
    }

    fn factor(&mut self) -> Option<V> {
        match self.next()? {
            Tok::Num(v, unit) => with_unit(v, unit),
            Tok::Open => {
                let v = self.sum()?;
                matches!(self.next()?, Tok::Close).then_some(v)
            }
            Tok::Ident(id) => {
                if matches!(self.peek(), Some(Tok::Open)) {
                    self.at += 1;
                    self.function(id)
                } else {
                    self.ident(id)
                }
            }
            _ => None,
        }
    }

    fn ident(&self, id: &str) -> Option<V> {
        if let Some((_, v)) = self.keywords.iter().find(|(k, _)| *k == id) {
            return Some(V::Num(*v));
        }
        Some(V::Num(match id {
            "pi" => std::f64::consts::PI,
            "e" => std::f64::consts::E,
            "infinity" => f64::INFINITY,
            "-infinity" => f64::NEG_INFINITY,
            "nan" => f64::NAN,
            _ => return None,
        }))
    }

    /// The comma-separated arguments of a math function; the opening parenthesis is consumed.
    fn args(&mut self) -> Option<Vec<V>> {
        let mut out = vec![self.sum()?];
        loop {
            match self.next()? {
                Tok::Comma => out.push(self.sum()?),
                Tok::Close => return Some(out),
                _ => return None,
            }
        }
    }

    fn function(&mut self, name: &str) -> Option<V> {
        if name == "calc" {
            let v = self.sum()?;
            return matches!(self.next()?, Tok::Close).then_some(v);
        }

        // `round(<strategy>, a, b)` starts with a keyword.
        if name == "round" {
            let strategy = match self.peek() {
                Some(Tok::Ident(s)) if matches!(s, "nearest" | "up" | "down" | "to-zero") => {
                    self.at += 1;
                    if !matches!(self.next()?, Tok::Comma) {
                        return None;
                    }
                    s
                }
                _ => "nearest",
            };
            let a = self.args()?;
            return round_fn(strategy, &a);
        }

        let a = self.args()?;
        math(name, &a)
    }
}

fn same_kind(a: V, b: V) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}

fn parts(v: V) -> Option<(u8, f64)> {
    match v {
        V::Num(x) => Some((0, x)),
        V::Pct(x) => Some((1, x)),
        V::Angle(x) => Some((2, x)),
        V::None => None,
    }
}

fn make(kind: u8, x: f64) -> V {
    match kind {
        0 => V::Num(x),
        1 => V::Pct(x),
        _ => V::Angle(x),
    }
}

fn add(a: V, b: V, minus: bool) -> Option<V> {
    if !same_kind(a, b) {
        return None;
    }
    let ((k, x), (_, y)) = (parts(a)?, parts(b)?);
    Some(make(k, if minus { x - y } else { x + y }))
}

fn mul(a: V, b: V) -> Option<V> {
    match (a, b) {
        (V::Num(x), o) | (o, V::Num(x)) => {
            let (k, y) = parts(o)?;
            Some(make(k, x * y))
        }
        _ => None,
    }
}

fn div(a: V, b: V) -> Option<V> {
    match (a, b) {
        (V::Pct(x), V::Pct(y)) | (V::Angle(x), V::Angle(y)) => Some(V::Num(x / y)),
        (o, V::Num(y)) => {
            let (k, x) = parts(o)?;
            Some(make(k, x / y))
        }
        _ => None,
    }
}

fn round_fn(strategy: &str, a: &[V]) -> Option<V> {
    let (x, step) = match a {
        [x] => (*x, V::Num(1.0)),
        [x, s] => (*x, *s),
        _ => return None,
    };
    if !same_kind(x, step) && !matches!(a, [_]) {
        return None;
    }
    let ((k, v), (_, s)) = (parts(x)?, parts(step)?);
    let q = v / s;
    let r = match strategy {
        "up" => q.ceil(),
        "down" => q.floor(),
        "to-zero" => q.trunc(),
        _ => q.round(),
    };
    Some(make(k, r * s))
}

fn math(name: &str, a: &[V]) -> Option<V> {
    // All arguments the same type, for the functions that keep it.
    let same = a.iter().all(|v| same_kind(*v, a[0]));
    let num = |v: V| match v {
        V::Num(x) => Some(x),
        _ => None,
    };
    // An angle argument in radians, or a plain number as radians.
    let rad = |v: V| match v {
        V::Num(x) => Some(x),
        V::Angle(d) => Some(d.to_radians()),
        _ => None,
    };

    Some(match (name, a.len()) {
        ("min" | "max", _) if same => {
            let (k, _) = parts(a[0])?;
            let xs: Vec<f64> = a
                .iter()
                .map(|v| parts(*v).map(|p| p.1))
                .collect::<Option<_>>()?;
            let f = if name == "min" { f64::min } else { f64::max };
            make(k, xs.into_iter().reduce(f)?)
        }
        ("clamp", 3) if same => {
            let (k, lo) = parts(a[0])?;
            let (_, v) = parts(a[1])?;
            let (_, hi) = parts(a[2])?;
            make(k, lo.max(v.min(hi)))
        }
        ("abs", 1) => {
            let (k, x) = parts(a[0])?;
            make(k, x.abs())
        }
        ("sign", 1) => {
            let (_, x) = parts(a[0])?;
            V::Num(if x == 0.0 { x } else { x.signum() })
        }
        ("mod", 2) if same => {
            let (k, x) = parts(a[0])?;
            let (_, y) = parts(a[1])?;
            make(k, x - y * (x / y).floor())
        }
        ("rem", 2) if same => {
            let (k, x) = parts(a[0])?;
            let (_, y) = parts(a[1])?;
            make(k, x % y)
        }
        ("sqrt", 1) => V::Num(num(a[0])?.sqrt()),
        ("exp", 1) => V::Num(num(a[0])?.exp()),
        ("pow", 2) => V::Num(num(a[0])?.powf(num(a[1])?)),
        ("log", 1) => V::Num(num(a[0])?.ln()),
        ("log", 2) => V::Num(num(a[0])?.ln() / num(a[1])?.ln()),
        ("hypot", _) if same => {
            let (k, _) = parts(a[0])?;
            let sum: f64 = a
                .iter()
                .map(|v| parts(*v).map(|p| p.1 * p.1))
                .sum::<Option<f64>>()?;
            make(k, sum.sqrt())
        }
        ("sin", 1) => V::Num(rad(a[0])?.sin()),
        ("cos", 1) => V::Num(rad(a[0])?.cos()),
        ("tan", 1) => V::Num(rad(a[0])?.tan()),
        ("asin", 1) => V::Angle(num(a[0])?.asin().to_degrees()),
        ("acos", 1) => V::Angle(num(a[0])?.acos().to_degrees()),
        ("atan", 1) => V::Angle(num(a[0])?.atan().to_degrees()),
        ("atan2", 2) if same => {
            let (_, y) = parts(a[0])?;
            let (_, x) = parts(a[1])?;
            V::Angle(y.atan2(x).to_degrees())
        }
        _ => return None,
    })
}

/// One channel or alpha component: `none`, a keyword, a number with a unit, or a `calc()` or math
/// function expression.
fn component(token: &str, keywords: &[(&str, f64)]) -> Option<V> {
    if token == "none" {
        return Some(V::None);
    }

    let toks = lex(token)?;

    // `calc(...)` is a parenthesized expression; every other function is a math function.
    let mut calc = Calc {
        toks,
        at: 0,
        keywords,
    };

    let v = match calc.peek()? {
        Tok::Ident(id) if matches!(calc.toks.get(1), Some(Tok::Open)) => {
            calc.at = 2;
            calc.function(id)?
        }
        // A single keyword or number; the constants are only valid inside a function.
        Tok::Ident(id) => {
            calc.at = 1;
            calc.keywords
                .iter()
                .find(|(k, _)| *k == id)
                .map(|(_, v)| V::Num(*v))?
        }
        Tok::Num(v, unit) => {
            calc.at = 1;
            with_unit(v, unit)?
        }
        _ => return None,
    };

    (calc.at == calc.toks.len()).then_some(v)
}

// ---- color space conversions ------------------------------------------------------------------

type M3 = [[f64; 3]; 3];

// Skia's color space gamuts to XYZ D50, as Chrome applies them.
const M_SRGB: M3 = [
    [0.436065674, 0.385147095, 0.143066406],
    [0.222488403, 0.716873169, 0.060607910],
    [0.013916016, 0.097076416, 0.714096069],
];
const M_P3: M3 = [
    [0.515102, 0.291965, 0.157153],
    [0.241182, 0.692236, 0.0665819],
    [-0.00104941, 0.0418818, 0.784378],
];
const M_A98: M3 = [
    [39960.0 / 65536.0, 13453.0 / 65536.0, 9777.0 / 65536.0],
    [20389.0 / 65536.0, 41004.0 / 65536.0, 4143.0 / 65536.0],
    [1276.0 / 65536.0, 3989.0 / 65536.0, 48796.0 / 65536.0],
];
const M_REC2020: M3 = [
    [0.673458, 0.165661, 0.1251],
    [0.279033, 0.675337, 0.0456288],
    [-0.00193139, 0.0299794, 0.797161],
];
const M_PRO: M3 = [
    [0.79767, 0.135194, 0.031356],
    [0.288039, 0.711874, 0.000086609],
    [0.0, 0.0, 0.825211],
];

/// The Lab white point Chrome uses (not the ICC D50 of the RGB matrices).
const LAB_WHITE: [f64; 3] = [0.9642, 1.0, 0.8251];

/// Bradford adaptation between D50 (ICC: 0.96422, 1, 0.82521) and D65 (0.3127, 0.3290).
const D50_TO_D65: M3 = [
    [0.955536672, -0.0230601727, 0.0632184897],
    [-0.0283153747, 1.0099514122, 0.0210259672],
    [0.0123087532, -0.0205005992, 1.3301947431],
];
// Bjorn Ottosson's OKLab, with the digits of the CSS Color 4 reference code.
#[allow(clippy::excessive_precision)]
const M1: M3 = [
    [0.8190224379967030, 0.3619062600528904, -0.1288737815209879],
    [0.0329836539323885, 0.9292868615863434, 0.0361446663506424],
    [0.0481771893596242, 0.2642395317527308, 0.6335478284694309],
];
#[allow(clippy::excessive_precision)]
const M2: M3 = [
    [0.2104542683093140, 0.7936177747023054, -0.0040720430116193],
    [1.9779985324311684, -2.4285922420485799, 0.4505937096174110],
    [0.0259040424655478, 0.7827717124575296, -0.8086757549230774],
];

fn mul3(m: &M3, v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

fn invert(m: &M3) -> M3 {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let c =
        |r1: usize, r2: usize, c1: usize, c2: usize| m[r1][c1] * m[r2][c2] - m[r1][c2] * m[r2][c1];

    [
        [c(1, 2, 1, 2) / d, -c(0, 2, 1, 2) / d, c(0, 1, 1, 2) / d],
        [-c(1, 2, 0, 2) / d, c(0, 2, 0, 2) / d, -c(0, 1, 0, 2) / d],
        [c(1, 2, 0, 1) / d, -c(0, 2, 0, 1) / d, c(0, 1, 0, 1) / d],
    ]
}

fn signed(x: f64, f: impl Fn(f64) -> f64) -> f64 {
    if x < 0.0 { -f(-x) } else { f(x) }
}

fn srgb_decode(x: f64) -> f64 {
    signed(x, |a| {
        if a <= 0.04045 {
            a / 12.92
        } else {
            ((a + 0.055) / 1.055).powf(2.4)
        }
    })
}

fn srgb_encode(x: f64) -> f64 {
    signed(x, |a| {
        if a <= 0.0031308 {
            a * 12.92
        } else {
            1.055 * a.powf(1.0 / 2.4) - 0.055
        }
    })
}

fn gamma_decode(x: f64, g: f64) -> f64 {
    signed(x, |a| a.powf(g))
}

fn gamma_encode(x: f64, g: f64) -> f64 {
    signed(x, |a| a.powf(1.0 / g))
}

fn prophoto_decode(x: f64) -> f64 {
    signed(x, |a| {
        if a <= 16.0 / 512.0 {
            a / 16.0
        } else {
            a.powf(1.8)
        }
    })
}

fn prophoto_encode(x: f64) -> f64 {
    signed(x, |a| {
        if a <= 1.0 / 512.0 {
            a * 16.0
        } else {
            a.powf(1.0 / 1.8)
        }
    })
}

const REC2020_ALPHA: f64 = 1.09929682680944;
const REC2020_BETA: f64 = 0.018053968510807;

fn rec2020_decode(x: f64) -> f64 {
    signed(x, |a| {
        if a < REC2020_BETA * 4.5 {
            a / 4.5
        } else {
            ((a + REC2020_ALPHA - 1.0) / REC2020_ALPHA).powf(1.0 / 0.45)
        }
    })
}

fn rec2020_encode(x: f64) -> f64 {
    signed(x, |a| {
        if a < REC2020_BETA {
            a * 4.5
        } else {
            REC2020_ALPHA * a.powf(0.45) - (REC2020_ALPHA - 1.0)
        }
    })
}

/// The matrix to XYZ D50, and the decode and encode functions of a transfer function.
type RgbSpace = (&'static M3, fn(f64) -> f64, fn(f64) -> f64);

fn rgb_space(s: Space) -> Option<RgbSpace> {
    Some(match s {
        Space::Srgb => (&M_SRGB, srgb_decode, srgb_encode),
        Space::SrgbLinear => (&M_SRGB, |x| x, |x| x),
        Space::DisplayP3 => (&M_P3, srgb_decode, srgb_encode),
        Space::A98 => (&M_A98, |x| gamma_decode(x, 2.2), |x| gamma_encode(x, 2.2)),
        Space::ProPhoto => (&M_PRO, prophoto_decode, prophoto_encode),
        Space::Rec2020 => (&M_REC2020, rec2020_decode, rec2020_encode),
        _ => return None,
    })
}

const EPSILON: f64 = 216.0 / 24389.0;
const KAPPA: f64 = 24389.0 / 27.0;

fn lab_to_xyz(l: f64, a: f64, b: f64) -> [f64; 3] {
    let fy = (l + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;
    let cube = |f: f64| {
        if f * f * f > EPSILON {
            f * f * f
        } else {
            (116.0 * f - 16.0) / KAPPA
        }
    };
    let y = if l > KAPPA * EPSILON {
        fy * fy * fy
    } else {
        l / KAPPA
    };

    [
        cube(fx) * LAB_WHITE[0],
        y * LAB_WHITE[1],
        cube(fz) * LAB_WHITE[2],
    ]
}

fn xyz_to_lab(x: [f64; 3]) -> [f64; 3] {
    let f = |v: f64| {
        if v > EPSILON {
            v.cbrt()
        } else {
            (KAPPA * v + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (
        f(x[0] / LAB_WHITE[0]),
        f(x[1] / LAB_WHITE[1]),
        f(x[2] / LAB_WHITE[2]),
    );

    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn oklab_to_xyz65(c: [f64; 3]) -> [f64; 3] {
    let lms = mul3(&invert(&M2), c);
    mul3(&invert(&M1), lms.map(|v| v * v * v))
}

fn xyz65_to_oklab(x: [f64; 3]) -> [f64; 3] {
    mul3(&M2, mul3(&M1, x).map(f64::cbrt))
}

/// Chrome turns a polar color into a rectangular one in 32-bit floats, so a right angle leaves a
/// `cos` of -4.37e-8 instead of 0.
fn polar(l: f64, c: f64, h: f64) -> [f64; 3] {
    let r = (h as f32).to_radians();
    [
        l,
        f64::from(c as f32 * r.cos()),
        f64::from(c as f32 * r.sin()),
    ]
}

fn unpolar(c: [f64; 3]) -> [f64; 3] {
    let (a, b) = (c[1] as f32, c[2] as f32);
    let mut h = b.atan2(a).to_degrees();
    if h < 0.0 {
        h += 360.0;
    }
    [c[0], f64::from(a.hypot(b)), f64::from(h)]
}

/// HSL, HWB and their way back to sRGB run in 32-bit floats, like Chrome; the pure hue is the
/// one `hwb()` already uses.
fn hsl_to_srgb(h: f64, s: f64, l: f64) -> [f64; 3] {
    super::hue_rgb_f32(h as f32, s as f32, l as f32).map(f64::from)
}

fn hwb_to_srgb(h: f64, w: f64, b: f64) -> [f64; 3] {
    let (w, b) = (w as f32, b as f32);

    if w + b >= 1.0 {
        let g = f64::from(w / (w + b));
        return [g, g, g];
    }

    let k = 1.0f32 - w - b;
    super::hue_rgb_f32(h as f32, 1.0, 0.5).map(|v| f64::from(v * k + w))
}

/// The hue (degrees), saturation and lightness of an sRGB color.
fn srgb_to_hsl(c: [f64; 3]) -> [f64; 3] {
    let c = c.map(|v| v as f32);
    let (max, min) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let l = (min + max) / 2.0;
    let d = max - min;
    let (mut h, mut s) = (0.0f32, 0.0f32);

    if d != 0.0 {
        s = if l == 0.0 || l == 1.0 {
            0.0
        } else {
            (max - l) / l.min(1.0 - l)
        };
        h = if max == c[0] {
            (c[1] - c[2]) / d + if c[1] < c[2] { 6.0 } else { 0.0 }
        } else if max == c[1] {
            (c[2] - c[0]) / d + 2.0
        } else {
            (c[0] - c[1]) / d + 4.0
        } * 60.0;
    }

    [h, s, l].map(f64::from)
}

fn srgb_to_hwb(c: [f64; 3]) -> [f64; 3] {
    let hsl = srgb_to_hsl(c);
    let c = c.map(|v| v as f32);
    let (max, min) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    [hsl[0], f64::from(min), f64::from(1.0 - max)]
}

fn to_xyz50(col: Col) -> [f64; 3] {
    let c = col.c;

    match col.space {
        s if rgb_space(s).is_some() => {
            let (m, decode, _) = rgb_space(s).unwrap();
            mul3(m, c.map(decode))
        }
        Space::XyzD50 => c,
        Space::XyzD65 => mul3(&invert(&D50_TO_D65), c),
        Space::Lab => lab_to_xyz(c[0], c[1], c[2]),
        Space::Lch => {
            let p = polar(c[0], c[1], c[2]);
            lab_to_xyz(p[0], p[1], p[2])
        }
        Space::Oklab => mul3(&invert(&D50_TO_D65), oklab_to_xyz65(c)),
        Space::Oklch => mul3(
            &invert(&D50_TO_D65),
            oklab_to_xyz65(polar(c[0], c[1], c[2])),
        ),
        Space::Hsl => to_xyz50(Col {
            space: Space::Srgb,
            c: hsl_to_srgb(c[0], c[1], c[2]),
            alpha: col.alpha,
        }),
        Space::Hwb => to_xyz50(Col {
            space: Space::Srgb,
            c: hwb_to_srgb(c[0], c[1], c[2]),
            alpha: col.alpha,
        }),
        _ => unreachable!(),
    }
}

fn from_xyz50(space: Space, x: [f64; 3], alpha: f64) -> Col {
    let c = match space {
        s if rgb_space(s).is_some() => {
            let (m, _, encode) = rgb_space(s).unwrap();
            mul3(&invert(m), x).map(encode)
        }
        Space::XyzD50 => x,
        Space::XyzD65 => mul3(&D50_TO_D65, x),
        Space::Lab => xyz_to_lab(x),
        Space::Lch => unpolar(xyz_to_lab(x)),
        Space::Oklab => xyz65_to_oklab(mul3(&D50_TO_D65, x)),
        Space::Oklch => unpolar(xyz65_to_oklab(mul3(&D50_TO_D65, x))),
        Space::Hsl => srgb_to_hsl(from_xyz50(Space::Srgb, x, alpha).c),
        Space::Hwb => srgb_to_hwb(from_xyz50(Space::Srgb, x, alpha).c),
        _ => unreachable!(),
    };

    Col { space, c, alpha }
}

/// The color in another color space. Same-space and sibling spaces (Lab and LCH, OKLab and OKLCH,
/// sRGB and HSL or HWB) convert directly, everything else through XYZ D50.
fn convert(col: Col, to: Space) -> Col {
    use Space::*;

    if col.space == to {
        return col;
    }

    let c = col.c;
    let direct = match (col.space, to) {
        (Lab, Lch) | (Oklab, Oklch) => Some(unpolar(c)),
        (Lch, Lab) | (Oklch, Oklab) => Some(polar(c[0], c[1], c[2])),
        (Srgb, Hsl) => Some(srgb_to_hsl(c)),
        (Srgb, Hwb) => Some(srgb_to_hwb(c)),
        (Hsl, Srgb) => Some(hsl_to_srgb(c[0], c[1], c[2])),
        (Hwb, Srgb) => Some(hwb_to_srgb(c[0], c[1], c[2])),
        (Hsl, Hwb) => Some(srgb_to_hwb(hsl_to_srgb(c[0], c[1], c[2]))),
        (Hwb, Hsl) => Some(srgb_to_hsl(hwb_to_srgb(c[0], c[1], c[2]))),
        _ => None,
    };

    match direct {
        Some(c) => Col {
            space: to,
            c,
            alpha: col.alpha,
        },
        None => from_xyz50(to, to_xyz50(col), col.alpha),
    }
}

// ---- origin colors ----------------------------------------------------------------------------

/// A component of an origin color as a plain value: `none` is 0, a percentage is
/// `pct_scale` units per 100%.
fn origin_value(token: &str, pct_scale: f64) -> Option<f64> {
    Some(match super::comp(token)? {
        super::Comp::None => 0.0,
        super::Comp::Num(v) => v,
        super::Comp::Pct(v) => v / 100.0 * pct_scale,
        super::Comp::Angle(_) => return None,
    })
}

fn origin_hue(token: &str) -> Option<f64> {
    match super::comp(token)? {
        super::Comp::None => Some(0.0),
        super::Comp::Num(v) | super::Comp::Angle(v) => Some(v),
        super::Comp::Pct(_) => None,
    }
}

fn origin_alpha(token: Option<&str>) -> Option<f64> {
    let Some(t) = token else { return Some(1.0) };
    Some(match super::comp(t)? {
        super::Comp::None => 0.0,
        super::Comp::Num(v) => v.clamp(0.0, 1.0),
        super::Comp::Pct(v) => (v / 100.0).clamp(0.0, 1.0),
        super::Comp::Angle(_) => return None,
    })
}

/// Splits `a b c [/ alpha]` (modern) or `a, b, c[, alpha]` (legacy) into channels and alpha.
fn origin_args(inner: &str) -> Option<(Vec<String>, Option<String>, bool)> {
    if inner.contains(',') {
        let parts: Vec<String> = inner.split(',').map(|p| p.trim().to_string()).collect();
        if parts
            .iter()
            .any(|p| p.is_empty() || p.contains(char::is_whitespace))
        {
            return None;
        }
        return match parts.len() {
            3 => Some((parts, None, true)),
            4 => Some((parts[..3].to_vec(), Some(parts[3].clone()), true)),
            _ => None,
        };
    }

    let toks = tokenize(inner)?;
    match toks.as_slice() {
        [a, b, c] => Some((vec![a.clone(), b.clone(), c.clone()], None, false)),
        [a, b, c, s, al] if s == "/" => Some((
            vec![a.clone(), b.clone(), c.clone()],
            Some(al.clone()),
            false,
        )),
        _ => None,
    }
}

fn origin_function(name: &str, inner: &str) -> Option<Col> {
    if inner.trim_start().starts_with("from")
        && inner.trim_start()[4..].starts_with(|c: char| c.is_ascii_whitespace())
    {
        let r = relative_result(name, inner)?;
        return Some(resolve(&r));
    }

    if name == "color" {
        let toks = tokenize(inner)?;
        let (space, words, alpha) = match toks.as_slice() {
            [s, a, b, c] => (s, [a, b, c], None),
            [s, a, b, c, slash, al] if slash == "/" => (s, [a, b, c], Some(al.as_str())),
            _ => return None,
        };
        let mut v = [0.0; 3];
        for (slot, w) in v.iter_mut().zip(words) {
            *slot = origin_value(w, 1.0)?;
        }
        return Some(Col {
            space: space_of(space)?,
            c: v,
            alpha: origin_alpha(alpha)?,
        });
    }

    let (w, alpha, legacy) = origin_args(inner)?;
    let alpha = origin_alpha(alpha.as_deref())?;

    Some(match name {
        "rgb" | "rgba" => {
            let ch = |t: &String| origin_value(t, 255.0).map(|v| v.clamp(0.0, 255.0) / 255.0);
            Col {
                space: Space::Srgb,
                c: [ch(&w[0])?, ch(&w[1])?, ch(&w[2])?],
                alpha,
            }
        }
        "hsl" | "hsla" => {
            let frac = |t: &String| origin_value(t, 100.0).map(|v| (v / 100.0).clamp(0.0, 1.0));
            Col {
                space: Space::Hsl,
                c: [origin_hue(&w[0])?, frac(&w[1])?, frac(&w[2])?],
                alpha,
            }
        }
        "hwb" if !legacy => {
            let frac = |t: &String| origin_value(t, 100.0).map(|v| v.max(0.0) / 100.0);
            Col {
                space: Space::Hwb,
                c: [origin_hue(&w[0])?, frac(&w[1])?, frac(&w[2])?],
                alpha,
            }
        }
        "lab" | "oklab" if !legacy => {
            let (lp, ab) = if name == "lab" {
                (100.0, 125.0)
            } else {
                (1.0, 0.4)
            };
            Col {
                space: if name == "lab" {
                    Space::Lab
                } else {
                    Space::Oklab
                },
                c: [
                    origin_value(&w[0], lp)?.clamp(0.0, lp),
                    origin_value(&w[1], ab)?,
                    origin_value(&w[2], ab)?,
                ],
                alpha,
            }
        }
        "lch" | "oklch" if !legacy => {
            let (lp, cp) = if name == "lch" {
                (100.0, 150.0)
            } else {
                (1.0, 0.4)
            };
            Col {
                space: if name == "lch" {
                    Space::Lch
                } else {
                    Space::Oklch
                },
                c: [
                    origin_value(&w[0], lp)?.clamp(0.0, lp),
                    origin_value(&w[1], cp)?.max(0.0),
                    origin_hue(&w[2])?.rem_euclid(360.0),
                ],
                alpha,
            }
        }
        _ => return None,
    })
}

/// The origin color: a keyword, a named color, a hex color or a color function.
fn origin(text: &str) -> Option<Col> {
    let srgb = |c: [f64; 4]| Col {
        space: Space::Srgb,
        c: [c[0], c[1], c[2]],
        alpha: c[3],
    };

    if text == "currentcolor" {
        return Some(srgb(CURRENT.with(Cell::get)));
    }
    if text == "transparent" {
        return Some(srgb([0.0, 0.0, 0.0, 0.0]));
    }
    if let Some(h) = text.strip_prefix('#') {
        return hex(h).map(srgb);
    }
    if let Some(body) = text.strip_suffix(')') {
        let open = body.find('(')?;
        return origin_function(&body[..open], &body[open + 1..]);
    }

    NAMED.iter().find(|(n, _)| *n == text).map(|(_, c)| {
        srgb([
            f64::from((c >> 16) & 0xff) / 255.0,
            f64::from((c >> 8) & 0xff) / 255.0,
            f64::from(c & 0xff) / 255.0,
            1.0,
        ])
    })
}

/// A computed relative color as an origin: a missing channel is 0.
fn resolve(r: &Res) -> Col {
    let c = r.c.map(|v| v.unwrap_or(0.0));
    Col {
        space: r.space,
        c,
        alpha: r.alpha.unwrap_or(0.0),
    }
}

// ---- the relative color function --------------------------------------------------------------

/// How a channel is read: its percentage scale and range, or an angle.
#[derive(Clone, Copy)]
enum Chan {
    /// A number or percentage (100% = `pct`), clamped to `lo..=hi`.
    Value {
        pct: f64,
        lo: f64,
        hi: f64,
    },
    Hue,
}

const FREE: f64 = f64::INFINITY;

fn finite(v: f64) -> f64 {
    // Chrome keeps a channel as a 32-bit float; NaN becomes 0 and infinity the largest float.
    if v.is_nan() {
        0.0
    } else {
        f64::from(v.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32)
    }
}

/// `value * scale` as a 32-bit float, the keyword value of a channel shown in other units.
fn scaled(value: f64, scale: f64) -> f64 {
    f64::from(value as f32 * scale as f32)
}

/// `value / scale` as a 32-bit float, back to the unit of the color.
fn unscaled(value: f64, scale: f64) -> f64 {
    f64::from(value as f32 / scale as f32)
}

fn channel(token: &str, kw: &[(&str, f64)], kind: Chan) -> Option<Option<f64>> {
    let v = component(token, kw)?;

    match (kind, v) {
        (_, V::None) => Some(None),
        (Chan::Hue, V::Num(x) | V::Angle(x)) => Some(Some(finite(x).rem_euclid(360.0))),
        (Chan::Value { lo, hi, .. }, V::Num(x)) => Some(Some(finite(x).clamp(lo, hi))),
        (Chan::Value { pct, lo, hi }, V::Pct(x)) => {
            Some(Some(finite(x / 100.0 * pct).clamp(lo, hi)))
        }
        _ => None,
    }
}

fn relative_result(name: &str, inner: &str) -> Option<Res> {
    let toks = tokenize(inner)?;
    let [first, origin_text, rest @ ..] = toks.as_slice() else {
        return None;
    };

    if first != "from" {
        return None;
    }

    let (target, rest) = if name == "color" {
        let [space, rest @ ..] = rest else {
            return None;
        };
        (space_of(space)?, rest)
    } else {
        (
            match name {
                "rgb" | "rgba" => Space::Srgb,
                "hsl" | "hsla" => Space::Hsl,
                "hwb" => Space::Hwb,
                "lab" => Space::Lab,
                "lch" => Space::Lch,
                "oklab" => Space::Oklab,
                "oklch" => Space::Oklch,
                _ => return None,
            },
            rest,
        )
    };

    // The names `rgba()` and `hsla()` are aliases of the functions.
    let (channels, alpha_token) = match rest {
        [a, b, c] => ([a, b, c], None),
        [a, b, c, slash, al] if slash == "/" => ([a, b, c], Some(al)),
        _ => return None,
    };

    let col = convert(origin(origin_text)?, target);
    let v = col.c;

    // Keywords: name and value, and how each channel is read.
    let (names, values, kinds): ([&str; 3], [f64; 3], [Chan; 3]) = match target {
        Space::Srgb if name != "color" => (
            ["r", "g", "b"],
            v.map(|x| scaled(x, 255.0)),
            [Chan::Value {
                pct: 255.0,
                lo: -FREE,
                hi: FREE,
            }; 3],
        ),
        Space::Hsl | Space::Hwb => {
            let second = if target == Space::Hsl {
                ["h", "s", "l"]
            } else {
                ["h", "w", "b"]
            };
            (
                second,
                [v[0], scaled(v[1], 100.0), scaled(v[2], 100.0)],
                [
                    Chan::Hue,
                    Chan::Value {
                        pct: 100.0,
                        lo: -FREE,
                        hi: FREE,
                    },
                    Chan::Value {
                        pct: 100.0,
                        lo: -FREE,
                        hi: FREE,
                    },
                ],
            )
        }
        Space::Lab | Space::Oklab => {
            let (lp, ab) = if target == Space::Lab {
                (100.0, 125.0)
            } else {
                (1.0, 0.4)
            };
            (
                ["l", "a", "b"],
                v,
                [
                    Chan::Value {
                        pct: lp,
                        lo: 0.0,
                        hi: lp,
                    },
                    Chan::Value {
                        pct: ab,
                        lo: -FREE,
                        hi: FREE,
                    },
                    Chan::Value {
                        pct: ab,
                        lo: -FREE,
                        hi: FREE,
                    },
                ],
            )
        }
        Space::Lch | Space::Oklch => {
            let (lp, cp) = if target == Space::Lch {
                (100.0, 150.0)
            } else {
                (1.0, 0.4)
            };
            (
                ["l", "c", "h"],
                v,
                [
                    Chan::Value {
                        pct: lp,
                        lo: 0.0,
                        hi: lp,
                    },
                    Chan::Value {
                        pct: cp,
                        lo: 0.0,
                        hi: FREE,
                    },
                    Chan::Hue,
                ],
            )
        }
        Space::XyzD50 | Space::XyzD65 => (
            ["x", "y", "z"],
            v,
            [Chan::Value {
                pct: 1.0,
                lo: -FREE,
                hi: FREE,
            }; 3],
        ),
        _ => (
            ["r", "g", "b"],
            v,
            [Chan::Value {
                pct: 1.0,
                lo: -FREE,
                hi: FREE,
            }; 3],
        ),
    };

    let mut kw: Vec<(&str, f64)> = names.iter().copied().zip(values).collect();
    kw.push(("alpha", col.alpha));

    let mut c = [None; 3];
    for i in 0..3 {
        c[i] = channel(channels[i], &kw, kinds[i])?;
    }

    let alpha = match alpha_token {
        None => Some(col.alpha),
        Some(t) => channel(
            t,
            &kw,
            Chan::Value {
                pct: 1.0,
                lo: 0.0,
                hi: 1.0,
            },
        )?,
    };
    let alpha = alpha.map(|a| a.clamp(0.0, 1.0));

    Some(match target {
        Space::Srgb if name != "color" => Res {
            space: Space::Srgb,
            c: c.map(|x| x.map(|x| unscaled(x, 255.0))),
            alpha,
        },
        Space::Hsl | Space::Hwb => {
            let [h, a, b] = c.map(|x| x.unwrap_or(0.0));
            let rgb = if target == Space::Hsl {
                hsl_to_srgb(h, unscaled(a, 100.0), unscaled(b, 100.0))
            } else {
                hwb_to_srgb(h, unscaled(a, 100.0), unscaled(b, 100.0))
            };
            Res {
                space: Space::Srgb,
                c: rgb.map(Some),
                alpha,
            }
        }
        _ => Res {
            space: target,
            c,
            alpha,
        },
    })
}

/// `X3DParser.convertColor` of `<name>(<inner>)` where `inner` starts with `from`; `None` where
/// Chrome rejects it.
pub(super) fn relative_color(name: &str, inner: &str) -> Option<[f64; 4]> {
    let r = relative_result(name, inner)?;
    let alpha = match r.alpha {
        Some(1.0) => None,
        a => Some(a),
    };
    let glued = !matches!(
        r.space,
        Space::Srgb
            | Space::SrgbLinear
            | Space::DisplayP3
            | Space::A98
            | Space::ProPhoto
            | Space::Rec2020
            | Space::XyzD50
            | Space::XyzD65
    );

    Some(unparsed_function(&r.c, glued, alpha))
}
