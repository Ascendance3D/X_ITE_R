/**
 * Dynamics test vectors from the pinned X_ITE's Ammo (Bullet 2.82, asm.js). Each scene is built the
 * way Components/RigidBodyPhysics builds it (RigidBody.js set_compoundShape__ and friends,
 * CollidableShape.js, RigidBodyCollection.js update loop, the joint nodes' addJoint) and stepped with
 * X_ITE's stepSimulation call. The output holds the scene description (with the matrices X_ITE's
 * Matrix4 computed) and, per step, every body's motion-state transform, velocities, world transform
 * and the contact manifolds, at full precision.
 *
 *   node oracle/tools/ammo_dyn_vectors.mjs [outDir] [sceneName...]
 *
 * crates/x3d-runtime/tests/bullet_dyn_scenes.rs replays the same scene description through
 * x3d_runtime::physics::bt and prints the maximum absolute error per scene and quantity.
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
const [ outArg = join (ROOT, "crates/x3d-runtime/tests/data/bullet_dyn"), ... only ] = process .argv .slice (2);

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

   for (const j of spec .joints ?? [ ])
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
            if (j .limit) joint .setLimit (... j .limit);
            if (j .motor) { joint .enableAngularMotor (true, j .motor [0], j .motor [1]); }
            break;
         case "slider":
            joint = new Ammo .btSliderConstraint (b1 .rb, b2 .rb, toTransform (j .frame_a), toTransform (j .frame_b), true);
            joint .setLowerAngLimit (0);
            joint .setUpperAngLimit (0);
            joint .setLowerLinLimit (j .lower);
            joint .setUpperLinLimit (j .upper);
            break;
         case "generic6dof":
            joint = new Ammo .btGeneric6DofConstraint (b1 .rb, b2 .rb, toTransform (j .frame_a), toTransform (j .frame_b), true);
            joint .setLinearLowerLimit (vec (j .lin_lower));
            joint .setLinearUpperLimit (vec (j .lin_upper));
            joint .setAngularLowerLimit (vec (j .ang_lower));
            joint .setAngularUpperLimit (vec (j .ang_upper));
            break;
      }

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
            b .rb .setGravity (vec (b .spec .use_global_gravity ? spec .gravity : [0, 0, 0]));
            b .rb .applyForce (vec (b .spec .force), vec ([0, 0, 0]));
            b .rb .applyTorque (vec (b .spec .torque));
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

const scenes = [ ];

scenes .push (scene ("s01_sphere_free_fall", "A sphere falls under gravity, X_ITE time step with the moving average.", {
   bodies: [ body ("ball", { position: [0, 10, 0], shapes: [{ type: "sphere", radius: 0.5 }] }) ],
   dts: xiteDts (90),
}));

scenes .push (scene ("s01b_sphere_fixed_dt", "Free fall with a constant frame time of 1/60.", {
   bodies: [ body ("ball", { position: [1, 10, -2], linear_velocity: [0.3, 2, 0.1], shapes: [{ type: "sphere", radius: 0.5 }] }) ],
   dts: dtsOf (120),
}));

scenes .push (scene ("s01c_sphere_prefer_accuracy", "Free fall in preferAccuracy mode (stepSimulation (dt, 0) per iteration).", {
   prefer_accuracy: true, iterations: 4,
   bodies: [ body ("ball", { position: [0, 3, 0], shapes: [{ type: "sphere", radius: 0.25 }] }) ],
   dts: dtsOf (40, 1 / 30),
}));

scenes .push (scene ("s02_box_tumbling", "A box with angular velocity, torque and a non-trivial inertia, in free fall.", {
   bodies: [ body ("box", {
      position: [1, 5, -1], rotation: [0.267261241912424, 0.534522483824849, 0.801783725737273, 0.7],
      mass: 2.5, inertia: [0.9, 0, 0, 0, 2.3, 0, 0, 0, 0.4], angular_velocity: [1.5, -0.7, 2.2], linear_velocity: [0.5, 0, -0.25],
      torque: [0.2, 0.05, -0.1], shapes: [{ type: "box", half: [0.5, 0.25, 0.75] }],
   }) ],
   dts: xiteDts (120),
}));

// --- contacts ------------------------------------------------------------------------------

const ground = (name = "ground", o = { }) => body (name, { fixed: true, position: [0, -0.5, 0], shapes: [{ type: "box", half: [20, 0.5, 20] }], ... o });
const bouncy = { friction: [0.5, 0], bounce: 0.6, bounce_enabled: true, min_bounce_speed: 0.1 };
const grippy = { friction: [0.5, 0], bounce: 0, bounce_enabled: false, min_bounce_speed: 0 };

scenes .push (scene ("s03_sphere_bounce_rest", "A sphere drops onto a static box plane, bounces (restitution 0.6) and comes to rest.", {
   collider: bouncy,
   bodies: [ ground (), body ("ball", { position: [0, 2.5, 0], shapes: [{ type: "sphere", radius: 0.5 }] }) ],
   dts: xiteDts (360),
}));

scenes .push (scene ("s03b_sphere_roll_friction", "A sphere with spin and sideways speed lands on the plane and rolls out (friction 0.5).", {
   collider: grippy,
   bodies: [ ground (), body ("ball", { position: [0, 0.8, 0], linear_velocity: [3, 0, 1], angular_velocity: [0, 0, 4], mass: 2, shapes: [{ type: "sphere", radius: 0.5 }] }) ],
   dts: dtsOf (240),
}));

scenes .push (scene ("s03c_box_tumble_land", "A tilted box with spin lands on the plane and settles.", {
   collider: bouncy,
   bodies: [ ground (), body ("box", { position: [0, 3, 0], rotation: [0.3, 0.5, 0.8, 0.6], angular_velocity: [1, 2, 0.5], mass: 3, shapes: [{ type: "box", half: [0.5, 0.3, 0.4] }] }) ],
   dts: xiteDts (360),
}));

scenes .push (scene ("s04_box_stack3", "Three boxes stacked on the ground plane.", {
   collider: grippy,
   bodies: [
      ground (),
      body ("b1", { position: [0, 0.5, 0], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("b2", { position: [0.05, 1.52, 0], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("b3", { position: [-0.03, 2.55, 0.02], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
   ],
   dts: dtsOf (300),
}));

const meshGround = (n, size) =>
{
   // an n x n grid of quads, slightly undulating, as triangles
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

scenes .push (scene ("s05_sphere_mesh", "A sphere rolls over a BVH triangle mesh.", {
   collider: grippy,
   bodies: [
      body ("mesh", { fixed: true, shapes: [{ type: "mesh", triangles: meshGround (8, 1.5) }] }),
      body ("ball", { position: [-2, 1.2, 0.3], linear_velocity: [1, 0, 0], shapes: [{ type: "sphere", radius: 0.4 }] }),
   ],
   dts: dtsOf (300),
}));

scenes .push (scene ("s05b_box_mesh", "A box falls on a BVH triangle mesh.", {
   collider: grippy,
   bodies: [
      body ("mesh", { fixed: true, shapes: [{ type: "mesh", triangles: meshGround (8, 1.5) }] }),
      body ("box", { position: [0.4, 1.5, 0.3], rotation: [0, 1, 0, 0.5], shapes: [{ type: "box", half: [0.4, 0.3, 0.5] }] }),
   ],
   dts: dtsOf (240),
}));

scenes .push (scene ("s06_box_incline_friction", "A box slides down an inclined static box with friction 0.3.", {
   collider: { friction: [0.3, 0], bounce: 0, bounce_enabled: false, min_bounce_speed: 0 },
   bodies: [
      body ("ramp", { fixed: true, position: [0, 0, 0], rotation: [0, 0, 1, 0.45], shapes: [{ type: "box", half: [8, 0.5, 4] }] }),
      body ("box", { position: [-3, 2.2, 0], rotation: [0, 0, 1, 0.45], shapes: [{ type: "box", half: [0.5, 0.4, 0.5] }] }),
   ],
   dts: dtsOf (240),
}));

scenes .push (scene ("s06b_cylinder_cone_hull", "A cylinder, a cone and a convex hull fall on the ground plane.", {
   collider: grippy,
   bodies: [
      ground (),
      body ("cyl", { position: [-2, 1.5, 0], rotation: [1, 0, 0, 0.3], shapes: [{ type: "cylinder", radius: 0.4, half_height: 0.6 }] }),
      body ("cone", { position: [0, 1.5, 0], rotation: [0, 0, 1, 0.4], shapes: [{ type: "cone", radius: 0.5, height: 1.0 }] }),
      body ("hull", { position: [2, 1.5, 0], rotation: [1, 1, 0, 0.5], shapes: [{ type: "hull", points: [[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0, 0.5, -0.5], [0, 0, 0.6], [0.4, 0.3, 0.2]] }] }),
   ],
   dts: dtsOf (240),
}));

// REA batch 06 (oracle/synthetic-rea-b06): the RigidBodyCollection scenes of X_ITE that x3d-drive diverged
// from, built at engine level: the exact CollidableShape shapes, the collider's bounce / friction, X_ITE's
// moving-average time steps (the baseline's physics.dt list).
const b06 = { friction: [0.5, 0], bounce: 0.3, bounce_enabled: true, min_bounce_speed: 0 };
const b06Floor = body ("F", { fixed: true, position: [0, 0, 0], shapes: [{ type: "box", half: [10, 0.5, 10] }] });

scenes .push (scene ("b06_shape_cylinder", "shape-cylinder.x3dv: a tilted cylinder (radius 0.5, height 1.5) falls on a box floor, bounce 0.3.", {
   collider: b06, feedback: true, xflow: true,
   bodies: [ b06Floor, body ("B", { position: [0, 3, 0], rotation: [1, 0, 0, 0.3], shapes: [{ type: "cylinder", radius: 0.5, half_height: 0.75 }] }) ],
   dts: xiteDts (188),
}));

scenes .push (scene ("b06_shape_cone", "shape-cone.x3dv: a tilted cone (bottomRadius 0.6, height 1.2) falls on a box floor, bounce 0.3.", {
   collider: b06, feedback: true, xflow: true,
   bodies: [ b06Floor, body ("B", { position: [0, 3, 0], rotation: [1, 0, 1, 0.5], shapes: [{ type: "cone", radius: 0.6, height: 1.2 }] }) ],
   dts: xiteDts (188),
}));

// The open Cone / Cylinder of the pinned X_ITE are concave: CollidableShape makes a btBvhTriangleMeshShape from the
// geometry's triangles (oracle/tools/ammo_dyn_meshes.json holds the arguments of its addTriangle calls).
const b06Meshes = JSON .parse (readFileSync (new URL ("./ammo_dyn_meshes.json", import .meta .url)));

for (const [ name, key, description ] of [
   [ "b06_shape_cone_open", "shape-cone-open", "shape-cone-open.x3dv: an open cone (bottom FALSE, a triangle mesh) falls on a box floor." ],
   [ "b06_shape_cylinder_open", "shape-cylinder-open", "shape-cylinder-open.x3dv: an open cylinder (top FALSE, a triangle mesh) falls on a box floor." ],
])
{
   scenes .push (scene (name, description, {
      collider: b06, feedback: true, xflow: true,
      bodies: [ b06Floor, body ("B", { position: [0, 3, 0], shapes: [{ type: "mesh", triangles: b06Meshes [key], rebuild: true }] }) ],
      dts: xiteDts (188),
   }));
}


scenes .push (scene ("b06_collider_friction_rolling", "collider-friction.x3dv R4: a sphere with spin on a box floor, friction 0.5 and rolling friction 0.5 (on both bodies).", {
   collider: { friction: [0.5, 0.5], bounce: 0, bounce_enabled: false, min_bounce_speed: 0 }, feedback: true, xflow: true,
   bodies: [
      body ("F4", { fixed: true, position: [120, 0, 0], shapes: [{ type: "box", half: [30, 0.5, 5] }] }),
      body ("B4", { position: [120, 1.02, 0], linear_velocity: [6, 0, 0], angular_velocity: [0, 0, -12], shapes: [{ type: "sphere", radius: 0.5 }] }),
   ],
   dts: xiteDts (188),
}));

scenes .push (scene ("s06c_cylinder_cone_hull_single_steps", "s06b with one internal step per stepSimulation call (preferAccuracy, iterations 1).", {
   collider: grippy, prefer_accuracy: true, iterations: 1,
   bodies: [
      ground (),
      body ("cyl", { position: [-2, 1.5, 0], rotation: [1, 0, 0, 0.3], shapes: [{ type: "cylinder", radius: 0.4, half_height: 0.6 }] }),
      body ("cone", { position: [0, 1.5, 0], rotation: [0, 0, 1, 0.4], shapes: [{ type: "cone", radius: 0.5, height: 1.0 }] }),
      body ("hull", { position: [2, 1.5, 0], rotation: [1, 1, 0, 0.5], shapes: [{ type: "hull", points: [[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0, 0.5, -0.5], [0, 0, 0.6], [0.4, 0.3, 0.2]] }] }),
   ],
   dts: dtsOf (240, 1 / 60),
}));

scenes .push (scene ("s03d_sphere_bounce_single_steps", "The bounce of s03 with one internal step per stepSimulation call (preferAccuracy, iterations 1).", {
   collider: bouncy, prefer_accuracy: true, iterations: 1,
   bodies: [ ground (), body ("ball", { position: [0, 2.5, 0], shapes: [{ type: "sphere", radius: 0.5 }] }) ],
   dts: dtsOf (200, 1 / 60),
}));

// --- joints, damping, sleeping, compound offsets -----------------------------------------------

const unit = a => { const l = Math .hypot (... a); return a .map (v => v / l); };
const sphereOf = r => [{ type: "sphere", radius: r }];

scenes .push (scene ("s07_ball_pendulum", "A ball joint pendulum: a sphere hangs from a fixed anchor and swings.", {
   bodies: [
      body ("anchor", { fixed: true, position: [0, 6, 0], shapes: [{ type: "box", half: [0.1, 0.1, 0.1] }] }),
      body ("bob", { position: [2.5, 6, 0], mass: 2, shapes: sphereOf (0.3) }),
   ],
   joints: [{ type: "ball", body1: "anchor", body2: "bob", pivot_a: [0, 0, 0], pivot_b: [-2.5, 0, 0] }],
   dts: dtsOf (300),
}));

scenes .push (scene ("s07b_ball_chain", "Three bodies on two ball joints with different masses, rotated bodies and initial velocity.", {
   bodies: [
      body ("anchor", { fixed: true, position: [0, 6, 0], shapes: [{ type: "box", half: [0.1, 0.1, 0.1] }] }),
      body ("b1", { position: [1.2, 6.3, 0.2], rotation: [0.2, 0.7, 0.4, 0.8], mass: 3, inertia: [0.6, 0, 0, 0, 0.9, 0, 0, 0, 0.5], linear_velocity: [0, 0.5, 1], shapes: [{ type: "box", half: [0.25, 0.15, 0.2] }] }),
      body ("b2", { position: [2.6, 6.1, -0.1], mass: 0.5, angular_velocity: [1, 2, -1], shapes: sphereOf (0.2) }),
   ],
   joints: [
      { type: "ball", body1: "anchor", body2: "b1", pivot_a: [0, 0, 0], pivot_b: [-1.2, -0.3, -0.2] },
      { type: "ball", body1: "b1", body2: "b2", pivot_a: [0.7, 0, 0.1], pivot_b: [-0.7, 0.2, 0] },
   ],
   dts: dtsOf (300),
}));

scenes .push (scene ("s08_hinge_pendulum", "A hinge pendulum: a bar hangs from a fixed anchor and swings about the z axis.", {
   bodies: [
      body ("anchor", { fixed: true, position: [0, 6, 0], shapes: [{ type: "box", half: [0.1, 0.1, 0.1] }] }),
      body ("bar", { position: [1.5, 6, 0], mass: 1, inertia: [0.1, 0, 0, 0, 0.8, 0, 0, 0, 0.8], shapes: [{ type: "box", half: [0.5, 0.1, 0.1] }] }),
   ],
   joints: [{ type: "hinge", body1: "anchor", body2: "bar", pivot_a: [0, 0, 0], pivot_b: [-1.5, 0, 0], axis_a: [0, 0, 1], axis_b: [0, 0, 1] }],
   dts: dtsOf (300),
}));

scenes .push (scene ("s08b_hinge_two_dynamic", "Two dynamic bodies on an oblique hinge axis, rotated frames, spinning start.", {
   bodies: [
      body ("a", { position: [0, 6, 0], rotation: [0, 1, 0, 0.5], mass: 2, shapes: [{ type: "box", half: [0.3, 0.3, 0.3] }], angular_velocity: [0.3, 0.1, 0] }),
      body ("b", { position: [1.4, 6.2, 0.1], rotation: [1, 0, 0, 0.3], mass: 1, shapes: [{ type: "box", half: [0.5, 0.1, 0.2] }] }),
   ],
   joints: [{ type: "hinge", body1: "a", body2: "b", pivot_a: [0.7, 0.1, 0], pivot_b: [-0.7, 0, 0], axis_a: unit ([0.3, 0.5, 0.8]), axis_b: unit ([0.2, 0.55, 0.8]) }],
   dts: dtsOf (300),
}));

scenes .push (scene ("s08c_hinge_antiparallel", "Hinge axes that are anti-parallel (shortest arc degenerate case) and parallel to a body axis.", {
   bodies: [
      body ("a", { fixed: true, position: [0, 5, 0], shapes: [{ type: "box", half: [0.1, 0.1, 0.1] }] }),
      body ("b", { position: [0, 4, 0], mass: 1, shapes: [{ type: "box", half: [0.1, 0.5, 0.1] }] }),
      body ("c", { position: [0, 3, 0], mass: 1, shapes: [{ type: "box", half: [0.1, 0.5, 0.1] }] }),
   ],
   joints: [
      { type: "hinge", body1: "a", body2: "b", pivot_a: [0, 0, 0], pivot_b: [0, 1, 0], axis_a: [1, 0, 0], axis_b: [-1, 0, 0] },
      { type: "hinge", body1: "b", body2: "c", pivot_a: [0, -0.5, 0], pivot_b: [0, 0.5, 0], axis_a: [0, 0, 1], axis_b: [0, 0, 1] },
   ],
   dts: dtsOf (240),
}));

scenes .push (scene ("s09_universal_joint_unconnected", "UniversalJoint makes no Ammo constraint in X_ITE: two bodies fall and spin independently.", {
   bodies: [
      body ("a", { position: [0, 5, 0], shapes: [{ type: "box", half: [0.3, 0.3, 0.3] }], angular_velocity: [1, 0, 0.5] }),
      body ("b", { position: [1, 5, 0], shapes: sphereOf (0.3), linear_velocity: [0, 1, 0] }),
   ],
   dts: dtsOf (120),
}));

/** SliderJoint.js: frameInA takes the (axis rotated) basis of body1 and the world position as origin; frameInB (a typo upstream) keeps an identity basis. */
const sliderFrames = (anchorPos, axis) =>
{
   const rot = new Rotation4 (); rot .setVectors (new Vector3 (1, 0, 0), new Vector3 (... axis));
   const origin = anchorPos;
   return { frame_a: m16 (origin, [... rot]), frame_b: m16 (origin, [0, 1, 0, 0]) };
};

scenes .push (scene ("s10_slider_limits", "A slider joint along x with linear limits 0 and 1.5; the body slides until it hits the upper limit.", {
   bodies: [
      body ("anchor", { fixed: true, position: [0, 3, 0], shapes: [{ type: "box", half: [0.1, 0.1, 0.1] }] }),
      body ("slide", { position: [0.5, 3, 0], mass: 1, linear_velocity: [3, 0, 0], shapes: [{ type: "box", half: [0.2, 0.2, 0.2] }] }),
   ],
   joints: [{ type: "slider", body1: "anchor", body2: "slide", ... sliderFrames ([0, 3, 0], [1, 0, 0]), lower: 0, upper: 1.5 }],
   dts: dtsOf (240),
}));

scenes .push (scene ("s10b_slider_two_dynamic", "A slider on a tilted axis between two dynamic bodies, limits -0.4 and 0.6, with a twist.", {
   bodies: [
      body ("a", { position: [0, 4, 0], mass: 2, shapes: [{ type: "box", half: [0.2, 0.2, 0.2] }] }),
      body ("b", { position: [0.4, 4.3, 0], mass: 1, angular_velocity: [0.5, 0, 0.2], linear_velocity: [0.5, 0, 0], shapes: [{ type: "box", half: [0.15, 0.15, 0.15] }] }),
   ],
   joints: [{ type: "slider", body1: "a", body2: "b", ... sliderFrames ([0, 4, 0], unit ([1, 0.5, 0])), lower: -0.4, upper: 0.6 }],
   dts: dtsOf (240),
}));

scenes .push (scene ("s11_damping", "Linear and angular damping (X_ITE autoDamp), with and without gravity.", {
   bodies: [
      body ("g0", { position: [0, 0, 0], use_global_gravity: false, auto_damp: true, linear_damping: 0.3, angular_damping: 0.2, linear_velocity: [2, 1, 0], angular_velocity: [1, 2, 3], shapes: [{ type: "box", half: [0.3, 0.2, 0.1] }], inertia: [0.4, 0, 0, 0, 0.7, 0, 0, 0, 0.9] }),
      body ("g1", { position: [3, 5, 0], auto_damp: true, linear_damping: 0.05, angular_damping: 0.5, angular_velocity: [0, 4, 0], shapes: sphereOf (0.4) }),
   ],
   dts: xiteDts (240),
}));

scenes .push (scene ("s12_sleep_wake", "A resting box falls asleep after two seconds (auto disable), a sphere dropped from 80 m wakes it.", {
   collider: grippy,
   bodies: [
      ground (),
      body ("rest", { position: [0, 0.505, 0], auto_disable: true, disable_linear_speed: 0.5, disable_angular_speed: 0.5, shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("drop", { position: [0.2, 80, 0.1], shapes: sphereOf (0.4) }),
   ],
   dts: dtsOf (420),
}));

scenes .push (scene ("s13_compound_offset", "A compound of a box and an offset sphere with a center of mass offset lands on the ground and tumbles.", {
   collider: bouncy,
   bodies: [
      ground (),
      body ("cmp", { position: [0, 2.5, 0], rotation: [0.3, 0.5, 0.8, 0.9], center_of_mass: [0.1, 0, 0], mass: 2,
         shapes: [{ type: "box", half: [0.4, 0.2, 0.3] }, { type: "sphere", radius: 0.3, offset: [0.7, 0.3, 0] }] }),
   ],
   dts: xiteDts (360),
}));

scenes .push (scene ("s13b_capsule", "A capsule falls on the ground and on a sphere.", {
   collider: grippy,
   bodies: [
      ground (),
      body ("ball", { fixed: true, position: [0, 0.3, 0], shapes: sphereOf (0.3) }),
      body ("cap", { position: [0.1, 2.5, 0], rotation: [0, 0, 1, 1.2], shapes: [{ type: "capsule", radius: 0.25, height: 0.8 }] }),
   ],
   dts: dtsOf (240),
}));

const rolly = { friction: [0.6, 0.4], bounce: 0, bounce_enabled: false, min_bounce_speed: 0 };

scenes .push (scene ("s14_rolling_friction_sphere", "A spinning sphere lands on the ground with rolling friction 0.4 (applied to both bodies, combined 0.16): rolling friction rows along the normal and two tangents.", {
   collider: rolly,
   bodies: [
      ground (),
      body ("ball", { position: [0, 1.2, 0], linear_velocity: [1, 0, 0.5], angular_velocity: [3, 1, -8], shapes: sphereOf (0.5) }),
   ],
   dts: dtsOf (240),
}));

scenes .push (scene ("s14b_rolling_friction_box", "A tumbling box on the ground with rolling friction 0.4, several contact points per manifold.", {
   collider: rolly,
   bodies: [
      ground (),
      body ("box", { position: [0, 1.0, 0], rotation: [1, 1, 0, 0.4], linear_velocity: [0.5, 0, 0.2], angular_velocity: [2, 1, -3], shapes: [{ type: "box", half: [0.5, 0.4, 0.3] }] }),
   ],
   dts: dtsOf (240),
}));

scenes .push (scene ("s14c_rolling_friction_single_axis", "A fast spinning sphere (rolling friction along the relative angular velocity, the single axis branch needs a finite threshold: not reached) with rolling friction 5.", {
   collider: { friction: [0.5, 5], bounce: 0, bounce_enabled: false, min_bounce_speed: 0 },
   bodies: [
      ground (),
      body ("ball", { position: [0, 0.52, 0], linear_velocity: [4, 0, 0], angular_velocity: [0, 0, -20], shapes: sphereOf (0.5) }),
   ],
   dts: dtsOf (200),
}));

scenes .push (scene ("s12b_box_kicked_awake", "A box resting on the ground is kicked (initial velocity of the s12 impact), no sleeping.", {
   collider: grippy,
   bodies: [
      ground (),
      body ("rest", { position: [0, 0.505, 0], linear_velocity: [-0.781, 0.7586, 0.2117], angular_velocity: [0.169, -0.051, 1.321], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
   ],
   dts: dtsOf (200),
}));

scenes .push (scene ("s12c_box_sleeps_then_kicked", "A box falls asleep, a sphere dropped from 8 m wakes it (shallower impact than s12).", {
   collider: grippy,
   bodies: [
      ground (),
      body ("rest", { position: [0, 0.505, 0], auto_disable: true, disable_linear_speed: 0.5, disable_angular_speed: 0.5, shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("drop", { position: [0.2, 12, 0.1], shapes: sphereOf (0.4) }),
   ],
   dts: dtsOf (420),
}));

scenes .push (scene ("s12d_sleep_wake_single_steps", "s12 with one internal step per stepSimulation call (preferAccuracy, iterations 1), so a divergence names the exact internal step.", {
   collider: grippy, prefer_accuracy: true, iterations: 1,
   bodies: [
      ground (),
      body ("rest", { position: [0, 0.505, 0], auto_disable: true, disable_linear_speed: 0.5, disable_angular_speed: 0.5, shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("drop", { position: [0.2, 80, 0.1], shapes: sphereOf (0.4) }),
   ],
   dts: dtsOf (420),
}));

scenes .push (scene ("s12e_awake_box_fast_sphere", "A box that never sleeps is hit by a sphere dropped from 80 m, substepped.", {
   collider: grippy,
   bodies: [
      ground (),
      body ("rest", { position: [0, 0.505, 0], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("drop", { position: [0.2, 80, 0.1], shapes: sphereOf (0.4) }),
   ],
   dts: dtsOf (300),
}));

scenes .push (scene ("s12f_sleeping_box_fast_sphere_8m", "The s12 box sleeps, the sphere falls from 30 m.", {
   collider: grippy,
   bodies: [
      ground (),
      body ("rest", { position: [0, 0.505, 0], auto_disable: true, disable_linear_speed: 0.5, disable_angular_speed: 0.5, shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("drop", { position: [0.2, 30, 0.1], shapes: sphereOf (0.4) }),
   ],
   dts: dtsOf (300),
}));

scenes .push (scene ("s12g_fast_sphere_hits_box_substeps", "A sphere thrown down at 39 m/s hits a resting box; 1/60 frames with ten substeps each.", {
   collider: grippy,
   bodies: [
      ground (),
      body ("rest", { position: [0, 0.505, 0], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("drop", { position: [0.2, 3, 0.1], linear_velocity: [0, -39, 0], shapes: sphereOf (0.4) }),
   ],
   dts: dtsOf (60),
}));

scenes .push (scene ("s12h_fast_sphere_hits_box_single_steps", "s12g with every substep a stepSimulation call of 1/600 (preferAccuracy, iterations 1).", {
   collider: grippy, prefer_accuracy: true, iterations: 1,
   bodies: [
      ground (),
      body ("rest", { position: [0, 0.505, 0], shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("drop", { position: [0.2, 3, 0.1], linear_velocity: [0, -39, 0], shapes: sphereOf (0.4) }),
   ],
   dts: dtsOf (600, 1 / 600),
}));

scenes .push (scene ("s12i_sleep_wake_fine_steps", "s12 with every substep a stepSimulation call of 1/600 (preferAccuracy, iterations 1).", {
   collider: grippy, prefer_accuracy: true, iterations: 1,
   bodies: [
      ground (),
      body ("rest", { position: [0, 0.505, 0], auto_disable: true, disable_linear_speed: 0.5, disable_angular_speed: 0.5, shapes: [{ type: "box", half: [0.5, 0.5, 0.5] }] }),
      body ("drop", { position: [0.2, 80, 0.1], shapes: sphereOf (0.4) }),
   ],
   dts: dtsOf (2700, 1 / 600),
}));

// --- narrow phase pair corpus --------------------------------------------------------------

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

const pairCases = (name, typeA, typeB, count, seed, mesh) =>
{
   const r = rng (seed), cases = [ ];
   for (let i = 0; i < count; ++ i)
   {
      const sa = randomShape (r, typeA), sb = mesh ? { type: "mesh", triangles: meshGround (4, 1.2) } : randomShape (r, typeB);
      const d = [r () - 0.5, r () - 0.5, r () - 0.5], dl = Math .hypot (... d) || 1;
      const reach = mesh ? 0.2 + 0.9 * r () * extent (sa) : (extent (sa) + extent (sb)) * (0.25 + 0.85 * r ());
      const pa = [r () - 0.5, r () - 0.5, r () - 0.5];
      const pb = mesh ? [(r () - 0.5) * 3, reach, (r () - 0.5) * 3] : pa .map ((v, k) => v + d [k] / dl * reach);
      cases .push (scene (`${name}_${i}`, "", {
         gravity: [0, 0, 0], detect: true,
         bodies: [
            body ("a", { position: pa, rotation: randomRotation (r), shapes: [sa] }),
            mesh ? body ("b", { fixed: true, position: [0, 0, 0], rotation: [0, 1, 0, 0], shapes: [sb] })
                 : body ("b", { position: pb, rotation: randomRotation (r), shapes: [sb] }),
         ],
         ... (mesh ? { bodies: [
            body ("a", { position: pb, rotation: randomRotation (r), shapes: [sa] }),
            body ("b", { fixed: true, position: [0, 0, 0], rotation: [0, 1, 0, 0], shapes: [sb] }) ] } : { }),
      }));
   }
   return cases;
};

/** One body dropped on the ground plane with a random pose and velocity: contacts, friction and the solver. */
const solveCases = (name, type, count, seed) =>
{
   const r = rng (seed), cases = [ ];
   for (let i = 0; i < count; ++ i)
   {
      const sh = randomShape (r, type), e = extent (sh), u = (a, b) => a + (b - a) * r ();
      cases .push (scene (`${name}_${i}`, "", {
         collider: { friction: [u (0.1, 0.9), 0], bounce: 0.3, bounce_enabled: r () < 0.5, min_bounce_speed: 0.1 },
         bodies: [
            ground (),
            body ("a", {
               position: [u (-1, 1), e * u (0.8, 1.3), u (-1, 1)], rotation: randomRotation (r), shapes: [sh],
               linear_velocity: [u (-2, 2), u (-3, 0), u (-2, 2)], angular_velocity: [u (-3, 3), u (-3, 3), u (-3, 3)],
               mass: u (0.5, 3), inertia: [u (0.2, 2), 0, 0, 0, u (0.2, 2), 0, 0, 0, u (0.2, 2)],
            }),
         ],
         prefer_accuracy: true, iterations: 1, dts: dtsOf (12, 1 / 60),
      }));
   }
   return cases;
};

/** A small box lying nearly flat on a big fixed box: the face-face clipping path with small tilts. */
const flatBoxCases = (name, count, seed, aligned, eps = 0.03) =>
{
   const r = rng (seed), cases = [ ], u = (a, b) => a + (b - a) * r ();
   for (let i = 0; i < count; ++ i)
   {
      const half = [u (0.2, 0.7), u (0.2, 0.7), u (0.2, 0.7)];
      let ax = unit ([r () - 0.5, r () - 0.5, r () - 0.5]);
      let tilt = r () < 0.3 ? 0 : u (-0.3, 0.3);
      let yaw = u (0, Math .PI * 2);
      // aligned: the yaw is a multiple of a quarter turn and the tilt is about x or z only
      if (aligned) { yaw = Math .floor (r () * 4) * Math .PI / 2; ax = r () < 0.5 ? [1, 0, 0] : [0, 0, 1]; tilt = u (-0.15, 0.15) * (r () < 0.2 ? 0.01 : 1); }
      // yaw about y composed with a tilt about a random horizontal axis, as one axis-angle through the matrices
      const rot = [... new Rotation4 (0, 1, 0, yaw) .multRight (new Rotation4 (ax [0], 0, ax [2], tilt))];
      const m = m16 ([0, 0, 0], rot);
      const support = half .reduce ((sum, h, k) => sum + Math .abs (m [1 + 4 * k]) * h, 0);
      cases .push (scene (`${name}_${i}`, "", {
         gravity: [0, 0, 0], detect: true,
         bodies: [
            body ("a", { fixed: true, position: [0, -0.5, 0], shapes: [{ type: "box", half: [20, 0.5, 20] }] }),
            body ("b", { position: [u (-2, 2), support + u (-eps, eps), u (-2, 2)], rotation: rot, shapes: [{ type: "box", half }] }),
         ],
      }));
   }
   return cases;
};

const pairKinds = [ ];
{
   const convex = ["sphere", "box", "cylinder", "cone", "hull"];
   let seed = 1;
   for (let i = 0; i < convex .length; ++ i) for (let j = i; j < convex .length; ++ j)
      pairKinds .push ([`pairs_${convex [i]}_${convex [j]}`, convex [i], convex [j], false, seed ++]);
}

const outDir = resolve (outArg);
await mkdir (outDir, { recursive: true });

for (const [name, a, b, mesh, seed] of pairKinds)
{
   if (only .length && !only .includes (name) && !only .includes ("pairs")) continue;
   Ammo = new AmmoFactory ();
   const cases = pairCases (name, a, b, 40, seed * 7919, mesh) .map (sc => { const { hooks, ... rest } = sc; return { scene: rest, ... runScene (sc) }; });
   await writeFile (join (outDir, `${name}.json`), JSON .stringify ({ cases }));
   console .log (`${name}: ${cases .length} cases`);
}

for (const [name, aligned, seed, eps] of [["pairs_box_flat", false, 90210, 0.03], ["pairs_box_aligned", true, 31337, 0.03], ["pairs_box_touching", false, 777, 4e-7], ["pairs_box_touching_aligned", true, 4242, 4e-7]])
{
   if (!only .length || only .includes (name) || only .includes ("pairs"))
   {
      Ammo = new AmmoFactory ();
      const cases = flatBoxCases (name, 400, seed, aligned, eps) .map (sc => { const { hooks, ... rest } = sc; return { scene: rest, ... runScene (sc) }; });
      await writeFile (join (outDir, `${name}.json`), JSON .stringify ({ cases }));
      console .log (`${name}: ${cases .length} cases`);
   }
}

for (const type of ["sphere", "box", "cylinder", "cone", "hull"])
{
   const name = `solve_${type}`;
   if (only .length && !only .includes (name) && !only .includes ("solve")) continue;
   Ammo = new AmmoFactory ();
   const cases = solveCases (name, type, 60, 424242 + type .length) .map (sc => { const { hooks, ... rest } = sc; return { scene: rest, ... runScene (sc) }; });
   await writeFile (join (outDir, `${name}.json`), JSON .stringify ({ cases }));
   console .log (`${name}: ${cases .length} cases`);
}

for (const s of scenes)
{
   if (only .length && !only .includes (s .name)) continue;
   Ammo = new AmmoFactory ();
   const { hooks, ... rest } = s;
   // AMMO_LOG=file: every Ammo call that matters, one JSON line each (compare with a log of the real X_ITE run).
   const callLog = [ ];
   if (process .env .AMMO_LOG && only .includes (s .name)) (await import (process .env .AMMO_WRAP)) .wrapAmmo (Ammo, callLog);
   const result = runScene (s);
   if (process .env .AMMO_LOG && only .includes (s .name)) await writeFile (process .env .AMMO_LOG, callLog .join ("\n"));
   await writeFile (join (outDir, `${s .name}.json`), JSON .stringify ({ scene: rest, ... result }));
   console .log (`${s .name}: ${result .steps .length} steps`);
}
