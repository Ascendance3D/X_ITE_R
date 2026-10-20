// sub:fields.math: the pinned X_ITE's standard/Math/Geometry classes (imported unchanged from oracle/x_ite-pin/src) on a fixed
// battery of inputs. Every number, input and output, is written as the 16 hex digits of its IEEE 754 double, so the Rust test
// (crates/x3d-viewer/tests/geom_math.rs) reads the same inputs and compares the outputs bit for bit.
//
//   node oracle/tools/geom_xite.mjs > oracle/geom-math-baseline.json
//
// Inputs: degenerate cases written out, then seeded pseudo-random ones (mulberry32, seed 0x5eed), so a run repeats exactly.

// Triangle3.js builds a libtess tesselator when it loads (for triangulatePolygon, which is not compared here): a stand-in that
// is never called.
globalThis .libtess = { GluTesselator: class { gluTessCallback () { } gluTessProperty () { } }, gluEnum: { }, windingRule: { } };

const src = new URL ("../x_ite-pin/src/standard/Math/", import.meta.url);
const load = async p => (await import (new URL (p, src))) .default;

const Vector2    = await load ("Numbers/Vector2.js");
const Vector3    = await load ("Numbers/Vector3.js");
const Vector4    = await load ("Numbers/Vector4.js");
const Rotation4  = await load ("Numbers/Rotation4.js");
const Matrix3    = await load ("Numbers/Matrix3.js");
const Matrix4    = await load ("Numbers/Matrix4.js");
const Line2      = await load ("Geometry/Line2.js");
const Line3      = await load ("Geometry/Line3.js");
const Plane3     = await load ("Geometry/Plane3.js");
const Sphere3    = await load ("Geometry/Sphere3.js");
const Cylinder3  = await load ("Geometry/Cylinder3.js");
const Triangle3  = await load ("Geometry/Triangle3.js");
const ViewVolume = await load ("Geometry/ViewVolume.js");
const Camera     = await load ("Geometry/Camera.js");
const Box3       = await load ("Geometry/Box3.js");

const view = new DataView (new ArrayBuffer (8));
const hex = x => { view .setFloat64 (0, Number (x)); return view .getBigUint64 (0) .toString (16) .padStart (16, "0"); };

let seed = 0x5eed;
function rnd () { seed |= 0; seed = seed + 0x6D2B79F5 | 0; let t = Math .imul (seed ^ seed >>> 15, 1 | seed); t = t + Math .imul (t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }
const r = (a = -5, b = 5) => a + (b - a) * rnd ();
const v2 = () => new Vector2 (r (), r ());
const v3 = () => new Vector3 (r (), r (), r ());
const m4 = () => { const m = new Matrix4 (); m .set (v3 (), new Rotation4 (v3 () .normalize (), r (-3, 3)), new Vector3 (r (0.2, 3), r (0.2, 3), r (0.2, 3))); return m; };
const m3 = () => { const m = new Matrix3 (); m .set (v2 (), r (-3, 3), new Vector2 (r (0.2, 3), r (0.2, 3))); return m; };

const flat = xs => xs .flatMap (x => typeof x === "number" || typeof x === "boolean" ? [Number (x)] : Array .from (x));
const cases = [];
const add = (op, input, output) => cases .push ({ op, in: flat (input) .map (hex), out: flat (output) .map (hex) });

// Line3
function line3 (a, b, q, c, d, A, B, C, m)
{
   const l = Line3 .fromPoints (a, b);
   add ("line3.points", [a, b, q], [l .point, l .direction, l .getClosestPointToPoint (q), l .getPerpendicularVectorToPoint (q)]);

   const k = Line3 .fromPoints (c, d), p = new Vector3 ();
   const hit = l .getClosestPointToLine (k, p);
   add ("line3.line", [a, b, c, d], [hit, p, l .getPerpendicularVectorToLine (k)]);

   const uvt = { u: 0, v: 0, t: 0 }, B1 = B .copy (), C1 = C .copy ();
   const tri = l .intersectsTriangle (A, B1, C1, uvt);
   add ("line3.triangle", [a, b, A, B, C], [tri, uvt .u, uvt .v, uvt .t, B1, C1]);

   const l1 = l .copy () .multMatrixLine (m), l2 = l .copy () .multLineMatrix (m);
   add ("line3.matrix", [a, b, m], [l1 .point, l1 .direction, l2 .point, l2 .direction]);
}

// Line2
function line2 (a, b, q, c, d, m)
{
   const l = Line2 .fromPoints (a, b), k = Line2 .fromPoints (c, d), p = new Vector2 ();
   const hit = l .intersectsLine (k, p);
   const l1 = l .copy () .multMatrixLine (m), l2 = l .copy () .multLineMatrix (m);
   add ("line2", [a, b, q, c, d, m], [l .point, l .direction, l .getClosestPointToPoint (q), l .getPerpendicularVectorToPoint (q), hit, p,
      l1 .point, l1 .direction, l2 .point, l2 .direction]);
}

// Plane3
function plane3 (p, n, q, a, b, m)
{
   const s = new Plane3 (p, n), x = new Vector3 ();
   const hit = s .intersectsLine (Line3 .fromPoints (a, b), x);
   const s1 = s .copy () .multRight (m), s2 = s .copy () .multLeft (m);
   add ("plane3", [p, n, q, a, b, m], [s .normal, s .distanceFromOrigin, s .getDistanceToPoint (q), s .getClosestPointToPoint (q),
      s .getPerpendicularVectorToPoint (q), hit, x, s1 .normal, s1 .distanceFromOrigin, s2 .normal, s2 .distanceFromOrigin]);
}

function sphere3 (radius, c, a, b)
{
   const enter = new Vector3 (), exit = new Vector3 ();
   const hit = new Sphere3 (radius, c) .intersectsLine (Line3 .fromPoints (a, b), enter, exit);
   add ("sphere3", [radius, c, a, b], [hit, enter, exit]);
}

function cylinder3 (a, b, radius, c, d)
{
   const enter = new Vector3 (), exit = new Vector3 ();
   const hit = new Cylinder3 (Line3 .fromPoints (a, b), radius) .intersectsLine (Line3 .fromPoints (c, d), enter, exit);
   add ("cylinder3", [a, b, radius, c, d], [hit, enter, exit]);
}

function triangle3 (a, b, c, d)
{
   const n = new Vector3 (), q = new Vector3 ();
   Triangle3 .normal (a, b, c, n);
   Triangle3 .quadNormal (a, b, c, d, q);
   add ("triangle3", [a, b, c, d], [Triangle3 .area (a, b, c), n, q]);
}

function viewVolume (mv, pm, vp, win, pt, a, b)
{
   const u = new Vector3 (), ray = new Line3 (), out = new Vector3 (), l2 = new Line2 ();
   ViewVolume .unProjectPoint (win .x, win .y, win .z, mv, pm, vp, u);
   ViewVolume .unProjectRay (win .x, win .y, mv, pm, vp, ray);
   ViewVolume .projectPoint (pt, mv, pm, vp, out);
   ViewVolume .projectLine (Line3 .fromPoints (a, b), mv, pm, vp, l2);
   add ("viewvolume", [mv, pm, vp, win, pt, a, b], [u, ray .point, ray .direction, out, l2 .point, l2 .direction]);
}

function ortho (l, r, b, t, n, f)
{
   add ("camera.ortho", [l, r, b, t, n, f], [Camera .ortho (l, r, b, t, n, f, new Matrix4 ())]);
}

function box3 (min, max, m)
{
   const box = Box3 .fromExtents (min, max) .multRight (m), lo = new Vector3 (), hi = new Vector3 ();
   box .getExtents (lo, hi);
   add ("box3", [min, max, m], [lo, hi]);
}

// Degenerate and special cases.
const O = new Vector3 (0, 0, 0), X = new Vector3 (1, 0, 0), Y = new Vector3 (0, 1, 0), Z = new Vector3 (0, 0, 1), I4 = new Matrix4 (), I3 = new Matrix3 ();

line3 (O, Z, X, X, new Vector3 (1, 0, 1), new Vector3 (-1, -1, 2), new Vector3 (1, -1, 2), new Vector3 (0, 1, 2), I4);                  // parallel lines, hit
line3 (O, X, Y, O, Y, new Vector3 (0, 0, 1), new Vector3 (1, 0, 1), new Vector3 (0, 1, 1), I4);                                      // crossing lines, triangle parallel
line3 (O, O, X, O, X, X, Y, Z, I4);                                                                                                  // zero direction (NaN)
line3 (O, Z, X, Y, new Vector3 (0, 1, 1), new Vector3 (-1, -1, -2), new Vector3 (1, -1, -2), new Vector3 (0, 1, -2), I4);             // triangle behind
line2 (new Vector2 (0, 0), new Vector2 (1, 0), new Vector2 (3, 4), new Vector2 (0, 1), new Vector2 (1, 1), I3);                      // parallel
line2 (new Vector2 (0, 0), new Vector2 (1, 1), new Vector2 (0, 2), new Vector2 (0, 2), new Vector2 (2, 0), I3);                      // crossing
plane3 (O, Z, new Vector3 (1, 2, 3), new Vector3 (0, 0, 5), new Vector3 (1, 0, 5), I4);                                              // line parallel to plane
plane3 (O, new Vector3 (0, 0, 2), new Vector3 (1, 2, 3), new Vector3 (0, 0, 5), O, I4);                                              // normal not unit
sphere3 (1, O, new Vector3 (-5, 1, 0), new Vector3 (5, 1, 0));                                                                       // tangent
sphere3 (1, O, new Vector3 (-5, 2, 0), new Vector3 (5, 2, 0));                                                                       // miss
sphere3 (1, O, O, X);                                                                                                                 // from inside
cylinder3 (O, Z, 1, new Vector3 (-5, 0, 0), new Vector3 (5, 0, 0));
cylinder3 (O, Z, 1, new Vector3 (0, 0, -5), new Vector3 (0, 0, 5));                                                                  // along the axis
cylinder3 (O, Z, 1, new Vector3 (-5, 2, 0), new Vector3 (5, 2, 0));                                                                  // miss
triangle3 (O, X, Y, new Vector3 (1, 1, 0));
triangle3 (O, X, new Vector3 (2, 0, 0), new Vector3 (3, 0, 0));                                                                      // degenerate
ortho (-1, 1, -1, 1, 0.1, 100);
ortho (1, 1, -1, 1, 0.1, 100);                                                                                                        // zero width
box3 (new Vector3 (-1, -2, -3), new Vector3 (1, 2, 3), I4);
box3 (O, O, I4);

{
   const pm = new Matrix4 (); Camera .perspective (0.785398, 0.125, 1e5, 800, 600, pm);
   viewVolume (I4, pm, new Vector4 (0, 0, 800, 600), new Vector3 (400, 300, 0.5), new Vector3 (0, 0, -10), new Vector3 (0, 0, -10), new Vector3 (1, 1, -20));
   viewVolume (I4, pm, new Vector4 (0, 0, 800, 600), new Vector3 (0, 0, 0), new Vector3 (0, 0, 10), new Vector3 (0, 0, 10), new Vector3 (0, 0, 20)); // behind the camera
}

// Seeded random cases.
for (let i = 0; i < 200; ++ i)
{
   line3 (v3 (), v3 (), v3 (), v3 (), v3 (), v3 (), v3 (), v3 (), m4 ());
   line2 (v2 (), v2 (), v2 (), v2 (), v2 (), m3 ());
   plane3 (v3 (), v3 (), v3 (), v3 (), v3 (), m4 ());
   sphere3 (r (0.1, 4), v3 (), v3 (), v3 ());
   cylinder3 (v3 (), v3 (), r (0.1, 4), v3 (), v3 ());
   triangle3 (v3 (), v3 (), v3 (), v3 ());
   ortho (r (-10, -1), r (1, 10), r (-10, -1), r (1, 10), r (0.01, 1), r (10, 1000));
   box3 (v3 (), v3 (), m4 ());

   const pm = new Matrix4 ();
   Camera .perspective (r (0.2, 2), r (0.01, 1), r (10, 1e5), r (100, 2000), r (100, 2000), pm);
   viewVolume (m4 (), pm, new Vector4 (0, 0, r (100, 2000), r (100, 2000)), new Vector3 (r (0, 2000), r (0, 2000), rnd ()), v3 (), v3 (), v3 ());
}

process .stdout .write (JSON .stringify ({ source: "X_ITE 16.5.0 @ 34cdf9f97, standard/Math/Geometry", cases }, null, 0) + "\n");
