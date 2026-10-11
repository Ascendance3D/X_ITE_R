//! fdlibm 5.3 `sin`, `cos`, `acos`, `exp`, `log`, `log10`, as used by V8 (src/base/ieee754.cc)
//! for `Math.*`. V8 keeps the original fdlibm kernels, which differ in the last bit from the
//! FreeBSD/musl versions in the `libm` crate for some inputs (e.g. `cos (0.1)`).
//!
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//! Developed at SunSoft, a Sun Microsystems, Inc. business. Permission to use, copy, modify,
//! and distribute this software is freely granted, provided that this notice is preserved.

fn high(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

fn low(x: f64) -> u32 {
    x.to_bits() as u32
}

fn from_words(hi: u32, lo: u32) -> f64 {
    f64::from_bits(((hi as u64) << 32) | lo as u64)
}

fn with_high(x: f64, hi: u32) -> f64 {
    from_words(hi, low(x))
}

fn with_low(x: f64, lo: u32) -> f64 {
    from_words(high(x) as u32, lo)
}

fn kernel_sin(x: f64, y: f64, iy: i32) -> f64 {
    const S1: f64 = -1.66666666666666324348e-01;
    const S2: f64 = 8.33333333332248946124e-03;
    const S3: f64 = -1.98412698298579493134e-04;
    const S4: f64 = 2.75573137070700676789e-06;
    const S5: f64 = -2.50507602534068634195e-08;
    const S6: f64 = 1.58969099521155010221e-10;

    let ix = high(x) & 0x7fffffff;

    if ix < 0x3e400000 && x as i32 == 0 {
        return x;
    }

    let z = x * x;
    let v = z * x;
    let r = S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)));

    if iy == 0 { x + v * (S1 + z * r) } else { x - ((z * (0.5 * y - v * r) - y) - v * S1) }
}

fn kernel_cos(x: f64, y: f64) -> f64 {
    const C1: f64 = 4.16666666666666019037e-02;
    const C2: f64 = -1.38888888888741095749e-03;
    const C3: f64 = 2.48015872894767294178e-05;
    const C4: f64 = -2.75573143513906633035e-07;
    const C5: f64 = 2.08757232129817482790e-09;
    const C6: f64 = -1.13596475577881948265e-11;

    let ix = high(x) & 0x7fffffff;

    if ix < 0x3e400000 && x as i32 == 0 {
        return 1.0;
    }

    let z = x * x;
    let r = z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))));

    if ix < 0x3FD33333 {
        1.0 - (0.5 * z - (z * r - x * y))
    } else {
        let qx = if ix > 0x3fe90000 { 0.28125 } else { from_words((ix - 0x00200000) as u32, 0) };
        let hz = 0.5 * z - qx;
        let a = 1.0 - qx;

        a - (hz - (z * r - x * y))
    }
}

const NPIO2_HW: [i32; 32] = [
    0x3FF921FB, 0x400921FB, 0x4012D97C, 0x401921FB, 0x401F6A7A, 0x4022D97C, 0x4025FDBB, 0x402921FB, 0x402C463A, 0x402F6A7A,
    0x4031475C, 0x4032D97C, 0x40346B9C, 0x4035FDBB, 0x40378FDB, 0x403921FB, 0x403AB41B, 0x403C463A, 0x403DD85A, 0x403F6A7A,
    0x40407E4C, 0x4041475C, 0x4042106C, 0x4042D97C, 0x4043A28C, 0x40446B9C, 0x404534AC, 0x4045FDBB, 0x4046C6CB, 0x40478FDB,
    0x404858EB, 0x404921FB,
];

/// __ieee754_rem_pio2 for |x| <= 2^19·π/2. Larger arguments return `None` (the
/// multi-precision __kernel_rem_pio2 path is not ported).
fn rem_pio2(x: f64) -> Option<(i32, f64, f64)> {
    const INVPIO2: f64 = 6.36619772367581382433e-01;
    const PIO2_1: f64 = 1.57079632673412561417e+00;
    const PIO2_1T: f64 = 6.07710050650619224932e-11;
    const PIO2_2: f64 = 6.07710050630396597660e-11;
    const PIO2_2T: f64 = 2.02226624879595063154e-21;
    const PIO2_3: f64 = 2.02226624871116645580e-21;
    const PIO2_3T: f64 = 8.47842766036889956997e-32;

    let hx = high(x);
    let ix = hx & 0x7FFFFFFF;

    if ix <= 0x3FE921FB {
        return Some((0, x, 0.0));
    }

    if ix < 0x4002D97C {
        if hx > 0 {
            let mut z = x - PIO2_1;

            if ix != 0x3FF921FB {
                let y0 = z - PIO2_1T;
                return Some((1, y0, (z - y0) - PIO2_1T));
            }

            z -= PIO2_2;
            let y0 = z - PIO2_2T;
            return Some((1, y0, (z - y0) - PIO2_2T));
        }

        let mut z = x + PIO2_1;

        if ix != 0x3FF921FB {
            let y0 = z + PIO2_1T;
            return Some((-1, y0, (z - y0) + PIO2_1T));
        }

        z += PIO2_2;
        let y0 = z + PIO2_2T;
        return Some((-1, y0, (z - y0) + PIO2_2T));
    }

    if ix <= 0x413921FB {
        let t = x.abs();
        let n = (t * INVPIO2 + 0.5) as i32;
        let fn_ = n as f64;
        let mut r = t - fn_ * PIO2_1;
        let mut w = fn_ * PIO2_1T;
        let y0;

        if n < 32 && ix != NPIO2_HW[(n - 1) as usize] {
            y0 = r - w;
        } else {
            let j = ix >> 20;
            let mut yy = r - w;
            let mut i = j - ((high(yy) >> 20) & 0x7FF);

            if i > 16 {
                let t = r;
                w = fn_ * PIO2_2;
                r = t - w;
                w = fn_ * PIO2_2T - ((t - r) - w);
                yy = r - w;
                i = j - ((high(yy) >> 20) & 0x7FF);

                if i > 49 {
                    let t = r;
                    w = fn_ * PIO2_3;
                    r = t - w;
                    w = fn_ * PIO2_3T - ((t - r) - w);
                    yy = r - w;
                }
            }

            y0 = yy;
        }

        let y1 = (r - y0) - w;

        return Some(if hx < 0 { (-n, -y0, -y1) } else { (n, y0, y1) });
    }

    if ix >= 0x7FF00000 {
        return Some((0, x - x, x - x));
    }

    None
}

pub fn sin(x: f64) -> f64 {
    let ix = high(x) & 0x7fffffff;

    if ix <= 0x3fe921fb {
        return kernel_sin(x, 0.0, 0);
    }

    if ix >= 0x7ff00000 {
        return x - x;
    }

    let Some((n, y0, y1)) = rem_pio2(x) else {
        return libm::sin(x);
    };

    match n & 3 {
        0 => kernel_sin(y0, y1, 1),
        1 => kernel_cos(y0, y1),
        2 => -kernel_sin(y0, y1, 1),
        _ => -kernel_cos(y0, y1),
    }
}

pub fn cos(x: f64) -> f64 {
    let ix = high(x) & 0x7fffffff;

    if ix <= 0x3fe921fb {
        return kernel_cos(x, 0.0);
    }

    if ix >= 0x7ff00000 {
        return x - x;
    }

    let Some((n, y0, y1)) = rem_pio2(x) else {
        return libm::cos(x);
    };

    match n & 3 {
        0 => kernel_cos(y0, y1),
        1 => -kernel_sin(y0, y1, 1),
        2 => -kernel_cos(y0, y1),
        _ => kernel_sin(y0, y1, 1),
    }
}

pub fn acos(x: f64) -> f64 {
    const PI: f64 = 3.14159265358979311600e+00;
    const PIO2_HI: f64 = 1.57079632679489655800e+00;
    const PIO2_LO: f64 = 6.12323399573676603587e-17;
    const PS0: f64 = 1.66666666666666657415e-01;
    const PS1: f64 = -3.25565818622400915405e-01;
    const PS2: f64 = 2.01212532134862925881e-01;
    const PS3: f64 = -4.00555345006794114027e-02;
    const PS4: f64 = 7.91534994289814532176e-04;
    const PS5: f64 = 3.47933107596021167570e-05;
    const QS1: f64 = -2.40339491173441421878e+00;
    const QS2: f64 = 2.02094576023350569471e+00;
    const QS3: f64 = -6.88283971605453293030e-01;
    const QS4: f64 = 7.70381505559019352791e-02;

    let hx = high(x);
    let ix = hx & 0x7FFFFFFF;

    let pq = |z: f64| {
        let p = z * (PS0 + z * (PS1 + z * (PS2 + z * (PS3 + z * (PS4 + z * PS5)))));
        let q = 1.0 + z * (QS1 + z * (QS2 + z * (QS3 + z * QS4)));
        p / q
    };

    if ix >= 0x3FF00000 {
        if ((ix - 0x3FF00000) as u32 | low(x)) == 0 {
            return if hx > 0 { 0.0 } else { PI + 2.0 * PIO2_LO };
        }

        return (x - x) / (x - x);
    }

    if ix < 0x3FE00000 {
        if ix <= 0x3C600000 {
            return PIO2_HI + PIO2_LO;
        }

        let r = pq(x * x);
        return PIO2_HI - (x - (PIO2_LO - x * r));
    }

    if hx < 0 {
        let z = (1.0 + x) * 0.5;
        let s = z.sqrt();
        let r = pq(z);
        let w = r * s - PIO2_LO;
        return PI - 2.0 * (s + w);
    }

    let z = (1.0 - x) * 0.5;
    let s = z.sqrt();
    let df = with_low(s, 0);
    let c = (z - df * df) / (s + df);
    let r = pq(z);
    let w = r * s + c;

    2.0 * (df + w)
}

const TWO54: f64 = 1.80143985094819840000e+16;

pub fn log(x: f64) -> f64 {
    const LN2_HI: f64 = 6.93147180369123816490e-01;
    const LN2_LO: f64 = 1.90821492927058770002e-10;
    const LG1: f64 = 6.666666666666735130e-01;
    const LG2: f64 = 3.999999999940941908e-01;
    const LG3: f64 = 2.857142874366239149e-01;
    const LG4: f64 = 2.222219843214978396e-01;
    const LG5: f64 = 1.818357216161805012e-01;
    const LG6: f64 = 1.531383769920937332e-01;
    const LG7: f64 = 1.479819860511658591e-01;

    let mut x = x;
    let mut hx = high(x);
    let lx = low(x);
    let mut k: i32 = 0;

    if hx < 0x00100000 {
        if ((hx & 0x7FFFFFFF) as u32 | lx) == 0 {
            return f64::NEG_INFINITY;
        }

        if hx < 0 {
            return f64::NAN;
        }

        k -= 54;
        x *= TWO54;
        hx = high(x);
    }

    if hx >= 0x7FF00000 {
        return x + x;
    }

    k += (hx >> 20) - 1023;
    hx &= 0x000FFFFF;

    let i = (hx + 0x95F64) & 0x100000;

    x = with_high(x, (hx | (i ^ 0x3FF00000)) as u32);
    k += i >> 20;

    let f = x - 1.0;

    if (0x000FFFFF & (2 + hx)) < 3 {
        if f == 0.0 {
            if k == 0 {
                return 0.0;
            }

            let dk = k as f64;
            return dk * LN2_HI + dk * LN2_LO;
        }

        let r = f * f * (0.5 - 0.33333333333333333 * f);

        if k == 0 {
            return f - r;
        }

        let dk = k as f64;
        return dk * LN2_HI - ((r - dk * LN2_LO) - f);
    }

    let s = f / (2.0 + f);
    let dk = k as f64;
    let z = s * s;
    let mut i = hx - 0x6147A;
    let w = z * z;
    let j = 0x6B851 - hx;
    let t1 = w * (LG2 + w * (LG4 + w * LG6));
    let t2 = z * (LG1 + w * (LG3 + w * (LG5 + w * LG7)));

    i |= j;

    let r = t2 + t1;

    if i > 0 {
        let hfsq = 0.5 * f * f;

        if k == 0 {
            f - (hfsq - s * (hfsq + r))
        } else {
            dk * LN2_HI - ((hfsq - (s * (hfsq + r) + dk * LN2_LO)) - f)
        }
    } else if k == 0 {
        f - s * (f - r)
    } else {
        dk * LN2_HI - ((s * (f - r) - dk * LN2_LO) - f)
    }
}

pub fn log10(x: f64) -> f64 {
    const IVLN10: f64 = 4.34294481903251816668e-01;
    const LOG10_2HI: f64 = 3.01029995663611771306e-01;
    const LOG10_2LO: f64 = 3.69423907715893078616e-13;

    let mut x = x;
    let mut hx = high(x);
    let mut lx = low(x);
    let mut k: i32 = 0;

    if hx < 0x00100000 {
        if ((hx & 0x7FFFFFFF) as u32 | lx) == 0 {
            return f64::NEG_INFINITY;
        }

        if hx < 0 {
            return f64::NAN;
        }

        k -= 54;
        x *= TWO54;
        hx = high(x);
        lx = low(x);
    }

    if hx >= 0x7FF00000 {
        return x + x;
    }

    if hx == 0x3FF00000 && lx == 0 {
        return 0.0;
    }

    k += (hx >> 20) - 1023;

    let i = ((k as u32 & 0x80000000) >> 31) as i32;

    hx = (hx & 0x000FFFFF) | ((0x3FF - i) << 20);

    let y = (k + i) as f64;

    x = from_words(hx as u32, lx);

    let z = y * LOG10_2LO + IVLN10 * log(x);

    z + y * LOG10_2HI
}

pub fn exp(x: f64) -> f64 {
    const O_THRESHOLD: f64 = 7.09782712893383973096e+02;
    const U_THRESHOLD: f64 = -7.45133219101941108420e+02;
    const LN2HI: [f64; 2] = [6.93147180369123816490e-01, -6.93147180369123816490e-01];
    const LN2LO: [f64; 2] = [1.90821492927058770002e-10, -1.90821492927058770002e-10];
    const HALF: [f64; 2] = [0.5, -0.5];
    const INVLN2: f64 = 1.44269504088896338700e+00;
    const P1: f64 = 1.66666666666666019037e-01;
    const P2: f64 = -2.77777777770155933842e-03;
    const P3: f64 = 6.61375632143793436117e-05;
    const P4: f64 = -1.65339022054652515390e-06;
    const P5: f64 = 4.13813679705723846039e-08;
    const E: f64 = std::f64::consts::E;
    const HUGE: f64 = 1.0e+300;
    const TWOM1000: f64 = 9.33263618503218878990e-302;
    const TWO1023: f64 = 8.988465674311579539e307;

    let mut x = x;
    let mut hx = high(x) as u32;
    let xsb = ((hx >> 31) & 1) as usize;
    let mut hi = 0.0;
    let mut lo = 0.0;
    let mut k: i32 = 0;

    hx &= 0x7FFFFFFF;

    if hx >= 0x40862E42 {
        if hx >= 0x7FF00000 {
            if ((hx & 0xFFFFF) | low(x)) != 0 {
                return x + x;
            }

            return if xsb == 0 { x } else { 0.0 };
        }

        if x > O_THRESHOLD {
            return HUGE * HUGE;
        }

        if x < U_THRESHOLD {
            return TWOM1000 * TWOM1000;
        }
    }

    if hx > 0x3FD62E42 {
        if hx < 0x3FF0A2B2 {
            // V8: special case exp(1) to return E exactly.
            if x == 1.0 {
                return E;
            }

            hi = x - LN2HI[xsb];
            lo = LN2LO[xsb];
            k = 1 - xsb as i32 - xsb as i32;
        } else {
            k = (INVLN2 * x + HALF[xsb]) as i32;

            let t = k as f64;

            hi = x - t * LN2HI[0];
            lo = t * LN2LO[0];
        }

        x = hi - lo;
    } else if hx < 0x3E300000 {
        if HUGE + x > 1.0 {
            return 1.0 + x;
        }
    } else {
        k = 0;
    }

    let t = x * x;
    let twopk = if k >= -1021 {
        from_words((0x3FF00000 + (k << 20)) as u32, 0)
    } else {
        from_words((0x3FF00000 + ((k + 1000) << 20)) as u32, 0)
    };
    let c = x - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));

    if k == 0 {
        return 1.0 - ((x * c) / (c - 2.0) - x);
    }

    let y = 1.0 - ((lo - (x * c) / (2.0 - c)) - hi);

    if k >= -1021 {
        if k == 1024 {
            return y * 2.0 * TWO1023;
        }

        y * twopk
    } else {
        y * twopk * TWOM1000
    }
}
