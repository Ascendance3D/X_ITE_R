//! Distances of float32 values in ulps, for the tests that allow a last-digit difference to the GL oracle.

use serde_json::Value;

/// The distance of two float32 values in representable steps (`u64::MAX` for a NaN).
pub fn f32_ulps(a: f64, b: f64) -> u64 {
    let (a, b) = (a as f32, b as f32);

    if a.is_nan() || b.is_nan() {
        return u64::MAX;
    }

    // Order the bit patterns as numbers (negative values count down from zero).
    let key = |x: f32| {
        let bits = x.to_bits() as i64;

        if bits & 0x8000_0000 != 0 {
            -(bits & 0x7fff_ffff)
        } else {
            bits
        }
    };

    key(a).abs_diff(key(b))
}

/// `same_value`, with numbers allowed to differ by `max_ulps` float32 ulps; `seen` takes the largest distance found.
pub fn within_ulps(a: &Value, b: &Value, max_ulps: u64, seen: &mut u64) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            let d = if x == y { 0 } else { f32_ulps(x, y) };

            *seen = (*seen).max(d);
            d <= max_ulps
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|(p, q)| within_ulps(p, q, max_ulps, seen))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, p)| y.get(k).is_some_and(|q| within_ulps(p, q, max_ulps, seen)))
        }
        _ => a == b,
    }
}
