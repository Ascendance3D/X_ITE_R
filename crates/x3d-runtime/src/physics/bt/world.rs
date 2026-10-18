//! btDiscreteDynamicsWorld with btDbvtBroadphase, btCollisionDispatcher and the sequential impulse
//! solver, Bullet 2.82 (zlib license), as ammo.js exposes it. The method names mirror the Ammo
//! calls X_ITE makes (snake_case).

use super::body::*;
use super::broadphase::{DbvtBroadphase, ALL_FILTER, DEFAULT_FILTER, STATIC_FILTER};
use super::constraint::Constraint;
use super::contact::{Manifold, ManifoldId, ManifoldPool, ManifoldResult, G_CONTACT_BREAKING_THRESHOLD};
use super::dispatch::{find_algorithm, Algo, ConstraintLink, Cx, Wrap};
use super::island::{quick_sort, UnionFind};
use super::math::*;
use super::shape::Shapes;
use super::solver::{SolveCx, Solver, SolverInfo};

/// btDefaultMotionState: the graphics transform (the center of mass offset is the identity).
#[derive(Clone, Copy, Debug)]
pub struct MotionState {
    pub tf: Tf,
}

/// Table sizes of a [`World`] (see [`World::stats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldStats {
    pub collision_objects: usize,
    pub non_static: usize,
    pub constraints: usize,
    pub constraint_slots: usize,
    pub links: usize,
    pub proxies: usize,
    pub proxy_slots: usize,
    pub pairs: usize,
    pub manifolds: usize,
    pub shape_slots: usize,
    pub body_slots: usize,
    pub motion_state_slots: usize,
}

pub struct World {
    pub shapes: Shapes,
    pub bodies: Vec<RigidBody>,
    pub motion_states: Vec<MotionState>,
    pub joints: Vec<Constraint>,
    /// Slots of `joints` whose constraint was removed.
    free_joints: Vec<usize>,
    links: Vec<ConstraintLink>,
    /// m_collisionObjects.
    collision_objects: Vec<BodyId>,
    /// m_nonStaticRigidBodies.
    non_static: Vec<BodyId>,
    /// m_constraints.
    constraints: Vec<usize>,
    broadphase: DbvtBroadphase<Algo>,
    pub pool: ManifoldPool,
    gravity: V3,
    local_time: f64,
    fixed_time_step: f64,
    latency_motion_state_interpolation: bool,
    pub solver_info: SolverInfo,
    solver: Solver,
    union_find: UnionFind,
    /// m_useContinuous of btDispatcherInfo.
    use_continuous: bool,
}

impl Default for World {
    fn default() -> Self {
        World::new()
    }
}

impl World {
    pub fn new() -> World {
        World {
            shapes: Shapes::default(),
            bodies: vec![],
            motion_states: vec![],
            joints: vec![],
            free_joints: vec![],
            links: vec![],
            collision_objects: vec![],
            non_static: vec![],
            constraints: vec![],
            broadphase: DbvtBroadphase::new(),
            pool: ManifoldPool::default(),
            gravity: [0.0, -10.0, 0.0],
            local_time: 0.0,
            fixed_time_step: 0.0,
            latency_motion_state_interpolation: true,
            solver_info: SolverInfo::default(),
            solver: Solver::new(),
            union_find: UnionFind::default(),
            use_continuous: true,
        }
    }

    // --- motion states and bodies ---

    pub fn create_motion_state(&mut self, tf: Tf) -> MotionStateId {
        self.motion_states.push(MotionState { tf: rt(tf) });
        self.motion_states.len() - 1
    }

    /// btMotionState::getWorldTransform.
    pub fn motion_state_world_transform(&self, id: MotionStateId) -> Tf {
        self.motion_states[id].tf
    }

    /// btMotionState::setWorldTransform.
    pub fn set_motion_state_world_transform(&mut self, id: MotionStateId, tf: Tf) {
        super::trace(|| format!("[\"btDefaultMotionState\",\"setWorldTransform\",{}]", super::tj(&tf)));
        self.motion_states[id].tf = rt(tf);
    }

    /// new btRigidBody (constructionInfo).
    pub fn create_rigid_body(&mut self, info: &ConstructionInfo) -> BodyId {
        let motion_tf = info.motion_state.map(|m| self.motion_states[m].tf);
        self.bodies.push(RigidBody::new(info, motion_tf));
        self.bodies.len() - 1
    }

    /// btRigidBody::setMotionState: the body takes the motion state's world transform (the
    /// inverse inertia tensor is not recomputed).
    pub fn set_motion_state(&mut self, id: BodyId, ms: MotionStateId) {
        super::trace(|| "[\"btRigidBody\",\"setMotionState\",\"<btDefaultMotionState>\"]".to_string());
        self.bodies[id].motion_state = Some(ms);
        self.bodies[id].world_tf = self.motion_states[ms].tf;
    }

    pub fn body(&self, id: BodyId) -> &RigidBody {
        &self.bodies[id]
    }

    pub fn body_mut(&mut self, id: BodyId) -> &mut RigidBody {
        &mut self.bodies[id]
    }

    // --- world ---

    /// btDiscreteDynamicsWorld::setGravity.
    pub fn set_gravity(&mut self, gravity: V3) {
        super::trace(|| format!("[\"btDiscreteDynamicsWorld\",\"setGravity\",{}]", super::vj(&gravity)));
        let gravity = r3(gravity);
        self.gravity = gravity;

        for &b in &self.non_static {
            let body = &mut self.bodies[b];
            if body.is_active() && body.flags & BT_DISABLE_WORLD_GRAVITY == 0 {
                body.set_gravity(gravity);
            }
        }
    }

    pub fn gravity(&self) -> V3 {
        self.gravity
    }

    /// addRigidBody (body).
    pub fn add_rigid_body(&mut self, id: BodyId) {
        super::trace(|| "[\"btDiscreteDynamicsWorld\",\"addRigidBody\",\"<btRigidBody>\"]".to_string());
        if !self.bodies[id].is_static_or_kinematic() && self.bodies[id].flags & BT_DISABLE_WORLD_GRAVITY == 0 {
            let g = self.gravity;
            self.bodies[id].set_gravity(g);
        }

        if !self.bodies[id].is_static() {
            self.non_static.push(id);
        } else {
            self.bodies[id].set_activation_state(ISLAND_SLEEPING);
        }

        let is_dynamic = !(self.bodies[id].is_static() || self.bodies[id].is_kinematic());
        let group = if is_dynamic { DEFAULT_FILTER } else { STATIC_FILTER };
        let mask = if is_dynamic { ALL_FILTER } else { ALL_FILTER ^ STATIC_FILTER };

        // btCollisionWorld::addCollisionObject
        self.collision_objects.push(id);
        let tf = self.bodies[id].world_tf;
        let (lo, hi) = self.shapes.aabb(self.bodies[id].shape, &tf);
        let proxy = self.broadphase.create_proxy(lo, hi, id, group, mask);
        self.bodies[id].proxy = Some(proxy);
    }

    /// removeRigidBody (body).
    pub fn remove_rigid_body(&mut self, id: BodyId) {
        super::trace(|| "[\"btDiscreteDynamicsWorld\",\"removeRigidBody\",\"<btRigidBody>\"]".to_string());
        if let Some(p) = self.non_static.iter().position(|&b| b == id) {
            let last = self.non_static.len() - 1;
            self.non_static.swap(p, last);
            self.non_static.pop();
        }

        // btCollisionWorld::removeCollisionObject
        if let Some(proxy) = self.bodies[id].proxy.take() {
            self.broadphase.clean_proxy_from_pairs(proxy);
            self.broadphase.destroy_proxy(proxy);
            self.release_dropped();
        }

        if let Some(p) = self.collision_objects.iter().position(|&b| b == id) {
            let last = self.collision_objects.len() - 1;
            self.collision_objects.swap(p, last);
            self.collision_objects.pop();
        }
    }

    /// addConstraint (constraint, disableCollisionsBetweenLinkedBodies); returns the constraint id.
    pub fn add_constraint(&mut self, c: Constraint, disable_collisions_between_linked_bodies: bool) -> usize {
        // (a removed constraint's slot is reused, so the table does not grow with joint churn)
        let link = ConstraintLink { a: c.body_a, b: c.body_b, enabled: c.enabled };
        let (a, b) = (c.body_a, c.body_b);
        let id = match self.free_joints.pop() {
            Some(id) => {
                self.links[id] = link;
                self.joints[id] = c;
                id
            }
            None => {
                self.links.push(link);
                self.joints.push(c);
                self.joints.len() - 1
            }
        };
        self.constraints.push(id);

        if disable_collisions_between_linked_bodies {
            for body in [a, b] {
                let rb = &mut self.bodies[body];
                if !rb.constraint_refs.contains(&id) {
                    rb.constraint_refs.push(id);
                }
                rb.check_collide_with = true;
            }
        }

        id
    }

    /// removeConstraint.
    pub fn remove_constraint(&mut self, id: usize) {
        if let Some(p) = self.constraints.iter().position(|&c| c == id) {
            let last = self.constraints.len() - 1;
            self.constraints.swap(p, last);
            self.constraints.pop();
        }

        let (a, b) = (self.joints[id].body_a, self.joints[id].body_b);

        for body in [a, b] {
            let rb = &mut self.bodies[body];
            if let Some(p) = rb.constraint_refs.iter().position(|&c| c == id) {
                let last = rb.constraint_refs.len() - 1;
                rb.constraint_refs.swap(p, last);
                rb.constraint_refs.pop();
            }
            rb.check_collide_with = !rb.constraint_refs.is_empty();
        }

        if !self.free_joints.contains(&id) {
            self.free_joints.push(id);
        }
    }

    // --- bookkeeping (batch 08: engine-level lifecycle counters) ---

    /// The sizes of every table of the world, for the lifecycle tests: live entries and the slots ever allocated.
    pub fn stats(&self) -> WorldStats {
        WorldStats {
            collision_objects: self.collision_objects.len(),
            non_static: self.non_static.len(),
            constraints: self.constraints.len(),
            constraint_slots: self.joints.len(),
            links: self.links.len(),
            proxies: self.broadphase.proxies.iter().filter(|p| p.is_some()).count(),
            proxy_slots: self.broadphase.proxies.len(),
            pairs: self.broadphase.pairs.len(),
            manifolds: self.pool.num_manifolds(),
            shape_slots: self.shapes.v.len(),
            body_slots: self.bodies.len(),
            motion_state_slots: self.motion_states.len(),
        }
    }

    // --- dispatcher ---

    pub fn num_manifolds(&self) -> usize {
        self.pool.num_manifolds()
    }

    /// btDispatcher::getManifoldByIndexInternal.
    pub fn manifold_by_index(&self, i: usize) -> &Manifold {
        self.pool.get(self.pool.by_index(i))
    }

    /// Release the manifolds of algorithms the pair cache dropped.
    fn release_dropped(&mut self) {
        let dropped = std::mem::take(&mut self.broadphase.dropped);
        if dropped.is_empty() {
            return;
        }
        let mut cx = Cx { shapes: &self.shapes, bodies: &self.bodies, pool: &mut self.pool, links: &self.links };
        for a in dropped {
            a.release(&mut cx);
        }
    }

    // --- simulation ---

    /// stepSimulation (timeStep, maxSubSteps, fixedTimeStep); returns the number of substeps.
    pub fn step_simulation(&mut self, time_step: f64, max_sub_steps: i32, fixed_time_step: f64) -> i32 {
        super::trace(|| format!("[\"btDiscreteDynamicsWorld\",\"stepSimulation\",{time_step},{max_sub_steps},{fixed_time_step}]"));
        let _guard = super::StepGuard::new();
        let mut max_sub_steps = max_sub_steps;
        let mut fixed_time_step = fixed_time_step;
        let num_simulation_sub_steps;

        if max_sub_steps != 0 {
            // fixed timestep with interpolation
            self.fixed_time_step = f(fixed_time_step);
            // asm.js keeps the unrounded sum in a register: the comparison, the quotient and the
            // remainder all use it, only the member is rounded when stored
            let sum = self.local_time + time_step;
            self.local_time = f(sum);

            if sum >= fixed_time_step {
                num_simulation_sub_steps = (sum / fixed_time_step) as i32;
                self.local_time = f(sum - num_simulation_sub_steps as f64 * fixed_time_step);
            } else {
                num_simulation_sub_steps = 0;
            }
        } else {
            // variable timestep
            fixed_time_step = time_step;
            self.local_time = if self.latency_motion_state_interpolation { 0.0 } else { f(time_step) };
            self.fixed_time_step = 0.0;

            if time_step.abs() < SIMD_EPSILON {
                num_simulation_sub_steps = 0;
                max_sub_steps = 0;
            } else {
                num_simulation_sub_steps = 1;
                max_sub_steps = 1;
            }
        }

        if num_simulation_sub_steps != 0 {
            // clamp the number of substeps, to prevent simulation grinding spiralling down to a halt
            let clamped = num_simulation_sub_steps.min(max_sub_steps);
            self.save_kinematic_state(fixed_time_step * clamped as f64);
            self.apply_gravity();

            for _ in 0..clamped {
                self.internal_single_step_simulation(fixed_time_step);
                self.synchronize_motion_states();
            }
        } else {
            self.synchronize_motion_states();
        }

        self.clear_forces();
        num_simulation_sub_steps
    }

    fn save_kinematic_state(&mut self, time_step: f64) {
        for i in 0..self.collision_objects.len() {
            let b = self.collision_objects[i];
            if self.bodies[b].activation_state != ISLAND_SLEEPING && self.bodies[b].is_kinematic() {
                // btRigidBody::saveKinematicState
                if let Some(ms) = self.bodies[b].motion_state {
                    self.bodies[b].world_tf = self.motion_states[ms].tf;
                }

                if time_step != 0.0 {
                    let body = &mut self.bodies[b];
                    let (lin, ang) = calculate_velocity(&body.interp_tf, &body.world_tf, time_step);
                    body.lin_vel = r3(lin);
                    body.ang_vel = r3(ang);
                    body.interp_lin = body.lin_vel;
                    body.interp_ang = body.ang_vel;
                    body.interp_tf = body.world_tf;
                }
            }
        }
    }

    fn apply_gravity(&mut self) {
        for &b in &self.non_static {
            if self.bodies[b].is_active() {
                self.bodies[b].apply_gravity();
            }
        }
    }

    fn clear_forces(&mut self) {
        for &b in &self.non_static {
            self.bodies[b].clear_forces();
        }
    }

    fn synchronize_single_motion_state(&mut self, b: BodyId) {
        let body = &self.bodies[b];

        if let (Some(ms), false) = (body.motion_state, body.is_static_or_kinematic()) {
            let t = if self.latency_motion_state_interpolation && self.fixed_time_step != 0.0 { self.local_time - self.fixed_time_step } else { self.local_time * body.hit_fraction };
            let tf = integrate_transform(&body.interp_tf, body.interp_lin, body.interp_ang, t);
            self.motion_states[ms].tf = rt(tf);
        }
    }

    fn synchronize_motion_states(&mut self) {
        for i in 0..self.non_static.len() {
            let b = self.non_static[i];
            if self.bodies[b].is_active() {
                self.synchronize_single_motion_state(b);
            }
        }
    }

    fn internal_single_step_simulation(&mut self, time_step: f64) {
        // apply gravity, predict motion
        self.predict_unconstraint_motion(time_step);
        // (createPredictiveContacts: only bodies with a CCD motion threshold take part)
        // perform collision detection
        self.perform_discrete_collision_detection();
        self.calculate_simulation_islands();
        self.solver_info.time_step = f(time_step);
        // solve contact and other joint constraints
        self.solve_constraints();
        // integrate transforms
        self.integrate_transforms(time_step);
        self.update_activation_state(time_step);
    }

    fn predict_unconstraint_motion(&mut self, time_step: f64) {
        for i in 0..self.non_static.len() {
            let b = self.non_static[i];
            let body = &mut self.bodies[b];
            if !body.is_static_or_kinematic() {
                // don't integrate/update velocities here, it happens in the constraint solver
                body.apply_damping(time_step);
                body.interp_tf = body.predict_integrated_transform(time_step);
            }
        }
    }

    fn integrate_transforms(&mut self, time_step: f64) {
        for i in 0..self.non_static.len() {
            let b = self.non_static[i];
            let body = &mut self.bodies[b];
            body.hit_fraction = 1.0;

            if body.is_active() && !body.is_static_or_kinematic() {
                let predicted = body.predict_integrated_transform(time_step);
                // (CCD motion clamping needs a CCD square motion threshold, X_ITE sets none)
                body.set_center_of_mass_transform(predicted);
            }
        }
    }

    fn update_activation_state(&mut self, time_step: f64) {
        for i in 0..self.non_static.len() {
            let b = self.non_static[i];
            let body = &mut self.bodies[b];
            body.update_deactivation(time_step);

            if body.wants_sleeping() {
                if body.is_static_or_kinematic() {
                    body.set_activation_state(ISLAND_SLEEPING);
                } else {
                    if body.activation_state == ACTIVE_TAG {
                        body.set_activation_state(WANTS_DEACTIVATION);
                    }

                    if body.activation_state == ISLAND_SLEEPING {
                        body.ang_vel = ZERO;
                        body.lin_vel = ZERO;
                    }
                }
            } else if body.activation_state != DISABLE_DEACTIVATION {
                body.set_activation_state(ACTIVE_TAG);
            }
        }
    }

    // --- collision detection ---

    fn update_single_aabb(&mut self, b: BodyId) {
        let body = &self.bodies[b];
        let (mut lo, mut hi) = self.shapes.aabb(body.shape, &body.world_tf);
        let contact_threshold = [G_CONTACT_BREAKING_THRESHOLD; 3];
        lo = r3(sub(lo, contact_threshold));
        hi = r3(add(hi, contact_threshold));

        if self.use_continuous && !body.is_static_or_kinematic() {
            let (lo2, hi2) = self.shapes.aabb(body.shape, &body.interp_tf);
            let lo2 = r3(sub(lo2, contact_threshold));
            let hi2 = r3(add(hi2, contact_threshold));

            for i in 0..3 {
                if lo2[i] < lo[i] {
                    lo[i] = lo2[i];
                }
                if hi2[i] > hi[i] {
                    hi[i] = hi2[i];
                }
            }
        }

        // moving objects should be moderately sized, probably something wrong if not
        if body.is_static() || len2(sub(hi, lo)) < 1e12 {
            let proxy = body.proxy.expect("body not in the world");
            self.broadphase.set_aabb(proxy, lo, hi);
        } else {
            self.bodies[b].set_activation_state(DISABLE_SIMULATION);
        }
    }

    /// btCollisionWorld::performDiscreteCollisionDetection.
    pub fn perform_discrete_collision_detection(&mut self) {
        // updateAabbs (m_forceUpdateAllAabbs)
        for i in 0..self.collision_objects.len() {
            let b = self.collision_objects[i];
            self.update_single_aabb(b);
        }

        // computeOverlappingPairs
        self.broadphase.calculate_overlapping_pairs();
        self.release_dropped();

        // dispatchAllCollisionPairs
        let mut i = 0;
        while i < self.broadphase.pairs.len() {
            let (p0, p1) = (self.broadphase.pairs[i].p0, self.broadphase.pairs[i].p1);
            let b0 = self.broadphase.proxy(p0).client;
            let b1 = self.broadphase.proxy(p1).client;
            self.near_callback(i, b0, b1);
            i += 1;
        }
    }

    /// btCollisionDispatcher::defaultNearCallback.
    fn near_callback(&mut self, pair: usize, b0: BodyId, b1: BodyId) {
        let mut algo = self.broadphase.pairs[pair].algo.take();
        let w0 = Wrap { shape: self.bodies[b0].shape, tf: self.bodies[b0].world_tf, body: b0 };
        let w1 = Wrap { shape: self.bodies[b1].shape, tf: self.bodies[b1].world_tf, body: b1 };
        let mut cx = Cx { shapes: &self.shapes, bodies: &self.bodies, pool: &mut self.pool, links: &self.links };

        if cx.needs_collision(b0, b1) {
            // dispatcher will keep algorithms persistent in the collision pair
            if algo.is_none() {
                algo = Some(Box::new(find_algorithm(&mut cx, &w0, &w1)));
            }

            if let Some(a) = algo.as_mut() {
                let mut res = ManifoldResult::new(b0, b1);
                a.process(&mut cx, &w0, &w1, &mut res);
            }
        }

        self.broadphase.pairs[pair].algo = algo;
    }

    // --- islands and the solver ---

    fn calculate_simulation_islands(&mut self) {
        // updateActivationState
        let mut index = 0;

        for &o in &self.collision_objects {
            let body = &mut self.bodies[o];
            if !body.is_static_or_kinematic() {
                body.island_tag = index;
                index += 1;
            }
            body.companion_id = -1;
            body.hit_fraction = 1.0;
        }

        self.union_find.reset(index as usize);

        // findUnions
        for pair in &self.broadphase.pairs {
            let c0 = self.broadphase.proxy(pair.p0).client;
            let c1 = self.broadphase.proxy(pair.p1).client;
            if self.bodies[c0].merges_simulation_islands() && self.bodies[c1].merges_simulation_islands() {
                self.union_find.unite(self.bodies[c0].island_tag, self.bodies[c1].island_tag);
            }
        }

        // merge islands based on constraints
        for &c in &self.constraints {
            let j = &self.joints[c];
            if j.enabled {
                let (a, b) = (j.body_a, j.body_b);
                if !self.bodies[a].is_static_or_kinematic() && !self.bodies[b].is_static_or_kinematic() {
                    self.union_find.unite(self.bodies[a].island_tag, self.bodies[b].island_tag);
                }
            }
        }

        // storeIslandActivationState: store the island id in each body
        let mut index = 0;

        for (i, &o) in self.collision_objects.iter().enumerate() {
            if !self.bodies[o].is_static_or_kinematic() {
                let tag = self.union_find.find(index);
                self.bodies[o].island_tag = tag;
                // set the correct object offset in the collision object array
                self.union_find.elements[index as usize].sz = i as i32;
                self.bodies[o].companion_id = -1;
                index += 1;
            } else {
                self.bodies[o].island_tag = -1;
                self.bodies[o].companion_id = -2;
            }
        }
    }

    fn constraint_island_id(&self, c: usize) -> i32 {
        let j = &self.joints[c];
        let t0 = self.bodies[j.body_a].island_tag;
        if t0 >= 0 { t0 } else { self.bodies[j.body_b].island_tag }
    }

    fn manifold_island_id(&self, m: ManifoldId) -> i32 {
        let mf = self.pool.get(m);
        let t0 = self.bodies[mf.body0].island_tag;
        if t0 >= 0 { t0 } else { self.bodies[mf.body1].island_tag }
    }

    /// solveConstraints.
    fn solve_constraints(&mut self) {
        // sort the constraints on their island
        let mut sorted: Vec<usize> = self.constraints.clone();
        {
            let ids: Vec<(usize, i32)> = sorted.iter().map(|&c| (c, self.constraint_island_id(c))).collect();
            let mut keyed = ids;
            quick_sort(&mut keyed, &|a: &(usize, i32), b: &(usize, i32)| a.1 < b.1);
            sorted = keyed.into_iter().map(|(c, _)| c).collect();
        }

        // buildIslands: update the sleeping state
        self.union_find.sort_islands();
        let num_elem = self.union_find.elements.len();
        let mut end_island_index;
        let mut start_island_index = 0;

        while start_island_index < num_elem {
            let island_id = self.union_find.elements[start_island_index].id;
            end_island_index = start_island_index + 1;
            while end_island_index < num_elem && self.union_find.elements[end_island_index].id == island_id {
                end_island_index += 1;
            }

            let mut all_sleeping = true;

            for idx in start_island_index..end_island_index {
                let i = self.union_find.elements[idx].sz as usize;
                let o = self.collision_objects[i];
                let body = &self.bodies[o];
                if body.island_tag == island_id {
                    if body.activation_state == ACTIVE_TAG {
                        all_sleeping = false;
                    }
                    if body.activation_state == DISABLE_DEACTIVATION {
                        all_sleeping = false;
                    }
                }
            }

            for idx in start_island_index..end_island_index {
                let i = self.union_find.elements[idx].sz as usize;
                let o = self.collision_objects[i];
                let body = &mut self.bodies[o];
                if body.island_tag == island_id {
                    if all_sleeping {
                        body.set_activation_state(ISLAND_SLEEPING);
                    } else if body.activation_state == ISLAND_SLEEPING {
                        body.set_activation_state(WANTS_DEACTIVATION);
                        body.deactivation_time = 0.0;
                    }
                }
            }

            start_island_index = end_island_index;
        }

        // collect the manifolds that take part in the simulation
        let mut island_manifold: Vec<ManifoldId> = vec![];
        let max_num_manifolds = self.pool.num_manifolds();

        for i in 0..max_num_manifolds {
            let m = self.pool.by_index(i);
            let (b0, b1) = {
                let mf = self.pool.get(m);
                (mf.body0, mf.body1)
            };

            if self.bodies[b0].activation_state != ISLAND_SLEEPING || self.bodies[b1].activation_state != ISLAND_SLEEPING {
                // kinematic objects don't merge islands, but wake up all connected objects
                if self.bodies[b0].is_kinematic() && self.bodies[b0].activation_state != ISLAND_SLEEPING && self.bodies[b0].has_contact_response() {
                    self.bodies[b1].activate(false);
                }

                if self.bodies[b1].is_kinematic() && self.bodies[b1].activation_state != ISLAND_SLEEPING && self.bodies[b1].has_contact_response() {
                    self.bodies[b0].activate(false);
                }

                // filtering for response
                let cx = Cx { shapes: &self.shapes, bodies: &self.bodies, pool: &mut self.pool, links: &self.links };
                if cx.needs_response(b0, b1) {
                    island_manifold.push(m);
                }
            }
        }

        {
            let keyed: Vec<(ManifoldId, i32)> = island_manifold.iter().map(|&m| (m, self.manifold_island_id(m))).collect();
            let mut keyed = keyed;
            quick_sort(&mut keyed, &|a: &(ManifoldId, i32), b: &(ManifoldId, i32)| a.1 < b.1);
            island_manifold = keyed.into_iter().map(|(m, _)| m).collect();
        }

        // process the islands
        let num_manifolds = island_manifold.len();
        let mut start_manifold_index = 0;
        let mut end_manifold_index = 1;
        let mut batch_bodies: Vec<BodyId> = vec![];
        let mut batch_manifolds: Vec<ManifoldId> = vec![];
        let mut batch_constraints: Vec<usize> = vec![];
        let mut start_island_index = 0;

        while start_island_index < num_elem {
            let island_id = self.union_find.elements[start_island_index].id;
            let mut island_sleeping = true;
            let mut island_bodies: Vec<BodyId> = vec![];
            let mut end_island_index = start_island_index;

            while end_island_index < num_elem && self.union_find.elements[end_island_index].id == island_id {
                let i = self.union_find.elements[end_island_index].sz as usize;
                let o = self.collision_objects[i];
                island_bodies.push(o);
                if self.bodies[o].is_active() {
                    island_sleeping = false;
                }
                end_island_index += 1;
            }

            // find the accompanying contact manifolds for this islandId
            let mut island_manifolds: &[ManifoldId] = &[];

            if start_manifold_index < num_manifolds {
                let cur_island_id = self.manifold_island_id(island_manifold[start_manifold_index]);

                if cur_island_id == island_id {
                    end_manifold_index = start_manifold_index + 1;
                    while end_manifold_index < num_manifolds && island_id == self.manifold_island_id(island_manifold[end_manifold_index]) {
                        end_manifold_index += 1;
                    }
                    island_manifolds = &island_manifold[start_manifold_index..end_manifold_index];
                }
            }

            if !island_sleeping {
                // processIsland: also add all non-contact constraints/joints for this island
                let mut start_constraint = None;
                let mut k = 0;
                while k < sorted.len() {
                    if self.constraint_island_id(sorted[k]) == island_id {
                        start_constraint = Some(k);
                        break;
                    }
                    k += 1;
                }

                let mut num_cur = 0;
                if start_constraint.is_some() {
                    while k < sorted.len() {
                        if self.constraint_island_id(sorted[k]) == island_id {
                            num_cur += 1;
                        }
                        k += 1;
                    }
                }

                // minimum solver batch size
                batch_bodies.extend_from_slice(&island_bodies);
                batch_manifolds.extend_from_slice(island_manifolds);

                if let Some(s) = start_constraint {
                    batch_constraints.extend_from_slice(&sorted[s..s + num_cur]);
                }

                if batch_constraints.len() + batch_manifolds.len() > self.solver_info.minimum_solver_batch_size {
                    self.process_constraints(&mut batch_bodies, &mut batch_manifolds, &mut batch_constraints);
                }
            }

            if !island_manifolds.is_empty() {
                start_manifold_index = end_manifold_index;
            }

            start_island_index = end_island_index;
        }

        self.process_constraints(&mut batch_bodies, &mut batch_manifolds, &mut batch_constraints);
    }

    /// InplaceSolverIslandCallback::processConstraints.
    fn process_constraints(&mut self, bodies: &mut Vec<BodyId>, manifolds: &mut Vec<ManifoldId>, constraints: &mut Vec<usize>) {
        let info = self.solver_info.clone();
        let mut cx = SolveCx { rigid: &mut self.bodies, pool: &mut self.pool, joints: &mut self.joints };
        self.solver.solve_group(&mut cx, bodies, manifolds, constraints, &info);
        bodies.clear();
        manifolds.clear();
        constraints.clear();

        for (i, j) in self.joints.iter().enumerate() {
            self.links[i].enabled = j.enabled;
        }
    }
}

/// btTransformUtil::calculateVelocity.
fn calculate_velocity(t0: &Tf, t1: &Tf, time_step: f64) -> (V3, V3) {
    let lin = scale(sub(t1.o, t0.o), 1.0 / time_step);
    let dorn = qmul(t1.rotation(), qinverse(t0.rotation()));
    // calculateDiffAxisAngleQuaternion
    let mut q = dorn;
    if q[3] < 0.0 {
        q = qneg(q);
    }
    let n = qnormalize(q);
    let angle = qangle(n);
    let s = (1.0 - n[3] * n[3]).max(0.0).sqrt();
    let axis = if s < 1e-4 { [1.0, 0.0, 0.0] } else { [n[0] / s, n[1] / s, n[2] / s] };
    (lin, scale(axis, angle / time_step))
}

/// BT_DISABLE_WORLD_GRAVITY.
pub const BT_DISABLE_WORLD_GRAVITY: i32 = 1;
