//! The part of fdlibm that V8's `Math.sin`/`Math.cos` (base::ieee754) runs for |x| <= pi/4, which is
//! every argument Bullet's `integrateTransform` produces (the half angle is limited to 0.393).
//! glibc's sin/cos differ from it in the last place for a few arguments, and the rotation matrix
//! built from them can amplify that into a visible float difference. Larger arguments fall back to
//! the platform libm.

const S1: f64 = -1.666_666_666_666_663_243_48e-01;
const S2: f64 = 8.333_333_333_322_489_461_24e-03;
const S3: f64 = -1.984_126_982_985_794_931_34e-04;
const S4: f64 = 2.755_731_370_707_006_767_89e-06;
const S5: f64 = -2.505_076_025_340_686_341_95e-08;
const S6: f64 = 1.589_690_995_211_550_102_21e-10;

const C1: f64 = 4.166_666_666_666_660_190_37e-02;
const C2: f64 = -1.388_888_888_887_410_957_49e-03;
const C3: f64 = 2.480_158_728_947_672_941_78e-05;
const C4: f64 = -2.755_731_435_139_066_330_35e-07;
const C5: f64 = 2.087_572_321_298_174_827_90e-09;
const C6: f64 = -1.135_964_755_778_819_482_65e-11;

fn high_word(x: f64) -> u32 {
    (x.to_bits() >> 32) as u32 & 0x7FFF_FFFF
}

/// __kernel_sin (x, 0, 0).
fn kernel_sin(x: f64) -> f64 {
    let ix = high_word(x);

    if ix < 0x3E40_0000 && x as i32 == 0 {
        return x;
    }

    let z = x * x;
    let v = z * x;
    let r = S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)));
    x + v * (S1 + z * r)
}

/// __kernel_cos (x, 0).
fn kernel_cos(x: f64) -> f64 {
    let ix = high_word(x);

    if ix < 0x3E40_0000 && x as i32 == 0 {
        return 1.0;
    }

    let z = x * x;
    let r = z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))));

    if ix < 0x3FD3_3333 {
        1.0 - (0.5 * z - (z * r))
    } else {
        let qx = if ix > 0x3FE9_0000 {
            0.28125
        } else {
            f64::from_bits(((ix - 0x0020_0000) as u64) << 32)
        };
        let iz = 0.5 * z - qx;
        let a = 1.0 - qx;
        a - (iz - (z * r))
    }
}

pub fn sin(x: f64) -> f64 {
    if high_word(x) <= 0x3FE9_21FB {
        kernel_sin(x)
    } else {
        x.sin()
    }
}

pub fn cos(x: f64) -> f64 {
    if high_word(x) <= 0x3FE9_21FB {
        kernel_cos(x)
    } else {
        x.cos()
    }
}
