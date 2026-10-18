//! btCollisionObject + btRigidBody + btDefaultMotionState of Bullet 2.82.

use super::math::*;
use super::shape::ShapeId;

pub type BodyId = usize;
pub type MotionStateId = usize;

pub const ACTIVE_TAG: i32 = 1;
pub const ISLAND_SLEEPING: i32 = 2;
pub const WANTS_DEACTIVATION: i32 = 3;
pub const DISABLE_DEACTIVATION: i32 = 4;
pub const DISABLE_SIMULATION: i32 = 5;

pub const CF_STATIC_OBJECT: i32 = 1;
pub const CF_KINEMATIC_OBJECT: i32 = 2;
pub const CF_NO_CONTACT_RESPONSE: i32 = 4;

/// gDeactivationTime.
pub const G_DEACTIVATION_TIME: f64 = 2.0;

/// btRigidBody::btRigidBodyConstructionInfo.
#[derive(Clone, Debug)]
pub struct ConstructionInfo {
    pub mass: f64,
    pub motion_state: Option<MotionStateId>,
    pub start_world_transform: Tf,
    pub shape: ShapeId,
    pub local_inertia: V3,
    pub linear_damping: f64,
    pub angular_damping: f64,
    pub friction: f64,
    pub rolling_friction: f64,
    pub restitution: f64,
    pub linear_sleeping_threshold: f64,
    pub angular_sleeping_threshold: f64,
    pub additional_damping: bool,
    pub additional_damping_factor: f64,
    pub additional_linear_damping_threshold_sqr: f64,
    pub additional_angular_damping_threshold_sqr: f64,
    pub additional_angular_damping_factor: f64,
}

impl ConstructionInfo {
    /// btRigidBodyConstructionInfo (mass, motionState, collisionShape, localInertia).
    pub fn new(mass: f64, motion_state: Option<MotionStateId>, shape: ShapeId) -> ConstructionInfo {
        ConstructionInfo {
            mass,
            motion_state,
            start_world_transform: Tf::IDENTITY,
            shape,
            local_inertia: ZERO,
            linear_damping: 0.0,
            angular_damping: 0.0,
            friction: 0.5,
            rolling_friction: 0.0,
            restitution: 0.0,
            linear_sleeping_threshold: f(0.8),
            angular_sleeping_threshold: 1.0,
            additional_damping: false,
            additional_damping_factor: f(0.005),
            additional_linear_damping_threshold_sqr: f(0.01),
            additional_angular_damping_threshold_sqr: f(0.01),
            additional_angular_damping_factor: f(0.01),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RigidBody {
    // btCollisionObject
    pub world_tf: Tf,
    pub interp_tf: Tf,
    pub interp_lin: V3,
    pub interp_ang: V3,
    pub shape: ShapeId,
    pub collision_flags: i32,
    pub island_tag: i32,
    pub companion_id: i32,
    pub activation_state: i32,
    pub deactivation_time: f64,
    pub friction: f64,
    pub rolling_friction: f64,
    pub restitution: f64,
    pub hit_fraction: f64,
    pub contact_processing_threshold: f64,
    pub ccd_motion_threshold: f64,
    pub ccd_swept_sphere_radius: f64,
    pub check_collide_with: bool,
    /// m_broadphaseHandle: the proxy id while the body is in the world.
    pub proxy: Option<usize>,
    // btRigidBody
    pub inv_inertia_world: M3,
    pub lin_vel: V3,
    pub ang_vel: V3,
    pub inverse_mass: f64,
    pub lin_factor: V3,
    pub gravity: V3,
    pub gravity_acceleration: V3,
    pub inv_inertia_local: V3,
    pub total_force: V3,
    pub total_torque: V3,
    pub lin_damping: f64,
    pub ang_damping: f64,
    pub additional_damping: bool,
    pub additional_damping_factor: f64,
    pub additional_linear_damping_threshold_sqr: f64,
    pub additional_angular_damping_threshold_sqr: f64,
    pub additional_angular_damping_factor: f64,
    pub lin_sleeping_threshold: f64,
    pub ang_sleeping_threshold: f64,
    pub motion_state: Option<MotionStateId>,
    pub ang_factor: V3,
    pub flags: i32,
    pub constraint_refs: Vec<usize>,
}

impl RigidBody {
    /// btRigidBody::setupRigidBody. `motion_tf` is the motion state's world transform, if any.
    pub fn new(info: &ConstructionInfo, motion_tf: Option<Tf>) -> RigidBody {
        let world_tf = match motion_tf {
            Some(t) => t,
            None => rt(info.start_world_transform),
        };

        let mut b = RigidBody {
            world_tf,
            interp_tf: world_tf,
            interp_lin: ZERO,
            interp_ang: ZERO,
            shape: info.shape,
            // btCollisionObject(): CF_STATIC_OBJECT
            collision_flags: CF_STATIC_OBJECT,
            island_tag: -1,
            companion_id: -1,
            activation_state: 1,
            deactivation_time: 0.0,
            friction: f(info.friction),
            rolling_friction: f(info.rolling_friction),
            restitution: f(info.restitution),
            hit_fraction: 1.0,
            contact_processing_threshold: BT_LARGE_FLOAT,
            ccd_motion_threshold: 0.0,
            ccd_swept_sphere_radius: 0.0,
            check_collide_with: false,
            proxy: None,
            inv_inertia_world: [[0.0; 3]; 3],
            lin_vel: ZERO,
            ang_vel: ZERO,
            inverse_mass: 0.0,
            lin_factor: [1.0; 3],
            gravity: ZERO,
            gravity_acceleration: ZERO,
            inv_inertia_local: ZERO,
            total_force: ZERO,
            total_torque: ZERO,
            lin_damping: 0.0,
            ang_damping: 0.0,
            additional_damping: info.additional_damping,
            additional_damping_factor: f(info.additional_damping_factor),
            additional_linear_damping_threshold_sqr: f(info.additional_linear_damping_threshold_sqr),
            additional_angular_damping_threshold_sqr: f(info.additional_angular_damping_threshold_sqr),
            additional_angular_damping_factor: f(info.additional_angular_damping_factor),
            lin_sleeping_threshold: f(info.linear_sleeping_threshold),
            ang_sleeping_threshold: f(info.angular_sleeping_threshold),
            motion_state: info.motion_state,
            ang_factor: [1.0; 3],
            flags: 0,
            constraint_refs: vec![],
        };

        b.set_damping(info.linear_damping, info.angular_damping);
        b.set_mass_props(info.mass, info.local_inertia);
        b.update_inertia_tensor();
        b
    }

    pub fn is_static(&self) -> bool {
        self.collision_flags & CF_STATIC_OBJECT != 0
    }

    pub fn is_kinematic(&self) -> bool {
        self.collision_flags & CF_KINEMATIC_OBJECT != 0
    }

    pub fn is_static_or_kinematic(&self) -> bool {
        self.collision_flags & (CF_KINEMATIC_OBJECT | CF_STATIC_OBJECT) != 0
    }

    pub fn has_contact_response(&self) -> bool {
        self.collision_flags & CF_NO_CONTACT_RESPONSE == 0
    }

    pub fn merges_simulation_islands(&self) -> bool {
        self.collision_flags & (CF_STATIC_OBJECT | CF_KINEMATIC_OBJECT | CF_NO_CONTACT_RESPONSE) == 0
    }

    pub fn is_active(&self) -> bool {
        self.activation_state != ISLAND_SLEEPING && self.activation_state != DISABLE_SIMULATION
    }

    /// btCollisionObject::setActivationState.
    pub fn set_activation_state(&mut self, s: i32) {
        if self.activation_state != DISABLE_DEACTIVATION && self.activation_state != DISABLE_SIMULATION {
            self.activation_state = s;
        }
    }

    pub fn force_activation_state(&mut self, s: i32) {
        self.activation_state = s;
    }

    /// btCollisionObject::activate (forceActivation).
    pub fn activate(&mut self, force: bool) {
        super::trace(|| "[\"btRigidBody\",\"activate\"]".to_string());
        if force || self.collision_flags & (CF_STATIC_OBJECT | CF_KINEMATIC_OBJECT) == 0 {
            self.set_activation_state(ACTIVE_TAG);
            self.deactivation_time = 0.0;
        }
    }

    /// btRigidBody::setDamping.
    pub fn set_damping(&mut self, lin: f64, ang: f64) {
        super::trace(|| format!("[\"btRigidBody\",\"setDamping\",{lin},{ang}]"));
        self.lin_damping = f(lin.clamp(0.0, 1.0));
        self.ang_damping = f(ang.clamp(0.0, 1.0));
    }

    /// btRigidBody::setMassProps.
    pub fn set_mass_props(&mut self, mass: f64, inertia: V3) {
        super::trace(|| format!("[\"btRigidBody\",\"setMassProps\",{mass},{}]", super::vj(&inertia)));
        if mass == 0.0 {
            self.collision_flags |= CF_STATIC_OBJECT;
            self.inverse_mass = 0.0;
        } else {
            self.collision_flags &= !CF_STATIC_OBJECT;
            self.inverse_mass = f(1.0 / mass);
        }

        // Fg = m * a
        self.gravity = r3(scale(self.gravity_acceleration, mass));
        self.inv_inertia_local = r3([
            if inertia[0] != 0.0 { 1.0 / inertia[0] } else { 0.0 },
            if inertia[1] != 0.0 { 1.0 / inertia[1] } else { 0.0 },
            if inertia[2] != 0.0 { 1.0 / inertia[2] } else { 0.0 },
        ]);
    }

    /// btRigidBody::updateInertiaTensor.
    pub fn update_inertia_tensor(&mut self) {
        let m = scaled(&self.world_tf.b, self.inv_inertia_local);
        self.inv_inertia_world = rm(m_mul(&m, &transpose(&self.world_tf.b)));
    }

    /// btRigidBody::setGravity.
    pub fn set_gravity(&mut self, acceleration: V3) {
        super::trace(|| format!("[\"btRigidBody\",\"setGravity\",{}]", super::vj(&acceleration)));
        let acceleration = r3(acceleration);
        if self.inverse_mass != 0.0 {
            self.gravity = r3(scale(acceleration, 1.0 / self.inverse_mass));
        }
        self.gravity_acceleration = acceleration;
    }

    /// btRigidBody::setCenterOfMassTransform.
    pub fn set_center_of_mass_transform(&mut self, xform: Tf) {
        super::trace(|| format!("[\"btRigidBody\",\"setCenterOfMassTransform\",{}]", super::tj(&xform)));
        let xform = rt(xform);
        if self.is_kinematic() {
            self.interp_tf = self.world_tf;
        } else {
            self.interp_tf = xform;
        }
        self.interp_lin = self.lin_vel;
        self.interp_ang = self.ang_vel;
        self.world_tf = xform;
        self.update_inertia_tensor();
    }

    pub fn set_linear_velocity(&mut self, v: V3) {
        super::trace(|| format!("[\"btRigidBody\",\"setLinearVelocity\",{}]", super::vj(&v)));
        self.lin_vel = r3(v);
    }

    pub fn set_angular_velocity(&mut self, v: V3) {
        super::trace(|| format!("[\"btRigidBody\",\"setAngularVelocity\",{}]", super::vj(&v)));
        self.ang_vel = r3(v);
    }

    pub fn set_angular_factor(&mut self, v: V3) {
        super::trace(|| format!("[\"btRigidBody\",\"setAngularFactor\",{}]", super::vj(&v)));
        self.ang_factor = r3(v);
    }

    pub fn set_linear_factor(&mut self, v: V3) {
        self.lin_factor = r3(v);
    }

    /// btRigidBody::applyCentralForce.
    pub fn apply_central_force(&mut self, force: V3) {
        self.total_force = r3(add(self.total_force, mulv(r3(force), self.lin_factor)));
    }

    /// btRigidBody::applyTorque.
    pub fn apply_torque(&mut self, torque: V3) {
        super::trace(|| format!("[\"btRigidBody\",\"applyTorque\",{}]", super::vj(&torque)));
        self.total_torque = r3(add(self.total_torque, mulv(r3(torque), self.ang_factor)));
    }

    /// btRigidBody::applyForce (force, rel_pos).
    pub fn apply_force(&mut self, force: V3, rel_pos: V3) {
        super::trace(|| format!("[\"btRigidBody\",\"applyForce\",{},{}]", super::vj(&force), super::vj(&rel_pos)));
        let _quiet = super::StepGuard::new();
        let force = r3(force);
        self.apply_central_force(force);
        self.apply_torque(cross(r3(rel_pos), mulv(force, self.lin_factor)));
    }

    /// btRigidBody::applyGravity.
    pub fn apply_gravity(&mut self) {
        if self.is_static_or_kinematic() {
            return;
        }
        let g = self.gravity;
        self.apply_central_force(g);
    }

    pub fn clear_forces(&mut self) {
        self.total_force = ZERO;
        self.total_torque = ZERO;
    }

    /// btRigidBody::applyCentralImpulse.
    pub fn apply_central_impulse(&mut self, impulse: V3) {
        self.lin_vel = r3(add(self.lin_vel, scale(mulv(impulse, self.lin_factor), self.inverse_mass)));
    }

    /// btRigidBody::applyTorqueImpulse.
    pub fn apply_torque_impulse(&mut self, torque: V3) {
        let t = m_mul_v(&self.inv_inertia_world, torque);
        self.ang_vel = r3(add(self.ang_vel, mulv(t, self.ang_factor)));
    }

    /// btRigidBody::applyImpulse.
    pub fn apply_impulse(&mut self, impulse: V3, rel_pos: V3) {
        if self.inverse_mass != 0.0 {
            self.apply_central_impulse(impulse);
            self.apply_torque_impulse(cross(rel_pos, mulv(impulse, self.lin_factor)));
        }
    }

    /// btRigidBody::getVelocityInLocalPoint.
    pub fn velocity_in_local_point(&self, rel_pos: V3) -> V3 {
        add(self.lin_vel, cross(self.ang_vel, rel_pos))
    }

    /// btRigidBody::applyDamping.
    pub fn apply_damping(&mut self, time_step: f64) {
        self.lin_vel = r3(scale(self.lin_vel, (1.0 - self.lin_damping).powf(time_step)));
        self.ang_vel = r3(scale(self.ang_vel, (1.0 - self.ang_damping).powf(time_step)));

        if self.additional_damping {
            // Additional damping can help avoiding lowpass jitter motion, help stability for ragdolls etc.
            if len2(self.ang_vel) < self.additional_angular_damping_threshold_sqr && len2(self.lin_vel) < self.additional_linear_damping_threshold_sqr {
                self.ang_vel = r3(scale(self.ang_vel, self.additional_damping_factor));
                self.lin_vel = r3(scale(self.lin_vel, self.additional_damping_factor));
            }

            let speed = len(self.lin_vel);
            if speed < self.lin_damping {
                let damp_vel = f(0.005);
                if speed > damp_vel {
                    let dir = normalize(self.lin_vel);
                    self.lin_vel = r3(sub(self.lin_vel, scale(dir, damp_vel)));
                } else {
                    self.lin_vel = ZERO;
                }
            }

            let ang_speed = len(self.ang_vel);
            if ang_speed < self.ang_damping {
                let ang_damp_vel = f(0.005);
                if ang_speed > ang_damp_vel {
                    let dir = normalize(self.ang_vel);
                    self.ang_vel = r3(sub(self.ang_vel, scale(dir, ang_damp_vel)));
                } else {
                    self.ang_vel = ZERO;
                }
            }
        }
    }

    /// btRigidBody::predictIntegratedTransform.
    pub fn predict_integrated_transform(&self, time_step: f64) -> Tf {
        rt(integrate_transform(&self.world_tf, self.lin_vel, self.ang_vel, time_step))
    }

    /// btRigidBody::updateDeactivation.
    pub fn update_deactivation(&mut self, time_step: f64) {
        if self.activation_state == ISLAND_SLEEPING || self.activation_state == DISABLE_DEACTIVATION {
            return;
        }

        if len2(self.lin_vel) < self.lin_sleeping_threshold * self.lin_sleeping_threshold && len2(self.ang_vel) < self.ang_sleeping_threshold * self.ang_sleeping_threshold {
            self.deactivation_time = f(self.deactivation_time + time_step);
        } else {
            self.deactivation_time = 0.0;
            self.set_activation_state(0);
        }
    }

    /// btRigidBody::wantsSleeping.
    pub fn wants_sleeping(&self) -> bool {
        if self.activation_state == DISABLE_DEACTIVATION {
            return false;
        }

        // disable deactivation
        if G_DEACTIVATION_TIME == 0.0 {
            return false;
        }

        if self.activation_state == ISLAND_SLEEPING || self.activation_state == WANTS_DEACTIVATION {
            return true;
        }

        self.deactivation_time > G_DEACTIVATION_TIME
    }

    /// btCollisionObject::checkCollideWith for the constraint refs of btRigidBody.
    pub fn can_collide_with(&self, other: BodyId, constraints_enabled_between: impl Fn(usize, BodyId) -> bool) -> bool {
        if !self.check_collide_with {
            return true;
        }
        !self.constraint_refs.iter().any(|c| constraints_enabled_between(*c, other))
    }
}
