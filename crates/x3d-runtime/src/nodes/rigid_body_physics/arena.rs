//! The Bullet worlds of RigidBodyPhysics and the arenas inside them.
//!
//! X_ITE creates the Ammo objects of a node in its constructor (`new Ammo .btCompoundShape ()`,
//! `new Ammo .btRigidBody (...)`) and hands them to a `btDiscreteDynamicsWorld` later, when a
//! RigidBodyCollection takes the body. The Rust engine (`physics::bt`) keeps shapes, bodies and
//! motion states in arenas that belong to a [`World`], so an object has to live in some world from the
//! start. Objects of a node that no collection has taken live in the runtime's *nursery*; the moment a
//! RigidBodyCollection adopts a RigidBody (`RigidBody .setCollection`), the body, its motion state and
//! the shape tree below it (the collidables' compound shapes and collision shapes) are copied into the
//! collection's world and the nursery slots are given back. The copy is exact (plain data, ids are
//! remapped), so nothing observable changes. Slots that are given back are reused, so a node that
//! comes and goes does not make an arena grow.
//!
//! A world is dropped with the collection that owns it.

use crate::physics::bt::body::{BodyId, ConstructionInfo, MotionStateId, RigidBody};
use crate::physics::bt::math::Tf;
use crate::physics::bt::shape::{Conv, Shape, ShapeId};
use crate::physics::bt::world::World;
use std::collections::HashMap;
use x3d_fields::NodeId;

/// Where the Ammo objects of a node live: `None` is the runtime's nursery, `Some(collection)` the
/// world of that RigidBodyCollection.
pub type Home = Option<NodeId>;

/// A Bullet world with the free lists of its arenas.
pub struct PWorld {
    pub w: World,
    free_shapes: Vec<ShapeId>,
    free_bodies: Vec<BodyId>,
    free_motion_states: Vec<MotionStateId>,
    /// Live slots (allocated and not freed), for diagnostics.
    live_shapes: usize,
    live_bodies: usize,
    live_motion_states: usize,
}

impl std::fmt::Debug for PWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PWorld")
            .field("shapes", &self.live_shapes)
            .field("bodies", &self.live_bodies)
            .field("motion_states", &self.live_motion_states)
            .finish()
    }
}

/// Sizes of a world's arenas (live slots, and the length of the backing vectors).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WorldCounts {
    pub shapes: usize,
    pub bodies: usize,
    pub motion_states: usize,
    pub shape_slots: usize,
    pub body_slots: usize,
    pub motion_state_slots: usize,
    /// Constraints that are in the dynamics world, and the constraint slots ever used.
    pub constraints: usize,
    pub constraint_slots: usize,
    /// Bodies in the dynamics world.
    pub in_world: usize,
}

impl Default for PWorld {
    fn default() -> Self {
        PWorld::new()
    }
}

impl PWorld {
    pub fn new() -> PWorld {
        PWorld {
            w: World::new(),
            free_shapes: vec![],
            free_bodies: vec![],
            free_motion_states: vec![],
            live_shapes: 0,
            live_bodies: 0,
            live_motion_states: 0,
        }
    }

    pub fn counts(&self) -> WorldCounts {
        WorldCounts {
            shapes: self.live_shapes,
            bodies: self.live_bodies,
            motion_states: self.live_motion_states,
            shape_slots: self.w.shapes.v.len(),
            body_slots: self.w.bodies.len(),
            motion_state_slots: self.w.motion_states.len(),
            constraints: 0,
            constraint_slots: self.w.joints.len(),
            in_world: 0,
        }
    }

    /// Put a shape into the arena (a freed slot first).
    pub fn alloc_shape(&mut self, s: Shape) -> ShapeId {
        self.live_shapes += 1;

        match self.free_shapes.pop() {
            Some(id) => {
                self.w.shapes.v[id] = s;
                id
            }
            None => self.w.shapes.add(s),
        }
    }

    /// `new Ammo .btCompoundShape ()`.
    pub fn new_compound(&mut self) -> ShapeId {
        let id = self.w.shapes.new_compound();
        let s = self.w.shapes.v.pop().expect("compound");

        // `new_compound` pushed at the end; hand the shape to the allocator.
        debug_assert_eq!(id, self.w.shapes.v.len());
        self.alloc_shape(s)
    }

    /// Create a primitive with the engine's constructor and put it into the arena.
    pub fn new_shape(
        &mut self,
        make: impl FnOnce(&mut crate::physics::bt::shape::Shapes) -> ShapeId,
    ) -> ShapeId {
        let before = self.w.shapes.v.len();
        let id = make(&mut self.w.shapes);

        debug_assert_eq!(id, before);
        debug_assert_eq!(self.w.shapes.v.len(), before + 1);

        let s = self.w.shapes.v.pop().expect("shape");

        self.alloc_shape(s)
    }

    /// Give a shape slot back. The content is dropped.
    pub fn free_shape(&mut self, id: ShapeId) {
        if id >= self.w.shapes.v.len() || self.free_shapes.contains(&id) {
            return;
        }

        self.w.shapes.v[id] = Shape::Conv(Conv::sphere(0.0));
        self.free_shapes.push(id);
        self.live_shapes = self.live_shapes.saturating_sub(1);
    }

    /// `new Ammo .btDefaultMotionState ()`: the identity.
    pub fn new_motion_state(&mut self, tf: Tf) -> MotionStateId {
        self.live_motion_states += 1;

        match self.free_motion_states.pop() {
            Some(id) => {
                self.w.motion_states[id].tf = crate::physics::bt::math::rt(tf);
                id
            }
            None => self.w.create_motion_state(tf),
        }
    }

    pub fn free_motion_state(&mut self, id: MotionStateId) {
        if id >= self.w.motion_states.len() || self.free_motion_states.contains(&id) {
            return;
        }

        self.free_motion_states.push(id);
        self.live_motion_states = self.live_motion_states.saturating_sub(1);
    }

    /// Put a rigid body into the arena (a freed slot first).
    pub fn alloc_body(&mut self, b: RigidBody) -> BodyId {
        self.live_bodies += 1;

        match self.free_bodies.pop() {
            Some(id) => {
                self.w.bodies[id] = b;
                id
            }
            None => {
                self.w.bodies.push(b);
                self.w.bodies.len() - 1
            }
        }
    }

    /// `new Ammo .btRigidBody (constructionInfo)`.
    pub fn new_rigid_body(&mut self, info: &ConstructionInfo) -> BodyId {
        let motion_tf = info.motion_state.map(|m| self.w.motion_states[m].tf);

        self.alloc_body(RigidBody::new(info, motion_tf))
    }

    /// Give a rigid body slot back (the body must not be in the dynamics world).
    pub fn free_body(&mut self, id: BodyId) {
        if id >= self.w.bodies.len() || self.free_bodies.contains(&id) {
            return;
        }

        let info = ConstructionInfo::new(0.0, None, 0);

        self.w.bodies[id] = RigidBody::new(&info, None);
        self.free_bodies.push(id);
        self.live_bodies = self.live_bodies.saturating_sub(1);
    }
}

/// Copy the shape tree below `id` from `src` into `dst`, remapping ids; `map` collects old to new.
pub fn clone_shape_tree(
    src: &PWorld,
    dst: &mut PWorld,
    id: ShapeId,
    map: &mut HashMap<ShapeId, ShapeId>,
) -> ShapeId {
    if let Some(&n) = map.get(&id) {
        return n;
    }

    let mut s = src.w.shapes.v[id].clone();

    if let Shape::Compound(c) = &mut s {
        for ch in &mut c.children {
            ch.shape = clone_shape_tree(src, dst, ch.shape, map);
        }
    }

    let n = dst.alloc_shape(s);

    map.insert(id, n);
    n
}

/// Copy a rigid body (and its motion state) from `src` into `dst`. `shape` is the body's new shape.
/// Returns the new body and motion state ids. The copy is not in any dynamics world.
pub fn clone_body(
    src: &PWorld,
    dst: &mut PWorld,
    body: BodyId,
    shape: ShapeId,
) -> (BodyId, MotionStateId) {
    let mut b = src.w.bodies[body].clone();
    let ms_tf = b
        .motion_state
        .map(|m| src.w.motion_states[m].tf)
        .unwrap_or(b.world_tf);
    let ms = dst.new_motion_state(ms_tf);

    b.shape = shape;
    b.motion_state = Some(ms);
    b.proxy = None;
    b.constraint_refs = vec![];

    (dst.alloc_body(b), ms)
}
