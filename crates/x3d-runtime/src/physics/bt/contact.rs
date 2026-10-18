//! btManifoldPoint, btPersistentManifold, btManifoldResult and the manifold pool of
//! btCollisionDispatcher (Bullet 2.82, zlib license).

use super::body::{BodyId, RigidBody};
use super::math::*;

pub type ManifoldId = usize;

/// gContactBreakingThreshold.
pub const G_CONTACT_BREAKING_THRESHOLD: f64 = 0.02_f32 as f64;

/// btManifoldPoint.
#[derive(Clone, Copy, Debug)]
pub struct ManifoldPoint {
    pub local_a: V3,
    pub local_b: V3,
    pub position_world_on_b: V3,
    pub position_world_on_a: V3,
    pub normal_world_on_b: V3,
    pub distance1: f64,
    pub combined_friction: f64,
    pub combined_rolling_friction: f64,
    pub combined_restitution: f64,
    pub lateral_friction_initialized: bool,
    pub applied_impulse: f64,
    pub applied_impulse_lateral1: f64,
    pub applied_impulse_lateral2: f64,
    pub contact_motion1: f64,
    pub contact_motion2: f64,
    pub contact_cfm1: f64,
    pub contact_cfm2: f64,
    pub life_time: i32,
    pub lateral_friction_dir1: V3,
    pub lateral_friction_dir2: V3,
}

impl ManifoldPoint {
    /// btManifoldPoint (pointA, pointB, normal, distance).
    pub fn new(point_a: V3, point_b: V3, normal: V3, distance: f64) -> ManifoldPoint {
        ManifoldPoint {
            local_a: r3(point_a),
            local_b: r3(point_b),
            position_world_on_b: ZERO,
            position_world_on_a: ZERO,
            normal_world_on_b: r3(normal),
            distance1: f(distance),
            combined_friction: 0.0,
            combined_rolling_friction: 0.0,
            combined_restitution: 0.0,
            lateral_friction_initialized: false,
            applied_impulse: 0.0,
            applied_impulse_lateral1: 0.0,
            applied_impulse_lateral2: 0.0,
            contact_motion1: 0.0,
            contact_motion2: 0.0,
            contact_cfm1: 0.0,
            contact_cfm2: 0.0,
            life_time: 0,
            lateral_friction_dir1: ZERO,
            lateral_friction_dir2: ZERO,
        }
    }

    pub fn distance(&self) -> f64 {
        self.distance1
    }
}

pub const MANIFOLD_CACHE_SIZE: usize = 4;

/// btPersistentManifold.
#[derive(Clone, Debug)]
pub struct Manifold {
    pub body0: BodyId,
    pub body1: BodyId,
    pub points: Vec<ManifoldPoint>,
    pub contact_breaking_threshold: f64,
    pub contact_processing_threshold: f64,
    pub index1a: usize,
}

impl Manifold {
    pub fn num_contacts(&self) -> usize {
        self.points.len()
    }

    /// getCacheEntry.
    pub fn get_cache_entry(&self, new_point: &ManifoldPoint) -> i32 {
        let mut shortest = self.contact_breaking_threshold * self.contact_breaking_threshold;
        let mut nearest = -1;

        for (i, mp) in self.points.iter().enumerate() {
            let diff_a = sub(mp.local_a, new_point.local_a);
            let dist = dot(diff_a, diff_a);
            if dist < shortest {
                shortest = dist;
                nearest = i as i32;
            }
        }

        nearest
    }

    /// sortCachedPoints (KEEP_DEEPEST_POINT, gContactCalcArea3Points).
    fn sort_cached_points(&self, pt: &ManifoldPoint) -> usize {
        let c = &self.points;
        let mut max_penetration_index: i32 = -1;
        let mut max_penetration = pt.distance();

        for (i, p) in c.iter().enumerate().take(4) {
            if p.distance() < max_penetration {
                max_penetration_index = i as i32;
                max_penetration = p.distance();
            }
        }

        let area = |a: V3, b: V3| len2(cross(a, b));
        let (mut res0, mut res1, mut res2, mut res3) = (0.0, 0.0, 0.0, 0.0);

        if max_penetration_index != 0 {
            res0 = area(sub(pt.local_a, c[1].local_a), sub(c[3].local_a, c[2].local_a));
        }
        if max_penetration_index != 1 {
            res1 = area(sub(pt.local_a, c[0].local_a), sub(c[3].local_a, c[2].local_a));
        }
        if max_penetration_index != 2 {
            res2 = area(sub(pt.local_a, c[0].local_a), sub(c[3].local_a, c[1].local_a));
        }
        if max_penetration_index != 3 {
            res3 = area(sub(pt.local_a, c[0].local_a), sub(c[2].local_a, c[1].local_a));
        }

        // btVector4.closestAxis4 (maxAxis4 of the absolute values)
        // (the areas stay unrounded in the compiled code, as in the btVector4 construction)
        let res = [res0, res1, res2, res3];
        let mut best = 0;
        let mut value = -BT_LARGE_FLOAT;
        for (i, r) in res.iter().enumerate() {
            if r.abs() > value {
                best = i;
                value = r.abs();
            }
        }
        best
    }

    /// addManifoldPoint.
    pub fn add_manifold_point(&mut self, new_point: ManifoldPoint) -> usize {
        let mut insert_index = self.num_contacts();

        if insert_index == MANIFOLD_CACHE_SIZE {
            insert_index = self.sort_cached_points(&new_point);
            self.points[insert_index] = new_point;
        } else {
            self.points.push(new_point);
        }

        insert_index
    }

    /// removeContactPoint.
    pub fn remove_contact_point(&mut self, index: usize) {
        let last = self.points.len() - 1;
        if index != last {
            self.points[index] = self.points[last];
        }
        self.points.pop();
    }

    /// replaceContactPoint.
    pub fn replace_contact_point(&mut self, new_point: ManifoldPoint, insert_index: usize) {
        let old = self.points[insert_index];
        let life_time = old.life_time;
        let applied = old.applied_impulse;
        let applied_l1 = old.applied_impulse_lateral1;
        let applied_l2 = old.applied_impulse_lateral2;
        let mut np = new_point;

        // (BT_CONTACT_PERSISTENCY) keep the cached impulses
        np.applied_impulse = applied;
        np.applied_impulse_lateral1 = applied_l1;
        np.applied_impulse_lateral2 = applied_l2;
        np.life_time = life_time;
        self.points[insert_index] = np;
    }

    fn valid_contact_distance(&self, pt: &ManifoldPoint) -> bool {
        pt.distance1 <= self.contact_breaking_threshold
    }

    /// refreshContactPoints (trA, trB).
    pub fn refresh_contact_points(&mut self, tr_a: &Tf, tr_b: &Tf) {
        // first refresh worldspace positions and distance
        for i in (0..self.points.len()).rev() {
            let mp = &mut self.points[i];
            // the asm keeps the unrounded world points in registers for the distance
            let wa = tr_a.apply(mp.local_a);
            let wb = tr_b.apply(mp.local_b);
            mp.position_world_on_a = r3(wa);
            mp.position_world_on_b = r3(wb);
            mp.distance1 = f(dot(sub(wa, wb), mp.normal_world_on_b));
            mp.life_time += 1;
        }

        // then
        for i in (0..self.points.len()).rev() {
            let mp = self.points[i];

            // contact becomes invalid when signed distance exceeds margin (projected on contactnormal direction)
            if !self.valid_contact_distance(&mp) {
                self.remove_contact_point(i);
            } else {
                // contact also becomes invalid when relative movement orthogonal to normal exceeds margin
                let projected_point = sub(mp.position_world_on_a, scale(mp.normal_world_on_b, mp.distance1));
                let projected_difference = sub(mp.position_world_on_b, projected_point);
                let distance2d = dot(projected_difference, projected_difference);
                if distance2d > self.contact_breaking_threshold * self.contact_breaking_threshold {
                    self.remove_contact_point(i);
                }
            }
        }
    }
}

/// The manifold pool and manifold array of btCollisionDispatcher.
#[derive(Clone, Debug, Default)]
pub struct ManifoldPool {
    pool: Vec<Option<Manifold>>,
    free: Vec<usize>,
    /// m_manifoldsPtr.
    pub order: Vec<ManifoldId>,
}

impl ManifoldPool {
    pub fn get(&self, id: ManifoldId) -> &Manifold {
        self.pool[id].as_ref().expect("released manifold")
    }

    pub fn get_mut(&mut self, id: ManifoldId) -> &mut Manifold {
        self.pool[id].as_mut().expect("released manifold")
    }

    /// btCollisionDispatcher::getNewManifold.
    pub fn new_manifold(&mut self, body0: BodyId, body1: BodyId, breaking: f64, processing: f64) -> ManifoldId {
        let m = Manifold { body0, body1, points: vec![], contact_breaking_threshold: f(breaking), contact_processing_threshold: processing, index1a: self.order.len() };
        let id = match self.free.pop() {
            Some(i) => {
                self.pool[i] = Some(m);
                i
            }
            None => {
                self.pool.push(Some(m));
                self.pool.len() - 1
            }
        };
        self.order.push(id);
        id
    }

    /// btCollisionDispatcher::releaseManifold.
    pub fn release_manifold(&mut self, id: ManifoldId) {
        let find = self.get(id).index1a;
        let last = self.order.len() - 1;
        self.order.swap(find, last);
        let moved = self.order[find];
        self.pool[moved].as_mut().unwrap().index1a = find;
        self.order.pop();
        self.pool[id] = None;
        self.free.push(id);
    }

    pub fn num_manifolds(&self) -> usize {
        self.order.len()
    }

    /// getManifoldByIndexInternal.
    pub fn by_index(&self, i: usize) -> ManifoldId {
        self.order[i]
    }
}

/// btManifoldResult: a persistent manifold plus the two collision object wrappers' root bodies.
#[derive(Clone, Copy, Debug)]
pub struct ManifoldResult {
    pub manifold: Option<ManifoldId>,
    pub body0: BodyId,
    pub body1: BodyId,
}

pub fn calculate_combined_friction(b0: &RigidBody, b1: &RigidBody) -> f64 {
    let friction = f(b0.friction * b1.friction);
    let max_friction = 10.0;
    friction.clamp(-max_friction, max_friction)
}

pub fn calculate_combined_rolling_friction(b0: &RigidBody, b1: &RigidBody) -> f64 {
    let friction = f(b0.rolling_friction * b1.rolling_friction);
    let max_friction = 10.0;
    friction.clamp(-max_friction, max_friction)
}

pub fn calculate_combined_restitution(b0: &RigidBody, b1: &RigidBody) -> f64 {
    f(b0.restitution * b1.restitution)
}

impl ManifoldResult {
    pub fn new(body0: BodyId, body1: BodyId) -> ManifoldResult {
        ManifoldResult { manifold: None, body0, body1 }
    }

    /// btManifoldResult::addContactPoint (normalOnBInWorld, pointInWorld, depth).
    pub fn add_contact_point(&self, pool: &mut ManifoldPool, bodies: &[RigidBody], normal_on_b_in_world: V3, point_in_world: V3, depth: f64) {
        let mf = self.manifold.expect("manifold result without manifold");
        let m = pool.get_mut(mf);

        // order in manifold needs to match
        if depth > m.contact_breaking_threshold {
            return;
        }

        let is_swapped = m.body0 != self.body0;

        let normal = r3(normal_on_b_in_world);
        let point = r3(point_in_world);
        let point_a = add(point, scale(normal, depth));

        let (local_a, local_b) = if is_swapped {
            (bodies[self.body1].world_tf.inv_xform(point_a), bodies[self.body0].world_tf.inv_xform(point))
        } else {
            (bodies[self.body0].world_tf.inv_xform(point_a), bodies[self.body1].world_tf.inv_xform(point))
        };

        let mut new_pt = ManifoldPoint::new(local_a, local_b, normal, depth);
        new_pt.position_world_on_a = r3(point_a);
        new_pt.position_world_on_b = point;

        let insert_index = m.get_cache_entry(&new_pt);

        let (b0, b1) = (&bodies[self.body0], &bodies[self.body1]);
        new_pt.combined_friction = calculate_combined_friction(b0, b1);
        new_pt.combined_restitution = calculate_combined_restitution(b0, b1);
        new_pt.combined_rolling_friction = calculate_combined_rolling_friction(b0, b1);
        let (d1, d2) = plane_space1(new_pt.normal_world_on_b);
        new_pt.lateral_friction_dir1 = d1;
        new_pt.lateral_friction_dir2 = d2;

        if insert_index >= 0 {
            m.replace_contact_point(new_pt, insert_index as usize);
        } else {
            m.add_manifold_point(new_pt);
        }
    }

    /// btManifoldResult::refreshContactPoints.
    pub fn refresh_contact_points(&self, pool: &mut ManifoldPool, bodies: &[RigidBody]) {
        let mf = self.manifold.expect("manifold result without manifold");
        let m = pool.get_mut(mf);

        if m.num_contacts() == 0 {
            return;
        }

        let is_swapped = m.body0 != self.body0;

        if is_swapped {
            let (a, b) = (bodies[self.body1].world_tf, bodies[self.body0].world_tf);
            m.refresh_contact_points(&a, &b);
        } else {
            let (a, b) = (bodies[self.body0].world_tf, bodies[self.body1].world_tf);
            m.refresh_contact_points(&a, &b);
        }
    }
}
