//! btSequentialImpulseConstraintSolver of Bullet 2.82 (zlib license): solver bodies, contact and
//! friction rows, joint rows from `getInfo1`/`getInfo2`, split impulse, warm starting, and the
//! write back, with the default btContactSolverInfo (10 iterations, split impulse, erp 0.2,
//! erp2 0.8, warm starting factor 0.85, SOLVER_USE_WARMSTARTING | SOLVER_SIMD, no randomizing).
//!
//! Members of the C++ structs are rounded to float when stored; locals are doubles.

use super::body::{BodyId, RigidBody};
use super::constraint::Constraint;
use super::contact::{ManifoldId, ManifoldPool};
use super::math::*;

/// btContactSolverInfo (defaults).
#[derive(Clone, Debug)]
pub struct SolverInfo {
    pub tau: f64,
    pub damping: f64,
    pub friction: f64,
    pub time_step: f64,
    pub restitution: f64,
    pub num_iterations: i32,
    pub max_error_reduction: f64,
    pub sor: f64,
    pub erp: f64,
    pub erp2: f64,
    pub global_cfm: f64,
    pub split_impulse: bool,
    pub split_impulse_penetration_threshold: f64,
    pub split_impulse_turn_erp: f64,
    pub linear_slop: f64,
    pub warmstarting_factor: f64,
    pub minimum_solver_batch_size: usize,
    pub single_axis_rolling_friction_threshold: f64,
}

impl Default for SolverInfo {
    fn default() -> Self {
        SolverInfo {
            tau: f(0.6),
            damping: 1.0,
            friction: f(0.3),
            time_step: f(1.0 / 60.0),
            restitution: 0.0,
            num_iterations: 10,
            max_error_reduction: 20.0,
            sor: 1.0,
            erp: f(0.2),
            erp2: f(0.8),
            global_cfm: 0.0,
            split_impulse: true,
            split_impulse_penetration_threshold: f(-0.04),
            split_impulse_turn_erp: f(0.1),
            linear_slop: 0.0,
            warmstarting_factor: f(0.85),
            minimum_solver_batch_size: 128,
            single_axis_rolling_friction_threshold: f(1e30),
        }
    }
}

/// btSolverBody.
#[derive(Clone, Debug)]
pub struct SolverBody {
    pub world_tf: Tf,
    pub delta_lin: V3,
    pub delta_ang: V3,
    pub ang_factor: V3,
    pub lin_factor: V3,
    pub inv_mass: V3,
    pub push: V3,
    pub turn: V3,
    pub lin_vel: V3,
    pub ang_vel: V3,
    pub ext_force_impulse: V3,
    pub ext_torque_impulse: V3,
    pub original: Option<BodyId>,
}

impl SolverBody {
    /// getVelocityInLocalPointNoDelta.
    fn velocity_no_delta(&self, rel_pos: V3) -> V3 {
        if self.original.is_some() {
            add(add(self.lin_vel, self.ext_force_impulse), cross(add(self.ang_vel, self.ext_torque_impulse), rel_pos))
        } else {
            ZERO
        }
    }

    /// internalApplyImpulse.
    fn apply_impulse(&mut self, linear: V3, angular: V3, magnitude: f64) {
        if self.original.is_some() {
            self.delta_lin = r3(add(self.delta_lin, mulv(scale(linear, magnitude), self.lin_factor)));
            self.delta_ang = r3(add(self.delta_ang, mulv(angular, scale(self.ang_factor, magnitude))));
        }
    }

    /// internalApplyPushImpulse.
    fn apply_push_impulse(&mut self, linear: V3, angular: V3, magnitude: f64) {
        if self.original.is_some() {
            self.push = r3(add(self.push, mulv(scale(linear, magnitude), self.lin_factor)));
            self.turn = r3(add(self.turn, mulv(angular, scale(self.ang_factor, magnitude))));
        }
    }
}

/// What a solver row was created from.
#[derive(Clone, Copy, Debug)]
pub enum Origin {
    None,
    /// (manifold, point index)
    Contact(ManifoldId, usize),
    Joint(usize),
}

/// btSolverConstraint.
#[derive(Clone, Copy, Debug)]
pub struct SolverConstraint {
    pub rel1_cross_n: V3,
    pub normal1: V3,
    pub rel2_cross_n: V3,
    pub normal2: V3,
    pub ang_comp_a: V3,
    pub ang_comp_b: V3,
    pub applied_push: f64,
    pub applied: f64,
    pub friction: f64,
    pub jac_diag_ab_inv: f64,
    pub rhs: f64,
    pub cfm: f64,
    pub lower: f64,
    pub upper: f64,
    pub rhs_penetration: f64,
    pub origin: Origin,
    pub override_iterations: i32,
    pub friction_index: usize,
    pub body_a: usize,
    pub body_b: usize,
}

impl SolverConstraint {
    pub fn zero() -> SolverConstraint {
        SolverConstraint {
            rel1_cross_n: ZERO,
            normal1: ZERO,
            rel2_cross_n: ZERO,
            normal2: ZERO,
            ang_comp_a: ZERO,
            ang_comp_b: ZERO,
            applied_push: 0.0,
            applied: 0.0,
            friction: 0.0,
            jac_diag_ab_inv: 0.0,
            rhs: 0.0,
            cfm: 0.0,
            lower: 0.0,
            upper: 0.0,
            rhs_penetration: 0.0,
            origin: Origin::None,
            override_iterations: 0,
            friction_index: 0,
            body_a: 0,
            body_b: 0,
        }
    }
}

/// btConstraintInfo2 with the rows it points into.
pub struct Info2<'a> {
    pub fps: f64,
    pub erp: f64,
    pub rows: &'a mut [SolverConstraint],
    pub num_iterations: i32,
    pub damping: f64,
}

/// resolveSingleConstraintRowGeneric.
fn resolve_row_generic(bodies: &mut [SolverBody], c: &mut SolverConstraint) {
    let (ia, ib) = (c.body_a, c.body_b);
    let mut delta_impulse = c.rhs - c.applied * c.cfm;
    let dv1 = dot(c.normal1, bodies[ia].delta_lin) + dot(c.rel1_cross_n, bodies[ia].delta_ang);
    let dv2 = dot(c.normal2, bodies[ib].delta_lin) + dot(c.rel2_cross_n, bodies[ib].delta_ang);
    delta_impulse -= dv1 * c.jac_diag_ab_inv;
    delta_impulse -= dv2 * c.jac_diag_ab_inv;
    let sum = c.applied + delta_impulse;

    if sum < c.lower {
        delta_impulse = c.lower - c.applied;
        c.applied = c.lower;
    } else if sum > c.upper {
        delta_impulse = c.upper - c.applied;
        c.applied = c.upper;
    } else {
        c.applied = f(sum);
    }

    let la = mulv(c.normal1, bodies[ia].inv_mass);
    bodies[ia].apply_impulse(la, c.ang_comp_a, delta_impulse);
    let lb = mulv(c.normal2, bodies[ib].inv_mass);
    bodies[ib].apply_impulse(lb, c.ang_comp_b, delta_impulse);
}

/// resolveSingleConstraintRowLowerLimit.
fn resolve_row_lower_limit(bodies: &mut [SolverBody], c: &mut SolverConstraint) {
    let (ia, ib) = (c.body_a, c.body_b);
    let mut delta_impulse = c.rhs - c.applied * c.cfm;
    let dv1 = dot(c.normal1, bodies[ia].delta_lin) + dot(c.rel1_cross_n, bodies[ia].delta_ang);
    let dv2 = dot(c.normal2, bodies[ib].delta_lin) + dot(c.rel2_cross_n, bodies[ib].delta_ang);
    delta_impulse -= dv1 * c.jac_diag_ab_inv;
    delta_impulse -= dv2 * c.jac_diag_ab_inv;
    let sum = c.applied + delta_impulse;

    if sum < c.lower {
        delta_impulse = c.lower - c.applied;
        c.applied = c.lower;
    } else {
        c.applied = f(sum);
    }

    let la = mulv(c.normal1, bodies[ia].inv_mass);
    bodies[ia].apply_impulse(la, c.ang_comp_a, delta_impulse);
    let lb = mulv(c.normal2, bodies[ib].inv_mass);
    bodies[ib].apply_impulse(lb, c.ang_comp_b, delta_impulse);
}

/// resolveSplitPenetrationImpulseCacheFriendly.
fn resolve_split_penetration(bodies: &mut [SolverBody], c: &mut SolverConstraint) {
    if c.rhs_penetration != 0.0 {
        let (ia, ib) = (c.body_a, c.body_b);
        let mut delta_impulse = c.rhs_penetration - c.applied_push * c.cfm;
        let dv1 = dot(c.normal1, bodies[ia].push) + dot(c.rel1_cross_n, bodies[ia].turn);
        let dv2 = dot(c.normal2, bodies[ib].push) + dot(c.rel2_cross_n, bodies[ib].turn);
        delta_impulse -= dv1 * c.jac_diag_ab_inv;
        delta_impulse -= dv2 * c.jac_diag_ab_inv;
        let sum = c.applied_push + delta_impulse;

        if sum < c.lower {
            delta_impulse = c.lower - c.applied_push;
            c.applied_push = c.lower;
        } else {
            c.applied_push = f(sum);
        }

        let la = mulv(c.normal1, bodies[ia].inv_mass);
        bodies[ia].apply_push_impulse(la, c.ang_comp_a, delta_impulse);
        let lb = mulv(c.normal2, bodies[ib].inv_mass);
        bodies[ib].apply_push_impulse(lb, c.ang_comp_b, delta_impulse);
    }
}

/// The solver and its scratch pools.
#[derive(Default)]
pub struct Solver {
    bodies: Vec<SolverBody>,
    contacts: Vec<SolverConstraint>,
    frictions: Vec<SolverConstraint>,
    rollings: Vec<SolverConstraint>,
    non_contacts: Vec<SolverConstraint>,
    fixed_body_id: i32,
    max_override_iterations: i32,
}

/// The mutable world pieces the solver works on.
pub struct SolveCx<'a> {
    pub rigid: &'a mut [RigidBody],
    pub pool: &'a mut ManifoldPool,
    pub joints: &'a mut [Constraint],
}

impl Solver {
    pub fn new() -> Solver {
        Solver { fixed_body_id: -1, ..Default::default() }
    }

    /// initSolverBody.
    fn init_solver_body(rb: Option<&RigidBody>, id: Option<BodyId>, time_step: f64) -> SolverBody {
        let mut sb = SolverBody {
            world_tf: Tf::IDENTITY,
            delta_lin: ZERO,
            delta_ang: ZERO,
            ang_factor: [1.0; 3],
            lin_factor: [1.0; 3],
            inv_mass: ZERO,
            push: ZERO,
            turn: ZERO,
            lin_vel: ZERO,
            ang_vel: ZERO,
            ext_force_impulse: ZERO,
            ext_torque_impulse: ZERO,
            original: None,
        };

        if let Some(rb) = rb {
            sb.world_tf = rb.world_tf;
            sb.inv_mass = r3(mulv([rb.inverse_mass; 3], rb.lin_factor));
            sb.original = id;
            sb.ang_factor = rb.ang_factor;
            sb.lin_factor = rb.lin_factor;
            sb.lin_vel = rb.lin_vel;
            sb.ang_vel = rb.ang_vel;
            sb.ext_force_impulse = r3(scale(scale(rb.total_force, rb.inverse_mass), time_step));
            // getTotalTorque() * getInvInertiaTensorWorld() * timeStep: vector times matrix
            sb.ext_torque_impulse = r3(scale(v_mul_m(rb.total_torque, &rb.inv_inertia_world), time_step));
        }

        sb
    }

    /// getOrInitSolverBody.
    fn get_or_init_solver_body(&mut self, rigid: &mut [RigidBody], body: BodyId, time_step: f64) -> usize {
        if rigid[body].companion_id >= 0 {
            return rigid[body].companion_id as usize;
        }

        let rb = &rigid[body];
        // convert both active and kinematic objects (for their velocity)
        if rb.inverse_mass != 0.0 || rb.is_kinematic() {
            let id = self.bodies.len();
            self.bodies.push(Self::init_solver_body(Some(rb), Some(body), time_step));
            rigid[body].companion_id = id as i32;
            id
        } else {
            if self.fixed_body_id < 0 {
                self.fixed_body_id = self.bodies.len() as i32;
                self.bodies.push(Self::init_solver_body(None, None, time_step));
            }
            self.fixed_body_id as usize
        }
    }

    /// setupFrictionConstraint.
    #[allow(clippy::too_many_arguments)]
    fn setup_friction_constraint(&self, rigid: &[RigidBody], sc: &mut SolverConstraint, normal_axis: V3, ia: usize, ib: usize, combined_friction: f64, rel_pos1: V3, rel_pos2: V3, relaxation: f64, desired_velocity: f64, cfm_slip: f64) {
        let (sba, sbb) = (&self.bodies[ia], &self.bodies[ib]);
        let body0 = sba.original.map(|i| &rigid[i]);
        let body1 = sbb.original.map(|i| &rigid[i]);
        sc.body_a = ia;
        sc.body_b = ib;
        sc.friction = f(combined_friction);
        sc.origin = Origin::None;
        sc.applied = 0.0;
        sc.applied_push = 0.0;

        // the compiled code keeps the cross products and angular components unrounded in locals
        // and stores them as floats
        let (mut cross1, mut cross2) = (ZERO, ZERO);
        let (mut ang1, mut ang2) = (ZERO, ZERO);

        if let Some(b0) = body0 {
            sc.normal1 = r3(normal_axis);
            cross1 = cross(rel_pos1, sc.normal1);
            ang1 = mulv(m_mul_v(&b0.inv_inertia_world, cross1), b0.ang_factor);
            sc.rel1_cross_n = r3(cross1);
            sc.ang_comp_a = r3(ang1);
        } else {
            sc.normal1 = ZERO;
            sc.rel1_cross_n = ZERO;
            sc.ang_comp_a = ZERO;
        }

        if let Some(b1) = body1 {
            sc.normal2 = r3(neg(normal_axis));
            cross2 = cross(rel_pos2, sc.normal2);
            ang2 = mulv(m_mul_v(&b1.inv_inertia_world, cross2), b1.ang_factor);
            sc.rel2_cross_n = r3(cross2);
            sc.ang_comp_b = r3(ang2);
        } else {
            sc.normal2 = ZERO;
            sc.rel2_cross_n = ZERO;
            sc.ang_comp_b = ZERO;
        }

        let mut denom0 = 0.0;
        let mut denom1 = 0.0;

        if let Some(b0) = body0 {
            let vec = cross(ang1, rel_pos1);
            denom0 = b0.inverse_mass + dot(normal_axis, vec);
        }

        if let Some(b1) = body1 {
            let vec = cross(neg(ang2), rel_pos2);
            denom1 = b1.inverse_mass + dot(normal_axis, vec);
        }

        let jac = relaxation / (denom0 + denom1);
        sc.jac_diag_ab_inv = f(jac);

        let vel1_dotn = dot(sc.normal1, if body0.is_some() { add(sba.lin_vel, sba.ext_force_impulse) } else { ZERO }) + dot(cross1, if body0.is_some() { sba.ang_vel } else { ZERO });
        let vel2_dotn = dot(sc.normal2, if body1.is_some() { add(sbb.lin_vel, sbb.ext_force_impulse) } else { ZERO }) + dot(cross2, if body1.is_some() { sbb.ang_vel } else { ZERO });
        let rel_vel = vel1_dotn + vel2_dotn;
        let velocity_error = desired_velocity - rel_vel;
        let velocity_impulse = velocity_error * jac;
        sc.rhs = f(velocity_impulse);
        sc.cfm = f(cfm_slip);
        sc.lower = f(-sc.friction);
        sc.upper = sc.friction;
    }

    /// setupContactConstraint.
    #[allow(clippy::too_many_arguments)]
    fn setup_contact_constraint(&mut self, rigid: &[RigidBody], sc: &mut SolverConstraint, ia: usize, ib: usize, cp: &super::contact::ManifoldPoint, info: &SolverInfo, rel_pos1: V3, rel_pos2: V3) -> f64 {
        let relaxation = 1.0;
        let rb0 = self.bodies[ia].original.map(|i| &rigid[i]);
        let rb1 = self.bodies[ib].original.map(|i| &rigid[i]);
        // the compiled code keeps the angular components unrounded in locals for the denominators
        let torque_axis0 = cross(rel_pos1, cp.normal_world_on_b);
        let ang_u0 = match rb0 {
            Some(b) => mulv(m_mul_v(&b.inv_inertia_world, torque_axis0), b.ang_factor),
            None => ZERO,
        };
        sc.ang_comp_a = r3(ang_u0);
        let torque_axis1 = cross(rel_pos2, cp.normal_world_on_b);
        let ang_u1 = match rb1 {
            Some(b) => mulv(m_mul_v(&b.inv_inertia_world, neg(torque_axis1)), b.ang_factor),
            None => ZERO,
        };
        sc.ang_comp_b = r3(ang_u1);

        let mut denom0 = 0.0;
        let mut denom1 = 0.0;

        if let Some(b0) = rb0 {
            let vec = cross(ang_u0, rel_pos1);
            denom0 = b0.inverse_mass + dot(cp.normal_world_on_b, vec);
        }

        if let Some(b1) = rb1 {
            let vec = cross(neg(ang_u1), rel_pos2);
            denom1 = b1.inverse_mass + dot(cp.normal_world_on_b, vec);
        }

        sc.jac_diag_ab_inv = f(relaxation / (denom0 + denom1));

        if rb0.is_some() {
            sc.normal1 = cp.normal_world_on_b;
            sc.rel1_cross_n = r3(torque_axis0);
        } else {
            sc.normal1 = ZERO;
            sc.rel1_cross_n = ZERO;
        }

        if rb1.is_some() {
            sc.normal2 = r3(neg(cp.normal_world_on_b));
            sc.rel2_cross_n = r3(neg(torque_axis1));
        } else {
            sc.normal2 = ZERO;
            sc.rel2_cross_n = ZERO;
        }

        let penetration = cp.distance() + info.linear_slop;
        let vel1 = match rb0 {
            Some(b) => b.velocity_in_local_point(rel_pos1),
            None => ZERO,
        };
        let vel2 = match rb1 {
            Some(b) => b.velocity_in_local_point(rel_pos2),
            None => ZERO,
        };
        let vel = sub(vel1, vel2);
        let rel_vel = dot(cp.normal_world_on_b, vel);
        sc.friction = f(cp.combined_friction);
        // restitutionCurve
        let mut restitution = cp.combined_restitution * -rel_vel;

        if restitution <= 0.0 {
            restitution = 0.0;
        }

        // warm starting
        // the first body gets the unrounded impulse, the second the stored float
        let applied_u = cp.applied_impulse * info.warmstarting_factor;
        sc.applied = f(applied_u);

        if rb0.is_some() {
            let lin = mulv(mulv(sc.normal1, self.bodies[ia].inv_mass), rb0.unwrap().lin_factor);
            self.bodies[ia].apply_impulse(lin, sc.ang_comp_a, applied_u);
        }

        if rb1.is_some() {
            let lin = mulv(mulv(neg(sc.normal2), self.bodies[ib].inv_mass), rb1.unwrap().lin_factor);
            self.bodies[ib].apply_impulse(lin, neg(sc.ang_comp_b), -sc.applied);
        }

        sc.applied_push = 0.0;

        let (sba, sbb) = (&self.bodies[ia], &self.bodies[ib]);
        let (ef_a, et_a) = if sba.original.is_some() { (sba.ext_force_impulse, sba.ext_torque_impulse) } else { (ZERO, ZERO) };
        let (ef_b, et_b) = if sbb.original.is_some() { (sbb.ext_force_impulse, sbb.ext_torque_impulse) } else { (ZERO, ZERO) };
        let vel1_dotn = dot(sc.normal1, add(sba.lin_vel, ef_a)) + dot(sc.rel1_cross_n, add(sba.ang_vel, et_a));
        let vel2_dotn = dot(sc.normal2, add(sbb.lin_vel, ef_b)) + dot(sc.rel2_cross_n, add(sbb.ang_vel, et_b));
        let rel_vel = vel1_dotn + vel2_dotn;

        let positional_error;
        let mut velocity_error = restitution - rel_vel;
        let mut erp = info.erp2;

        if !info.split_impulse || penetration > info.split_impulse_penetration_threshold {
            erp = info.erp;
        }

        if penetration > 0.0 {
            positional_error = 0.0;
            velocity_error -= penetration / info.time_step;
        } else {
            positional_error = -penetration * erp / info.time_step;
        }

        let penetration_impulse = positional_error * sc.jac_diag_ab_inv;
        let velocity_impulse = velocity_error * sc.jac_diag_ab_inv;

        if !info.split_impulse || penetration > info.split_impulse_penetration_threshold {
            sc.rhs = f(penetration_impulse + velocity_impulse);
            sc.rhs_penetration = 0.0;
        } else {
            sc.rhs = f(velocity_impulse);
            sc.rhs_penetration = f(penetration_impulse);
        }

        sc.cfm = 0.0;
        sc.lower = 0.0;
        sc.upper = f(1e10_f32 as f64);
        relaxation
    }

    /// convertContact.
    fn convert_contact(&mut self, cx: &mut SolveCx, manifold: ManifoldId, info: &SolverInfo) {
        let (b0, b1) = {
            let m = cx.pool.get(manifold);
            (m.body0, m.body1)
        };
        let ia = self.get_or_init_solver_body(cx.rigid, b0, info.time_step);
        let ib = self.get_or_init_solver_body(cx.rigid, b1, info.time_step);

        // avoid collision response between two static objects
        let zero_mass = |s: &SolverBody| s.inv_mass == ZERO;
        if zero_mass(&self.bodies[ia]) && zero_mass(&self.bodies[ib]) {
            return;
        }

        let mut rolling_friction = 1;
        let n = cx.pool.get(manifold).points.len();
        let processing = cx.pool.get(manifold).contact_processing_threshold;

        for j in 0..n {
            let cp = cx.pool.get(manifold).points[j];

            if cp.distance() > processing {
                continue;
            }

            let friction_index = self.contacts.len();
            let mut sc = SolverConstraint::zero();
            sc.body_a = ia;
            sc.body_b = ib;
            sc.origin = Origin::Contact(manifold, j);
            let pos1 = cp.position_world_on_a;
            let pos2 = cp.position_world_on_b;
            let rel_pos1 = sub(pos1, cx.rigid[b0].world_tf.o);
            let rel_pos2 = sub(pos2, cx.rigid[b1].world_tf.o);
            let vel1 = self.bodies[ia].velocity_no_delta(rel_pos1);
            let vel2 = self.bodies[ib].velocity_no_delta(rel_pos2);
            let vel = sub(vel1, vel2);
            let rel_vel = dot(cp.normal_world_on_b, vel);
            let relaxation = self.setup_contact_constraint(cx.rigid, &mut sc, ia, ib, &cp, info, rel_pos1, rel_pos2);
            sc.friction_index = self.frictions.len();

            let ang_vel_a = cx.rigid[b0].ang_vel;
            let ang_vel_b = cx.rigid[b1].ang_vel;
            let rel_ang_vel = sub(ang_vel_b, ang_vel_a);

            if cp.combined_rolling_friction > 0.0 && rolling_friction > 0 {
                // only a single rollingFriction per manifold
                rolling_friction -= 1;

                if len(rel_ang_vel) > info.single_axis_rolling_friction_threshold {
                    let mut rav = normalize(rel_ang_vel);
                    if len(rav) > 0.001 {
                        rav = r3(rav);
                        let mut r = SolverConstraint::zero();
                        r.friction_index = friction_index;
                        self.setup_rolling_friction_constraint(cx.rigid, &mut r, rav, ia, ib, cp.combined_rolling_friction, rel_pos1, rel_pos2, relaxation);
                        self.rollings.push(r);
                    }
                } else {
                    let mut r = SolverConstraint::zero();
                    r.friction_index = friction_index;
                    self.setup_rolling_friction_constraint(cx.rigid, &mut r, cp.normal_world_on_b, ia, ib, cp.combined_rolling_friction, rel_pos1, rel_pos2, relaxation);
                    self.rollings.push(r);
                    let (axis0, axis1) = plane_space1(cp.normal_world_on_b);

                    if len(axis0) > 0.001 {
                        let mut r = SolverConstraint::zero();
                        r.friction_index = friction_index;
                        self.setup_rolling_friction_constraint(cx.rigid, &mut r, axis0, ia, ib, cp.combined_rolling_friction, rel_pos1, rel_pos2, relaxation);
                        self.rollings.push(r);
                    }

                    if len(axis1) > 0.001 {
                        let mut r = SolverConstraint::zero();
                        r.friction_index = friction_index;
                        self.setup_rolling_friction_constraint(cx.rigid, &mut r, axis1, ia, ib, cp.combined_rolling_friction, rel_pos1, rel_pos2, relaxation);
                        self.rollings.push(r);
                    }
                }
            }

            // one velocity dependent friction direction (SOLVER_USE_2_FRICTION_DIRECTIONS is off)
            let mut dir1 = sub(vel, scale(cp.normal_world_on_b, rel_vel));
            let lat_rel_vel = len2(dir1);
            let mut fr = SolverConstraint::zero();
            fr.friction_index = friction_index;

            if lat_rel_vel > SIMD_EPSILON {
                dir1 = scale(dir1, 1.0 / lat_rel_vel.sqrt());
                let dir1 = r3(dir1);
                cx.pool.get_mut(manifold).points[j].lateral_friction_dir1 = dir1;
                self.setup_friction_constraint(cx.rigid, &mut fr, dir1, ia, ib, cp.combined_friction, r3(rel_pos1), r3(rel_pos2), relaxation, 0.0, 0.0);
            } else {
                let (d1, d2) = plane_space1(cp.normal_world_on_b);
                let (d1, d2) = (r3(d1), r3(d2));
                let pt = &mut cx.pool.get_mut(manifold).points[j];
                pt.lateral_friction_dir1 = d1;
                pt.lateral_friction_dir2 = d2;
                self.setup_friction_constraint(cx.rigid, &mut fr, d1, ia, ib, cp.combined_friction, r3(rel_pos1), r3(rel_pos2), relaxation, 0.0, 0.0);
            }

            self.frictions.push(fr);

            // setFrictionConstraintImpulse
            let b0o = self.bodies[ia].original;
            let b1o = self.bodies[ib].original;
            let fidx = sc.friction_index;
            let applied_l1 = cx.pool.get(manifold).points[j].applied_impulse_lateral1;
            let mut fc = self.frictions[fidx];
            let applied_u = applied_l1 * info.warmstarting_factor;
            fc.applied = f(applied_u);

            if let Some(o0) = b0o {
                let lin = mulv(scale(fc.normal1, cx.rigid[o0].inverse_mass), cx.rigid[o0].lin_factor);
                self.bodies[ia].apply_impulse(lin, fc.ang_comp_a, applied_u);
            }

            if let Some(o1) = b1o {
                let lin = mulv(scale(neg(fc.normal2), cx.rigid[o1].inverse_mass), cx.rigid[o1].lin_factor);
                self.bodies[ib].apply_impulse(lin, neg(fc.ang_comp_b), -fc.applied);
            }

            self.frictions[fidx] = fc;
            self.contacts.push(sc);
        }
    }

    /// setupRollingFrictionConstraint.
    #[allow(clippy::too_many_arguments)]
    fn setup_rolling_friction_constraint(&self, rigid: &[RigidBody], sc: &mut SolverConstraint, normal_axis1: V3, ia: usize, ib: usize, combined_rolling: f64, _rel_pos1: V3, _rel_pos2: V3, _relaxation: f64) {
        let (sba, sbb) = (&self.bodies[ia], &self.bodies[ib]);
        let body0 = sba.original.map(|i| &rigid[i]);
        let body1 = sbb.original.map(|i| &rigid[i]);
        sc.normal1 = ZERO;
        sc.normal2 = ZERO;
        sc.body_a = ia;
        sc.body_b = ib;
        sc.friction = f(combined_rolling);
        sc.origin = Origin::None;
        sc.applied = 0.0;
        sc.applied_push = 0.0;

        let ftorque_axis1 = neg(normal_axis1);
        sc.rel1_cross_n = r3(ftorque_axis1);
        sc.ang_comp_a = match body0 {
            Some(b) => r3(mulv(m_mul_v(&b.inv_inertia_world, ftorque_axis1), b.ang_factor)),
            None => ZERO,
        };
        let ftorque_axis2 = normal_axis1;
        sc.rel2_cross_n = r3(ftorque_axis2);
        sc.ang_comp_b = match body1 {
            Some(b) => r3(mulv(m_mul_v(&b.inv_inertia_world, ftorque_axis2), b.ang_factor)),
            None => ZERO,
        };

        let imja_a = match body0 {
            Some(b) => m_mul_v(&b.inv_inertia_world, sc.rel1_cross_n),
            None => ZERO,
        };
        let imja_b = match body1 {
            Some(b) => m_mul_v(&b.inv_inertia_world, sc.rel2_cross_n),
            None => ZERO,
        };
        let mut sum = 0.0;
        sum += dot(imja_a, sc.rel1_cross_n);
        sum += dot(imja_b, sc.rel2_cross_n);
        let jac = 1.0 / sum;
        sc.jac_diag_ab_inv = f(jac);

        let vel1_dotn = dot(sc.normal1, if body0.is_some() { add(sba.lin_vel, sba.ext_force_impulse) } else { ZERO }) + dot(sc.rel1_cross_n, if body0.is_some() { sba.ang_vel } else { ZERO });
        let vel2_dotn = dot(sc.normal2, if body1.is_some() { add(sbb.lin_vel, sbb.ext_force_impulse) } else { ZERO }) + dot(sc.rel2_cross_n, if body1.is_some() { sbb.ang_vel } else { ZERO });
        let rel_vel = vel1_dotn + vel2_dotn;
        let velocity_error = 0.0 - rel_vel;
        // (the compiled code multiplies with the quotient it has just stored: unrounded, as in setup_friction_constraint)
        sc.rhs = f(velocity_error * jac);
        sc.cfm = 0.0;
        sc.lower = f(-sc.friction);
        sc.upper = sc.friction;
    }

    /// solveGroupCacheFriendlySetup.
    fn setup(&mut self, cx: &mut SolveCx, bodies: &[BodyId], manifolds: &[ManifoldId], constraints: &[usize], info: &SolverInfo) {
        self.fixed_body_id = -1;
        self.max_override_iterations = 0;

        for &b in bodies {
            cx.rigid[b].companion_id = -1;
        }

        self.bodies.clear();
        self.contacts.clear();
        self.frictions.clear();
        self.rollings.clear();
        self.non_contacts.clear();

        // convert all bodies
        for &b in bodies {
            self.get_or_init_solver_body(cx.rigid, b, info.time_step);
        }

        for &j in constraints {
            cx.joints[j].build_jacobian(cx.rigid);
            cx.joints[j].applied_impulse = 0.0;
        }

        // calculate the total number of constraint rows
        let mut sizes = vec![0usize; constraints.len()];
        let mut total = 0;

        for (k, &j) in constraints.iter().enumerate() {
            if cx.joints[j].enabled {
                sizes[k] = cx.joints[j].get_info1(cx.rigid).0;
            }
            total += sizes[k];
        }

        // setup the btSolverConstraints
        let mut rows_all: Vec<SolverConstraint> = vec![SolverConstraint::zero(); total];
        let mut current_row = 0;

        for (k, &j) in constraints.iter().enumerate() {
            if sizes[k] == 0 {
                continue;
            }

            let (rb_a, rb_b) = (cx.joints[j].body_a, cx.joints[j].body_b);
            let ia = self.get_or_init_solver_body(cx.rigid, rb_a, info.time_step);
            let ib = self.get_or_init_solver_body(cx.rigid, rb_b, info.time_step);
            let override_iterations = if cx.joints[j].override_iterations > 0 { cx.joints[j].override_iterations } else { info.num_iterations };

            if override_iterations > self.max_override_iterations {
                self.max_override_iterations = override_iterations;
            }

            let rows = &mut rows_all[current_row..current_row + sizes[k]];

            for r in rows.iter_mut() {
                *r = SolverConstraint::zero();
                r.lower = -SIMD_INFINITY;
                r.upper = SIMD_INFINITY;
                r.body_a = ia;
                r.body_b = ib;
                r.override_iterations = override_iterations;
            }

            for id in [ia, ib] {
                let b = &mut self.bodies[id];
                b.delta_lin = ZERO;
                b.delta_ang = ZERO;
                b.push = ZERO;
                b.turn = ZERO;
            }

            let cfm0 = f(info.global_cfm);

            for r in rows.iter_mut() {
                r.cfm = cfm0;
            }

            let info2_damping = {
                let mut info2 = Info2 { fps: f(1.0 / info.time_step), erp: info.erp, rows, num_iterations: info.num_iterations, damping: info.damping };
                cx.joints[j].get_info2(&mut info2, cx.rigid);
                info2.damping
            };

            // finalize the constraint setup
            let breaking = cx.joints[j].breaking_impulse_threshold;
            let inv_a = cx.rigid[rb_a].inverse_mass;
            let inv_b = cx.rigid[rb_b].inverse_mass;
            let inv_ia = cx.rigid[rb_a].inv_inertia_world;
            let inv_ib = cx.rigid[rb_b].inv_inertia_world;
            let af_a = cx.rigid[rb_a].ang_factor;
            let af_b = cx.rigid[rb_b].ang_factor;
            let lv_a = cx.rigid[rb_a].lin_vel;
            let av_a = cx.rigid[rb_a].ang_vel;
            let lv_b = cx.rigid[rb_b].lin_vel;
            let av_b = cx.rigid[rb_b].ang_vel;
            let (ef_a, et_a) = if self.bodies[ia].original.is_some() { (self.bodies[ia].ext_force_impulse, self.bodies[ia].ext_torque_impulse) } else { (ZERO, ZERO) };
            let (ef_b, et_b) = if self.bodies[ib].original.is_some() { (self.bodies[ib].ext_force_impulse, self.bodies[ib].ext_torque_impulse) } else { (ZERO, ZERO) };

            for r in rows.iter_mut() {
                if r.upper >= breaking {
                    r.upper = f(breaking);
                }

                if r.lower <= -breaking {
                    r.lower = f(-breaking);
                }

                r.origin = Origin::Joint(j);
                r.ang_comp_a = r3(mulv(m_mul_v(&inv_ia, r.rel1_cross_n), af_a));
                r.ang_comp_b = r3(mulv(m_mul_v(&inv_ib, r.rel2_cross_n), af_b));

                let imjl_a = scale(r.normal1, inv_a);
                let imja_a = m_mul_v(&inv_ia, r.rel1_cross_n);
                let imjl_b = scale(r.normal2, inv_b);
                let imja_b = m_mul_v(&inv_ib, r.rel2_cross_n);
                let mut sum = dot(imjl_a, r.normal1);
                sum += dot(imja_a, r.rel1_cross_n);
                sum += dot(imjl_b, r.normal2);
                sum += dot(imja_b, r.rel2_cross_n);
                let fsum = sum.abs();
                // the asm keeps the unrounded reciprocal in a register for the rhs below
                let jac_u = if fsum > SIMD_EPSILON { 1.0 / sum } else { 0.0 };
                r.jac_diag_ab_inv = f(jac_u);

                let vel1_dotn = dot(r.normal1, add(lv_a, ef_a)) + dot(r.rel1_cross_n, add(av_a, et_a));
                let vel2_dotn = dot(r.normal2, add(lv_b, ef_b)) + dot(r.rel2_cross_n, add(av_b, et_b));
                let rel_vel = vel1_dotn + vel2_dotn;
                let restitution = 0.0;
                let positional_error = r.rhs;
                let velocity_error = restitution - rel_vel * info2_damping;
                let penetration_impulse = positional_error * jac_u;
                let velocity_impulse = velocity_error * jac_u;
                r.rhs = f(penetration_impulse + velocity_impulse);
                r.applied = 0.0;
            }

            current_row += sizes[k];
        }

        self.non_contacts = rows_all;

        for &m in manifolds {
            self.convert_contact(cx, m, info);
        }
    }

    /// solveSingleIteration (SOLVER_SIMD branch without SOLVER_INTERLEAVE_CONTACT_AND_FRICTION).
    fn single_iteration(&mut self, iteration: i32, info: &SolverInfo) {
        let Solver { bodies, contacts, frictions, rollings, non_contacts, .. } = self;

        for c in non_contacts.iter_mut() {
            if iteration < c.override_iterations {
                resolve_row_generic(bodies, c);
            }
        }

        if iteration < info.num_iterations {
            // (solveConstraintObsolete is not used by the getInfo2 based joints)
            for c in contacts.iter_mut() {
                resolve_row_lower_limit(bodies, c);
            }

            for fr in frictions.iter_mut() {
                let total_impulse = contacts[fr.friction_index].applied;

                if total_impulse > 0.0 {
                    fr.lower = f(-(fr.friction * total_impulse));
                    fr.upper = f(fr.friction * total_impulse);
                    resolve_row_generic(bodies, fr);
                }
            }

            for rf in rollings.iter_mut() {
                let total_impulse = contacts[rf.friction_index].applied;

                if total_impulse > 0.0 {
                    let mut magnitude = rf.friction * total_impulse;

                    if magnitude > rf.friction {
                        magnitude = rf.friction;
                    }

                    rf.lower = f(-magnitude);
                    rf.upper = f(magnitude);
                    resolve_row_generic(bodies, rf);
                }
            }
        }
    }

    /// solveGroupCacheFriendlyIterations.
    fn iterations(&mut self, info: &SolverInfo) {
        // split impulse iterations
        if info.split_impulse {
            for _ in 0..info.num_iterations {
                let Solver { bodies, contacts, .. } = self;
                for c in contacts.iter_mut() {
                    resolve_split_penetration(bodies, c);
                }
            }
        }

        let max_iterations = self.max_override_iterations.max(info.num_iterations);

        for it in 0..max_iterations {
            self.single_iteration(it, info);
        }
    }

    /// solveGroupCacheFriendlyFinish.
    fn finish(&mut self, cx: &mut SolveCx, info: &SolverInfo) {
        // warm starting
        for (k, c) in self.contacts.iter().enumerate() {
            if let Origin::Contact(m, j) = c.origin {
                let pt = &mut cx.pool.get_mut(m).points[j];
                pt.applied_impulse = c.applied;
                pt.applied_impulse_lateral1 = self.frictions[c.friction_index].applied;
                let _ = k;
            }
        }

        for c in &self.non_contacts {
            if let Origin::Joint(j) = c.origin {
                let jt = &mut cx.joints[j];
                jt.applied_impulse = c.applied;

                if c.applied.abs() >= jt.breaking_impulse_threshold {
                    jt.enabled = false;
                }
            }
        }

        for sb in self.bodies.iter_mut() {
            let Some(id) = sb.original else { continue };

            // writebackVelocityAndTransform / writebackVelocity
            sb.lin_vel = r3(add(sb.lin_vel, sb.delta_lin));
            sb.ang_vel = r3(add(sb.ang_vel, sb.delta_ang));

            if info.split_impulse {
                if sb.push != ZERO || sb.turn != ZERO {
                    let nt = integrate_transform(&sb.world_tf, sb.push, r3(scale(sb.turn, info.split_impulse_turn_erp)), info.time_step);
                    sb.world_tf = rt(nt);
                }
            }

            let body = &mut cx.rigid[id];
            body.lin_vel = r3(add(sb.lin_vel, sb.ext_force_impulse));
            body.ang_vel = r3(add(sb.ang_vel, sb.ext_torque_impulse));

            if info.split_impulse {
                body.world_tf = sb.world_tf;
            }

            body.companion_id = -1;
        }

        self.contacts.clear();
        self.non_contacts.clear();
        self.frictions.clear();
        self.rollings.clear();
        self.bodies.clear();
    }

    /// solveGroup.
    pub fn solve_group(&mut self, cx: &mut SolveCx, bodies: &[BodyId], manifolds: &[ManifoldId], constraints: &[usize], info: &SolverInfo) {
        self.setup(cx, bodies, manifolds, constraints, info);
        self.iterations(info);
        self.finish(cx, info);
    }
}

