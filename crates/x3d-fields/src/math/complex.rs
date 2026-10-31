//! Complex (standard/Math/Numbers/Complex.js): only the value type and its `toString`.

use crate::jsmath;

/// Complex.js: `x` is the real part, `y` the imaginary part.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Complex {
    pub x: f64,
    pub y: f64,
}

impl Complex {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Complex.toString: the real part when it is nonzero or the imaginary part is falsy (zero or NaN), a "+" when both are
/// truthy and the imaginary part is positive, then the imaginary part and "i" when it is truthy (nonzero, not NaN).
impl std::fmt::Display for Complex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (x, y) = (self.x, self.y);
        let truthy = |v: f64| v != 0.0 && !v.is_nan();
        let mut s = String::new();
        if truthy(x) || !truthy(y) {
            s.push_str(&jsmath::number_to_string(x));
        }
        if truthy(x) && y > 0.0 {
            s.push('+');
        }
        if truthy(y) {
            s.push_str(&jsmath::number_to_string(y));
            s.push('i');
        }
        f.write_str(&s)
    }
}
