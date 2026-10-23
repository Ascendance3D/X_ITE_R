//! LinearMath of Bullet 2.82 (zlib license, Erwin Coumans): btVector3, btQuaternion, btMatrix3x3 and
//! btTransform, scalar (non-SSE) code paths, as compiled into ammo.js.
//!
//! ## Rounding convention
//!
//! ammo.js is asm.js without `Math.fround`: every `float` expression is evaluated in double
//! precision and a value is rounded to float only when it is stored to the heap. All functions here
//! return the unrounded double result. The caller rounds ([`r3`], [`rm`], [`rq`], [`rt`]) where
//! Bullet stores the value in a member, an array or an escaping object, exactly like `bullet.rs`.
//! Locals that stay in registers after inlining are not rounded.

pub type V3 = [f64; 3];
pub type Q4 = [f64; 4];
/// btMatrix3x3: three rows.
pub type M3 = [V3; 3];

pub const SIMD_EPSILON: f64 = f32::EPSILON as f64;
pub const SIMD_INFINITY: f64 = f32::MAX as f64;
pub const BT_LARGE_FLOAT: f64 = 1e18_f32 as f64;
pub const SIMD_PI: f64 = std::f32::consts::PI as f64;
pub const SIMD_HALF_PI: f64 = SIMD_PI * (0.5_f32 as f64);
pub const SIMDSQRT12: f64 = 0.707_106_781_186_547_5_f32 as f64;
/// ANGULAR_MOTION_THRESHOLD = btScalar(0.5) * SIMD_HALF_PI.
pub const ANGULAR_MOTION_THRESHOLD: f64 = 0.5 * SIMD_HALF_PI;

/// Round to float (a store to the HEAPF32).
#[inline]
pub fn f(x: f64) -> f64 {
    x as f32 as f64
}

#[inline]
pub fn r3(v: V3) -> V3 {
    [f(v[0]), f(v[1]), f(v[2])]
}

#[inline]
pub fn rq(q: Q4) -> Q4 {
    [f(q[0]), f(q[1]), f(q[2]), f(q[3])]
}

#[inline]
pub fn rm(m: M3) -> M3 {
    [r3(m[0]), r3(m[1]), r3(m[2])]
}

#[inline]
pub fn rt(t: Tf) -> Tf {
    Tf {
        b: rm(t.b),
        o: r3(t.o),
    }
}

pub const ZERO: V3 = [0.0; 3];
pub const IDENTITY3: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

// --- btVector3 -----------------------------------------------------------------------------

#[inline]
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub fn neg(a: V3) -> V3 {
    [-a[0], -a[1], -a[2]]
}

/// `v * s`.
#[inline]
pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// `v * w` component-wise.
#[inline]
pub fn mulv(a: V3, b: V3) -> V3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

/// `v / s` is `v * (1 / s)`.
#[inline]
pub fn div(a: V3, s: f64) -> V3 {
    scale(a, 1.0 / s)
}

#[inline]
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
pub fn len2(a: V3) -> f64 {
    dot(a, a)
}

#[inline]
pub fn len(a: V3) -> f64 {
    len2(a).sqrt()
}

/// btVector3.normalize: `*this /= length ()`.
#[inline]
pub fn normalize(a: V3) -> V3 {
    div(a, len(a))
}

#[inline]
pub fn abs3(a: V3) -> V3 {
    [a[0].abs(), a[1].abs(), a[2].abs()]
}

/// btVector3.maxAxis.
pub fn max_axis(v: V3) -> usize {
    if v[0] < v[1] {
        if v[1] < v[2] { 2 } else { 1 }
    } else if v[0] < v[2] {
        2
    } else {
        0
    }
}

/// btVector3.minAxis.
pub fn min_axis(v: V3) -> usize {
    if v[0] < v[1] {
        if v[0] < v[2] { 0 } else { 2 }
    } else if v[1] < v[2] {
        1
    } else {
        2
    }
}

/// btVector3.closestAxis.
pub fn closest_axis(v: V3) -> usize {
    max_axis(abs3(v))
}

/// btPlaneSpace1 (n, p, q).
pub fn plane_space1(n: V3) -> (V3, V3) {
    let mut p = [0.0; 3];
    let mut q = [0.0; 3];

    if n[2].abs() > SIMDSQRT12 {
        // choose p in y-z plane
        let a = n[1] * n[1] + n[2] * n[2];
        let k = 1.0 / a.sqrt();
        p[0] = 0.0;
        p[1] = f(-n[2] * k);
        p[2] = f(n[1] * k);
        q[0] = f(a * k);
        q[1] = f(-n[0] * p[2]);
        q[2] = f(n[0] * p[1]);
    } else {
        // choose p in x-y plane
        let a = n[0] * n[0] + n[1] * n[1];
        let k = 1.0 / a.sqrt();
        p[0] = f(-n[1] * k);
        p[1] = f(n[0] * k);
        p[2] = 0.0;
        q[0] = f(-n[2] * p[1]);
        q[1] = f(n[2] * p[0]);
        q[2] = f(a * k);
    }

    (p, q)
}

// --- btMatrix3x3 ---------------------------------------------------------------------------

/// `m * v`.
#[inline]
pub fn m_mul_v(m: &M3, v: V3) -> V3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}

/// btMatrix3x3::tdotx/y/z: `v * m` (m^T v).
#[inline]
pub fn v_mul_m(v: V3, m: &M3) -> V3 {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

/// `a * b` (matrix product): `b.tdot* (a[i])`.
pub fn m_mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| v_mul_m(a[i], b))
}

pub fn transpose(m: &M3) -> M3 {
    [
        [m[0][0], m[1][0], m[2][0]],
        [m[0][1], m[1][1], m[2][1]],
        [m[0][2], m[1][2], m[2][2]],
    ]
}

/// btMatrix3x3::transposeTimes.
pub fn transpose_times(a: &M3, m: &M3) -> M3 {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| a[0][i] * m[0][j] + a[1][i] * m[1][j] + a[2][i] * m[2][j])
    })
}

/// btMatrix3x3::timesTranspose.
pub fn times_transpose(a: &M3, m: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| dot(a[i], m[j])))
}

/// btMatrix3x3::scaled (columns scaled).
pub fn scaled(m: &M3, s: V3) -> M3 {
    std::array::from_fn(|i| mulv(m[i], s))
}

pub fn absolute(m: &M3) -> M3 {
    [abs3(m[0]), abs3(m[1]), abs3(m[2])]
}

fn cofac(m: &M3, r1: usize, c1: usize, r2: usize, c2: usize) -> f64 {
    m[r1][c1] * m[r2][c2] - m[r1][c2] * m[r2][c1]
}

/// btMatrix3x3::inverse.
pub fn inverse3(m: &M3) -> M3 {
    let co = [
        cofac(m, 1, 1, 2, 2),
        cofac(m, 1, 2, 2, 0),
        cofac(m, 1, 0, 2, 1),
    ];
    let det = dot(m[0], co);
    let s = 1.0 / det;
    [
        [
            co[0] * s,
            cofac(m, 0, 2, 2, 1) * s,
            cofac(m, 0, 1, 1, 2) * s,
        ],
        [
            co[1] * s,
            cofac(m, 0, 0, 2, 2) * s,
            cofac(m, 0, 2, 1, 0) * s,
        ],
        [
            co[2] * s,
            cofac(m, 0, 1, 2, 0) * s,
            cofac(m, 0, 0, 1, 1) * s,
        ],
    ]
}

/// btMatrix3x3::setRotation (scalar path).
pub fn m_from_quat(q: Q4) -> M3 {
    let d = qlen2(q);
    let s = 2.0 / d;
    let (xs, ys, zs) = (q[0] * s, q[1] * s, q[2] * s);
    let (wx, wy, wz) = (q[3] * xs, q[3] * ys, q[3] * zs);
    let (xx, xy, xz) = (q[0] * xs, q[0] * ys, q[0] * zs);
    let (yy, yz, zz) = (q[1] * ys, q[1] * zs, q[2] * zs);
    [
        [1.0 - (yy + zz), xy - wz, xz + wy],
        [xy + wz, 1.0 - (xx + zz), yz - wx],
        [xz - wy, yz + wx, 1.0 - (xx + yy)],
    ]
}

/// btMatrix3x3::getRotation (scalar path).
pub fn quat_from_m(m: &M3) -> Q4 {
    let trace = m[0][0] + m[1][1] + m[2][2];
    let mut temp = [0.0; 4];

    if trace > 0.0 {
        let mut s = (trace + 1.0).sqrt();
        temp[3] = s * 0.5;
        s = 0.5 / s;
        temp[0] = (m[2][1] - m[1][2]) * s;
        temp[1] = (m[0][2] - m[2][0]) * s;
        temp[2] = (m[1][0] - m[0][1]) * s;
    } else {
        let i = if m[0][0] < m[1][1] {
            if m[1][1] < m[2][2] { 2 } else { 1 }
        } else if m[0][0] < m[2][2] {
            2
        } else {
            0
        };
        let j = (i + 1) % 3;
        let k = (i + 2) % 3;

        let mut s = (m[i][i] - m[j][j] - m[k][k] + 1.0).sqrt();
        temp[i] = s * 0.5;
        s = 0.5 / s;
        temp[3] = (m[k][j] - m[j][k]) * s;
        temp[j] = (m[j][i] + m[i][j]) * s;
        temp[k] = (m[k][i] + m[i][k]) * s;
    }

    temp
}

// --- btQuaternion --------------------------------------------------------------------------

pub fn qdot(a: Q4, b: Q4) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

pub fn qlen2(q: Q4) -> f64 {
    qdot(q, q)
}

pub fn qlen(q: Q4) -> f64 {
    qlen2(q).sqrt()
}

/// btQuaternion.normalize: `*this /= length ()`, i.e. times the reciprocal.
pub fn qnormalize(q: Q4) -> Q4 {
    let s = 1.0 / qlen(q);
    [q[0] * s, q[1] * s, q[2] * s, q[3] * s]
}

/// `q1 * q2` (operator*).
pub fn qmul(a: Q4, b: Q4) -> Q4 {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] + a[1] * b[3] + a[2] * b[0] - a[0] * b[2],
        a[3] * b[2] + a[2] * b[3] + a[0] * b[1] - a[1] * b[0],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

pub fn qinverse(q: Q4) -> Q4 {
    [-q[0], -q[1], -q[2], q[3]]
}

pub fn qneg(q: Q4) -> Q4 {
    [-q[0], -q[1], -q[2], -q[3]]
}

/// btQuaternion::getAngle.
pub fn qangle(q: Q4) -> f64 {
    2.0 * acos_clamped(q[3])
}

/// btAcos.
pub fn acos_clamped(x: f64) -> f64 {
    x.clamp(-1.0, 1.0).acos()
}

/// btQuaternion::setRotation (axis, angle).
pub fn quat_axis_angle(axis: V3, angle: f64) -> Q4 {
    let d = len(axis);
    let s = super::fdlibm::sin(angle * 0.5) / d;
    [
        axis[0] * s,
        axis[1] * s,
        axis[2] * s,
        super::fdlibm::cos(angle * 0.5),
    ]
}

// --- btTransform ---------------------------------------------------------------------------

/// btTransform: a basis (rows) and an origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tf {
    pub b: M3,
    pub o: V3,
}

impl Tf {
    pub const IDENTITY: Tf = Tf {
        b: IDENTITY3,
        o: ZERO,
    };

    pub fn new(b: M3, o: V3) -> Tf {
        Tf { b, o }
    }

    /// btTransform::operator()(x).
    pub fn apply(&self, x: V3) -> V3 {
        add(m_mul_v(&self.b, x), self.o)
    }

    /// btTransform::invXform.
    pub fn inv_xform(&self, p: V3) -> V3 {
        m_mul_v(&transpose(&self.b), sub(p, self.o))
    }

    /// btTransform::inverse.
    pub fn inverse(&self) -> Tf {
        let inv = transpose(&self.b);
        Tf {
            b: inv,
            o: m_mul_v(&inv, neg(self.o)),
        }
    }

    /// btTransform::operator*.
    pub fn mul(&self, t: &Tf) -> Tf {
        Tf {
            b: m_mul(&self.b, &t.b),
            o: self.apply(t.o),
        }
    }

    /// btTransform::inverseTimes.
    pub fn inverse_times(&self, t: &Tf) -> Tf {
        let v = sub(t.o, self.o);
        Tf {
            b: transpose_times(&self.b, &t.b),
            o: v_mul_m(v, &self.b),
        }
    }

    pub fn rotation(&self) -> Q4 {
        quat_from_m(&self.b)
    }

    pub fn set_rotation(&mut self, q: Q4) {
        self.b = m_from_quat(q);
    }
}

/// btTransformUtil::integrateTransform. Returns the unrounded transform; the origin and basis are
/// stored into a btTransform by the caller (use [`rt`]).
pub fn integrate_transform(cur: &Tf, linvel: V3, angvel: V3, time_step: f64) -> Tf {
    let o = add(cur.o, scale(linvel, time_step));
    let mut angle = len(angvel);

    // limit the angular motion
    if angle * time_step > ANGULAR_MOTION_THRESHOLD {
        angle = ANGULAR_MOTION_THRESHOLD / time_step;
    }

    let axis = if angle < 0.001 {
        // use Taylor's expansions of sync function
        scale(
            angvel,
            0.5 * time_step
                - (time_step * time_step * time_step) * (0.020833333333_f32 as f64) * angle * angle,
        )
    } else {
        // sync(fAngle) = sin(c*fAngle)/t
        scale(angvel, super::fdlibm::sin(0.5 * angle * time_step) / angle)
    };

    // dorn * orn0, normalize and setRotation with the association the asm uses
    let (o_, n_, m_, q_) = (
        axis[0],
        axis[1],
        axis[2],
        super::fdlibm::cos(angle * time_step * 0.5),
    );
    let orn0 = rq(cur.rotation());
    let (b_, g_, i_, p_) = (orn0[0], orn0[1], orn0[2], orn0[3]);
    let x = q_ * b_ + o_ * p_ + n_ * i_ - m_ * g_;
    let y = m_ * b_ + (n_ * p_ + q_ * g_) - o_ * i_;
    let z = m_ * p_ + q_ * i_ + o_ * g_ - n_ * b_;
    let w = q_ * p_ - o_ * b_ - n_ * g_ - m_ * i_;
    let c = 1.0 / (w * w + (z * z + (x * x + y * y))).sqrt();
    let (d, j, l, c) = (x * c, y * c, z * c, w * c);
    let s = 2.0 / (c * c + (l * l + (d * d + j * j)));
    let (i, g, b) = (d * s, j * s, l * s);
    let basis = [
        [1.0 - (j * g + l * b), d * g - c * b, d * b + c * g],
        [d * g + c * b, 1.0 - (d * i + l * b), j * b - c * i],
        [d * b - c * g, j * b + c * i, 1.0 - (d * i + j * g)],
    ];
    Tf { b: basis, o }
}
