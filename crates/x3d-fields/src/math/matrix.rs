//! Matrix3 / Matrix4 (standard/Math/Numbers/Matrix{3,4}.js), row-major element order as in
//! X_ITE (`this [0] .. this [15]`). `factor` / `get_transform` port the
//! eigen-decomposition based decomposition from Matrix4.js with
//! `eigen_decomposition` from standard/Math/Algorithms/eigen_decomposition.js
//! (SGI Inventor Jacobi rotations).

use super::rotation::Rotation4;
use super::vector::{Vector2, Vector3, Vector4};
use crate::jsmath;

/// Eigenvalues and eigenvectors of a symmetric 3x3 matrix.
///
/// Ports `eigen_decomposition` (SGI Inventor Jacobi rotations). `matrix` is
/// row-major; `values` receives the eigenvalues and `vectors[j][i]` the j-th
/// component of the i-th eigenvector, as in X_ITE.
#[allow(clippy::needless_range_loop)] // index loops mirror the JS source
fn eigen_decomposition(matrix: &Matrix3, values: &mut [f64; 3], vectors: &mut [[f64; 3]; 3]) {
    const ORDER: usize = 3;
    const SIZE: usize = 9;

    let mut a = [[0.0; ORDER]; ORDER];
    let mut b = [0.0; ORDER];
    let mut z = [0.0; ORDER];

    for i in 0..ORDER {
        b[i] = matrix.0[i * ORDER + i];
        values[i] = b[i];

        for j in 0..ORDER {
            vectors[i][j] = if i == j { 1.0 } else { 0.0 };
            a[i][j] = matrix.0[j * ORDER + i];
        }
    }

    // 50 sweeps, as in the original algorithm.
    for sweep in 0..50 {
        let mut sm = 0.0;

        for p in 0..ORDER - 1 {
            for q in p + 1..ORDER {
                sm += a[p][q].abs();
            }
        }

        if sm == 0.0 {
            break;
        }

        let thresh = if sweep < 3 {
            0.2 * sm / SIZE as f64
        } else {
            0.0
        };

        for p in 0..ORDER - 1 {
            for q in p + 1..ORDER {
                let g = 100.0 * a[p][q].abs();

                if sweep > 3
                    && values[p].abs() + g == values[p].abs()
                    && values[q].abs() + g == values[q].abs()
                {
                    a[p][q] = 0.0;
                } else if a[p][q].abs() > thresh {
                    let h = values[q] - values[p];

                    let t = if h.abs() + g == h.abs() {
                        a[p][q] / h
                    } else {
                        let theta = 0.5 * h / a[p][q];
                        let mut t = 1.0 / (theta.abs() + (1.0 + theta * theta).sqrt());

                        if theta < 0.0 {
                            t = -t;
                        }

                        t
                    };

                    let c = 1.0 / (1.0 + t * t).sqrt();
                    let s = t * c;
                    let tau = s / (1.0 + c);
                    let h = t * a[p][q];
                    z[p] -= h;
                    z[q] += h;
                    values[p] -= h;
                    values[q] += h;
                    a[p][q] = 0.0;

                    for j in 0..p {
                        let (g, h) = (a[j][p], a[j][q]);
                        a[j][p] = g - s * (h + g * tau);
                        a[j][q] = h + s * (g - h * tau);
                    }

                    for j in p + 1..q {
                        let (g, h) = (a[p][j], a[j][q]);
                        a[p][j] = g - s * (h + g * tau);
                        a[j][q] = h + s * (g - h * tau);
                    }

                    for j in q + 1..ORDER {
                        let (g, h) = (a[p][j], a[q][j]);
                        a[p][j] = g - s * (h + g * tau);
                        a[q][j] = h + s * (g - h * tau);
                    }

                    for j in 0..ORDER {
                        let (g, h) = (vectors[j][p], vectors[j][q]);
                        vectors[j][p] = g - s * (h + g * tau);
                        vectors[j][q] = h + s * (g - h * tau);
                    }
                }
            }
        }

        for p in 0..ORDER {
            values[p] = b[p] + z[p];
            b[p] = values[p];
            z[p] = 0.0;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix3(pub [f64; 9]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4(pub [f64; 16]);

impl Default for Matrix3 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Default for Matrix4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Matrix3 {
    pub const IDENTITY: Matrix3 = Matrix3([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    pub const ZERO: Matrix3 = Matrix3([0.0; 9]);

    pub fn equals(&self, other: &Self) -> bool {
        self.0.iter().zip(other.0.iter()).all(|(a, b)| a == b)
    }

    pub fn determinant(&self) -> f64 {
        let [m0, m1, m2, m3, m4, m5, m6, m7, m8] = self.0;

        m0 * (m4 * m8 - m5 * m7) - m1 * (m3 * m8 - m5 * m6) + m2 * (m3 * m7 - m4 * m6)
    }

    pub fn transpose(&mut self) -> &mut Self {
        let m = &mut self.0;
        m.swap(1, 3);
        m.swap(2, 6);
        m.swap(5, 7);
        self
    }

    pub fn inverse(&mut self) -> &mut Self {
        let [m0, m1, m2, m3, m4, m5, m6, m7, m8] = self.0;
        let t4 = m0 * m4;
        let t6 = m0 * m7;
        let t8 = m3 * m1;
        let t10 = m3 * m7;
        let t12 = m6 * m1;
        let t14 = m6 * m4;

        let mut d = t4 * m8 - t6 * m5 - t8 * m8 + t10 * m2 + t12 * m5 - t14 * m2;

        if d == 0.0 || d.is_nan() {
            self.0 = Self::ZERO.0;
            return self;
        }

        d = 1.0 / d;

        let b0 = (m4 * m8 - m7 * m5) * d;
        let b1 = -(m1 * m8 - m7 * m2) * d;
        let b2 = (m1 * m5 - m4 * m2) * d;
        let b3 = -(m3 * m8 - m6 * m5) * d;
        let b4 = (m0 * m8 - m6 * m2) * d;
        let b5 = -(m0 * m5 - m3 * m2) * d;

        self.0 = [
            b0,
            b1,
            b2,
            b3,
            b4,
            b5,
            (t10 - t14) * d,
            -(t6 - t12) * d,
            (t4 - t8) * d,
        ];
        self
    }

    pub fn mult_left(&mut self, matrix: &Matrix3) -> &mut Self {
        let [a0, a1, a2, a3, a4, a5, a6, a7, a8] = self.0;
        let b = &matrix.0;
        let mut r = [0.0; 9];

        for row in 0..3 {
            let (b0, b1, b2) = (b[row * 3], b[row * 3 + 1], b[row * 3 + 2]);

            r[row * 3] = a0 * b0 + a3 * b1 + a6 * b2;
            r[row * 3 + 1] = a1 * b0 + a4 * b1 + a7 * b2;
            r[row * 3 + 2] = a2 * b0 + a5 * b1 + a8 * b2;
        }

        self.0 = r;
        self
    }

    pub fn mult_right(&mut self, matrix: &Matrix3) -> &mut Self {
        let [b0, b1, b2, b3, b4, b5, b6, b7, b8] = matrix.0;
        let a = self.0;
        let mut r = [0.0; 9];

        for row in 0..3 {
            let (a0, a1, a2) = (a[row * 3], a[row * 3 + 1], a[row * 3 + 2]);

            r[row * 3] = a0 * b0 + a1 * b3 + a2 * b6;
            r[row * 3 + 1] = a0 * b1 + a1 * b4 + a2 * b7;
            r[row * 3 + 2] = a0 * b2 + a1 * b5 + a2 * b8;
        }

        self.0 = r;
        self
    }

    pub fn mult_vec_matrix2(&self, v: &mut Vector2) {
        let m = &self.0;
        let (x, y) = (v.x, v.y);
        let w = x * m[2] + y * m[5] + m[8];

        v.x = (x * m[0] + y * m[3] + m[6]) / w;
        v.y = (x * m[1] + y * m[4] + m[7]) / w;
    }

    pub fn mult_vec_matrix3(&self, v: &mut Vector3) {
        let m = &self.0;
        let (x, y, z) = (v.x, v.y, v.z);

        v.x = x * m[0] + y * m[3] + z * m[6];
        v.y = x * m[1] + y * m[4] + z * m[7];
        v.z = x * m[2] + y * m[5] + z * m[8];
    }

    pub fn mult_matrix_vec3(&self, v: &mut Vector3) {
        let m = &self.0;
        let (x, y, z) = (v.x, v.y, v.z);

        v.x = x * m[0] + y * m[1] + z * m[2];
        v.y = x * m[3] + y * m[4] + z * m[5];
        v.z = x * m[6] + y * m[7] + z * m[8];
    }

    pub fn translate(&mut self, t: &Vector2) -> &mut Self {
        let m = &mut self.0;

        m[6] += m[0] * t.x + m[3] * t.y;
        m[7] += m[1] * t.x + m[4] * t.y;
        self
    }

    /// Matrix3.fromRotation: 2D rotation by `angle` radians.
    pub fn from_rotation(angle: f64) -> Matrix3 {
        let s = jsmath::sin(angle);
        let c = jsmath::cos(angle);

        Matrix3([c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0])
    }

    pub fn rotate(&mut self, angle: f64) -> &mut Self {
        self.mult_left(&Matrix3::from_rotation(angle))
    }

    pub fn scale(&mut self, s: &Vector2) -> &mut Self {
        let m = &mut self.0;

        m[0] *= s.x;
        m[3] *= s.y;
        m[1] *= s.x;
        m[4] *= s.y;
        self
    }
}

impl Matrix4 {
    pub const IDENTITY: Matrix4 = Matrix4([
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);
    pub const ZERO: Matrix4 = Matrix4([0.0; 16]);

    pub fn equals(&self, other: &Self) -> bool {
        self.0.iter().zip(other.0.iter()).all(|(a, b)| a == b)
    }

    pub fn determinant(&self) -> f64 {
        let [
            m00,
            m01,
            m02,
            m03,
            m04,
            m05,
            m06,
            m07,
            m08,
            m09,
            m10,
            m11,
            m12,
            m13,
            m14,
            m15,
        ] = self.0;

        let b0 = m00 * m05 - m01 * m04;
        let b1 = m00 * m06 - m02 * m04;
        let b2 = m01 * m06 - m02 * m05;
        let b3 = m08 * m13 - m09 * m12;
        let b4 = m08 * m14 - m10 * m12;
        let b5 = m09 * m14 - m10 * m13;
        let b6 = m00 * b5 - m01 * b4 + m02 * b3;
        let b7 = m04 * b5 - m05 * b4 + m06 * b3;
        let b8 = m08 * b2 - m09 * b1 + m10 * b0;
        let b9 = m12 * b2 - m13 * b1 + m14 * b0;

        m07 * b6 - m03 * b7 + m15 * b8 - m11 * b9
    }

    pub fn transpose(&mut self) -> &mut Self {
        let m = &mut self.0;
        m.swap(1, 4);
        m.swap(2, 8);
        m.swap(3, 12);
        m.swap(6, 9);
        m.swap(7, 13);
        m.swap(11, 14);
        self
    }

    pub fn inverse(&mut self) -> &mut Self {
        let [
            m00,
            m01,
            m02,
            m03,
            m04,
            m05,
            m06,
            m07,
            m08,
            m09,
            m10,
            m11,
            m12,
            m13,
            m14,
            m15,
        ] = self.0;

        let b00 = m00 * m05 - m01 * m04;
        let b01 = m00 * m06 - m02 * m04;
        let b02 = m00 * m07 - m03 * m04;
        let b03 = m01 * m06 - m02 * m05;
        let b04 = m01 * m07 - m03 * m05;
        let b05 = m02 * m07 - m03 * m06;
        let b06 = m08 * m13 - m09 * m12;
        let b07 = m08 * m14 - m10 * m12;
        let b08 = m08 * m15 - m11 * m12;
        let b09 = m09 * m14 - m10 * m13;
        let b10 = m09 * m15 - m11 * m13;
        let b11 = m10 * m15 - m11 * m14;

        let mut d = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;

        if d == 0.0 || d.is_nan() {
            self.0 = Self::ZERO.0;
            return self;
        }

        d = 1.0 / d;

        self.0 = [
            (m05 * b11 - m06 * b10 + m07 * b09) * d,
            (m02 * b10 - m01 * b11 - m03 * b09) * d,
            (m13 * b05 - m14 * b04 + m15 * b03) * d,
            (m10 * b04 - m09 * b05 - m11 * b03) * d,
            (m06 * b08 - m04 * b11 - m07 * b07) * d,
            (m00 * b11 - m02 * b08 + m03 * b07) * d,
            (m14 * b02 - m12 * b05 - m15 * b01) * d,
            (m08 * b05 - m10 * b02 + m11 * b01) * d,
            (m04 * b10 - m05 * b08 + m07 * b06) * d,
            (m01 * b08 - m00 * b10 - m03 * b06) * d,
            (m12 * b04 - m13 * b02 + m15 * b00) * d,
            (m09 * b02 - m08 * b04 - m11 * b00) * d,
            (m05 * b07 - m04 * b09 - m06 * b06) * d,
            (m00 * b09 - m01 * b07 + m02 * b06) * d,
            (m13 * b01 - m12 * b03 - m14 * b00) * d,
            (m08 * b03 - m09 * b01 + m10 * b00) * d,
        ];
        self
    }

    pub fn mult_left(&mut self, matrix: &Matrix4) -> &mut Self {
        let a = self.0;
        let b = &matrix.0;
        let mut r = [0.0; 16];

        for row in 0..4 {
            let (b0, b1, b2, b3) = (b[row * 4], b[row * 4 + 1], b[row * 4 + 2], b[row * 4 + 3]);

            for col in 0..4 {
                r[row * 4 + col] =
                    a[col] * b0 + a[4 + col] * b1 + a[8 + col] * b2 + a[12 + col] * b3;
            }
        }

        self.0 = r;
        self
    }

    pub fn mult_right(&mut self, matrix: &Matrix4) -> &mut Self {
        let a = self.0;
        let b = &matrix.0;
        let mut r = [0.0; 16];

        for row in 0..4 {
            let (a0, a1, a2, a3) = (a[row * 4], a[row * 4 + 1], a[row * 4 + 2], a[row * 4 + 3]);

            for col in 0..4 {
                r[row * 4 + col] =
                    a0 * b[col] + a1 * b[4 + col] + a2 * b[8 + col] + a3 * b[12 + col];
            }
        }

        self.0 = r;
        self
    }

    pub fn mult_vec_matrix3(&self, v: &mut Vector3) {
        let m = &self.0;
        let (x, y, z) = (v.x, v.y, v.z);
        let w = 1.0 / (x * m[3] + y * m[7] + z * m[11] + m[15]);

        v.x = (x * m[0] + y * m[4] + z * m[8] + m[12]) * w;
        v.y = (x * m[1] + y * m[5] + z * m[9] + m[13]) * w;
        v.z = (x * m[2] + y * m[6] + z * m[10] + m[14]) * w;
    }

    pub fn mult_vec_matrix4(&self, v: &mut Vector4) {
        let m = &self.0;
        let (x, y, z, w) = (v.x, v.y, v.z, v.w);

        v.x = x * m[0] + y * m[4] + z * m[8] + w * m[12];
        v.y = x * m[1] + y * m[5] + z * m[9] + w * m[13];
        v.z = x * m[2] + y * m[6] + z * m[10] + w * m[14];
        v.w = x * m[3] + y * m[7] + z * m[11] + w * m[15];
    }

    pub fn mult_matrix_vec3(&self, v: &mut Vector3) {
        let m = &self.0;
        let (x, y, z) = (v.x, v.y, v.z);
        let w = 1.0 / (x * m[12] + y * m[13] + z * m[14] + m[15]);

        v.x = (x * m[0] + y * m[1] + z * m[2] + m[3]) * w;
        v.y = (x * m[4] + y * m[5] + z * m[6] + m[7]) * w;
        v.z = (x * m[8] + y * m[9] + z * m[10] + m[11]) * w;
    }

    pub fn mult_dir_matrix(&self, v: &mut Vector3) {
        let m = &self.0;
        let (x, y, z) = (v.x, v.y, v.z);

        v.x = x * m[0] + y * m[4] + z * m[8];
        v.y = x * m[1] + y * m[5] + z * m[9];
        v.z = x * m[2] + y * m[6] + z * m[10];
    }

    pub fn mult_matrix_dir(&self, v: &mut Vector3) {
        let m = &self.0;
        let (x, y, z) = (v.x, v.y, v.z);

        v.x = x * m[0] + y * m[1] + z * m[2];
        v.y = x * m[4] + y * m[5] + z * m[6];
        v.z = x * m[8] + y * m[9] + z * m[10];
    }

    pub fn translate(&mut self, t: &Vector3) -> &mut Self {
        let m = &mut self.0;
        let (x, y, z) = (t.x, t.y, t.z);

        m[12] += m[0] * x + m[4] * y + m[8] * z;
        m[13] += m[1] * x + m[5] * y + m[9] * z;
        m[14] += m[2] * x + m[6] * y + m[10] * z;
        self
    }

    pub fn rotate(&mut self, rotation: &Rotation4) -> &mut Self {
        let m = rotation.quaternion().get_matrix4();
        self.mult_left(&m)
    }

    pub fn scale(&mut self, s: &Vector3) -> &mut Self {
        let m = &mut self.0;

        m[0] *= s.x;
        m[4] *= s.y;
        m[8] *= s.z;
        m[1] *= s.x;
        m[5] *= s.y;
        m[9] *= s.z;
        m[2] *= s.x;
        m[6] *= s.y;
        m[10] *= s.z;
        self
    }

    /// Matrix4.setTransform (translation, rotation, scale, scaleOrientation, center).
    pub fn set_transform(
        &mut self,
        translation: Option<&Vector3>,
        rotation: Option<&Rotation4>,
        scale: Option<&Vector3>,
        scale_orientation: Option<&Rotation4>,
        center: Option<&Vector3>,
    ) -> &mut Self {
        *self = Self::IDENTITY;

        if let Some(t) = translation
            && !t.equals(&Vector3::ZERO)
        {
            self.translate(t);
        }

        let has_center = center.is_some_and(|c| !c.equals(&Vector3::ZERO));

        if has_center {
            self.translate(center.unwrap());
        }

        if let Some(r) = rotation
            && !r.equals(&Rotation4::identity())
        {
            self.rotate(r);
        }

        if let Some(s) = scale
            && !s.equals(&Vector3::ONE)
        {
            match scale_orientation {
                Some(so) if !so.equals(&Rotation4::identity()) => {
                    self.rotate(so);
                    self.scale(s);
                    let mut inv = *so;
                    inv.inverse();
                    self.rotate(&inv);
                }
                _ => {
                    self.scale(s);
                }
            }
        }

        if has_center {
            let mut c = *center.unwrap();
            c.negate();
            self.translate(&c);
        }

        self
    }

    pub fn submatrix(&self) -> Matrix3 {
        let m = &self.0;

        Matrix3([m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]])
    }

    /// Matrix4.factor: decompose into translation, rotation, scale and
    /// scaleOrientation (each optional). Ports Matrix4.js `factor`, including
    /// the sign rule from `det A` and the `U = !R ~S R A` rotation recovery.
    pub fn factor(
        &self,
        translation: Option<&mut Vector3>,
        rotation: Option<&mut Rotation4>,
        scale: Option<&mut Vector3>,
        scale_orientation: Option<&mut Rotation4>,
    ) {
        let m = &self.0;

        // (1) Get translation.
        if let Some(t) = translation {
            t.set(m[12], m[13], m[14]);
        }

        if rotation.is_none() && scale.is_none() && scale_orientation.is_none() {
            return;
        }

        // (2) Create 3x3 matrix.
        let mut a = self.submatrix();

        // (3) Compute det A. If negative, sign = -1, else sign = 1.
        let det = a.determinant();
        let det_sign = if det < 0.0 { -1.0 } else { 1.0 };

        // (4) B = A * !A (here !A means A transpose), then decompose.
        let mut b = a;
        b.transpose();
        b.mult_left(&a);

        let mut values = [0.0; 3];
        let mut vectors = [[0.0; 3]; 3];
        eigen_decomposition(&b, &mut values, &mut vectors);

        let mut so = Matrix3([
            vectors[0][0],
            vectors[1][0],
            vectors[2][0],
            vectors[0][1],
            vectors[1][1],
            vectors[2][1],
            vectors[0][2],
            vectors[1][2],
            vectors[2][2],
        ]);

        if let Some(sorient) = scale_orientation {
            sorient.set_matrix(&so);
        }

        // Compute s = sqrt(eigen values), with sign.
        let mut s = Vector3::ZERO;
        s.set(
            det_sign * values[0].sqrt(),
            det_sign * values[1].sqrt(),
            det_sign * values[2].sqrt(),
        );

        if let Some(sc) = scale {
            sc.assign(&s);
        }

        if let Some(rot) = rotation {
            let mut si = Matrix3::ZERO;
            si.0[0] = 1.0 / s.x;
            si.0[4] = 1.0 / s.y;
            si.0[8] = 1.0 / s.z;

            // (5) Compute U = !R ~S R A.
            a.mult_left(&so);
            a.mult_left(&si);
            so.transpose();
            a.mult_left(&so);
            rot.set_matrix(&a);
        }
    }

    /// Matrix4.getTransform: as `factor`, but first removes `center`
    /// (P' = T * C * R * SR * S * -SR * -C * P callers pass center through).
    pub fn get_transform(
        &self,
        translation: Option<&mut Vector3>,
        rotation: Option<&mut Rotation4>,
        scale: Option<&mut Vector3>,
        scale_orientation: Option<&mut Rotation4>,
        center: Option<&Vector3>,
    ) {
        match center {
            Some(c) => {
                let mut neg = *c;
                neg.negate();
                let mut m = Matrix4::IDENTITY;
                m.translate(&neg);
                m.mult_left(self);
                m.translate(c);
                m.factor(translation, rotation, scale, scale_orientation);
            }
            None => self.factor(translation, rotation, scale, scale_orientation),
        }
    }
}

impl std::fmt::Display for Matrix3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for (i, val) in self.0.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(&jsmath::number_to_string(*val));
        }
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for Matrix4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for (i, val) in self.0.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(&jsmath::number_to_string(*val));
        }
        write!(f, "{}", s)
    }
}
