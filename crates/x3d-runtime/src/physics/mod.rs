//! Physics engines of the runtime.
//!
//! [`bt`] is the Bullet 2.82 dynamics pipeline as X_ITE drives it through Ammo.js: the nodes of
//! RigidBodyPhysics (`nodes/rigid_body_physics.rs`) call it the way the JavaScript classes call Ammo.
//! The narrow phase it builds on lives in `crate::bullet` (Batch 03).

pub mod bt;
