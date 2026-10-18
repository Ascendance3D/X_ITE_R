//! CollisionSensor and Contact (X_ITE `CollisionSensor.js`, `Contact.js`).

use super::*;
use crate::physics::bt::body::BodyId;
use crate::physics::bt::math::V3;
use std::collections::HashMap;

/// The members of a CollisionSensor.
#[derive(Debug, Default)]
pub struct SensorRt {
    /// `this .colliderNode`.
    pub(super) collider: Option<NodeId>,
    /// `this .contactCache`: the Contact nodes made so far.
    pub(super) contact_cache: Vec<NodeId>,
}

/// One contact point of a manifold, as the sensor reads it.
struct Point {
    distance: f64,
    position_on_a: V3,
    normal_on_b: V3,
}

impl Runtime {
    fn sensor_rt(&self, node: NodeId) -> &SensorRt {
        match self.phys(node) {
            Some(PhysState::Sensor(s)) => s,
            _ => panic!("node {node} is not a CollisionSensor"),
        }
    }

    fn sensor_rt_mut(&mut self, node: NodeId) -> &mut SensorRt {
        match self.phys_mut(node) {
            Some(PhysState::Sensor(s)) => s,
            _ => panic!("node {node} is not a CollisionSensor"),
        }
    }

    pub(super) fn sensor_initialize(&mut self, node: NodeId) {
        // this .getLive () .addInterest ("set_live__", this)
        self.watch_live(node);
        self.pcb(node, "collider", PhysicsCb::SensorSetCollider);
        self.sensor_set_collider(node);
    }

    /// `set_live__ ()`. (`enabled` has no interest in X_ITE.)
    pub(super) fn sensor_set_live(&mut self, node: NodeId) {
        let sensor = self.sensor_events_field();

        if self.phys_live(node) && self.get_bool(node, "enabled") && self.sensor_rt(node).collider.is_some() {
            self.add_interest(sensor, node, Cb::Physics(PhysicsCb::SensorUpdate));
        } else {
            self.remove_interest(sensor, node, Cb::Physics(PhysicsCb::SensorUpdate));
        }
    }

    pub(super) fn sensor_set_collider(&mut self, node: NodeId) {
        let collider = self.phys_cast_field(node, "collider", is_collision_collection);

        self.sensor_rt_mut(node).collider = collider;
        self.sensor_set_live(node);
    }

    /// `getContact (index)`.
    fn sensor_contact(&mut self, node: NodeId, index: usize) -> Option<NodeId> {
        if let Some(&c) = self.sensor_rt(node).contact_cache.get(index) {
            return Some(c);
        }

        let ctx = self.scene.node(node).ctx;
        let contact = self.create_node_setup(ctx, "Contact")?;

        self.sensor_rt_mut(node).contact_cache.push(contact);
        Some(contact)
    }

    pub(super) fn sensor_update(&mut self, node: NodeId) {
        let Some(collider) = self.sensor_rt(node).collider else {
            return;
        };

        // collidableNodesIndex: the body (its pointer) to the collidable that names it, the last one wins.
        let mut index: HashMap<(NodeId, BodyId), NodeId> = HashMap::new();
        let mut worlds: Vec<NodeId> = vec![];

        for c in self.cc_collidables(collider) {
            let Some(body) = self.collidable_body(c) else {
                continue;
            };

            let rt = self.body_rt(body);

            // The key is the pointer of the btRigidBody: its world and its id there.
            if let Some(h) = rt.home {
                index.insert((h, rt.body), c);
            }

            if let Some(collection) = self.body_collection(body) {
                if !worlds.contains(&collection) {
                    worlds.push(collection);
                }
            }
        }

        let mut intersections: Vec<NodeId> = vec![];
        let mut contacts: Vec<NodeId> = vec![];

        for world in worlds {
            // dispatcher .getManifoldByIndexInternal (i)
            let manifolds: Vec<(BodyId, BodyId, Vec<Point>)> = self.with_world(Some(world), |pw| {
                (0..pw.w.num_manifolds())
                    .map(|i| {
                        let m = pw.w.manifold_by_index(i);
                        let points = m.points.iter().map(|p| Point { distance: p.distance1, position_on_a: p.position_world_on_a, normal_on_b: p.normal_world_on_b }).collect();

                        (m.body0, m.body1, points)
                    })
                    .collect()
            });

            for (body0, body1, points) in manifolds {
                for pt in points {
                    if pt.distance > 0.0 {
                        continue;
                    }

                    let collidable1 = index.get(&(world, body0)).copied();
                    let collidable2 = index.get(&(world, body1)).copied();

                    if collidable1.is_none() && collidable2.is_none() {
                        continue;
                    }

                    let Some(contact) = self.sensor_contact(node, contacts.len()) else {
                        continue;
                    };

                    self.set_v3(contact, "position", Vector3::new(pt.position_on_a[0], pt.position_on_a[1], pt.position_on_a[2]));
                    self.set_v3(contact, "contactNormal", Vector3::new(pt.normal_on_b[0], pt.normal_on_b[1], pt.normal_on_b[2]));
                    self.set_f(contact, "depth", -pt.distance);

                    // frictionDirection is commented out upstream.
                    for (to, from) in [
                        ("appliedParameters", "appliedParameters"),
                        ("bounce", "bounce"),
                        ("minBounceSpeed", "minBounceSpeed"),
                        ("frictionCoefficients", "frictionCoefficients"),
                        ("surfaceSpeed", "surfaceSpeed"),
                        ("slipCoefficients", "slipFactors"),
                        ("softnessConstantForceMix", "softnessConstantForceMix"),
                        ("softnessErrorCorrection", "softnessErrorCorrection"),
                    ] {
                        if let (Some(t), Some(v)) = (self.pre(contact, to), self.pre_value(collider, from).cloned()) {
                            self.set_value(t, v);
                        }
                    }

                    // `contactNode .geometry1_ = collidableNode1` (a typo upstream) sets a property of the
                    // JavaScript object, not the field: geometry1, geometry2, body1 and body2 stay null.
                    for c in [collidable1, collidable2].into_iter().flatten() {
                        if !intersections.contains(&c) {
                            intersections.push(c);
                        }
                    }

                    contacts.push(contact);
                }
            }
        }

        let active = !contacts.is_empty();

        if self.get_bool(node, "isActive") != active {
            self.set_b_field(node, "isActive", active);
        }

        if !intersections.is_empty() {
            if let Some(k) = self.pre(node, "intersections") {
                self.set_value(k, FieldValue::MFNode(intersections.into_iter().map(Some).collect()));
            }
        }

        if !contacts.is_empty() {
            if let Some(k) = self.pre(node, "contacts") {
                self.set_value(k, FieldValue::MFNode(contacts.into_iter().map(Some).collect()));
            }
        }
    }

    fn set_b_field(&mut self, node: NodeId, name: &str, v: bool) {
        if let Some(k) = self.pre(node, name) {
            self.set_value(k, FieldValue::SFBool(v));
        }
    }
}
