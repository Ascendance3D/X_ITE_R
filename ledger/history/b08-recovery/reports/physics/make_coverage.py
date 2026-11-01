#!/usr/bin/env python3
"""Builds coverage.tsv from the Ammo call trace of pinned X_ITE (trace_union.json, from
oracle/x_ite-anim-ammotrace.mjs over the 47 physics worlds and the 5 picking worlds) plus the static rg list
and the rows X_ITE never reaches. Every traced symbol needs an entry in ROWS, or the script stops."""
import json, os, sys
here = os.path.dirname(os.path.abspath(__file__))
union = json.load(open(os.path.join(here, "trace_union.json")))

E = "x3d-runtime/src/physics/bt"      # dynamics engine
N = "x3d-runtime/src/nodes/rigid_body_physics"  # node layer
P = "x3d-runtime/src/bullet.rs + x3d-viewer/src/pick_shapes.rs"  # picking pipeline

# tests
DYN = "bullet_dyn_scenes.rs (66 vs Ammo)"
B08 = "bullet_b08_engine.rs (37 vs Ammo)"
BOX = "bullet_box_box.rs"
WLD = "rea_b06.py check / physics_b06_baseline.rs (47 worlds vs X_ITE)"
LIF = "physics_b08_lifecycle.rs (5) + physics_b06_lifecycle.rs (6)"
PIK = "rea_b03_pick_vectors.rs (4000 vs Ammo) + 20000-case sweep"
PW = "rea_b03.py check vpick-* (5 worlds vs X_ITE)"

# key (trace key without the "[Class]" suffix) -> (X_ITE call site, Rust implementation, tests)
ROWS = {
 "new btVector3": ("RigidBody/CollidableShape/joints/RigidBodyCollection/VolumePicker/PointPickSensor (0 or 3 args)", f"{E}/math.rs V3 = [f64;3], rounding at stores: r3 / f", f"{DYN}; {B08}"),
 "btVector3.setValue": ("everywhere a vector is handed to Ammo", f"{E}/math.rs r3 (float store)", f"{DYN}; {B08}"),
 "btVector3.x": ("RigidBody.update, CollisionSensor (reads)", f"{E}/math.rs V3 index", f"{DYN}: pos/lin/ang/pt_a/pt_n errors 0"),
 "btVector3.y": ("same", "same", f"{DYN}"),
 "btVector3.z": ("same", "same", f"{DYN}"),
 "btVector3.length": ("RigidBodyCollection.set_bounce__ (|linearVelocity| >= minBounceSpeed)", f"{N}/collection.rs set_bounce (double sqrt of float members, as the asm.js)", f"{DYN} b06_shape_cylinder/cone (bounce); {WLD} collider-bounce"),
 "new btQuaternion": ("RigidBody.set_centerOfMass__ (0,0,0,1)", f"{E}/body.rs set_center_of_mass_transform (identity basis)", f"{DYN} (every scene sets it); {WLD}"),
 "btQuaternion.x": ("RigidBody.update (reads motion state rotation)", f"{E}/math.rs rq (Tf::rotation, btMatrix3x3::getRotation)", f"{DYN}: quat/wquat 0; {WLD}"),
 "btQuaternion.y": ("same", "same", "same"),
 "btQuaternion.z": ("same", "same", "same"),
 "btQuaternion.w": ("same", "same", "same"),
 "new btTransform": ("0 args: RigidBody, CollidableShape, joints, VolumePicker; 2 args (quaternion, origin): set_centerOfMass__", f"{E}/math.rs Tf {{ b, o }}; the 0-arg object is uninitialized in Ammo and X_ITE only uses it after setValue/setOrigin (RigidBody.js uses it as the zero transform of addChildShape: generator zeroTransform)", f"{DYN}; {B08}"),
 "btTransform.getBasis": ("RigidBody/CollidableShape/VolumePicker/Slider: getBasis().setValue (9 floats)", f"{E}/math.rs Tf.b + rt (rounding)", f"{DYN}; {B08}"),
 "btMatrix3x3.setValue": ("same (9 args)", f"{E}/math.rs rt", f"{DYN}; {B08}"),
 "btTransform.setOrigin": ("same", f"{E}/math.rs Tf.o", f"{DYN}; {B08}"),
 "btTransform.getOrigin": ("RigidBody.update", f"{E}/world.rs motion_state_world_transform", f"{DYN}: pos 0; {WLD}"),
 "btTransform.getRotation": ("RigidBody.update", f"{E}/math.rs Tf::rotation", f"{DYN}: quat 0; {WLD}"),
 "new btDefaultMotionState": ("RigidBody ctor, VolumePicker", f"{E}/world.rs create_motion_state; {N}/arena.rs PWorld::new_motion_state", f"{DYN}; {LIF}"),
 "btDefaultMotionState.setWorldTransform": ("RigidBody.set_transform__", f"{E}/world.rs set_motion_state_world_transform", f"{DYN}; {WLD}"),
 "btDefaultMotionState.getWorldTransform": ("RigidBody.update", f"{E}/world.rs motion_state_world_transform", f"{DYN}; {WLD}"),
 "new btRigidBodyConstructionInfo": ("RigidBody ctor (0, motionState, compoundShape), VolumePicker", f"{E}/body.rs ConstructionInfo::new", f"{DYN}; {LIF}"),
 "new btRigidBody": ("RigidBody ctor, VolumePicker (2 static bodies)", f"{E}/world.rs create_rigid_body; {N}/arena.rs PWorld::new_rigid_body", f"{DYN}; {LIF}; {PIK}"),
 "btRigidBody.setMotionState": ("RigidBody.set_transform__", f"{E}/world.rs set_motion_state", f"{DYN}; {WLD}"),
 "btRigidBody.setLinearVelocity": ("RigidBody.set_linearVelocity__ (also per frame via the field write in update)", f"{E}/body.rs set_linear_velocity", f"{DYN}; {B08} o05_runtime_fields; {WLD}"),
 "btRigidBody.setAngularVelocity": ("RigidBody.set_angularVelocity__", f"{E}/body.rs set_angular_velocity", f"{DYN}; {B08} o05; {WLD}"),
 "btRigidBody.getLinearVelocity": ("RigidBody.update, set_bounce__", f"{E}/body.rs RigidBody.lin_vel", f"{DYN}: lin 0; {WLD}"),
 "btRigidBody.getAngularVelocity": ("RigidBody.update", f"{E}/body.rs RigidBody.ang_vel", f"{DYN}: ang 0; {WLD}"),
 "btRigidBody.activate": ("after setLinear/AngularVelocity and setDamping", f"{E}/body.rs activate(false)", f"{DYN} s12*; {B08} o05, o06; {WLD} sleep-wake"),
 "btRigidBody.isActive": ("tracked field @bodyActive of the oracle only (X_ITE source does not call it)", f"{E}/body.rs is_active", f"{DYN}: active 0 (every step); {WLD}"),
 "btRigidBody.setAngularFactor": ("RigidBody.set_finiteRotationAxis__", f"{E}/body.rs set_angular_factor", f"{DYN}; {WLD} finite-rotation"),
 "btRigidBody.setDamping": ("RigidBody.set_damping__", f"{E}/body.rs set_damping", f"{DYN} s11_damping; {B08} o05; {WLD} damping"),
 "btRigidBody.setCenterOfMassTransform": ("RigidBody.set_centerOfMass__", f"{E}/body.rs set_center_of_mass_transform", f"{DYN} s13_compound_offset; {WLD} center-of-mass"),
 "btRigidBody.setMassProps": ("RigidBody.set_massProps__ (mass 0 when fixed)", f"{E}/body.rs set_mass_props", f"{DYN}; {B08} o05 (mass change at run time)"),
 "btRigidBody.setSleepingThresholds": ("RigidBody.set_disable__", f"{E}/body.rs lin/ang_sleeping_threshold", f"{DYN} s12*; {B08} o05; {WLD} sleep-rest, sleep-wake"),
 "btRigidBody.setGravity": ("RigidBody.applyForces (every frame)", f"{E}/body.rs set_gravity", f"{DYN}; {B08} o04 (gravity change); {WLD} gravity-flags"),
 "btRigidBody.applyForce": ("RigidBody.applyForces (force, rel_pos 0)", f"{E}/body.rs apply_force", f"{DYN} s02; {B08} o05; {WLD} forces-torques"),
 "btRigidBody.applyTorque": ("RigidBody.applyForces", f"{E}/body.rs apply_torque", f"{DYN} s02; {B08} o05; {WLD} forces-torques"),
 "btRigidBody.setFriction": ("RigidBodyCollection.set_colliderParameters__", f"{E}/body.rs friction field; {N}/collection.rs", f"{DYN} b06_collider_friction_rolling; {B08} o05; {WLD} collider-friction"),
 "btRigidBody.setRollingFriction": ("same", f"{E}/body.rs rolling_friction; {E}/contact.rs calculate_combined_rolling_friction", f"{DYN} s14, s14b, s14c; {WLD} collider-friction"),
 "btRigidBody.setRestitution": ("RigidBodyCollection.set_bounce__ (every update)", f"{E}/body.rs restitution; {N}/collection.rs", f"{DYN} s03*, b06_*; {WLD} collider-bounce"),
 "btRigidBody.getCollisionShape": ("RigidBodyCollection.set_contactSurfaceThickness__", f"{E}/body.rs RigidBody.shape", f"{B08} o04_margin_gravity; {WLD} shape-thickness"),
 "btCollisionShape.setMargin": ("same (setMargin on the body compound)", f"{E}/shape.rs Shapes::set_margin", f"{B08} o04_margin_gravity; {DYN} (margin 0 path); {WLD} shape-thickness"),
 "new btCompoundShape": ("RigidBody, X3DNBodyCollidableNode, VolumePicker, PointPickSensor", f"{E}/shape.rs Shapes::new_compound; {N}/arena.rs PWorld::new_compound", f"{DYN}; {B08}; {LIF}"),
 "btCompoundShape.addChildShape": ("RigidBody.set_compoundShape__, CollidableShape, VolumePicker, PointPickSensor", f"{E}/shape.rs add_child_shape (dynamic AABB tree)", f"{DYN}; {B08} multi_collidables (120 cases); {PIK}"),
 "btCompoundShape.removeChildShapeByIndex": ("RigidBody.set_compoundShape__ (backwards), CollidableShape, VolumePicker", f"{E}/shape.rs remove_child_shape_by_index", f"{DYN} b06_*; {LIF}; {PIK}"),
 "btCompoundShape.updateChildTransform": ("RigidBody.set_transform__ (recalc false), CollidableShape (true), PointPickSensor", f"{E}/shape.rs update_child_transform", f"{DYN} (feedback in b06_*); {B08}; {WLD}"),
 "btCompoundShape.recalculateLocalAabb": ("RigidBody.set_transform__", f"{E}/shape.rs recalculate_local_aabb", f"{DYN}; {B08}"),
 "btCompoundShape.getNumChildShapes": ("RigidBody, VolumePicker", f"{E}/shape.rs num_child_shapes", f"{DYN}; {PIK}"),
 "btCompoundShape.setLocalScaling": ("CollidableShape (1,1,1 then restored), VolumePicker.setChildShape, PointPickSensor", f"{E}/shape.rs set_local_scaling; picking: pick_shapes.rs place_scaled", f"{DYN} (xflow); {B08}; {PIK}"),
 "btCompoundShape.getLocalScaling": ("CollidableShape.set_collidableGeometry__", f"{E}/shape.rs get_local_scaling", f"{DYN} (xflow); {WLD}"),
 "btCompoundShape.calculateLocalInertia": ("RigidBody.set_massProps__", f"{E}/shape.rs calculate_local_inertia", f"{DYN}: inertia 0; {B08} o05"),
 "new btBoxShape": ("CollidableShape Box", f"{E}/shape.rs Conv::box_", f"{DYN}; {B08}; {BOX}; {WLD} shape-box"),
 "new btSphereShape": ("CollidableShape Sphere; PointPickSensor btSphereShape(0)", f"{E}/shape.rs Conv::sphere", f"{DYN} pairs_sphere_*; {WLD} shape-sphere; {PIK} point-*"),
 "new btCylinderShape": ("CollidableShape Cylinder (side, top, bottom)", f"{E}/shape.rs Conv::cylinder", f"{DYN} pairs_*cylinder*; {B08} pairs_cylinder_mesh/height, solve_cylinder_*; {WLD} shape-cylinder"),
 "new btConeShape": ("CollidableShape Cone (side, bottom)", f"{E}/shape.rs Conv::cone", f"{DYN} pairs_*cone*; {B08} pairs_cone_mesh/height; {WLD} shape-cone"),
 "new btConvexHullShape": ("CollidableShape other geometry, convex TRUE", f"{E}/shape.rs Shapes::new_convex_hull", f"{DYN} pairs_*hull*; {B08}; {WLD} shape-ifs-dynamic"),
 "btConvexHullShape.addPoint": ("same (addPoint(p, false))", f"{E}/shape.rs Conv::hull_add_point", f"{DYN}; {B08}"),
 "btConvexHullShape.recalcLocalAabb": ("same", f"{E}/shape.rs Conv::recalc_local_aabb", f"{DYN}; {B08}"),
 "new btTriangleMesh": ("CollidableShape concave geometry", f"{E}/shape.rs Shapes::new_bvh_triangle_mesh (mesh data kept in BvhMesh)", f"{DYN} s05, s05b; {B08} pairs_*_mesh, solve_*_mesh; {WLD} shape-ifs-floor, shape-cone-open"),
 "btTriangleMesh.addTriangle": ("same (3 btVector3)", f"{E}/shape.rs new_bvh_triangle_mesh", "same"),
 "new btBvhTriangleMeshShape": ("same (mesh, false)", f"{E}/shape.rs Shape::Bvh + dispatch.rs convex-concave", "same"),
 "new btHeightfieldTerrainShape": ("CollidableShape ElevationGrid (9 args, PHY_FLOAT, flipQuadEdges)", f"{E}/shape.rs Shapes::new_heightfield; bullet.rs HeightField", f"{B08} h01-h06, pairs_*_height, solve_*_height (new); {WLD} shape-elevation(-origin); {PIK} *-height"),
 "btHeightfieldTerrainShape.setLocalScaling": ("same (xSpacing, 1, zSpacing)", f"{E}/shape.rs new_heightfield(scaling)", "same"),
 "Ammo._malloc": ("ElevationGrid height data (4*xDim*zDim)", "Rust Vec in HeightField.heights (no heap emulation needed)", f"{B08} h06_height_big_wavy (24x20)"),
 "Ammo.destroy": ("RigidBody.dispose (rigidBody, constructionInfo, motionState, compoundShape), CollidableShape (shape, triangleMesh), joints", f"{N}/arena.rs PWorld::free_shape/free_body/free_motion_state; {E}/world.rs remove_rigid_body/remove_constraint", f"{LIF}: 100 cycles flat (proxy slot leak found and fixed)"),
 "new btDbvtBroadphase": ("RigidBodyCollection, VolumePicker", f"{E}/broadphase.rs DbvtBroadphase (btHashedOverlappingPairCache order); picking: only one pair exists", f"{DYN} (pair order, all scenes); {LIF}; {PIK}"),
 "new btDefaultCollisionConfiguration": ("RigidBodyCollection, VolumePicker", f"{E}/dispatch.rs (algorithm matrix: sphere-sphere, box-box detector, convex-convex GJK/EPA, convex-concave, compound)", f"{DYN} pairs_*; {B08} pairs_*"),
 "new btCollisionDispatcher": ("RigidBodyCollection, VolumePicker", f"{E}/world.rs + contact.rs ManifoldPool (getNewManifold / releaseManifold order)", f"{DYN}: manifold_count, point_count 0; {LIF}"),
 "new btSequentialImpulseConstraintSolver": ("RigidBodyCollection", f"{E}/solver.rs Solver (SOLVER_RANDMIZE_ORDER off, friction/rolling friction rows, warm starting)", f"{DYN}; {B08}; {WLD}"),
 "new btDiscreteDynamicsWorld": ("RigidBodyCollection", f"{E}/world.rs World::new", f"{DYN}; {LIF}"),
 "btDiscreteDynamicsWorld.setGravity": ("RigidBodyCollection.set_gravity__", f"{E}/world.rs set_gravity", f"{DYN}; {B08} o04"),
 "btDiscreteDynamicsWorld.addRigidBody": ("RigidBodyCollection.set_dynamicsWorld__", f"{E}/world.rs add_rigid_body", f"{DYN}; {B08} o01, o06, o07; {WLD} rt-body; {LIF}"),
 "btDiscreteDynamicsWorld.removeRigidBody": ("same (all bodies removed, enabled ones added again)", f"{E}/world.rs remove_rigid_body (+ broadphase cleanProxyFromPairs, destroyProxy)", f"{B08} o01, o06, o07 (new); {WLD} rt-body; {LIF}"),
 "btDiscreteDynamicsWorld.addConstraint": ("joints addJoint (constraint, true)", f"{E}/world.rs add_constraint", f"{DYN} s07-s10; {B08} j01-j03, o02, o03, o07; {WLD} joint-*"),
 "btDiscreteDynamicsWorld.removeConstraint": ("joints removeJoint", f"{E}/world.rs remove_constraint (slot reuse)", f"{B08} o02_joint_remove_add (new); {WLD} rt-joint; {LIF}"),
 "btDiscreteDynamicsWorld.stepSimulation": ("RigidBodyCollection.update (dt, iterations+2, dt/iterations) and preferAccuracy (dt, 0)", f"{E}/world.rs step_simulation", f"{DYN} s01, s01b, s01c, s03d, s12*; {B08}; {WLD} prefer-accuracy"),
 "btDiscreteDynamicsWorld.getDispatcher": ("CollisionSensor.update", f"{N}/sensor.rs (the world's ManifoldPool)", f"{WLD} shape-*, collision-space, sensor-partial"),
 "btCollisionDispatcher.getNumManifolds": ("CollisionSensor, VolumePicker.contactTest", f"{E}/world.rs num_manifolds; picking: bullet::pair_manifold", f"{DYN}: manifold_count; {PIK}"),
 "btCollisionDispatcher.getManifoldByIndexInternal": ("same", f"{E}/world.rs manifold_by_index", f"{DYN}: manifold_bodies; {WLD}"),
 "btDispatcher.getManifoldByIndexInternal": ("(same call through the world's dispatcher)", "same", "same"),
 "btDispatcher.getNumManifolds": ("same", "same", "same"),
 "btPersistentManifold.getNumContacts": ("CollisionSensor, VolumePicker", f"{E}/contact.rs Manifold.points.len()", f"{DYN}: point_count; {PIK}"),
 "btPersistentManifold.getContactPoint": ("same", f"{E}/contact.rs Manifold.points[i]", f"{DYN}: pt_* errors 0; {WLD} contacts"),
 "btPersistentManifold.getBody0": ("CollisionSensor (.ptr maps to a collidable)", f"{E}/contact.rs Manifold.body0 (BodyId)", f"{DYN}: manifold_bodies; {WLD} contacts geometry1/2"),
 "btPersistentManifold.getBody1": ("same", "Manifold.body1", "same"),
 "btManifoldPoint.getDistance": ("CollisionSensor (<= 0, depth = -distance), VolumePicker.contactTest", f"{E}/contact.rs ManifoldPoint.distance1", f"{DYN}: pt_distance 0; {WLD}; {PIK}"),
 "btManifoldPoint.getPositionWorldOnA": ("CollisionSensor contact position", f"{E}/contact.rs position_world_on_a", f"{DYN}: pt_a 0; {WLD} contacts.position"),
 "btManifoldPoint.get_m_normalWorldOnB": ("CollisionSensor contactNormal", f"{E}/contact.rs normal_world_on_b", f"{DYN}: pt_n 0; {WLD} contacts.contactNormal"),
 "new btPoint2PointConstraint": ("BallJoint.addJoint (body, body, zero, zero)", f"{E}/constraint.rs Constraint::point2point", f"{DYN} s07, s07b; {B08} j01, o02, o03, o07; {WLD} joint-ball*"),
 "btPoint2PointConstraint.setPivotA": ("BallJoint.set_anchorPoint__ (also live change)", f"{E}/constraint.rs set_pivot_a", f"{DYN}; {B08} o03_pivot_change (new, run-time change); {WLD} rt-joint"),
 "btPoint2PointConstraint.setPivotB": ("same", f"{E}/constraint.rs set_pivot_b", "same"),
 "new btHingeConstraint": ("SingleAxisHingeJoint, DoubleAxisHingeJoint (7 args, refFrame false)", f"{E}/constraint.rs Constraint::hinge (+ get_info1/2, calculate_transforms)", f"{DYN} s08, s08b, s08c; {B08} j01, j02, j03 (DoubleAxisHinge axes, new); {WLD} joint-hinge-*"),
 "new btSliderConstraint": ("SliderJoint.addJoint (frames, true; frameInA typo reproduced)", f"{E}/constraint.rs Constraint::slider", f"{DYN} s10, s10b; {B08} j01 (new); {WLD} joint-slider*"),
 "btSliderConstraint.setLowerAngLimit": ("SliderJoint.addJoint (0)", f"{E}/constraint.rs set_lower_ang_limit", f"{DYN} s10; {WLD} joint-slider"),
 "btSliderConstraint.setUpperAngLimit": ("same", f"{E}/constraint.rs set_upper_ang_limit", "same"),
 "btSliderConstraint.setLowerLinLimit": ("SliderJoint.set_separation__ (minSeparation)", f"{E}/constraint.rs set_lower_lin_limit", f"{DYN} s10; {WLD} rt-joint"),
 "btSliderConstraint.setUpperLinLimit": ("same (maxSeparation)", f"{E}/constraint.rs set_upper_lin_limit", "same"),
 "new btCollisionWorld": ("VolumePicker (3 args)", f"{P}: PickShapes (one persistent 2-body world: the picker only ever holds rigidBody1 and rigidBody2)", f"{PIK}; {PW}"),
 "btCollisionWorld.addCollisionObject": ("VolumePicker ctor (the two static bodies)", f"{P}: PickShapes::set_child / place", f"{PIK}"),
 "btCollisionWorld.performDiscreteCollisionDetection": ("VolumePicker.contactTest", f"{P}: bullet::pair_manifold (broadphase AABB test, compound tree walk, convex-convex / convex-concave / box-box, manifold threshold history)", f"{PIK}; {PW}"),
 "btBvhTriangleMeshShape.__destroy__": ("(Ammo.destroy of the collision shape)", f"{N}/arena.rs free_shape", f"{LIF}"),
 "btSphereShape.__destroy__": ("same", "same", "same"),
 "btConeShape.__destroy__": ("same", "same", "same"),
 "btCylinderShape.__destroy__": ("same", "same", "same"),
 "btConvexHullShape.__destroy__": ("same", "same", "same"),
 "btTriangleMesh.__destroy__": ("same", "same", "same"),
 "btHingeConstraint.__destroy__": ("same", f"{E}/world.rs remove_constraint (+ free slot)", f"{LIF}; {B08} o02"),
 "btPoint2PointConstraint.__destroy__": ("same", "same", "same"),
 "btSliderConstraint.__destroy__": ("same", "same", "same"),
}

def base(k):
    # "btFoo.method [btFoo]" -> "btFoo.method"
    return k.split(" [")[0]

keys = {}
for k, v in union.items():
    keys.setdefault(base(k), []).append(v)

missing = [k for k in keys if k not in ROWS]
if missing:
    print("NO ROW FOR", missing); sys.exit(1)

# static-only rows (not reached by any of the 52 traced worlds, but in the source): from `rg Ammo` over oracle/x_ite-pin/src
STATIC = [
 ("Ammo._free", "CollidableShape.removeCollidableGeometry (ElevationGrid height data)", "Rust Vec dropped with the shape", f"{B08} h06; {LIF}: shape slots freed", "STATIC_ONLY: no traced world rebuilds an ElevationGrid shape; the free is memory management only"),
 ("Ammo.HEAPF32[...] =", "CollidableShape (writes the heights as floats)", f"{E}/shape.rs new_heightfield (f() rounds each height)", f"{B08} h01-h06", "STATIC_ONLY: covered by the new height field scenes"),
 (".ptr (btRigidBody / manifold body)", "CollisionSensor (maps manifold bodies to collidables)", "BodyId", f"{WLD} contacts", "value"),
]

# never reached
NEVER = [
 ("UniversalJoint, MotorJoint", "no Ammo constraint is created (rg: the two files do not import AmmoClass)", f"{E}/ (nothing to port); node fields only: {N}/joints.rs", f"{DYN} s09_universal_joint_unconnected (bodies fall independently); {WLD} joint-universal, joint-motor"),
 ("btStaticPlaneShape", "not constructed anywhere in X_ITE src", "not needed", "-"),
 ("btCapsuleShape", "excluded by X_ITE (no Capsule collidable; Capsule is not an X3D RigidBodyPhysics geometry here)", f"{E}/shape.rs Conv::capsule exists", f"{DYN} s13b_capsule (bonus)"),
 ("kinematic bodies: setCollisionFlags, CF_KINEMATIC_OBJECT, setActivationState", "RigidBody.fixed means mass 0 (static), never kinematic", f"{E}/body.rs is_kinematic exists", "fixed bodies: every scene with ground; static-ness in World::add_rigid_body"),
 ("btHingeConstraint.setLimit, enableAngularMotor, setMotor*", "minAngle/maxAngle/stopBounce/stopErrorCorrection are NOT forwarded by SingleAxisHingeJoint (only the 7 arg constructor)", "not ported", "-"),
 ("btGeneric6DofConstraint, btConeTwistConstraint, btFixedConstraint, btGeneric6DofSpringConstraint", "not used by any X_ITE node", "not ported", "-"),
 ("btCollisionWorld.rayTest, convexSweepTest, contactTest, contactPairTest, btGhostObject, btPairCachingGhostObject, btGhostPairCallback", "X_ITE picking uses its own math (LinePickSensor, RayPick) and VolumePicker only calls performDiscreteCollisionDetection", "not ported", "rg over oracle/x_ite-pin/src finds no use"),
 ("btRigidBody.applyCentralForce/Impulse, applyImpulse, clearForces, setLinearFactor, setCcdMotionThreshold, setContactProcessingThreshold, setDeactivationTime, forceActivationState", "not called by X_ITE", f"{E}/body.rs has most (used by the solver)", "-"),
 ("btRaycastVehicle, btKinematicCharacterController, btSoftBody*, btAxisSweep3, btConvexTriangleMeshShape, btShapeHull, btConvexPolyhedron", "not used", "not ported", "-"),
 ("btDiscreteDynamicsWorld.setGravity called with preferAccuracy per iteration", "(covered: stepSimulation row)", "-", "-"),
]

out = os.path.join(here, "coverage.tsv")
with open(out, "w") as fh:
    fh.write("section\tammo_symbol\ttraced_calls\ttraced_worlds\tx_ite_use\trust_implementation\ttests\tstatus\n")
    for k in sorted(keys):
        calls = sum(v["calls"] for v in keys[k]); files = max(v["files"] for v in keys[k])
        use, impl, tests = ROWS[k]
        status = "TESTED_VS_AMMO_OR_XITE" if tests not in ("-", "") else "GAP"
        fh.write("reached\t%s\t%d\t%d\t%s\t%s\t%s\t%s\n" % (k, calls, files, use, impl, tests, status))
    for sym, use, impl, tests, st in STATIC:
        fh.write("static\t%s\t0\t0\t%s\t%s\t%s\t%s\n" % (sym, use, impl, tests, st))
    for sym, why, impl, tests in NEVER:
        fh.write("never_reached\t%s\t0\t0\t%s\t%s\t%s\tNOT_REACHED\n" % (sym, why, impl, tests))
print("rows:", sum(1 for _ in open(out)) - 1, "traced symbols:", len(keys))
