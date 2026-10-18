//! btTypedConstraint and the joints X_ITE creates (btPoint2PointConstraint, btHingeConstraint and
//! btSliderConstraint), Bullet 2.82, zlib license.
//!
//! The rounding follows the compiled ammo.js (asm.js, doubles everywhere, float rounding only at
//! heap stores): the `getInfo2` rows are written with [`r3`]/[`f`] at the stores and the arithmetic
//! is associated exactly like the asm.
//!
//! Reachable from X_ITE: `btPoint2PointConstraint(rbA, rbB, 0, 0)` + `setPivotA/B` (BallJoint),
//! `btHingeConstraint(rbA, rbB, pivotA, pivotB, axisA, axisB, false)` without limit or motor
//! (SingleAxisHingeJoint, DoubleAxisHingeJoint) and `btSliderConstraint(rbA, rbB, frameA, frameB,
//! true)` with `setLower/UpperAngLimit(0)` and `setLower/UpperLinLimit` (SliderJoint). Motors,
//! hinge limits, constraint flags (ERP/CFM overrides) and the pre-2.80 `useOffsetForConstraintFrame
//! = false` code paths are not reachable and are not ported.

use super::body::{BodyId, RigidBody};
use super::math::*;
use super::solver::Info2;

const SIMD_2_PI: f64 = 6.283_185_307_179_586_f32 as f64;

/// btPoint2PointConstraint.
#[derive(Clone, Debug)]
pub struct Point2Point {
    pub pivot_a: V3,
    pub pivot_b: V3,
    /// m_setting.m_damping.
    pub damping: f64,
    /// m_setting.m_impulseClamp.
    pub impulse_clamp: f64,
}

/// btHingeConstraint (angular only = false, no limit, no motor).
#[derive(Clone, Debug)]
pub struct Hinge {
    pub frame_a: Tf,
    pub frame_b: Tf,
    pub angular_only: bool,
}

/// btSliderConstraint (useOffsetForConstraintFrame = true).
#[derive(Clone, Debug)]
pub struct Slider {
    pub frame_a: Tf,
    pub frame_b: Tf,
    pub use_linear_reference_frame_a: bool,
    pub lower_lin: f64,
    pub upper_lin: f64,
    pub lower_ang: f64,
    pub upper_ang: f64,
    pub calc_a: Tf,
    pub calc_b: Tf,
    pub real_pivot_a: V3,
    pub real_pivot_b: V3,
    pub slider_axis: V3,
    pub delta: V3,
    pub depth: V3,
    pub lin_pos: f64,
    pub ang_pos: f64,
    pub ang_depth: f64,
    pub solve_lin: bool,
    pub solve_ang: bool,
    pub softness_ortho_lin: f64,
    pub softness_ortho_ang: f64,
    pub softness_lim_lin: f64,
    pub softness_lim_ang: f64,
    pub damping_lim_lin: f64,
    pub damping_lim_ang: f64,
}

/// The joint kinds.
#[derive(Clone, Debug)]
pub enum Kind {
    Point2Point(Point2Point),
    Hinge(Hinge),
    Slider(Slider),
}

#[derive(Clone, Debug)]
pub struct Constraint {
    pub kind: Kind,
    pub body_a: BodyId,
    pub body_b: BodyId,
    pub enabled: bool,
    pub breaking_impulse_threshold: f64,
    pub applied_impulse: f64,
    pub override_iterations: i32,
}

/// Column `j` of a matrix.
fn col(m: &M3, j: usize) -> V3 {
    [m[0][j], m[1][j], m[2][j]]
}

/// btNormalizeAngle.
fn normalize_angle(a: f64) -> f64 {
    let a = a % SIMD_2_PI;
    if a < -SIMD_PI {
        a + SIMD_2_PI
    } else if a > SIMD_PI {
        a - SIMD_2_PI
    } else {
        a
    }
}

/// btAdjustAngleToLimits.
fn adjust_angle_to_limits(angle: f64, lo: f64, hi: f64) -> f64 {
    if lo >= hi {
        angle
    } else if angle < lo {
        let diff_lo = normalize_angle(lo - angle).abs();
        let diff_hi = normalize_angle(hi - angle).abs();
        if diff_lo < diff_hi { angle } else { angle + SIMD_2_PI }
    } else if angle > hi {
        let diff_hi = normalize_angle(angle - hi).abs();
        let diff_lo = normalize_angle(angle - lo).abs();
        if diff_lo < diff_hi { angle - SIMD_2_PI } else { angle }
    } else {
        angle
    }
}

impl Constraint {
    fn new(kind: Kind, body_a: BodyId, body_b: BodyId) -> Constraint {
        Constraint { kind, body_a, body_b, enabled: true, breaking_impulse_threshold: SIMD_INFINITY, applied_impulse: 0.0, override_iterations: -1 }
    }

    /// `new Ammo.btPoint2PointConstraint(rbA, rbB, pivotA, pivotB)`.
    pub fn point2point(body_a: BodyId, body_b: BodyId, pivot_a: V3, pivot_b: V3) -> Constraint {
        Constraint::new(Kind::Point2Point(Point2Point { pivot_a: r3(pivot_a), pivot_b: r3(pivot_b), damping: 1.0, impulse_clamp: 0.0 }), body_a, body_b)
    }

    /// `setPivotA` of a point to point constraint.
    pub fn set_pivot_a(&mut self, p: V3) {
        if let Kind::Point2Point(c) = &mut self.kind {
            c.pivot_a = r3(p);
        }
    }

    /// `setPivotB` of a point to point constraint.
    pub fn set_pivot_b(&mut self, p: V3) {
        if let Kind::Point2Point(c) = &mut self.kind {
            c.pivot_b = r3(p);
        }
    }

    /// `new Ammo.btHingeConstraint(rbA, rbB, pivotInA, pivotInB, axisInA, axisInB, useReferenceFrameA)`.
    /// The frames use the center of mass transforms the bodies have right now.
    pub fn hinge(rigid: &[RigidBody], body_a: BodyId, body_b: BodyId, pivot_a: V3, pivot_b: V3, axis_a: V3, axis_b: V3, _use_reference_frame_a: bool) -> Constraint {
        let (pivot_a, pivot_b, axis_a, axis_b) = (r3(pivot_a), r3(pivot_b), r3(axis_a), r3(axis_b));
        let ta = &rigid[body_a].world_tf;
        let (m, n, o) = (ta.b[0][0], ta.b[1][0], ta.b[2][0]);
        let (p, q, t) = (axis_a[0], axis_a[1], axis_a[2]);
        let proj = m * p + n * q + o * t;

        // rbAxisA1/2: the middle case keeps them unrounded in registers
        let (a1, a2): (V3, V3);

        if proj >= 1.0 - SIMD_EPSILON {
            a1 = neg(col(&ta.b, 2));
            a2 = col(&ta.b, 1);
        } else if proj <= -1.0 + SIMD_EPSILON {
            a1 = col(&ta.b, 2);
            a2 = col(&ta.b, 1);
        } else {
            let a2u = [o * q - n * t, m * t - o * p, n * p - m * q];
            a1 = [t * a2u[1] - q * a2u[2], p * a2u[2] - t * a2u[0], q * a2u[0] - p * a2u[1]];
            a2 = a2u;
        }

        let frame_a = Tf { b: rm([[a1[0], a2[0], axis_a[0]], [a1[1], a2[1], axis_a[1]], [a1[2], a2[2], axis_a[2]]]), o: pivot_a };

        // shortestArcQuat (axisInA, axisInB)
        let (p0, p1, p2) = (axis_a[0], axis_a[1], axis_a[2]);
        let (x, w, v) = (axis_b[0], axis_b[1], axis_b[2]);
        let d = v * p2 + (p1 * w + x * p0);
        let (qx, qy, qz, qw);

        if d < -1.0 + SIMD_EPSILON {
            if p2.abs() > SIMDSQRT12 {
                let k = 1.0 / (p1 * p1 + p2 * p2).sqrt();
                qy = -(p2 * k);
                qx = 0.0;
                qz = p1 * k;
                qw = 0.0;
            } else {
                let k = 1.0 / (p1 * p1 + p0 * p0).sqrt();
                qy = p0 * k;
                qx = -(p1 * k);
                qz = 0.0;
                qw = 0.0;
            }
        } else {
            let s = ((d + 1.0) * 2.0).sqrt();
            qy = (p2 * x - v * p0) * (1.0 / s);
            qw = s * 0.5;
            qz = (w * p0 - p1 * x) * (1.0 / s);
            qx = (p1 * v - p2 * w) * (1.0 / s);
        }

        // quatRotate (rotationArc, rbAxisA1)
        let (z0, a0, y0) = (a1[0], a1[1], a1[2]);
        let dd = z0 * qw + y0 * qy - a0 * qz;
        let q_ = a0 * qw + z0 * qz - y0 * qx;
        let p_ = y0 * qw + a0 * qx - z0 * qy;
        let c_ = -(z0 * qx) - a0 * qy - y0 * qz;
        let (iz, io, iy) = (-qx, -qy, -qz);
        let bx = q_ * iy + (qw * dd + c_ * iz) - p_ * io;
        let by = p_ * iz + (qw * q_ + c_ * io) - dd * iy;
        let bz = dd * io + (c_ * iy + qw * p_) - q_ * iz;
        let b1 = [bx, by, bz];
        let b2 = [w * bz - v * by, v * bx - x * bz, x * by - w * bx];

        let frame_b = Tf { b: rm([[b1[0], b2[0], axis_b[0]], [b1[1], b2[1], axis_b[1]], [b1[2], b2[2], axis_b[2]]]), o: pivot_b };

        Constraint::new(Kind::Hinge(Hinge { frame_a, frame_b, angular_only: false }), body_a, body_b)
    }

    /// `new Ammo.btSliderConstraint(rbA, rbB, frameInA, frameInB, useLinearReferenceFrameA)`; initParams
    /// runs calculateTransforms with the bodies' current transforms.
    pub fn slider(rigid: &[RigidBody], body_a: BodyId, body_b: BodyId, frame_a: Tf, frame_b: Tf, use_linear_reference_frame_a: bool) -> Constraint {
        let s = Slider {
            frame_a: rt(frame_a),
            frame_b: rt(frame_b),
            use_linear_reference_frame_a,
            lower_lin: 1.0,
            upper_lin: -1.0,
            lower_ang: 0.0,
            upper_ang: 0.0,
            calc_a: Tf::IDENTITY,
            calc_b: Tf::IDENTITY,
            real_pivot_a: ZERO,
            real_pivot_b: ZERO,
            slider_axis: ZERO,
            delta: ZERO,
            depth: ZERO,
            lin_pos: 0.0,
            ang_pos: 0.0,
            ang_depth: 0.0,
            solve_lin: false,
            solve_ang: false,
            softness_ortho_lin: 1.0,
            softness_ortho_ang: 1.0,
            softness_lim_lin: 1.0,
            softness_lim_ang: 1.0,
            damping_lim_lin: 1.0,
            damping_lim_ang: 1.0,
        };
        let mut c = Constraint::new(Kind::Slider(s), body_a, body_b);
        let (ta, tb) = (rigid[body_a].world_tf, rigid[body_b].world_tf);
        if let Kind::Slider(s) = &mut c.kind {
            s.calculate_transforms(&ta, &tb);
        }
        c
    }

    pub fn set_lower_lin_limit(&mut self, v: f64) {
        if let Kind::Slider(s) = &mut self.kind {
            s.lower_lin = f(v);
        }
    }

    pub fn set_upper_lin_limit(&mut self, v: f64) {
        if let Kind::Slider(s) = &mut self.kind {
            s.upper_lin = f(v);
        }
    }

    pub fn set_lower_ang_limit(&mut self, v: f64) {
        if let Kind::Slider(s) = &mut self.kind {
            s.lower_ang = f(v);
        }
    }

    pub fn set_upper_ang_limit(&mut self, v: f64) {
        if let Kind::Slider(s) = &mut self.kind {
            s.upper_ang = f(v);
        }
    }

    /// buildJacobian (empty for the getInfo2 based joints).
    pub fn build_jacobian(&mut self, _rigid: &[RigidBody]) {}

    /// getInfo1: (number of rows, nub).
    pub fn get_info1(&mut self, rigid: &[RigidBody]) -> (usize, usize) {
        let (a, b) = (self.body_a, self.body_b);
        match &mut self.kind {
            Kind::Point2Point(_) => (3, 3),
            // no limit and no motor: the hinge adds its sixth row only for those
            Kind::Hinge(_) => (5, 1),
            Kind::Slider(s) => {
                let (ta, tb) = (rigid[a].world_tf, rigid[b].world_tf);
                let mut rows = 4;
                let mut nub = 2;
                s.calculate_transforms(&ta, &tb);
                s.test_ang_limits();
                s.test_lin_limits();

                if s.solve_lin {
                    rows += 1;
                    nub -= 1;
                }

                if s.solve_ang {
                    rows += 1;
                    nub -= 1;
                }

                (rows, nub)
            }
        }
    }

    /// getInfo2.
    pub fn get_info2(&mut self, info: &mut Info2, rigid: &[RigidBody]) {
        let (a, b) = (self.body_a, self.body_b);
        match &self.kind {
            Kind::Point2Point(c) => c.get_info2(info, rigid, a, b),
            Kind::Hinge(c) => c.get_info2(info, rigid, a, b),
            Kind::Slider(c) => c.get_info2(info, rigid, a, b),
        }
    }
}

/// Store a vector into a row field.
fn put(dst: &mut V3, v: V3) {
    *dst = r3(v);
}

impl Point2Point {
    fn get_info2(&self, info: &mut Info2, rigid: &[RigidBody], a: BodyId, b: BodyId) {
        let (ta, tb) = (&rigid[a].world_tf, &rigid[b].world_tf);

        // a1 = trA.getBasis() * pivotInA, a2 = trB.getBasis() * pivotInB
        let a1 = [dot(ta.b[0], self.pivot_a), dot(ta.b[1], self.pivot_a), dot(ta.b[2], self.pivot_a)];
        let a2 = [dot(tb.b[0], self.pivot_b), dot(tb.b[1], self.pivot_b), dot(tb.b[2], self.pivot_b)];
        let (o, m, e) = (a1[0], a1[1], a1[2]);
        let (p, n, d) = (a2[0], a2[1], a2[2]);

        put(&mut info.rows[0].normal1, [1.0, 0.0, 0.0]);
        put(&mut info.rows[1].normal1, [0.0, 1.0, 0.0]);
        put(&mut info.rows[2].normal1, [0.0, 0.0, 1.0]);
        put(&mut info.rows[0].normal2, [-1.0, 0.0, 0.0]);
        put(&mut info.rows[1].normal2, [0.0, -1.0, 0.0]);
        put(&mut info.rows[2].normal2, [0.0, 0.0, -1.0]);

        put(&mut info.rows[0].rel1_cross_n, [0.0, e, -m]);
        put(&mut info.rows[1].rel1_cross_n, [-e, 0.0, o]);
        put(&mut info.rows[2].rel1_cross_n, [m, -o, 0.0]);

        put(&mut info.rows[0].rel2_cross_n, [0.0, -d, n]);
        put(&mut info.rows[1].rel2_cross_n, [d, 0.0, -p]);
        put(&mut info.rows[2].rel2_cross_n, [-n, p, 0.0]);

        // setting.m_tau is used only with the BT_P2P_FLAGS_ERP flag
        let l = info.erp * info.fps;
        info.rows[0].rhs = f(l * (p + tb.o[0] - o - ta.o[0]));
        info.rows[1].rhs = f(l * (n + tb.o[1] - m - ta.o[1]));
        info.rows[2].rhs = f(l * (d + tb.o[2] - e - ta.o[2]));

        if self.impulse_clamp > 0.0 {
            for r in info.rows.iter_mut().take(3) {
                r.lower = f(-self.impulse_clamp);
                r.upper = f(self.impulse_clamp);
            }
        }

        info.damping = self.damping;
    }
}

impl Hinge {
    /// getInfo2InternalUsingFrameOffset (no limit, no motor).
    fn get_info2(&self, info: &mut Info2, rigid: &[RigidBody], a: BodyId, b: BodyId) {
        let (ta, tb) = (&rigid[a].world_tf, &rigid[b].world_tf);
        let (fa, fb) = (&self.frame_a, &self.frame_b);
        let w = rigid[a].inverse_mass;
        let x = rigid[b].inverse_mass;

        let c1a = col(&fa.b, 1);
        let c2a = col(&fa.b, 2);
        let c2b = col(&fb.b, 2);

        // trA.getOrigin () and the vector from body A to it
        let tra_o: V3 = std::array::from_fn(|k| ta.o[k] + dot(ta.b[k], fa.o));
        let trb_o: V3 = std::array::from_fn(|k| tb.o[k] + dot(tb.b[k], fb.o));
        let rel_a0: V3 = std::array::from_fn(|k| tra_o[k] - ta.o[k]);
        let rel_b0: V3 = std::array::from_fn(|k| trb_o[k] - tb.o[k]);
        let ofs = sub(trb_o, tra_o);

        let fact_a = if w + x > 0.0 { x / (w + x) } else { 0.5 };
        let has_static = w < SIMD_EPSILON || x < SIMD_EPSILON;
        let nfa = 1.0 - fact_a;

        // the hinge axes in the world, columns two of the frames
        let ax1a: V3 = std::array::from_fn(|k| dot(ta.b[k], c2a));
        let ax1b: V3 = std::array::from_fn(|k| dot(tb.b[k], c2b));
        let un: V3 = std::array::from_fn(|k| ax1a[k] * fact_a + ax1b[k] * nfa);
        let ka = 1.0 / (un[2] * un[2] + (un[0] * un[0] + un[1] * un[1])).sqrt();
        let ax1: V3 = [un[0] * ka, un[1] * ka, un[2] * ka];

        let dot_zxy = |r: V3| r[2] * ax1[2] + (r[0] * ax1[0] + r[1] * ax1[1]);
        let dot_b = dot_zxy(rel_b0);
        let ortho_b: V3 = std::array::from_fn(|k| rel_b0[k] - ax1[k] * dot_b);
        let dot_a = dot_zxy(rel_a0);
        let ortho_a: V3 = std::array::from_fn(|k| rel_a0[k] - ax1[k] * dot_a);
        let td: V3 = std::array::from_fn(|k| ax1[k] * dot_a - ax1[k] * dot_b);
        let rel_a: V3 = std::array::from_fn(|k| ortho_a[k] + fact_a * td[k]);
        let rel_b: V3 = std::array::from_fn(|k| ortho_b[k] - nfa * td[k]);

        // the average ortho to the hinge axis
        let pv: V3 = std::array::from_fn(|k| fact_a * ortho_b[k] + nfa * ortho_a[k]);
        let len2 = pv[2] * pv[2] + (pv[0] * pv[0] + pv[1] * pv[1]);
        let p: V3 = if len2 > SIMD_EPSILON {
            let s = 1.0 / len2.sqrt();
            [pv[0] * s, pv[1] * s, pv[2] * s]
        } else {
            [dot(ta.b[0], c1a), dot(ta.b[1], c1a), dot(ta.b[2], c1a)]
        };

        let q = cross(ax1, p);

        // three rows
        let ra = &mut info.rows;
        put(&mut ra[0].rel1_cross_n, cross(rel_a, p));
        put(&mut ra[0].rel2_cross_n, neg(cross(rel_b, p)));

        // the hinge never has an angular limit here (getSolveLimit () is false)
        put(&mut ra[1].rel1_cross_n, cross(rel_a, q));
        put(&mut ra[1].rel2_cross_n, neg(cross(rel_b, q)));

        let mut tmp_a = cross(rel_a, ax1);
        let mut tmp_b = cross(rel_b, ax1);

        if has_static {
            tmp_a = scale(tmp_a, fact_a);
            tmp_b = scale(tmp_b, nfa);
        }

        put(&mut ra[2].rel1_cross_n, tmp_a);
        put(&mut ra[2].rel2_cross_n, neg(tmp_b));

        let k = info.fps * info.erp;
        let ra = &mut info.rows;

        if !self.angular_only {
            put(&mut ra[0].normal1, p);
            put(&mut ra[1].normal1, q);
            put(&mut ra[2].normal1, ax1);
            put(&mut ra[0].normal2, neg(p));
            put(&mut ra[1].normal2, neg(q));
            put(&mut ra[2].normal2, neg(ax1));

            ra[0].rhs = f(k * dot(ofs, p));
            ra[1].rhs = f(k * dot(ofs, q));
            ra[2].rhs = f(k * dot(ofs, ax1));
        }

        // rows three and four keep the angular velocities perpendicular to the axis equal
        put(&mut ra[3].rel1_cross_n, p);
        put(&mut ra[4].rel1_cross_n, q);
        put(&mut ra[3].rel2_cross_n, neg(p));
        put(&mut ra[4].rel2_cross_n, neg(q));

        let u = cross(ax1a, ax1b);
        ra[3].rhs = f(k * dot(u, p));
        ra[4].rhs = f(k * dot(u, q));
    }
}

impl Slider {
    /// calculateTransforms (useLinearReferenceFrameA, not obsolete).
    pub fn calculate_transforms(&mut self, trans_a: &Tf, trans_b: &Tf) {
        self.calc_a = rt(trans_a.mul(&self.frame_a));
        self.calc_b = rt(trans_b.mul(&self.frame_b));
        self.real_pivot_a = self.calc_a.o;
        self.real_pivot_b = self.calc_b.o;
        self.slider_axis = col(&self.calc_a.b, 0);

        // m_delta is stored, but the dots use the unrounded difference
        let d = sub(self.real_pivot_b, self.real_pivot_a);
        self.delta = r3(d);

        for i in 0..3 {
            let c = col(&self.calc_a.b, i);
            self.depth[i] = f(d[0] * c[0] + d[1] * c[1] + d[2] * c[2]);
        }
    }

    /// testLinLimits.
    fn test_lin_limits(&mut self) {
        self.solve_lin = false;
        self.lin_pos = self.depth[0];
        let (lo, hi, j) = (self.lower_lin, self.upper_lin, self.lin_pos);

        if lo <= hi {
            if hi < j {
                self.depth[0] = f(j - hi);
                self.solve_lin = true;
            } else if lo > j {
                self.depth[0] = f(j - lo);
                self.solve_lin = true;
            } else {
                self.depth[0] = 0.0;
            }
        } else {
            self.depth[0] = 0.0;
        }
    }

    /// testAngLimits.
    fn test_ang_limits(&mut self) {
        self.ang_depth = 0.0;
        self.solve_ang = false;
        let (lo, hi) = (self.lower_ang, self.upper_ang);

        if lo <= hi {
            let a0 = col(&self.calc_a.b, 1);
            let a1 = col(&self.calc_a.b, 2);
            let b0 = col(&self.calc_b.b, 1);
            let rot = (a1[0] * b0[0] + a1[1] * b0[1] + a1[2] * b0[2]).atan2(a0[0] * b0[0] + a0[1] * b0[1] + a0[2] * b0[2]);
            let rot = adjust_angle_to_limits(rot, lo, hi);
            self.ang_pos = f(rot);

            if rot < lo {
                self.ang_depth = f(rot - lo);
                self.solve_ang = true;
            } else if rot > hi {
                self.ang_depth = f(rot - hi);
                self.solve_ang = true;
            }
        }
    }

    /// getInfo2NonVirtual (useOffsetForConstraintFrame = true, no motors, no flags).
    fn get_info2(&self, info: &mut Info2, rigid: &[RigidBody], a: BodyId, b: BodyId) {
        let (tra, trb) = (&self.calc_a, &self.calc_b);
        let (body_a, body_b) = (&rigid[a], &rigid[b]);
        let (w, x) = (body_a.inverse_mass, body_b.inverse_mass);
        let sign_fact = if self.use_linear_reference_frame_a { 1.0 } else { -1.0 };
        let fps = info.fps;
        let erp = info.erp;

        let ofs = sub(trb.o, tra.o);
        let fact_a = if w + x > 0.0 { x / (w + x) } else { 0.5 };
        let nfa = 1.0 - fact_a;
        let has_static = w < SIMD_EPSILON || x < SIMD_EPSILON;

        let ax1a = col(&tra.b, 0);
        let ax1b = col(&trb.b, 0);

        // the desired slider axis: the weighted sum of both frame axes
        let e = fact_a * ax1a[0];
        let l = fact_a * ax1a[1];
        let k = fact_a * ax1a[2];
        let g = nfa * ax1b[0];
        let m = nfa * ax1b[1];
        let k = k + nfa * ax1b[2];
        let n = 1.0 / ((e + g) * (e + g) + (l + m) * (l + m) + k * k).sqrt();
        let nu = [(e + g) * n, (l + m) * n, k * n];
        let ax1 = r3(nu);

        // btPlaneSpace1 on the unrounded axis; p and q stay unrounded
        let (mut p, mut q): (V3, V3);

        if nu[2].abs() > SIMDSQRT12 {
            let aa = nu[2] * nu[2] + nu[1] * nu[1];
            let kk = 1.0 / aa.sqrt();
            p = [0.0, -(nu[2] * kk), nu[1] * kk];
            q = [aa * kk, -(nu[0] * (nu[1] * kk)), nu[0] * -(nu[2] * kk)];
        } else {
            let aa = nu[0] * nu[0] + nu[1] * nu[1];
            let kk = 1.0 / aa.sqrt();
            p = [-(nu[1] * kk), nu[0] * kk, 0.0];
            q = [-(nu[2] * (nu[0] * kk)), nu[2] * -(nu[1] * kk), aa * kk];
        }

        let rows = &mut *info.rows;
        put(&mut rows[0].rel1_cross_n, p);
        put(&mut rows[1].rel1_cross_n, q);
        put(&mut rows[0].rel2_cross_n, neg(p));
        put(&mut rows[1].rel2_cross_n, neg(q));

        // the orthogonal angular rows
        let kk = (self.softness_ortho_ang * erp) * fps;
        let u = cross(ax1a, ax1b);
        rows[0].rhs = f(kk * (u[0] * p[0] + u[1] * p[1] + u[2] * p[2]));
        rows[1].rhs = f(kk * (u[0] * q[0] + u[1] * q[1] + u[2] * q[2]));

        // the offset between the frames along the slider axis
        let rel_b: V3 = sub(trb.o, body_b.world_tf.o);
        let rel_a: V3 = sub(tra.o, body_a.world_tf.o);
        let dot_b = rel_b[0] * ax1[0] + rel_b[1] * ax1[1] + rel_b[2] * ax1[2];
        let dot_a = rel_a[0] * ax1[0] + rel_a[1] * ax1[1] + rel_a[2] * ax1[2];
        let slider_offs = self.lin_pos - self.depth[0];
        let td: V3 = std::array::from_fn(|i| (dot_a * ax1[i] + slider_offs * ax1[i]) - dot_b * ax1[i]);
        let ortho_a: V3 = std::array::from_fn(|i| rel_a[i] - dot_a * ax1[i]);
        let ortho_b: V3 = std::array::from_fn(|i| rel_b[i] - dot_b * ax1[i]);
        let rel_a2: V3 = std::array::from_fn(|i| ortho_a[i] + fact_a * td[i]);
        let rel_b2: V3 = std::array::from_fn(|i| ortho_b[i] - nfa * td[i]);

        // the average ortho to the slider axis
        let pv: V3 = std::array::from_fn(|i| fact_a * ortho_b[i] + nfa * ortho_a[i]);
        let len2 = pv[2] * pv[2] + (pv[0] * pv[0] + pv[1] * pv[1]);

        if len2 > SIMD_EPSILON {
            let s = 1.0 / len2.sqrt();
            p = [pv[0] * s, pv[1] * s, pv[2] * s];
        } else {
            p = col(&tra.b, 1);
        }

        q = cross(ax1, p);

        put(&mut rows[2].rel1_cross_n, cross(rel_a2, p));
        put(&mut rows[2].rel2_cross_n, neg(cross(rel_b2, p)));

        let mut tmp_a = cross(rel_a2, q);
        let mut tmp_b = cross(rel_b2, q);

        if has_static && self.solve_ang {
            tmp_a = scale(tmp_a, fact_a);
            tmp_b = scale(tmp_b, nfa);
        }

        put(&mut rows[3].rel1_cross_n, tmp_a);
        put(&mut rows[3].rel2_cross_n, neg(tmp_b));
        put(&mut rows[2].normal1, p);
        put(&mut rows[3].normal1, q);
        put(&mut rows[2].normal2, neg(p));
        put(&mut rows[3].normal2, neg(q));

        // the orthogonal linear rows; the x component of p is the register value (unrounded when
        // it was just normalized), y and z are reloaded floats
        let kk = (self.softness_ortho_lin * erp) * fps;
        let pr = r3(p);
        let px = if len2 > SIMD_EPSILON { p[0] } else { pr[0] };
        rows[2].rhs = f(kk * (ofs[0] * px + ofs[1] * pr[1] + ofs[2] * pr[2]));
        rows[3].rhs = f(kk * (ofs[0] * q[0] + ofs[1] * q[1] + ofs[2] * q[2]));

        let mut next = 4;

        // the linear limit row
        let (limit, limit_err) = if self.solve_lin {
            let le = self.depth[0] * sign_fact;
            (if le > 0.0 { 2 } else { 1 }, le)
        } else {
            (0, 0.0)
        };

        if limit != 0 {
            let r = next;
            put(&mut rows[r].normal1, ax1);
            put(&mut rows[r].normal2, neg(ax1));

            if !has_static {
                put(&mut rows[r].rel1_cross_n, cross(rel_a2, ax1));
                put(&mut rows[r].rel2_cross_n, neg(cross(rel_b2, ax1)));
            }

            let (lostop, histop) = (self.lower_lin, self.upper_lin);
            rows[r].rhs = f(0.0 + limit_err * (erp * fps));

            if lostop == histop {
                rows[r].lower = -SIMD_INFINITY;
                rows[r].upper = SIMD_INFINITY;
            } else if limit == 1 {
                rows[r].lower = -SIMD_INFINITY;
                rows[r].upper = 0.0;
            } else {
                rows[r].lower = 0.0;
                rows[r].upper = SIMD_INFINITY;
            }

            let bounce = (1.0 - self.damping_lim_lin).abs();

            if bounce > 0.0 {
                let vel = sign_fact * (dot(body_a.lin_vel, ax1) - dot(body_b.lin_vel, ax1));

                if limit == 1 {
                    if vel < 0.0 {
                        let newc = -bounce * vel;
                        if newc > rows[r].rhs {
                            rows[r].rhs = f(newc);
                        }
                    }
                } else if vel > 0.0 {
                    let newc = -bounce * vel;
                    if newc < rows[r].rhs {
                        rows[r].rhs = f(newc);
                    }
                }
            }

            rows[r].rhs = f(self.softness_lim_lin * rows[r].rhs);
            next += 1;
        }

        // the angular limit row
        let (limit, limit_err) = if self.solve_ang {
            let le = self.ang_depth;
            (if le > 0.0 { 1 } else { 2 }, le)
        } else {
            (0, 0.0)
        };

        if limit != 0 {
            let r = next;
            put(&mut rows[r].rel1_cross_n, ax1);
            put(&mut rows[r].rel2_cross_n, neg(ax1));
            let (lostop, histop) = (self.lower_ang, self.upper_ang);
            rows[r].rhs = f(rows[r].rhs + limit_err * (erp * fps));

            if lostop == histop {
                rows[r].lower = -SIMD_INFINITY;
                rows[r].upper = SIMD_INFINITY;
            } else if limit == 1 {
                rows[r].lower = 0.0;
                rows[r].upper = SIMD_INFINITY;
            } else {
                rows[r].lower = -SIMD_INFINITY;
                rows[r].upper = 0.0;
            }

            let bounce = (1.0 - self.damping_lim_ang).abs();

            if bounce > 0.0 {
                let vel = dot(body_a.ang_vel, ax1) - dot(body_b.ang_vel, ax1);

                if limit == 1 {
                    if vel < 0.0 {
                        let newc = -bounce * vel;
                        if newc > rows[r].rhs {
                            rows[r].rhs = f(newc);
                        }
                    }
                } else if vel > 0.0 {
                    let newc = -bounce * vel;
                    if newc < rows[r].rhs {
                        rows[r].rhs = f(newc);
                    }
                }
            }

            rows[r].rhs = f(self.softness_lim_ang * rows[r].rhs);
        }
    }
}
