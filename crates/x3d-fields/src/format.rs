//! Number formatting identical to X_ITE's generator
//! (src/x_ite/InputOutput/X3DGenerator.js: createFloatFormat, FloatFormat, DoubleFormat).
//!
//! X_ITE uses `Intl.NumberFormat ("en", { maximumSignificantDigits, useGrouping: false })`,
//! with `notation: "standard"` or `"scientific"`. ICU starts from the shortest round-trip
//! decimal representation of the double and rounds it half-away-from-zero ("halfExpand") to
//! the requested number of significant digits. That is what this module does.

use crate::jsmath;

/// Shortest round-trip decimal digits of |value| (no leading/trailing zeros) and the decimal
/// exponent of the first digit, i.e. value = 0.d1d2d3… × 10^(exp+1) = d1.d2d3… × 10^exp.
/// `value` must be finite and non-zero.
pub fn shortest_digits(value: f64) -> (bool, String, i32) {
    let negative = value.is_sign_negative();
    // Rust's `{:e}` prints the shortest representation that round-trips.
    let s = format!("{:e}", value.abs());
    let (mantissa, exp) = s.split_once('e').expect("exponent");
    let exp: i32 = exp.parse().expect("exponent value");
    let mut digits: String = mantissa.chars().filter(|c| *c != '.').collect();

    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }

    (negative, digits, exp)
}

/// Round a digit string to at most `precision` significant digits, half away from zero.
/// Returns the new digits (trailing zeros stripped) and the possibly incremented exponent.
fn round_digits(digits: &str, exp: i32, precision: usize) -> (String, i32) {
    if digits.len() <= precision {
        return (digits.to_string(), exp);
    }

    let mut kept: Vec<u8> = digits.as_bytes()[..precision].to_vec();
    let round_up = digits.as_bytes()[precision] >= b'5';
    let mut exp = exp;

    if round_up {
        let mut i = precision;

        loop {
            if i == 0 {
                kept.insert(0, b'1');
                kept.pop();
                exp += 1;
                break;
            }

            i -= 1;

            if kept[i] == b'9' {
                kept[i] = b'0';
            } else {
                kept[i] += 1;
                break;
            }
        }
    }

    while kept.len() > 1 && *kept.last().unwrap() == b'0' {
        kept.pop();
    }

    (String::from_utf8(kept).unwrap(), exp)
}

/// `Intl.NumberFormat ("en", { notation: "standard", maximumSignificantDigits: precision, useGrouping: false })`.
pub fn intl_standard(value: f64, precision: usize) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() { "-0".into() } else { "0".into() };
    }

    let (negative, digits, exp) = shortest_digits(value);
    let (digits, exp) = round_digits(&digits, exp, precision);
    let mut out = String::new();

    if negative {
        out.push('-');
    }

    if exp >= 0 {
        let int_len = (exp + 1) as usize;

        if digits.len() <= int_len {
            out.push_str(&digits);
            for _ in digits.len()..int_len {
                out.push('0');
            }
        } else {
            out.push_str(&digits[..int_len]);
            out.push('.');
            out.push_str(&digits[int_len..]);
        }
    } else {
        out.push_str("0.");
        for _ in 0..(-exp - 1) {
            out.push('0');
        }
        out.push_str(&digits);
    }

    out
}

/// `Intl.NumberFormat ("en", { notation: "scientific", maximumSignificantDigits: precision, useGrouping: false })`,
/// then lower-cased like X_ITE does (`1.5E-5` → `1.5e-5`).
pub fn intl_scientific_lower(value: f64, precision: usize) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() { "-0e0".into() } else { "0e0".into() };
    }

    let (negative, digits, exp) = shortest_digits(value);
    let (digits, exp) = round_digits(&digits, exp, precision);
    let mut out = String::new();

    if negative {
        out.push('-');
    }

    out.push_str(&digits[..1]);

    if digits.len() > 1 {
        out.push('.');
        out.push_str(&digits[1..]);
    }

    out.push('e');
    out.push_str(&exp.to_string());
    out
}

/// X3DGenerator.js: FloatFormat / DoubleFormat with the given precision.
pub fn format_number(value: f64, precision: usize) -> String {
    if value.is_finite() {
        let exponent = jsmath::log10(value.abs());

        if ((precision as f64) > exponent && exponent >= -4.0) || value == 0.0 {
            return intl_standard(value, precision);
        }

        intl_scientific_lower(value, precision)
    } else {
        jsmath::number_to_string(value)
    }
}

/// Default generator precision for single-precision fields.
pub const FLOAT_PRECISION: usize = 7;
/// Default generator precision for double-precision fields.
pub const DOUBLE_PRECISION: usize = 15;

pub fn float_format(value: f64) -> String {
    format_number(value, FLOAT_PRECISION)
}

pub fn double_format(value: f64) -> String {
    format_number(value, DOUBLE_PRECISION)
}
