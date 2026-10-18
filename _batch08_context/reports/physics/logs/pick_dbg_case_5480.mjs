/**
 * REA batch 03: contact test vectors from the pinned X_ITE's Ammo (Bullet, asm.js), built the way
 * Browser/Picking/VolumePicker.js and Components/RigidBodyPhysics/CollidableShape.js build them
 * (`convex TRUE`, compound shapes, matrix decomposition with X_ITE's Matrix4.getTransform).
 *
 *   node oracle/tools/ammo_pick_vectors.mjs oracle/pick-vectors/ammo-contacts.json [count] [seed]
 *
 * Each case: shape `a` (a point of PointPickSensor or a pick shape) and shape `b`, each with its X_ITE
 * model matrix, and Ammo's `contactTest` result. crates/x3d-viewer/tests/rea_b03_pick_vectors.rs
 * runs the Rust contact test on the same cases. Every case is synthetic (seeded random numbers).
 */
import { writeFile, mkdir } from "node:fs/promises";
import { dirname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SRC = "/home/ryan/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup/oracle/x_ite-pin/src";
const { default: AmmoFactory } = await import (process.env.AMMO_JS ?? join (SRC, "lib/ammojs/ammo.js"));
globalThis.__dbg = (...a) => console.log ("DBG", a.map (v => typeof v === "number" ? (Number.isInteger (v) ? String (v) : String (a[0] === "Jg" ? v : Math.fround (v))) : String (v)).join (" "));
const { default: Matrix4 } = await import (join (SRC, "standard/Math/Numbers/Matrix4.js"));
const { default: Rotation4 } = await import (join (SRC, "standard/Math/Numbers/Rotation4.js"));
const { default: Vector3 } = await import (join (SRC, "standard/Math/Numbers/Vector3.js"));
const Ammo = new AmmoFactory ();

const [ outArg, countArg = "4000", seedArg = "20261017" ] = process .argv .slice (2);

// mulberry32
let seed = Number (seedArg) >>> 0;
const rand = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math .imul (t ^ (t >>> 15), t | 1); t ^= t + Math .imul (t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const range = (a, b) => a + (b - a) * rand ();
const f32 = x => Math .fround (x);

// --- VolumePicker ---------------------------------------------------------------------------
const broadphase = new Ammo .btDbvtBroadphase ();
const config     = new Ammo .btDefaultCollisionConfiguration ();
const dispatcher = new Ammo .btCollisionDispatcher (config);
const world      = new Ammo .btCollisionWorld (dispatcher, broadphase, config);
const compound1  = new Ammo .btCompoundShape ();
const compound2  = new Ammo .btCompoundShape ();
const body = c => new Ammo .btRigidBody (new Ammo .btRigidBodyConstructionInfo (0, new Ammo .btDefaultMotionState (), c));
const body1 = body (compound1), body2 = body (compound2);
world .addCollisionObject (body1);
world .addCollisionObject (body2);
// The compounds' local AABBs (btRigidBody.getAabb at the identity) for AMMO_TRACE.
const aabbs = () => [body1, body2] .map (b => { const lo = new Ammo .btVector3 (), hi = new Ammo .btVector3 (); b .getAabb (lo, hi); return [lo .x (), lo .y (), lo .z (), hi .x (), hi .y (), hi .z ()] .map (v => +v .toFixed (5)); });

const getTransform = (translation, rotation) =>
{
   const m = new Matrix4 (); m .setTransform (translation, rotation);
   const t = new Ammo .btTransform ();
   t .getBasis () .setValue (m [0], m [4], m [8], m [1], m [5], m [9], m [2], m [6], m [10]);
   t .setOrigin (new Ammo .btVector3 (m [12], m [13], m [14]));
   return t;
};

const setChildShape = (compound, matrix, childShape) =>
{
   if (compound .getNumChildShapes ()) compound .removeChildShapeByIndex (0);
   if (!childShape .getNumChildShapes ()) return;
   const translation = new Vector3 (), rotation = new Rotation4 (), scale = new Vector3 ();
   matrix .getTransform (translation, rotation, scale);
   childShape .setLocalScaling (new Ammo .btVector3 (scale .x, scale .y, scale .z));
   compound .addChildShape (getTransform (translation, rotation), childShape);
};

const setChildShapeComponents = (compound, transform, localScaling, childShape) =>
{
   if (compound .getNumChildShapes ()) compound .removeChildShapeByIndex (0);
   if (!childShape .getNumChildShapes ()) return;
   childShape .setLocalScaling (localScaling);
   compound .addChildShape (transform, childShape);
};

const contactTest = () =>
{
   world .performDiscreteCollisionDetection ();
   const n = dispatcher .getNumManifolds ();
   for (let i = 0; i < n; ++ i)
   {
      const m = dispatcher .getManifoldByIndexInternal (i);
      for (let j = 0, c = m .getNumContacts (); j < c; ++ j)
         if (m .getContactPoint (j) .getDistance () <= 0) return true;
   }
   return false;
};

// --- CollidableShape (convex TRUE) ----------------------------------------------------------
const identity = () => { const t = new Ammo .btTransform (); t .setIdentity (); return t; };

function pickShape (s)
{
   const compound = new Ammo .btCompoundShape ();
   let shape = null, offset = [0, 0, 0];

   switch (s .kind)
   {
      case "box": shape = new Ammo .btBoxShape (new Ammo .btVector3 (s .size [0] / 2, s .size [1] / 2, s .size [2] / 2)); break;
      case "sphere": shape = new Ammo .btSphereShape (s .r); break;
      case "cone": shape = new Ammo .btConeShape (s .r, s .h); break;
      case "cylinder": shape = new Ammo .btCylinderShape (new Ammo .btVector3 (s .r, s .h * 0.5, s .r)); break;
      case "hull":
      {
         shape = new Ammo .btConvexHullShape ();
         for (let i = 0; i < s .points .length; i += 3)
            shape .addPoint (new Ammo .btVector3 (s .points [i], s .points [i + 1], s .points [i + 2]), false);
         shape .recalcLocalAabb ();
         break;
      }
      case "mesh":
      {
         const mesh = new Ammo .btTriangleMesh ();
         const p = (i) => new Ammo .btVector3 (s .tris [i], s .tris [i + 1], s .tris [i + 2]);
         for (let i = 0; i < s .tris .length; i += 9) mesh .addTriangle (p (i), p (i + 3), p (i + 6));
         shape = new Ammo .btBvhTriangleMeshShape (mesh, false);
         break;
      }
      case "height":
      {
         const { xd, zd, xs, zs, heights } = s;
         const ptr = Ammo ._malloc (4 * xd * zd);
         let min = Infinity, max = -Infinity;
         heights .forEach ((h, i) => { min = Math .min (min, h); max = Math .max (max, h); Ammo .HEAPF32 [(ptr + i * 4) >>> 2] = h; });
         shape = new Ammo .btHeightfieldTerrainShape (xd, zd, ptr, 1, min, max, 1, "PHY_FLOAT", true);
         shape .setLocalScaling (new Ammo .btVector3 (xs, 1, zs));
         offset = [xs * (xd - 1) * 0.5, (min + max) * 0.5, zs * (zd - 1) * 0.5];
         break;
      }
   }

   const t = identity ();
   t .setOrigin (new Ammo .btVector3 (... offset));
   compound .addChildShape (t, shape);
   return compound;
}

function pointShape (p)
{
   const compound = new Ammo .btCompoundShape ();
   const t = identity ();
   t .setOrigin (new Ammo .btVector3 (... p));
   compound .addChildShape (t, new Ammo .btSphereShape (0));
   return compound;
}

// --- random cases ---------------------------------------------------------------------------
const KINDS = ["box", "sphere", "cone", "cylinder", "hull", "mesh", "height"];

function randomShape (kind)
{
   switch (kind)
   {
      case "box": return { kind, size: [f32 (range (0.3, 2.5)), f32 (range (0.3, 2.5)), f32 (range (0.3, 2.5))] };
      case "sphere": return { kind, r: f32 (range (0.2, 1.3)) };
      case "cone": return { kind, r: f32 (range (0.3, 1.3)), h: f32 (range (0.4, 2.5)) };
      case "cylinder": return { kind, r: f32 (range (0.2, 1.3)), h: f32 (range (0.3, 2.5)) };
      case "hull":
      {
         const n = 4 + Math .floor (rand () * 14), points = [];
         for (let i = 0; i < n * 3; ++ i) points .push (f32 (range (-1, 1)));
         if (rand () < 0.2) for (let i = 1; i < points .length; i += 3) points [i] = 0; // flat
         return { kind, points };
      }
      case "mesh":
      {
         const n = 1 + Math .floor (rand () * 10), tris = [];
         for (let i = 0; i < n * 9; ++ i) tris .push (f32 (range (-1.2, 1.2)));
         return { kind, tris };
      }
      case "height":
      {
         const xd = 2 + Math .floor (rand () * 4), zd = 2 + Math .floor (rand () * 4), heights = [];
         for (let i = 0; i < xd * zd; ++ i) heights .push (f32 (range (-0.5, 0.8)));
         return { kind, xd, zd, xs: f32 (range (0.3, 1)), zs: f32 (range (0.3, 1)), heights };
      }
   }
}

function randomMatrix (center, spread)
{
   const m = new Matrix4 ();
   const t = new Vector3 (f32 (center [0] + range (-spread, spread)), f32 (center [1] + range (-spread, spread)), f32 (center [2] + range (-spread, spread)));
   const r = rand () < 0.25 ? new Rotation4 () : new Rotation4 (range (-1, 1), range (-1, 1), range (-1, 1), range (-3, 3));
   const u = rand () < 0.4;
   const s = u ? new Vector3 (1, 1, 1) : new Vector3 (f32 (range (0.6, 1.8)), f32 (range (0.6, 1.8)), f32 (range (0.6, 1.8)));
   m .setTransform (t, r, s);
   return m;
}

const cases = [];
const count = Number (countArg);

// Debug: AMMO_POSE='{"a": shape, "b": shape, "ta": [[t], [r]], "tb": [[t], [r], [s]]}' prints the
// contact distances of one pose (fresh picker), built with Matrix4.setTransform.
if (process .env .AMMO_POSE)
{
   const c = JSON .parse (process .env .AMMO_POSE);
   const mat = ([t, r, s = [1, 1, 1]]) => { const m = new Matrix4 (); m .setTransform (new Vector3 (... t), new Rotation4 (... r), new Vector3 (... s)); return m; };
   setChildShape (compound1, mat (c .ta), pickShape (c .a));
   setChildShape (compound2, mat (c .tb), pickShape (c .b));
   world .performDiscreteCollisionDetection ();
   const d = [];
   for (let k = 0, n = dispatcher .getNumManifolds (); k < n; ++ k)
   {
      const m = dispatcher .getManifoldByIndexInternal (k);
      for (let j = 0; j < m .getNumContacts (); ++ j) d .push (m .getContactPoint (j) .getDistance ());
   }
   console .log ("pose distances", d .map (x => x .toExponential (4)) .join (" "), "matrix a", Array .from (mat (c .ta)) .join (" "));
   process .exit (0);
}


if (process .env .AMMO_DIRECT)
{
   const data = JSON .parse (await (await import ("node:fs/promises")) .readFile (process .env .AMMO_DIRECT .split (":") [0], "utf8"));
   const c = data .cases [+process .env .AMMO_DIRECT .split (":") [1]];
   const mk = (s, m, which = "a") =>
   {
      const mode = (which === "a" ? process .env .AMMO_MODEA : process .env .AMMO_MODEB) ?? process .env .AMMO_MODE ?? "inner";
      const so = which === "a" ? process .env .AMMO_SA : process .env .AMMO_SB;
      const translation = new Vector3 (), rotation = new Rotation4 (), scale = new Vector3 ();
      new Matrix4 (... m) .getTransform (translation, rotation, scale);
      if (so) { const v = so .split (",") .map (Number); scale .x = v [0]; scale .y = v [1]; scale .z = v [2]; }
      let shape = pickShape (s);
      if (mode === "bare") { /* the cylinder itself */ shape = new Ammo .btCylinderShape (new Ammo .btVector3 (s .r, s .h * 0.5, s .r)); }
      if (mode === "prescaled")
      {
         // scale the cylinder itself, then wrap it in a compound without scaling the compound
         const cyl = new Ammo .btCylinderShape (new Ammo .btVector3 (s .r, s .h * 0.5, s .r));
         cyl .setLocalScaling (new Ammo .btVector3 (scale .x, scale .y, scale .z));
         const t = new Ammo .btTransform (); t .setIdentity ();
         shape = new Ammo .btCompoundShape (); shape .addChildShape (t, cyl);
      }
      else if (mode === "noscale") { }
      else if (mode !== "bare0") shape .setLocalScaling (new Ammo .btVector3 (scale .x, scale .y, scale .z));
      else shape .setLocalScaling (new Ammo .btVector3 (scale .x, scale .y, scale .z));
      if (process .env .AMMO_SCALELOG) { const lo = new Ammo .btVector3 (), hi = new Ammo .btVector3 (); const t = new Ammo .btTransform (); t .setIdentity (); shape .getAabb (t, lo, hi); console .log ("aabb local", [lo .x (), lo .y (), lo .z (), hi .x (), hi .y (), hi .z ()] .map (v => v .toFixed (4)) .join (" "), "scale", scale .x, scale .y, scale .z); }
      const ms = new Ammo .btDefaultMotionState (getTransform (translation, rotation));
      const rb = new Ammo .btRigidBody (new Ammo .btRigidBodyConstructionInfo (0, ms, shape));
      return rb;
   };
   const bd = new Ammo .btDbvtBroadphase (), cfg = new Ammo .btDefaultCollisionConfiguration (), disp = new Ammo .btCollisionDispatcher (cfg);
   const w = new Ammo .btCollisionWorld (disp, bd, cfg);
   const b1 = mk (c .a, c .ma, "a"), b2 = mk (c .b, c .mb, "b");
   w .addCollisionObject (b1); w .addCollisionObject (b2);
   w .performDiscreteCollisionDetection ();
   const d = [];
   for (let k = 0, n = disp .getNumManifolds (); k < n; ++ k) { const m = disp .getManifoldByIndexInternal (k); for (let j = 0; j < m .getNumContacts (); ++ j) d .push (m .getContactPoint (j) .getDistance ()); }
   console .log ("direct", "manifolds", disp .getNumManifolds (), "distances", d .join (" "));
   process .exit (0);
}

// Debug: AMMO_DEBUG=<json file>:<i,j,...> prints every manifold point distance of those cases.
if (process .env .AMMO_DEBUG)
{
   const [file, list] = process .env .AMMO_DEBUG .split (":");
   const data = JSON .parse (await (await import ("node:fs/promises")) .readFile (file, "utf8"));
   for (const i of list .split (",") .map (Number))
   {
      const c = data .cases [i];
      const ma = new Matrix4 (... c .ma), mb = new Matrix4 (... c .mb);
      if (process .env .AMMO_GROW)
      {
         // Grow both compounds' local AABBs (removeChildShapeByIndex keeps them) first.
         const g = Number (process .env .AMMO_GROW), big = { kind: "box", size: [g, g, g] };
         setChildShape (compound1, new Matrix4 (), pickShape (big));
         setChildShape (compound2, new Matrix4 (), pickShape (big));
         world .performDiscreteCollisionDetection ();
      }
      if (c .a .kind === "point")
      {
         const translation = new Vector3 (), rotation = new Rotation4 (), scale = new Vector3 ();
         ma .getTransform (translation, rotation, scale);
         setChildShapeComponents (compound1, getTransform (translation, rotation), new Ammo .btVector3 (scale .x, scale .y, scale .z), pointShape (c .a .p));
      }
      else setChildShape (compound1, ma, pickShape (c .a));
      setChildShape (compound2, mb, pickShape (c .b));
      world .performDiscreteCollisionDetection ();
      const d = [];
      for (let k = 0, n = dispatcher .getNumManifolds (); k < n; ++ k)
      {
         const m = dispatcher .getManifoldByIndexInternal (k);
         for (let j = 0; j < m .getNumContacts (); ++ j) d .push (m .getContactPoint (j) .getDistance ());
      }
      console .log (i, c .a .kind, c .b .kind, "hit", c .hit, "distances", d .map (x => x .toExponential (3)) .join (" "), "manifolds", dispatcher .getNumManifolds (), "aabbs", JSON .stringify (aabbs ()));
   }
   process .exit (0);
}

for (let i = 0; i < count; ++ i)
{
   const point = rand () < 0.35;
   const a = point ? { kind: "point", p: [f32 (range (-1, 1)), f32 (range (-1, 1)), f32 (range (-1, 1))] } : randomShape (KINDS [Math .floor (rand () * KINDS .length)]);
   const b = randomShape (KINDS [Math .floor (rand () * KINDS .length)]);
   const ma = randomMatrix ([0, 0, 0], 0.2);
   const mb = randomMatrix ([0, 0, 0], point ? 1.2 : 2.2);

   if (point)
   {
      const translation = new Vector3 (), rotation = new Rotation4 (), scale = new Vector3 ();
      ma .getTransform (translation, rotation, scale);
      setChildShapeComponents (compound1, getTransform (translation, rotation), new Ammo .btVector3 (scale .x, scale .y, scale .z), pointShape (a .p));
   }
   else
   {
      setChildShape (compound1, ma, pickShape (a));
   }

   setChildShape (compound2, mb, pickShape (b));
   const hit = contactTest ();
   if (process .env .AMMO_TRACE && process .env .AMMO_TRACE .split (",") .includes (String (i)))
   {
      const d = [];
      for (let k = 0, n = dispatcher .getNumManifolds (); k < n; ++ k)
      {
         const m = dispatcher .getManifoldByIndexInternal (k);
         for (let j = 0; j < m .getNumContacts (); ++ j) d .push (m .getContactPoint (j) .getDistance ());
      }
      console .log ("trace", i, a .kind, b .kind, hit, dispatcher .getNumManifolds (), d .map (x => x .toExponential (3)) .join (" "), JSON .stringify (aabbs ()));
   }
   cases .push ({ a, b, ma: Array .from (ma), mb: Array .from (mb), hit });
}

await mkdir (dirname (resolve (outArg)), { recursive: true });
await writeFile (resolve (outArg), JSON .stringify ({ seed: seedArg, count, cases }));
const hits = cases .filter (c => c .hit) .length;
console .log (`${count} cases, ${hits} contacts`);
