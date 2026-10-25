//! Number formatting identical to X_ITE's generator
//! (src/x_ite/InputOutput/X3DGenerator.js: createFloatFormat, FloatFormat, DoubleFormat).
//!
//! X_ITE uses `Intl.NumberFormat ("en", { maximumSignificantDigits, useGrouping: false })`,
//! with `notation: "standard"` or `"scientific"`. This is ICU's number formatting
//! (number_decimalquantity.cpp `DecimalQuantity::setToDouble` / `_setToDoubleFast` /
//! `roundToMagnitude`, "halfExpand"), ported including its fast path: a double is first turned
//! into an *approximate* decimal (`round (n * 10^fracLength)`, at most 16 digits) and only
//! converted to the shortest round-trip digits when the rounding digit lies in the zone of
//! uncertainty. That fast path is observable: Chrome prints 0.9999999999999994 with 15
//! significant digits as `1` (the product is exactly ...999.5), not `0.999999999999999`. The
//! port reproduces Chrome 155 on 62,100 formatted values (oracle/README.md, "Followers").

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

/// ICU `DecimalQuantity` as far as double rounding needs it: BCD digits (least significant
/// first), `scale` (power of ten of digit 0), the approximate-double flag and the source double.
struct Quantity {
    digits: Vec<u8>,
    scale: i32,
    approximate: bool,
    original: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Lower,
    Upper,
    Midpoint,
    LowerEdge,
    UpperEdge,
}

const DOUBLE_MULTIPLIERS: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

impl Quantity {
    fn precision(&self) -> i32 {
        self.digits.len() as i32
    }

    /// getDigitPos.
    fn digit(&self, position: i32) -> u8 {
        if position < 0 || position >= self.precision() {
            0
        } else {
            self.digits[position as usize]
        }
    }

    fn magnitude(&self) -> i32 {
        self.scale + self.precision() - 1
    }

    fn set_long(&mut self, n: u64) {
        self.digits = n.to_string().bytes().rev().map(|b| b - b'0').collect();
        self.scale = 0;
    }

    /// compact (): remove trailing zeros.
    fn compact(&mut self) {
        let zeros = self.digits.iter().take_while(|d| **d == 0).count();

        if zeros == self.digits.len() {
            self.digits.clear();
            self.scale = 0;
            self.approximate = false;
            return;
        }

        self.digits.drain(..zeros);
        self.scale += zeros as i32;
    }

    /// DecimalQuantity::setToDouble for a finite, positive, non-zero double.
    fn from_double(mut n: f64) -> Quantity {
        let mut q = Quantity {
            digits: vec![],
            scale: 0,
            approximate: true,
            original: n,
        };
        let exponent = ((n.to_bits() >> 52) & 0x7ff) as i32 - 0x3ff;

        if exponent <= 52 && n.trunc() == n && n < 9.3e18 {
            q.set_long(n as u64);
            q.compact();
            return q;
        }

        if exponent == -1023 || exponent == 1024 {
            q.accurate();
            return q;
        }

        // V8's `DoubleToAscii` divides by its own long literal for log2(10) (`conversions.cc`),
        // not by `f64::consts::LOG2_10`, and this port reproduces the literal so that
        // `frac_length` lands on the same integer V8 computes. The two differ beyond an f64's
        // precision, but `as i32` truncates, so a value one ULP away can fall on the other side
        // of an integer boundary and shift the decimal digits this function then emits.
        #[allow(
            clippy::approx_constant,
            clippy::excessive_precision,
            reason = "V8's own log2(10) literal, kept verbatim so the truncation matches"
        )]
        let frac_length =
            ((52 - exponent) as f64 / 3.32192809488736234787031942948939017586) as i32;

        if frac_length >= 0 {
            let mut i = frac_length;

            while i >= 22 {
                n *= 1e22;
                i -= 22;
            }

            n *= DOUBLE_MULTIPLIERS[i as usize];
        } else {
            let mut i = frac_length;

            while i <= -22 {
                n /= 1e22;
                i += 22;
            }

            n /= DOUBLE_MULTIPLIERS[(-i) as usize];
        }

        // uprv_round: half away from zero.
        let result = (n + 0.5).floor() as u64;

        if result != 0 {
            q.set_long(result);
            q.scale -= frac_length;
        }

        q.compact();
        q
    }

    /// convertToAccurateDouble: the shortest round-trip digits (setBcdToZero clears the flag).
    fn accurate(&mut self) {
        let (_, digits, exp) = shortest_digits(self.original);

        self.digits = digits.bytes().rev().map(|b| b - b'0').collect();
        self.scale = exp - (self.digits.len() as i32 - 1);
        self.approximate = false;
    }

    /// DecimalQuantity::roundToMagnitude (no nickel rounding, RoundingMode::HALF_UP).
    fn round_to_magnitude(&mut self, magnitude: i32) {
        let position = magnitude - self.scale;
        let trailing = self.digit(position);

        if position <= 0 && !self.approximate {
            return;
        }

        if self.digits.is_empty() {
            return;
        }

        let leading = self.digit(position - 1);
        let precision = self.precision();
        let section;

        if !self.approximate {
            section = if leading < 5 {
                Section::Lower
            } else if leading > 5 {
                Section::Upper
            } else {
                let mut s = Section::Midpoint;
                let mut p = position - 2;

                while p >= 0 {
                    if self.digit(p) != 0 {
                        s = Section::Upper;
                        break;
                    }

                    p -= 1;
                }

                s
            };
        } else {
            let mut p = position - 2;
            let min_p = (precision - 14).max(0);

            let mut s;

            if leading == 0 {
                s = Section::LowerEdge;

                while p >= min_p {
                    if self.digit(p) != 0 {
                        s = Section::Lower;
                        break;
                    }

                    p -= 1;
                }
            } else if leading == 4 {
                s = Section::Midpoint;

                while p >= min_p {
                    if self.digit(p) != 9 {
                        s = Section::Lower;
                        break;
                    }

                    p -= 1;
                }
            } else if leading == 5 {
                s = Section::Midpoint;

                while p >= min_p {
                    if self.digit(p) != 0 {
                        s = Section::Upper;
                        break;
                    }

                    p -= 1;
                }
            } else if leading == 9 {
                s = Section::UpperEdge;

                while p >= min_p {
                    if self.digit(p) != 9 {
                        s = Section::Upper;
                        break;
                    }

                    p -= 1;
                }
            } else if leading < 5 {
                s = Section::Lower;
            } else {
                s = Section::Upper;
            }

            // HALF_UP rounds at the midpoint, so only a midpoint section (or a position too deep
            // into the digits) needs the exact representation.
            if position - 1 < precision - 14 || s == Section::Midpoint {
                self.accurate();
                return self.round_to_magnitude(magnitude);
            }

            // The value is confirmed: turn off the approximate flag.
            self.approximate = false;

            if position <= 0 {
                return;
            }

            if s == Section::LowerEdge {
                s = Section::Lower;
            }

            if s == Section::UpperEdge {
                s = Section::Upper;
            }

            section = s;
        }

        // HALF_UP (halfExpand): everything but the lower section rounds up.
        let round_down = section == Section::Lower;

        // Truncation.
        if position >= precision {
            self.digits.clear();
            self.scale = magnitude;
        } else {
            self.digits.drain(..position as usize);
            self.scale += position;
        }

        // Bubble the result to the higher digits.
        if !round_down {
            if self.digits.is_empty() {
                self.digits.push(1);
            } else {
                let mut i = 0;

                loop {
                    if i == self.digits.len() {
                        self.digits.push(1);
                        break;
                    }

                    if self.digits[i] == 9 {
                        self.digits[i] = 0;
                        i += 1;
                    } else {
                        self.digits[i] += 1;
                        break;
                    }
                }
            }
        }

        let _ = trailing;
        self.compact();
    }
}

/// `Intl.NumberFormat` with `maximumSignificantDigits: precision` applied to |value| (finite,
/// non-zero): the decimal digits (trailing zeros stripped) and the exponent of the first digit.
fn icu_digits(value: f64, precision: usize) -> (String, i32) {
    let mut q = Quantity::from_double(value.abs());
    let magnitude = q.magnitude();

    q.round_to_magnitude(magnitude - precision as i32 + 1);

    if q.digits.is_empty() {
        return ("0".into(), 0);
    }

    let digits: String = q.digits.iter().rev().map(|d| (b'0' + d) as char).collect();
    (digits, q.magnitude())
}

/// `Intl.NumberFormat ("en", { notation: "standard", maximumSignificantDigits: precision, useGrouping: false })`.
pub fn intl_standard(value: f64, precision: usize) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }

    let negative = value.is_sign_negative();
    let (digits, exp) = icu_digits(value, precision);
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
        return if value.is_sign_negative() {
            "-0e0".into()
        } else {
            "0e0".into()
        };
    }

    let negative = value.is_sign_negative();
    let (digits, exp) = icu_digits(value, precision);
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
