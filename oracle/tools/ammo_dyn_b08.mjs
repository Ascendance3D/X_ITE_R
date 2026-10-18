/**
 * Batch 08 (physics lane): engine-level dynamics vectors from the pinned X_ITE's Ammo (Bullet 2.82, asm.js) for what
 * oracle/tools/ammo_dyn_vectors.mjs (batch 06R) does not cover: btHeightfieldTerrainShape (ElevationGrid), convex
 * shapes against BVH meshes and height fields (pair and solver corpora), bodies with several collidables, a mixed
 * joint chain, DoubleAxisHinge frames, and the run-time changes X_ITE makes to a live world (set_dynamicsWorld__
 * remove / add of every body, removeConstraint + a new constraint, setPivotA/B, contactSurfaceThickness,
 * gravity, friction, mass props, damping, sleeping thresholds, a new transform, a new velocity).
 *
 * The scene builder, the stepper and the dump are copied from ammo_dyn_vectors.mjs (the 06R generator is not edited);
 * the differences are: the "height" shape, `ops` (run-time changes after step N), and makeJoint ().
 *
 *   node oracle/tools/ammo_dyn_b08.mjs [outDir] [sceneName...]
 *
 * crates/x3d-runtime/tests/bullet_b08_engine.rs replays the same scene description through x3d_runtime::physics::bt
 * and asserts a maximum absolute error of zero for every recorded quantity.
 */
import { writeFile, mkdir } from "node:fs/promises";
import { readFileSync } from "node:fs";
import { join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SRC = normalize (join (fileURLToPath (import.meta.url), "../../x_ite-pin/src"));
const { default: AmmoFactory } = await import (join (SRC, "lib/ammojs/ammo.js"));
const { default: Matrix4 } = await import (join (SRC, "standard/Math/Numbers/Matrix4.js"));
const { default: Rotation4 } = await import (join (SRC, "standard/Math/Numbers/Rotation4.js"));
const { default: Vector3 } = await import (join (SRC, "standard/Math/Numbers/Vector3.js"));
const { default: Quaternion } = await import (join (SRC, "standard/Math/Numbers/Quaternion.js"));
let Ammo = new AmmoFactory (); // replaced by a fresh heap per output file: worlds are never freed

const ROOT = resolve (fileURLToPath (import.meta.url), "../../..");
const [ outArg = join (ROOT, "crates/x3d-runtime/tests/data/bullet_b08"), ... only ] = process .argv .slice (2);

// --- helpers -------------------------------------------------------------------------------

const vec = a => new Ammo .btVector3 (a [0], a [1], a [2]);
const m16 = (position, rotation) => { const m = new Matrix4 (); m .setTransform (new Vector3 (... position), new Rotation4 (... rotation)); return [... m]; };
const inv16 = m => { const r = new Matrix4 (... m) .inverse (); return [... r]; };

/** RigidBody.set_transform__ / CollidableShape.getLocalTransform: matrix -> btTransform. */
const toTransform = (m, t = new Ammo .btTransform ()) =>
{
   t .getBasis () .setValue (m [0], m [4], m [8], m [1], m [5], m [9], m [2], m [6], m [10]);
   t .setOrigin (new Ammo .btVector3 (m [12], m [13], m [14]));
   return t;
};

/** `new Ammo .btTransform ()` is not initialized (RigidBody.js uses it as the zero transform of addChildShape). */
const zeroTransform = () => { const t = new Ammo .btTransform (); t .getBasis () .setValue (0, 0, 0, 0, 0, 0, 0, 0, 0); t .setOrigin (new Ammo .btVector3 (0, 0, 0)); return t; };

const vx = v => [v .x (), v .y (), v .z ()];
const qx = q => [q .x (), q .y (), q .z (), q .w ()];

const makePrimitive = (s, keep) =>
{
   switch (s .type)
   {
      case "sphere": return new Ammo .btSphereShape (s .radius);
      case "box": return new Ammo .btBoxShape (vec (s .half));
      case "cylinder": return new Ammo .btCylinderShape (new Ammo .btVector3 (s .radius, s .half_height, s .radius));
      case "cone": return new Ammo .btConeShape (s .radius, s .height);
      case "capsule": return new Ammo .btCapsuleShape (s .radius, s .height);
      case "hull":
      {
         const hull = new Ammo .btConvexHullShape ();
         for (const p of s .points) hull .addPoint (vec (p), false);
         hull .recalcLocalAabb ();
         return hull;
      }
      case "height":
      {
         // CollidableShape.js, ElevationGrid: Ammo ._malloc, HEAPF32, btHeightfieldTerrainShape (..., 1, min, max, 1, "PHY_FLOAT", true),
         // setLocalScaling (xSpacing, 1, zSpacing). (The offset is the shape's `offset`.)
         const ptr = Ammo ._malloc (4 * s .xd * s .zd);
         let min = Number .POSITIVE_INFINITY, max = Number .NEGATIVE_INFINITY;
         s .heights .forEach ((h, i) => { min = Math .min (min, h); max = Math .max (max, h); Ammo .HEAPF32 [(ptr + i * 4) >>> 2] = h; });
         const shape = new Ammo .btHeightfieldTerrainShape (s .xd, s .zd, ptr, 1, min, max, 1, "PHY_FLOAT", true);
         shape .setLocalScaling (new Ammo .btVector3 (s .xs, 1, s .zs));
         return shape;
      }
      case "mesh":
      {
         const mesh = new Ammo .btTriangleMesh ();
         keep .push (mesh);
         for (const [a, b, c] of s .triangles) mesh .addTriangle (vec (a), vec (b), vec (c));
         return new Ammo .btBvhTriangleMeshShape (mesh, false);
      }
   }
   throw new Error (`shape ${s .type}`);
};

// --- scene runner --------------------------------------------------------------------------

function runScene (spec)
{
   const keep = [ ];
   let gravityNow = spec .gravity;
   const broadphase = new Ammo .btDbvtBroadphase ();
   const config = new Ammo .btDefaultCollisionConfiguration ();
   const dispatcher = new Ammo .btCollisionDispatcher (config);
   const solver = new Ammo .btSequentialImpulseConstraintSolver ();
   const world = new Ammo .btDiscreteDynamicsWorld (dispatcher, broadphase, solver, config);

   // The world exists before its bodies: X_ITE's first sensorEvents frames (the load) already step it
   // (RigidBodyCollection.update, empty world), and the time left over in m_localTime stays.
   world .setGravity (vec (spec .gravity));
   for (const dt of spec .pre_dts ?? [ ]) world .stepSimulation (dt, spec .iterations + 2, dt / spec .iterations);

   const makeCollidables = b => b .shapes .map (s =>
   {
      const cc = new Ammo .btCompoundShape ();
      const prim = makePrimitive (s, keep);
      if (s .margin !== undefined) prim .setMargin (s .margin);
      if (spec .xflow) cc .setLocalScaling (vec ([1, 1, 1]));
      cc .addChildShape (spec .xflow ? new Ammo .btTransform (new Ammo .btQuaternion (0, 0, 0, 1), new Ammo .btVector3 (0, 0, 0)) : toTransform (s .local), prim);
      cc .setLocalScaling (vec ([1, 1, 1]));
      keep .push (prim);
      return cc;
   });

   // (X_ITE makes every CollidableShape before the RigidBodies that hold them.)
   const made = spec .xflow ? spec .bodies .map (makeCollidables) : null;

   const bodies = [ ];

   for (const b of spec .bodies)
   {
      // RigidBody constructor.
      const compound = new Ammo .btCompoundShape ();
      const motionState = new Ammo .btDefaultMotionState ();
      const info = new Ammo .btRigidBodyConstructionInfo (0, motionState, compound);
      const rb = new Ammo .btRigidBody (info);
      const body = { spec: b, compound, motionState, info, rb, ptr: Ammo .getPointer (rb) };
      bodies .push (body);

      // Collidables.
      body .collidables = made ? made [bodies .length - 1] : makeCollidables (b);

      // RigidBody.set_compoundShape__.
      const zero = zeroTransform ();
      for (const cc of body .collidables) compound .addChildShape (zero, cc);
      body .setTransform = () =>
      {
         const t = toTransform (body .m ?? b .m), it = toTransform (body .im ?? b .im);
         for (let i = 0, length = compound .getNumChildShapes (); i < length; ++ i)
            compound .updateChildTransform (i, it, false);
         compound .recalculateLocalAabb ();
         motionState .setWorldTransform (t);
         rb .setMotionState (motionState);
      };
      body .setTransform ();

      body .init = () =>
      {
         rb .setLinearVelocity (vec (b .fixed ? [0, 0, 0] : b .linear_velocity)); rb .activate ();
         rb .setAngularVelocity (vec (b .fixed ? [0, 0, 0] : b .angular_velocity)); rb .activate ();
         rb .setAngularFactor (vec (b .use_finite_rotation ? b .finite_rotation_axis : [1, 1, 1]));
         if (b .auto_damp) rb .setDamping (b .linear_damping, b .angular_damping); else rb .setDamping (0, 0);
         rb .activate ();
         {
            const com = new Ammo .btTransform (new Ammo .btQuaternion (0, 0, 0, 1), vec (b .center_of_mass));
            rb .setCenterOfMassTransform (com);
         }
         {
            const i = b .inertia, mass = b .fixed ? 0 : b .mass;
            const li = new Ammo .btVector3 (i [0] + i [1] + i [2], i [3] + i [4] + i [5], i [6] + i [7] + i [8]);
            compound .calculateLocalInertia (mass, li);
            body .inertia = vx (li);
            rb .setMassProps (mass, li);
         }
         if (b .auto_disable) rb .setSleepingThresholds (b .disable_linear_speed, b .disable_angular_speed);
         else rb .setSleepingThresholds (0, 0);
      };
      body .init ();

      // _transform event after initialization.
      if (! spec .xflow) body .setTransform ();
   }

   // RigidBodyCollection.
   const g = vec (spec .gravity);
   world .setGravity (g);

   const collider = spec .collider;
   for (const body of bodies)
   {
      body .rb .setFriction (collider ? collider .friction [0] : 0.5);
      body .rb .setRollingFriction (collider ? collider .friction [1] : 0);
      body .rb .getCollisionShape () .setMargin (spec .contact_surface_thickness);
   }

   for (const body of bodies) if (body .spec .enabled !== false) world .addRigidBody (body .rb);

   // X_ITE's load, as the real call log shows it (RigidBody / CollidableShape events after the bodies are in
   // the world): every RigidBody makes its compound again (set_compoundShape__), the collidables learn their
   // position (eventsProcessed) and the bodies their transform; the first time a Sphere, Cone or Cylinder is
   // live its _rebuild makes the CollidableShape's collision shape again, and the body its compound.
   if (spec .xflow)
   {
      const zero = zeroTransform ();
      const rebuilds = s => s .rebuild ?? ["sphere", "cone", "cylinder"] .includes (s .type);
      const remake = body => { body .compound .removeChildShapeByIndex (0); for (const cc of body .collidables) body .compound .addChildShape (zero, cc); body .setTransform (); body .init (); };
      const localOf = (body, k) => { const m = new Matrix4 (... body .spec .m); if (body .spec .shapes [k] .offset) m .translate (new Vector3 (... body .spec .shapes [k] .offset)); return [... m]; };

      for (const body of bodies) remake (body);
      for (const body of bodies) body .collidables .forEach ((cc, k) => cc .updateChildTransform (0, toTransform (localOf (body, k)), true));
      for (const body of bodies) body .setTransform ();

      for (const body of bodies)
      {
         if (! body .spec .shapes .some (rebuilds)) continue;

         body .collidables .forEach ((cc, k) =>
         {
            const s = body .spec .shapes [k];
            if (! rebuilds (s)) return;
            cc .removeChildShapeByIndex (0);
            cc .setLocalScaling (vec ([1, 1, 1]));
            const prim = makePrimitive (s, keep);
            if (s .margin !== undefined) prim .setMargin (s .margin);
            cc .addChildShape (toTransform (s .local), prim);
            cc .setLocalScaling (vec ([1, 1, 1]));
            keep .push (prim);
         });

         remake (body);
         body .collidables .forEach ((cc, k) => cc .updateChildTransform (0, toTransform (localOf (body, k)), true));
         body .setTransform ();
      }
   }

   const joints = [ ];

   const makeJoint = j =>
   {
      const b1 = bodies .find (b => b .spec .name === j .body1), b2 = bodies .find (b => b .spec .name === j .body2);
      let joint;

      switch (j .type)
      {
         case "ball":
            joint = new Ammo .btPoint2PointConstraint (b1 .rb, b2 .rb, new Ammo .btVector3 (), new Ammo .btVector3 ());
            joint .setPivotA (vec (j .pivot_a));
            joint .setPivotB (vec (j .pivot_b));
            break;
         case "hinge":
            joint = new Ammo .btHingeConstraint (b1 .rb, b2 .rb, vec (j .pivot_a), vec (j .pivot_b), vec (j .axis_a), vec (j .axis_b), false);
            break;
         case "slider":
            joint = new Ammo .btSliderConstraint (b1 .rb, b2 .rb, toTransform (j .frame_a), toTransform (j .frame_b), true);
            joint .setLowerAngLimit (0);
            joint .setUpperAngLimit (0);
            joint .setLowerLinLimit (j .lower);
            joint .setUpperLinLimit (j .upper);
            break;
      }

      return joint;
   };

   for (const j of spec .joints ?? [ ])
   {
      const joint = makeJoint (j);
      world .addConstraint (joint, true);
      joints .push (joint);
   }

   // RigidBody.update () and what its field writes trigger (set_position__ / set_transform__ /
   // set_linearVelocity__ ...): X_ITE feeds the motion state back into Ammo every frame.
   const feedback = bodies =>
   {
      // RigidBody.update (): the motion state, then the four field writes (position, orientation, velocities).
      const next = bodies .map (b =>
      {
         const t = new Ammo .btTransform ();
         b .motionState .getWorldTransform (t);
         const o = t .getOrigin (), q = t .getRotation ();
         const rotation = new Rotation4 ();
         rotation .setQuaternion (new Quaternion (q .x (), q .y (), q .z (), q .w ()));
         const m = new Matrix4 ();
         m .setTransform (new Vector3 (o .x (), o .y (), o .z ()), rotation);
         const lv = b .rb .getLinearVelocity (), av = b .rb .getAngularVelocity ();
         return { m: [... m], lv: vx (lv), av: vx (av) };
      });

      // The events they cause run in this order: set_linearVelocity__ / set_angularVelocity__ of every body ...
      bodies .forEach ((b, i) =>
      {
         b .rb .setLinearVelocity (vec (b .spec .fixed ? [0, 0, 0] : next [i] .lv)); b .rb .activate ();
         b .rb .setAngularVelocity (vec (b .spec .fixed ? [0, 0, 0] : next [i] .av)); b .rb .activate ();
      });

      // ... the collidables' eventsProcessed (geometryNode ._translation = position, ._rotation = orientation) ...
      bodies .forEach ((b, i) =>
      {
         b .collidables .forEach ((cc, k) =>
         {
            const lm = new Matrix4 (... next [i] .m);
            if (b .spec .shapes [k] .offset) lm .translate (new Vector3 (... b .spec .shapes [k] .offset));
            cc .updateChildTransform (0, toTransform ([... lm]), true);
         });
      });

      // ... and the bodies' set_transform__.
      bodies .forEach ((b, i) =>
      {
         b .m = next [i] .m;
         b .im = inv16 (next [i] .m);
         b .setTransform ();
      });
   };

   // Run-time changes (`ops` of the scene, after step N, before the dump of step N): what the field handlers of
   // RigidBodyCollection, RigidBody and the joint nodes do to a live world.
   let added = bodies .filter (b => b .spec .enabled !== false);
   const named = n => bodies .find (b => b .spec .name === n);
   const applyOp = op =>
   {
      const b = op .name === undefined ? null : named (op .name);
      switch (op .op)
      {
         case "gravity": gravityNow = op .v; world .setGravity (vec (op .v)); break;
         case "margin": for (const x of bodies) x .rb .getCollisionShape () .setMargin (op .v); break;
         case "friction": b .rb .setFriction (op .f); b .rb .setRollingFriction (op .rf); break;
         case "velocity": b .rb .setLinearVelocity (vec (op .lin)); b .rb .activate (); b .rb .setAngularVelocity (vec (op .ang)); b .rb .activate (); break;
         case "force": b .force = op .force; b .torque = op .torque; break;
         case "enable": b .enabled = op .value; break;
         case "rebuild_world": // RigidBodyCollection.set_dynamicsWorld__
            for (const x of added) world .removeRigidBody (x .rb);
            added = bodies .filter (x => x .enabled ?? (x .spec .enabled !== false));
            for (const x of added) world .addRigidBody (x .rb);
            break;
         case "remove_joint": world .removeConstraint (joints [op .index]); Ammo .destroy (joints [op .index]); joints [op .index] = null; break;
         case "add_joint": joints [op .index] = makeJoint ({ ... spec .joints [op .index], ... (op .override ?? { }) }); world .addConstraint (joints [op .index], true); break;
         case "pivot": joints [op .index] .setPivotA (vec (op .a)); joints [op .index] .setPivotB (vec (op .b)); break;
         case "mass":
         {
            const li = new Ammo .btVector3 (op .inertia [0] + op .inertia [1] + op .inertia [2], op .inertia [3] + op .inertia [4] + op .inertia [5], op .inertia [6] + op .inertia [7] + op .inertia [8]);
            b .compound .calculateLocalInertia (op .mass, li);
            b .rb .setMassProps (op .mass, li);
            b .inertia = vx (li);
            break;
         }
         case "damping": b .rb .setDamping (op .lin, op .ang); b .rb .activate (); break;
         case "thresholds": b .rb .setSleepingThresholds (op .lin, op .ang); break;
         case "teleport": b .m = op .m; b .im = inv16 (op .m); b .setTransform (); break;
         default: throw new Error (`op ${op .op}`);
      }
   };

   // Update loop.
   const idx = ptr => bodies .findIndex (b => b .ptr === ptr);
   const steps = [ ];
   const dump = () =>
   {
      const state = bodies .map (b =>
      {
         const t = new Ammo .btTransform ();
         b .motionState .getWorldTransform (t);
         const wt = b .rb .getWorldTransform ();
         const lv = b .rb .getLinearVelocity (), av = b .rb .getAngularVelocity ();
         return { pos: vx (t .getOrigin ()), quat: qx (t .getRotation ()), lin: vx (lv), ang: vx (av), wpos: vx (wt .getOrigin ()), wquat: qx (wt .getRotation ()), wbasis: [0, 1, 2] .flatMap (i => vx (wt .getBasis () .getRow (i))), active: b .rb .isActive (), inertia: b .inertia, aabb: (() => { const lo = new Ammo .btVector3 (), hi = new Ammo .btVector3 (); b .rb .getAabb (lo, hi); return [... vx (lo), ... vx (hi)]; })() };
      });

      const manifolds = [ ];
      for (let i = 0, n = dispatcher .getNumManifolds (); i < n; ++ i)
      {
         const m = dispatcher .getManifoldByIndexInternal (i);
         const points = [ ];
         for (let k = 0, nc = m .getNumContacts (); k < nc; ++ k)
         {
            const p = m .getContactPoint (k);
            points .push ({ a: vx (p .getPositionWorldOnA ()), b: vx (p .getPositionWorldOnB ()), n: vx (p .get_m_normalWorldOnB ()), distance: p .getDistance (), impulse: p .getAppliedImpulse (), la: vx (p .get_m_localPointA ()), lb: vx (p .get_m_localPointB ()) });
         }
         manifolds .push ({ b0: idx (Ammo .getPointer (m .getBody0 ())), b1: idx (Ammo .getPointer (m .getBody1 ())), points });
      }

      return { bodies: state, manifolds };
   };

   if (spec .detect) world .performDiscreteCollisionDetection ();
   steps .push (dump ());

   for (const dt of spec .dts)
   {
      // set_bounce__
      if (collider ?.bounce_enabled)
      {
         for (const b of bodies)
         {
            const v = b .rb .getLinearVelocity ();
            if (Math .hypot (v .x (), v .y (), v .z ()) >= collider .min_bounce_speed) b .rb .setRestitution (collider .bounce);
            else b .rb .setRestitution (0);
         }
      }
      else for (const b of bodies) b .rb .setRestitution (0);

      let substeps = 0;
      const applyForces = () =>
      {
         for (const b of bodies)
         {
            if (b .spec .fixed) continue;
            b .rb .setGravity (vec (b .spec .use_global_gravity ? gravityNow : [0, 0, 0]));
            b .rb .applyForce (vec (b .force ?? b .spec .force), vec ([0, 0, 0]));
            b .rb .applyTorque (vec (b .torque ?? b .spec .torque));
         }
      };

      if (spec .prefer_accuracy)
      {
         const d = dt / spec .iterations;
         for (let i = 0; i < spec .iterations; ++ i) { applyForces (); world .stepSimulation (d, 0); }
      }
      else if (spec .sub_at === steps .length)
      {
         // Each fixed substep on its own (maxSubSteps 0 steps exactly the given time), dumped one by one.
         applyForces ();
         const d = dt / spec .iterations;
         substeps = spec .sub_n;
         var subs = [ ];
         for (let i = 0; i < substeps; ++ i) { world .stepSimulation (d, 0); subs .push (dump ()); }
      }
      else
      {
         applyForces ();
         substeps = world .stepSimulation (dt, spec .iterations + 2, dt / spec .iterations);
      }

      if (spec .feedback) feedback (bodies);

      for (const hook of spec .hooks?.[steps .length] ?? [ ]) hook (bodies, world);
      for (const op of spec .ops?.[steps .length] ?? [ ]) applyOp (op);
      steps .push ({ ... dump (), substeps, ... (typeof subs !== "undefined" && spec .sub_at === steps .length ? { subs } : { }) });
   }

   return { steps };
}

// --- scene descriptions --------------------------------------------------------------------

const body = (name, o) =>
{
   const b = {
      name, fixed: false, mass: 1, inertia: [1, 0, 0, 0, 1, 0, 0, 0, 1], position: [0, 0, 0], rotation: [0, 1, 0, 0],
      linear_velocity: [0, 0, 0], angular_velocity: [0, 0, 0], use_finite_rotation: false, finite_rotation_axis: [0, 1, 0],
      auto_damp: false, linear_damping: 0.001, angular_damping: 0.001, center_of_mass: [0, 0, 0], auto_disable: false,
      disable_linear_speed: 0, disable_angular_speed: 0, use_global_gravity: true, force: [0, 0, 0], torque: [0, 0, 0],
      shapes: [ ], ... o,
   };
   b .m = m16 (b .position, b .rotation);
   b .im = inv16 (b .m);
   b .shapes = b .shapes .map (s => ({ ... s, local: s .local ?? m16 (b .position, b .rotation) .map ((v, i) => v) }));
   // CollidableShape.getLocalTransform: matrix, translated by the offset.
   b .shapes = b .shapes .map (s =>
   {
      const m = new Matrix4 (... b .m);
      if (s .offset) m .translate (new Vector3 (... s .offset));
      return { ... s, local: [... m] };
   });
   return b;
};

const scene = (name, description, o) => ({
   name, description, gravity: [0, -9.8, 0], iterations: 10, prefer_accuracy: false, contact_surface_thickness: 0, collider: null,
   bodies: [ ], joints: [ ], dts: [ ], ... o,
});

const frame = 1 / 60;
const dtsOf = (n, f = frame) => Array .from ({ length: n }, () => f);
// X_ITE's moving average time step: ((DELAY - 1) * previous + dt) / DELAY, starting at 0.
const xiteDts = (n, fps = 60) => { let d = 0; return Array .from ({ length: n }, () => (d = (14 * d + 1 / Math .max (10, fps)) / 15)); };

// --- helpers copied from ammo_dyn_vectors.mjs ------------------------------------------------

const ground = (name = "ground", o = { }) => body (name, { fixed: true, position: [0, -0.5, 0], shapes: [{ type: "box", half: [20, 0.5, 20] }], ... o });
const bouncy = { friction: [0.5, 0], bounce: 0.6, bounce_enabled: true, min_bounce_speed: 0.1 };
const grippy = { friction: [0.5, 0], bounce: 0, bounce_enabled: false, min_bounce_speed: 0 };
const unit = a => { const l = Math .hypot (... a); return a .map (v => v / l); };
const sphereOf = r => [{ type: "sphere", radius: r }];

const meshGround = (n, size) =>
{
   const h = (x, z) => 0.15 * Math .sin (x * 0.9) * Math .cos (z * 0.7) - 0.05 * x;
   const tris = [ ];
   for (let i = 0; i < n; ++ i) for (let j = 0; j < n; ++ j)
   {
      const x0 = (i - n / 2) * size, x1 = x0 + size, z0 = (j - n / 2) * size, z1 = z0 + size;
      const a = [x0, h (x0, z0), z0], b = [x1, h (x1, z0), z0], c = [x1, h (x1, z1), z1], d = [x0, h (x0, z1), z1];
      tris .push ([a, d, b], [b, d, c]);
   }
   return tris;
};

/** SliderJoint.js: frameInA takes the (axis rotated) basis of body1 and the world position as origin; frameInB (a typo upstream) keeps an identity basis. */
const sliderFrames = (anchorPos, axis) =>
{
   const rot = new Rotation4 (); rot .setVectors (new Vector3 (1, 0, 0), new Vector3 (... axis));
   return { frame_a: m16 (anchorPos, [... rot]), frame_b: m16 (anchorPos, [0, 1, 0, 0]) };
};

/** mulberry32 */
const rng = seed => () => { seed |= 0; seed = seed + 0x6D2B79F5 | 0; let t = Math .imul (seed ^ seed >>> 15, 1 | seed); t = t + Math .imul (t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; };

const randomShape = (r, type) =>
{
   const u = (a, b) => a + (b - a) * r ();
   switch (type)
   {
      case "sphere": return { type, radius: u (0.25, 0.7) };
      case "box": return { type, half: [u (0.2, 0.7), u (0.2, 0.7), u (0.2, 0.7)] };
      case "cylinder": return { type, radius: u (0.2, 0.6), half_height: u (0.2, 0.7) };
      case "cone": return { type, radius: u (0.2, 0.6), height: u (0.4, 1.2) };
      case "hull":
         return { type, points: Array .from ({ length: 5 + Math .floor (r () * 4) }, () => [u (-0.6, 0.6), u (-0.6, 0.6), u (-0.6, 0.6)]) };
   }
};

const randomRotation = r =>
{
   const a = [r () - 0.5, r () - 0.5, r () - 0.5], l = Math .hypot (... a) || 1;
   return [a [0] / l, a [1] / l, a [2] / l, r () * Math .PI * 2];
};

const extent = s => s .type === "sphere" ? s .radius : s .type === "box" ? Math .hypot (... s .half) : s .type === "cylinder" ? Math .hypot (s .radius, s .half_height)
   : s .type === "cone" ? Math .hypot (s .radius, s .height) : Math .max (... s .points .map (p => Math .hypot (... p)));

// --- height fields (ElevationGrid) -----------------------------------------------------------

/** The CollidableShape of an ElevationGrid: the shape plus the offset CollidableShape.js gives it. */
const heightShape = (xd, zd, xs, zs, heights) =>
{
   const min = Math .min (... heights), max = Math .max (... heights);
   return { type: "height", xd, zd, xs, zs, heights, offset: [xs * (xd - 1) * 0.5, (min + max) * 0.5, zs * (zd - 1) * 0.5] };
};

const heightsOf = (xd, zd, seed, amp = 0.5) => { const r = rng (seed); return Array .from ({ length: xd * zd }, () => Math .fround (amp * r ())); };
const hillsOf = (xd, zd, xs, zs) => Array .from ({ length: xd * zd }, (_, i) => Math .fround (0.4 * Math .sin ((i % xd) * xs * 0.9) * Math .cos (Math .floor (i / xd) * zs * 0.8) + 0.2));

const terrain = (name = "terrain", xd = 6, zd = 5, xs = 1.2, zs = 0.8, heights = hillsOf (xd, zd, xs, zs), o = { }) =>
   body (name, { fixed: true, shapes: [heightShape (xd, zd, xs, zs, heights)], ... o });

const scenes = [ ];

scenes .push (scene ("h01_height_sphere", "A sphere rolls down onto and across an ElevationGrid height field (6 x 5, spacing 1.2 x 0.8).", {
   collider: grippy,
   bodies: [ terrain (), body ("ball", { position: [1.2, 2.5, 1.0], linear_velocity: [0.8, 0, 0.3], shapes: sphereOf (0.35) }) ],
   dts: dtsOf (360),
}));

scenes .push (scene ("h02_height_box", "A tilted box falls on an ElevationGrid and tumbles (restitution).", {
   collider: bouncy,
   bodies: [ terrain (), body ("box", { position: [2.0, 2.0, 1.6], rotation: [0.3, 0.5, 0.8, 0.6], angular_velocity: [1, 2, 0.5], shapes: [{ type: "box", half: [0.4, 0.25, 0.3] }] }) ],
   dts: xiteDts (360),
}));

scenes .push (scene ("h03_height_cylinder_cone_hull", "A cylinder, a cone and a convex hull fall on an ElevationGrid.", {
   collider: grippy,
   bodies: [
      terrain (),
      body ("cyl", { position: [1.0, 2.2, 1.0], rotation: [1, 0, 0, 0.3], shapes: [{ type: "cylinder", radius: 0.35, half_height: 0.5 }] }),
      body ("cone", { position: [2.6, 2.2, 1.4], rotation: [0, 0, 1, 0.4], shapes: [{ type: "cone", radius: 0.4, height: 0.9 }] }),
      body ("hull", { position: [4.0, 2.2, 2.0], rotation: [1, 1, 0, 0.5], shapes: [{ type: "hull", points: [[-0.4, -0.4, -0.4], [0.4, -0.4, -0.4], [0, 0.4, -0.4], [0, 0, 0.5], [0.3, 0.2, 0.1]] }] }),
   ],
   dts: dtsOf (300),
}));

scenes .push (scene ("h04_height_flat", "A flat ElevationGrid (min == max) with a sphere and a box on it.", {
   collider: grippy,
   bodies: [ terrain ("flat", 4, 4, 1.5, 1.5, Array .from ({ length: 16 }, () => 0.25)), body ("ball", { position: [2.0, 1.5, 2.0], shapes: sphereOf (0.3) }), body ("box", { position: [3.0, 1.2, 1.2], shapes: [{ type: "box", half: [0.3, 0.3, 0.3] }] }) ],
   dts: dtsOf (200),
}));

scenes .push (scene ("h05_height_compound_body", "A body made of two collidables (a hull and an offset cone) falls on an ElevationGrid; non-trivial center of mass.", {
   collider: bouncy,
   bodies: [
      terrain (),
      body ("cmp", { position: [2.0, 2.4, 1.6], rotation: [0.3, 0.5, 0.8, 0.9], center_of_mass: [0.05, 0, 0], mass: 2,
         shapes: [{ type: "hull", points: [[-0.4, -0.3, -0.3], [0.4, -0.3, -0.3], [0, 0.4, -0.3], [0, 0, 0.4]] }, { type: "cone", radius: 0.3, height: 0.8, offset: [0.6, 0.2, 0] }] }),
   ],
   dts: xiteDts (360),
}));

scenes .push (scene ("h06_height_big_wavy", "A 24 x 20 ElevationGrid with random heights, three spheres and a box dropped on it.", {
   collider: grippy,
   bodies: [
      terrain ("terrain", 24, 20, 0.5, 0.5, heightsOf (24, 20, 99, 0.8)),
      body ("s1", { position: [2, 3, 2], shapes: sphereOf (0.3) }),
      body ("s2", { position: [4, 3, 5], shapes: sphereOf (0.25), linear_velocity: [0.5, 0, 0] }),
      body ("s3", { position: [7, 3, 7], shapes: sphereOf (0.4) }),
      body ("bx", { position: [5, 3, 3], rotation: [0, 1, 0, 0.7], shapes: [{ type: "box", half: [0.3, 0.2, 0.3] }] }),
   ],
   dts: dtsOf (300),
}));

// --- a mixed joint chain; DoubleAxisHinge frames ---------------------------------------------

scenes .push (scene ("j01_mixed_chain", "A fixed anchor, a ball joint, a hinge and a slider in one chain of three dynamic bodies.", {
   bodies: [
      body ("anchor", { fixed: true, position: [0, 6, 0], shapes: [{ type: "box", half: [0.1, 0.1, 0.1] }] }),
      body ("b1", { position: [1.2, 6.2, 0.1], rotation: [0.2, 0.7, 0.4, 0.8], mass: 3, inertia: [0.6, 0, 0, 0, 0.9, 0, 0, 0, 0.5], linear_velocity: [0, 0.5, 1], shapes: [{ type: "box", half: [0.25, 0.15, 0.2] }] }),
      body ("b2", { position: [2.6, 6.0, -0.1], mass: 0.5, angular_velocity: [1, 2, -1], shapes: [{ type: "box", half: [0.3, 0.1, 0.1] }] }),
      body ("b3", { position: [3.6, 6.0, -0.1], mass: 1, shapes: sphereOf (0.2) }),
   ],
   joints: [
      { type: "ball", body1: "anchor", body2: "b1", pivot_a: [0, 0, 0], pivot_b: [-1.2, -0.2, -0.1] },
      { type: "hinge", body1: "b1", body2: "b2", pivot_a: [0.7, 0, 0.1], pivot_b: [-0.7, 0.2, 0], axis_a: unit ([0.3, 0.5, 0.8]), axis_b: unit ([0.2, 0.55, 0.8]) },
      { type: "slider", body1: "b2", body2: "b3", ... sliderFrames ([2.6, 6.0, -0.1], [1, 0, 0]), lower: -0.2, upper: 0.8 },
   ],
   dts: dtsOf (300),
}));

scenes .push (scene ("j02_double_axis_hinge", "DoubleAxisHingeJoint.js: the same btHingeConstraint call as SingleAxisHinge with distinct local axes of two rotated bodies and an anchor between them.", {
   bodies: [
      body ("a", { position: [0, 5, 0], rotation: [0, 0, 1, 0.6], mass: 2, shapes: [{ type: "box", half: [0.4, 0.2, 0.2] }], angular_velocity: [0.2, 0.3, 0] }),
      body ("b", { position: [1.1, 5.1, 0.3], rotation: [1, 0, 0, -0.4], mass: 1, shapes: [{ type: "box", half: [0.5, 0.1, 0.2] }], inertia: [0.3, 0, 0, 0, 0.5, 0, 0, 0, 0.8] }),
   ],
   joints: [{ type: "hinge", body1: "a", body2: "b", pivot_a: [0.5, 0, 0.1], pivot_b: [-0.6, 0.1, -0.2], axis_a: unit ([0.1, 0.9, 0.2]), axis_b: unit ([0.3, 0.8, 0.1]) }],
   dts: xiteDts (300),
}));

scenes .push (scene ("j03_hinge_floor", "A hinged door falling against a floor (joint plus contacts, disableCollisionsBetweenLinkedBodies).", {
   collider: grippy,
   bodies: [
      ground (),
      body ("post", { fixed: true, position: [0, 1.0, 0], shapes: [{ type: "box", half: [0.1, 1.0, 0.1] }] }),
      body ("door", { position: [0.9, 1.9, 0], mass: 1, inertia: [0.3, 0, 0, 0, 0.3, 0, 0, 0, 0.3], shapes: [{ type: "box", half: [0.8, 0.9, 0.05] }] }),
   ],
   joints: [{ type: "hinge", body1: "post", body2: "door", pivot_a: [0.1, 0.9, 0], pivot_b: [-0.8, 0, 0], axis_a: [0, 0, 1], axis_b: [0, 0, 1] }],
   dts: dtsOf (300),
}));

// --- run-time changes of a live world --------------------------------------------------------

const stackBodies = () => [
   ground (),
   body ("b1", { position: [0, 0.5, 0], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
   body ("b2", { position: [0.05, 1.52, 0], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
   body ("b3", { position: [-0.03, 2.55, 0.02], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
];

scenes .push (scene ("o01_rebuild_world", "set_dynamicsWorld__: the middle box is disabled (all bodies removed and the enabled ones added again), later enabled again.", {
   collider: grippy,
   bodies: stackBodies (),
   ops: { 60: [{ op: "enable", name: "b2", value: false }, { op: "rebuild_world" }], 120: [{ op: "enable", name: "b2", value: true }, { op: "rebuild_world" }], 180: [{ op: "rebuild_world" }] },
   dts: dtsOf (260),
}));

scenes .push (scene ("o02_joint_remove_add", "removeConstraint + Ammo.destroy, later a new constraint with another pivot (a joint field change rebuilds the joint).", {
   bodies: [
      body ("anchor", { fixed: true, position: [0, 6, 0], shapes: [{ type: "box", half: [0.1, 0.1, 0.1] }] }),
      body ("b1", { position: [1.2, 6.3, 0.2], rotation: [0.2, 0.7, 0.4, 0.8], mass: 3, inertia: [0.6, 0, 0, 0, 0.9, 0, 0, 0, 0.5], linear_velocity: [0, 0.5, 1], shapes: [{ type: "box", half: [0.25, 0.15, 0.2] }] }),
      body ("b2", { position: [2.6, 6.1, -0.1], mass: 0.5, angular_velocity: [1, 2, -1], shapes: sphereOf (0.2) }),
   ],
   joints: [
      { type: "ball", body1: "anchor", body2: "b1", pivot_a: [0, 0, 0], pivot_b: [-1.2, -0.3, -0.2] },
      { type: "ball", body1: "b1", body2: "b2", pivot_a: [0.7, 0, 0.1], pivot_b: [-0.7, 0.2, 0] },
   ],
   ops: { 80: [{ op: "remove_joint", index: 1 }], 140: [{ op: "add_joint", index: 1, override: { pivot_a: [0.5, 0.1, 0], pivot_b: [-0.9, 0, 0.1] } }] },
   dts: dtsOf (260),
}));

scenes .push (scene ("o03_pivot_change", "BallJoint.set_anchorPoint__ on a live joint: setPivotA / setPivotB with new vectors.", {
   bodies: [
      body ("anchor", { fixed: true, position: [0, 6, 0], shapes: [{ type: "box", half: [0.1, 0.1, 0.1] }] }),
      body ("bob", { position: [2.5, 6, 0], mass: 2, shapes: sphereOf (0.3) }),
   ],
   joints: [{ type: "ball", body1: "anchor", body2: "bob", pivot_a: [0, 0, 0], pivot_b: [-2.5, 0, 0] }],
   ops: { 90: [{ op: "pivot", index: 0, a: [0.5, 0, 0], b: [-1.5, 0.5, 0] }] },
   dts: dtsOf (240),
}));

scenes .push (scene ("o04_margin_gravity", "contactSurfaceThickness (setMargin on the body compounds) and the gravity change while a stack rests.", {
   collider: grippy,
   bodies: stackBodies (),
   ops: { 50: [{ op: "margin", v: 0.05 }], 100: [{ op: "gravity", v: [1.5, -3, 0.5] }], 160: [{ op: "margin", v: 0 }, { op: "gravity", v: [0, -9.8, 0] }] },
   dts: dtsOf (260),
}));

scenes .push (scene ("o05_runtime_fields", "A kick (velocity), a new mass, damping, a new force and torque, a friction change, sleeping thresholds and a teleport of a body.", {
   collider: grippy,
   bodies: [
      ground (),
      body ("box", { position: [0, 0.5, 0], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }], auto_disable: true, disable_linear_speed: 0.4, disable_angular_speed: 0.4 }),
      body ("ball", { position: [3, 0.5, 0], shapes: sphereOf (0.5) }),
   ],
   ops: {
      30: [{ op: "velocity", name: "box", lin: [2, 0, 0.5], ang: [0, 1, 0] }],
      70: [{ op: "mass", name: "ball", mass: 3, inertia: [0.5, 0, 0, 0, 0.5, 0, 0, 0, 0.5] }, { op: "damping", name: "box", lin: 0.4, ang: 0.3 }],
      90: [{ op: "force", name: "ball", force: [4, 0, 0], torque: [0, 0, 1] }, { op: "friction", name: "box", f: 0.9, rf: 0.2 }],
      130: [{ op: "thresholds", name: "ball", lin: 1, ang: 1 }, { op: "force", name: "ball", force: [0, 0, 0], torque: [0, 0, 0] }],
      170: [{ op: "teleport", name: "ball", m: m16 ([0.5, 3, 0.2], [0, 1, 0, 0.5]), im: inv16 (m16 ([0.5, 3, 0.2], [0, 1, 0, 0.5])) }],
   },
   dts: dtsOf (300),
}));

scenes .push (scene ("o06_sleeping_body_removed_readded", "A body that fell asleep is removed from the world and added again (set_dynamicsWorld__), then woken by a falling sphere.", {
   collider: grippy,
   bodies: [
      ground (),
      body ("rest", { position: [0, 0.505, 0], auto_disable: true, disable_linear_speed: 0.5, disable_angular_speed: 0.5, shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("drop", { position: [0.2, 40, 0.1], shapes: sphereOf (0.4) }),
   ],
   ops: { 200: [{ op: "rebuild_world" }], 260: [{ op: "enable", name: "rest", value: false }, { op: "rebuild_world" }], 300: [{ op: "enable", name: "rest", value: true }, { op: "rebuild_world" }] },
   dts: dtsOf (520),
}));

scenes .push (scene ("o07_body_removed_with_joint", "A body with a joint is removed from the world (the constraint stays), then added again.", {
   bodies: [
      body ("anchor", { fixed: true, position: [0, 6, 0], shapes: [{ type: "box", half: [0.1, 0.1, 0.1] }] }),
      body ("bob", { position: [2.5, 6, 0], mass: 2, shapes: sphereOf (0.3) }),
      body ("free", { position: [5, 6, 0], shapes: sphereOf (0.3) }),
   ],
   joints: [{ type: "ball", body1: "anchor", body2: "bob", pivot_a: [0, 0, 0], pivot_b: [-2.5, 0, 0] }],
   ops: { 60: [{ op: "enable", name: "free", value: false }, { op: "rebuild_world" }], 120: [{ op: "enable", name: "free", value: true }, { op: "rebuild_world" }] },
   dts: dtsOf (200),
}));

// --- random corpora --------------------------------------------------------------------------

/** One convex body against a terrain (a BVH mesh or a height field): one detection pass each. */
const terrainPairCases = (name, type, kind, count, seed) =>
{
   const r = rng (seed), cases = [ ];
   const hf = kind === "height" ? heightShape (5, 5, 1.2, 1.2, heightsOf (5, 5, seed ^ 0x5bd1, 0.5)) : null;
   for (let i = 0; i < count; ++ i)
   {
      const sa = randomShape (r, type), e = extent (sa);
      const [cx, cz] = kind === "height" ? [2.4, 2.4] : [0, 0];
      const pos = [cx + (r () - 0.5) * 3.5, 0.1 + 0.9 * r () * e + (kind === "height" ? 0.25 : 0), cz + (r () - 0.5) * 3.5];
      cases .push (scene (`${name}_${i}`, "", {
         gravity: [0, 0, 0], detect: true,
         bodies: [
            body ("a", { position: pos, rotation: randomRotation (r), shapes: [sa] }),
            body ("b", { fixed: true, position: [0, 0, 0], shapes: [kind === "height" ? hf : { type: "mesh", triangles: meshGround (4, 1.2) }] }),
         ],
      }));
   }
   return cases;
};

/** A random convex body dropped on a terrain: contacts, friction and the solver. */
const terrainSolveCases = (name, type, kind, count, seed) =>
{
   const r = rng (seed), cases = [ ], u = (a, b) => a + (b - a) * r ();
   const hf = kind === "height" ? heightShape (6, 6, 1.0, 1.0, hillsOf (6, 6, 1.0, 1.0)) : null;
   for (let i = 0; i < count; ++ i)
   {
      const sh = randomShape (r, type), e = extent (sh);
      const [cx, cz] = kind === "height" ? [2.5, 2.5] : [0, 0];
      cases .push (scene (`${name}_${i}`, "", {
         collider: { friction: [u (0.1, 0.9), 0], bounce: 0.3, bounce_enabled: r () < 0.5, min_bounce_speed: 0.1 },
         bodies: [
            body ("t", { fixed: true, position: [0, 0, 0], shapes: [kind === "height" ? hf : { type: "mesh", triangles: meshGround (4, 1.2) }] }),
            body ("a", {
               position: [cx + u (-1, 1), 0.6 + e * u (0.8, 1.3), cz + u (-1, 1)], rotation: randomRotation (r), shapes: [sh],
               linear_velocity: [u (-2, 2), u (-3, 0), u (-2, 2)], angular_velocity: [u (-3, 3), u (-3, 3), u (-3, 3)],
               mass: u (0.5, 3), inertia: [u (0.2, 2), 0, 0, 0, u (0.2, 2), 0, 0, 0, u (0.2, 2)],
            }),
         ],
         prefer_accuracy: true, iterations: 1, dts: dtsOf (12, 1 / 60),
      }));
   }
   return cases;
};

/** Two bodies with 2 or 3 collidables each (a compound tree of compounds), one detection pass and a few solver steps. */
const multiCases = (name, count, seed) =>
{
   const r = rng (seed), cases = [ ], types = ["sphere", "box", "cylinder", "cone", "hull"], u = (a, b) => a + (b - a) * r ();
   const multi = () => Array .from ({ length: 2 + Math .floor (r () * 2) }, () => ({ ... randomShape (r, types [Math .floor (r () * types .length)]), offset: [u (-0.8, 0.8), u (-0.5, 0.5), u (-0.8, 0.8)] }));
   for (let i = 0; i < count; ++ i)
   {
      const pa = [r () - 0.5, r () - 0.5, r () - 0.5], d = [r () - 0.5, r () - 0.5, r () - 0.5], dl = Math .hypot (... d) || 1, reach = u (0.8, 2.6);
      cases .push (scene (`${name}_${i}`, "", {
         gravity: [0, 0, 0], detect: i % 2 === 0,
         bodies: [
            body ("a", { position: pa, rotation: randomRotation (r), shapes: multi () }),
            body ("b", { position: pa .map ((v, k) => v + d [k] / dl * reach), rotation: randomRotation (r), shapes: multi () }),
         ],
         prefer_accuracy: true, iterations: 1, dts: dtsOf (i % 2 === 0 ? 0 : 10, 1 / 60),
      }));
   }
   return cases;
};

const convexTypes = ["sphere", "box", "cylinder", "cone", "hull"];
const corpora = [ ];
let corpusSeed = 8000;
for (const t of convexTypes) corpora .push ([`pairs_${t}_mesh`, terrainPairCases (`pairs_${t}_mesh`, t, "mesh", 60, corpusSeed ++)]);
for (const t of convexTypes) corpora .push ([`pairs_${t}_height`, terrainPairCases (`pairs_${t}_height`, t, "height", 60, corpusSeed ++)]);
for (const t of convexTypes) corpora .push ([`solve_${t}_mesh`, terrainSolveCases (`solve_${t}_mesh`, t, "mesh", 40, corpusSeed ++)]);
for (const t of convexTypes) corpora .push ([`solve_${t}_height`, terrainSolveCases (`solve_${t}_height`, t, "height", 40, corpusSeed ++)]);
corpora .push (["multi_collidables", multiCases ("multi_collidables", 120, corpusSeed ++)]);

// --- output ----------------------------------------------------------------------------------

const outDir = resolve (outArg);
await mkdir (outDir, { recursive: true });

for (const [name] of corpora)
{
   if (only .length && !only .includes (name) && !only .includes ("corpora")) continue;
   Ammo = new AmmoFactory ();
   const cs = corpora .find (c => c [0] === name) [1];
   const cases = cs .map (sc => { const { hooks, ... rest } = sc; return { scene: rest, ... runScene (sc) }; });
   await writeFile (join (outDir, `${name}.json`), JSON .stringify ({ cases }));
   console .log (`${name}: ${cases .length} cases`);
}

for (const s of scenes)
{
   if (only .length && !only .includes (s .name) && !only .includes ("scenes")) continue;
   Ammo = new AmmoFactory ();
   const { hooks, ... rest } = s;
   const result = runScene (s);
   await writeFile (join (outDir, `${s .name}.json`), JSON .stringify ({ scene: rest, ... result }));
   console .log (`${s .name}: ${result .steps .length} steps`);
}
