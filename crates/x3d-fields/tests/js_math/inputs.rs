// Inputs and output hash of the Chrome-flavor Math golden test (tests/js_math_golden.rs). The
// expected hashes there were made with CORE-MATH (the `core-math` crate 1.4.0) on these inputs.

/// Edge values every function sees, besides the generated ones.
pub const EDGES: &[f64] = &[
    0.0,
    -0.0,
    1.0,
    -1.0,
    0.5,
    -0.5,
    2.0,
    10.0,
    0.1,
    1e-300,
    -1e-300,
    5e-324,
    -5e-324,
    2.2250738585072014e-308,
    f64::MAX,
    f64::MIN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NAN,
    f64::EPSILON,
    std::f64::consts::PI,
    std::f64::consts::FRAC_PI_2,
    std::f64::consts::FRAC_PI_4,
    std::f64::consts::E,
    1e22,
    1e300,
    709.782712893384,
    -745.1332191019411,
    1.7976931348623157e308,
];

fn next(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

/// `n` inputs: even indices any bit pattern, odd indices uniform in [-range, range]
/// (|x| for `positive`), then the edges.
pub fn inputs(seed: u64, n: usize, range: f64, positive: bool) -> Vec<f64> {
    let mut s = seed;
    let mut v: Vec<f64> = (0..n)
        .map(|i| {
            if i % 2 == 0 {
                f64::from_bits(next(&mut s))
            } else {
                let x = ((next(&mut s) >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0) * range;
                if positive { x.abs() } else { x }
            }
        })
        .collect();
    v.extend_from_slice(EDGES);
    v
}

/// FNV-1a over the result bits, every NaN as one pattern.
pub fn hash(values: impl Iterator<Item = f64>) -> u64 {
    let mut h = 0xcbf29ce484222325u64;

    for x in values {
        let bits = if x.is_nan() {
            0x7ff8000000000000
        } else {
            x.to_bits()
        };

        for b in bits.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }

    h
}

/// The one-argument cases: name, seed, range, positive only.
pub const CASES: &[(&str, u64, f64, bool)] = &[
    ("sin", 1, 1e3, false),
    ("cos", 2, 1e3, false),
    ("tan", 3, 1e3, false),
    ("asin", 4, 1.0, false),
    ("acos", 5, 1.0, false),
    ("atan", 6, 1e3, false),
    ("exp", 7, 700.0, false),
    ("log", 8, 1e6, true),
    ("log10", 9, 1e6, true),
];

/// Inputs per case.
pub const N: usize = 1_000_000;

/// atan2 (y, x) pairs: consecutive generated values in [-100, 100] or any bit pattern, then every
/// pair of edges.
pub fn pairs() -> Vec<(f64, f64)> {
    let v = inputs(10, 2 * N, 100.0, false);
    let mut p: Vec<(f64, f64)> = v[..2 * N].chunks_exact(2).map(|c| (c[0], c[1])).collect();

    for &y in EDGES {
        for &x in EDGES {
            p.push((y, x));
        }
    }

    p
}
