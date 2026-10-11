//! JavaScript `Math` and number-conversion semantics that X_ITE relies on.
//!
//! Two flavors, because the two oracles disagree in the last bit:
//!
//! * [`MathFlavor::Node`] (default): V8 as embedded in node — `Math.sin`, `cos`, `acos`, `exp`,
//!   `log`, `log10` are the original fdlibm 5.3 algorithms (ported in `fdlibm.rs`, bit-exact in
//!   the X1 oracle tests, which run in node); `tan`, `asin`, `atan2` use the `libm` crate.
//! * [`MathFlavor::Chrome`]: headless Chromium (155), where the X0/X1/items dump oracles run.
//!   Its `Math.sin/cos/tan/asin/acos/atan2/exp/log/log10` are *correctly rounded*: on 100,000
//!   random inputs per function Chrome's results equal the CORE-MATH library (the `core-math`
//!   crate) bit for bit (0 differences), and equal an arbitrary-precision reference on 40,000
//!   inputs. glibc (Rust's `f64::sin` ...) differs from Chrome in 0.05% to 0.25% of inputs (log10:
//!   4%), node's fdlibm in 3% to 12%. Selected with [`set_flavor`] by the runtime; see
//!   oracle/README.md ("Followers"). Found through a last-digit rotation axis in a
//!   OrientationChaser trace.
//!
//! `Math.hypot` is V8's own algorithm in both, ported here.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathFlavor {
    Node = 0,
    Chrome = 1,
}

static FLAVOR: AtomicU8 = AtomicU8::new(0);

/// Select the JavaScript engine whose Math functions are reproduced (process-wide).
pub fn set_flavor(f: MathFlavor) {
    FLAVOR.store(f as u8, Ordering::Relaxed);
}

pub fn flavor() -> MathFlavor {
    if FLAVOR.load(Ordering::Relaxed) == 1 { MathFlavor::Chrome } else { MathFlavor::Node }
}

fn chrome() -> bool {
    FLAVOR.load(Ordering::Relaxed) == 1
}

pub fn sin(x: f64) -> f64 {
    if chrome() { core_math::sin(x) } else { crate::fdlibm::sin(x) }
}

pub fn cos(x: f64) -> f64 {
    if chrome() { core_math::cos(x) } else { crate::fdlibm::cos(x) }
}

pub fn exp(x: f64) -> f64 {
    if chrome() { core_math::exp(x) } else { crate::fdlibm::exp(x) }
}

pub fn log(x: f64) -> f64 {
    if chrome() { core_math::log(x) } else { crate::fdlibm::log(x) }
}

pub fn tan(x: f64) -> f64 {
    if chrome() { core_math::tan(x) } else { libm::tan(x) }
}

pub fn acos(x: f64) -> f64 {
    if chrome() { core_math::acos(x) } else { crate::fdlibm::acos(x) }
}

pub fn asin(x: f64) -> f64 {
    if chrome() { core_math::asin(x) } else { libm::asin(x) }
}

pub fn atan2(y: f64, x: f64) -> f64 {
    if chrome() { core_math::atan2(y, x) } else { libm::atan2(y, x) }
}

pub fn log10(x: f64) -> f64 {
    if chrome() { core_math::log10(x) } else { crate::fdlibm::log10(x) }
}

pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

/// V8 `Math.hypot` (src/builtins/math.tq: MathHypot): scaled sum of squares with Kahan
/// compensation. Differs from libm `hypot` in the last bit for some inputs.
pub fn hypot(values: &[f64]) -> f64 {
    let mut max = 0.0f64;
    let mut one_arg_is_nan = false;

    for &v in values {
        let a = v.abs();

        if a.is_nan() {
            one_arg_is_nan = true;
        } else if a > max {
            max = a;
        }
    }

    if max == f64::INFINITY {
        return f64::INFINITY;
    }

    if one_arg_is_nan {
        return f64::NAN;
    }

    if max == 0.0 {
        return 0.0;
    }

    let mut sum = 0.0f64;
    let mut compensation = 0.0f64;

    for &v in values {
        let n = v.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }

    sum.sqrt() * max
}

/// JS `Math.min (Math.max (value, min), max)` (standard/Math/Algorithm.js: clamp).
/// NaN propagates like in JS.
pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    js_min(js_max(value, min), max)
}

pub fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == b {
        // Math.max (-0, 0) === 0
        if a == 0.0 && (a.is_sign_negative() != b.is_sign_negative()) {
            return 0.0;
        }
        return a;
    }
    if a > b { a } else { b }
}

pub fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == b {
        if a == 0.0 && (a.is_sign_negative() != b.is_sign_negative()) {
            return -0.0;
        }
        return a;
    }
    if a < b { a } else { b }
}

/// standard/Math/Algorithm.js: interval. JS `%` is C `fmod`, same as Rust `%` on f64.
pub fn interval(value: f64, low: f64, high: f64) -> f64 {
    if value >= high {
        return ((value - low) % (high - low)) + low;
    }

    if value < low {
        return ((value - high) % (high - low)) + high;
    }

    value
}

/// ECMAScript ToInt32 (`value | 0`).
pub fn to_int32(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }

    let t = value.trunc();
    let m = t.rem_euclid(4294967296.0);
    let u = m as u64 as u32;

    u as i32
}

/// ECMAScript ToUint8 as used by `Uint8Array` stores of booleans/numbers.
pub fn to_uint8(value: f64) -> u8 {
    if !value.is_finite() {
        return 0;
    }

    (value.trunc().rem_euclid(256.0)) as u8
}

/// `Math.fround`.
pub fn fround(value: f64) -> f64 {
    value as f32 as f64
}

/// `String (number)` for the cases X_ITE prints without a formatter (integers, and the
/// non-finite fallbacks of FloatFormat/DoubleFormat).
pub fn number_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value == f64::INFINITY {
        return "Infinity".into();
    }
    if value == f64::NEG_INFINITY {
        return "-Infinity".into();
    }
    if value == 0.0 {
        return "0".into();
    }

    // ECMAScript Number::toString (radix 10): shortest round-trip digits, with the
    // 1e21 / 1e-7 switch to exponential notation.
    let (negative, digits, exp) = crate::format::shortest_digits(value);
    let k = digits.len() as i32;
    let n = exp + 1; // position of the decimal point relative to the digit string
    let mut out = String::new();

    if negative {
        out.push('-');
    }

    if k <= n && n <= 21 {
        out.push_str(&digits);
        for _ in 0..(n - k) {
            out.push('0');
        }
    } else if 0 < n && n <= 21 {
        out.push_str(&digits[..n as usize]);
        out.push('.');
        out.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        for _ in 0..(-n) {
            out.push('0');
        }
        out.push_str(&digits);
    } else {
        let e = n - 1;
        out.push_str(&digits[..1]);
        if k > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if e >= 0 { '+' } else { '-' });
        out.push_str(&e.abs().to_string());
    }

    out
}
