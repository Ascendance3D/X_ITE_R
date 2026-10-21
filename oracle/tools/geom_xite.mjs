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
const Box2       = await load ("Geometry/Box2.js");
const Triangle2  = await load ("Geometry/Triangle2.js");
const Spheroid3  = await load ("Geometry/Spheroid3.js");
const SAT        = await load ("Algorithms/SAT.js");
const Bezier     = await load ("Algorithms/Bezier.js");
const eigen      = await load ("Algorithms/eigen_decomposition.js");
const MergeSort  = await load ("Algorithms/MergeSort.js");
const QuickSort  = await load ("Algorithms/QuickSort.js");

const view = new DataView (new ArrayBuffer (8));
const hex = x => { view .setFloat64 (0, Number (x)); return view .getBigUint64 (0) .toString (16) .padStart (16, "0"); };

let seed = 0x5eed;
function rnd () { seed |= 0; seed = seed + 0x6D2B79F5 | 0; let t = Math .imul (seed ^ seed >>> 15, 1 | seed); t = t + Math .imul (t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }
const r = (a = -5, b = 5) => a + (b - a) * rnd ();
const v2 = () => new Vector2 (r (), r ());
const v3 = () => new Vector3 (r (), r (), r ());
// setTransform (translation, rotation, scale), and Rotation4 (x, y, z, angle): Matrix4 .set takes the 16 elements and Rotation4
// four numbers (the first version of this file passed vectors, so every random matrix read as NaN).
const rot = () => { const t = v3 (), a = v3 () .normalize (); return [t, new Rotation4 (a .x, a .y, a .z, r (-3, 3))]; };
const m4 = () => { const [t, q] = rot (), m = new Matrix4 (); m .setTransform (t, q, new Vector3 (r (0.2, 3), r (0.2, 3), r (0.2, 3))); return m; };
const m3 = () => { const m = new Matrix3 (); m .setTransform (v2 (), r (-3, 3), new Vector2 (r (0.2, 3), r (0.2, 3))); return m; };

const flat = xs => xs .flatMap (x => typeof x === "number" || typeof x === "boolean" ? [Number (x)] : flat (Array .from (x)));
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

// --- The rest of standard/Math (checkpoint after 804802d). Appended after the cases above, so their inputs (and the random
// sequence they draw) are unchanged.

function box2 (min, max, m, mb, p, size, center, q1, q2, q3, omin, omax)
{
   const b = Box2 .fromExtents (min, max), c = new Box2 (size, center), lo = new Vector2 (), hi = new Vector2 ();
   const out = [Array .from (c .matrix), b .isEmpty (), new Box2 () .isEmpty (), b .containsPoint (p), b .equals (b .copy ()), b .size .copy (), b .center .copy ()];
   b .multRight (m); b .getExtents (lo, hi); out .push (lo .copy (), hi .copy (), b .containsPoint (p));
   b .multLeft (mb); b .getExtents (lo, hi); out .push (lo .copy (), hi .copy (), b .size .copy ());
   const o = Box2 .fromExtents (omin, omax);
   b .add (o); b .getExtents (lo, hi); out .push (lo .copy (), hi .copy ());
   const e = new Box2 () .add (o); e .getExtents (lo, hi); out .push (lo .copy (), hi .copy ());
   Box2 .fromPoints ([q1, q2, q3]) .getExtents (lo, hi); out .push (lo .copy (), hi .copy ());
   out .push (Array .from (Box2 .fromPoints ([]) .matrix));
   add ("box2", [min, max, m, mb, p, size, center, q1, q2, q3, omin, omax], out);
}

function triangle2 (a, b, c, p)
{
   const bc = Triangle2 .toBarycentric (p, a, b, c);
   add ("triangle2", [a, b, c, p], [Triangle2 .area (a, b, c), Triangle2 .isPointInTriangle (p, a, b, c), bc .u, bc .v, bc .t]);
}

function spheroid3 (a, c, f)
{
   const s = new Spheroid3 (a, c, f);
   add ("spheroid3", [a, c, f], [s .semiMajorAxis, s .semiMinorAxis, s .equals (s .copy ())]);
}

const pts = b => b .getPoints (Array .from ({ length: 8 }, () => new Vector3 ()));

function box3more (size, center, m, min, max, mo, p, arr)
{
   const a = new Box3 (size, center), b = Box3 .fromExtents (min, max) .multLeft (mo), lo = new Vector3 (), hi = new Vector3 ();
   const out = [Array .from (a .matrix), a .isEmpty (), new Box3 () .isEmpty (), pts (a), a .getAxes ([new Vector3 (), new Vector3 (), new Vector3 ()]),
      a .getNormals ([new Vector3 (), new Vector3 (), new Vector3 ()]), a .containsPoint (p), a .intersectsBox (b), b .intersectsBox (a),
      a .intersectsBox (new Box3 ()), pts (b), b .getNormals ([new Vector3 (), new Vector3 (), new Vector3 ()])];
   const c = a .copy () .multLeft (m); c .getExtents (lo, hi); out .push (lo .copy (), hi .copy ());
   const d = a .copy () .add (b); d .getExtents (lo, hi); out .push (lo .copy (), hi .copy ());
   const e = new Box3 () .add (b); out .push (Array .from (e .matrix));
   Box3 .fromPoints ([size, center, min, max, p]) .getExtents (lo, hi); out .push (lo .copy (), hi .copy ());
   Box3 .fromArray (arr, 3) .getExtents (lo, hi); out .push (lo .copy (), hi .copy ());
   Box3 .fromArray (arr, 4) .getExtents (lo, hi); out .push (lo .copy (), hi .copy ());
   out .push (Array .from (Box3 .fromPoints ([]) .matrix), Array .from (Box3 .fromArray ([]) .matrix), a .size .copy ());
   const o = new Matrix4 (); Camera .orthoBox (b, o); out .push (Array .from (o));
   add ("box3.more", [size, center, m, min, max, mo, p, arr], out);
}

function camera (l, r, b, t, n, f, fov, w, h)
{
   const m1 = new Matrix4 (), m2 = new Matrix4 (), m3 = new Matrix4 ();
   Camera .frustum (l, r, b, t, n, f, m1);
   Camera .perspective (fov, n, f, w, h, m2);
   Camera .perspective2 (fov, n, f, w, h, m3);
   add ("camera", [l, r, b, t, n, f, fov, w, h], [Array .from (m1), Array .from (m2), Array .from (m3)]);
}

function viewVolumeObject (pm, vp, radius, center, size, bc, m)
{
   const v = new ViewVolume (pm, vp), b = new Box3 (size, bc) .multRight (m);
   const out = [v .points, v .normals, v .planes .flatMap (p => [p .normal, p .distanceFromOrigin]), v .getEdges (), v .getViewport (),
      v .intersectsSphere (radius, center), v .intersectsBox (b), v .intersectsBox (new Box3 (size, center))];
   add ("viewvolume.object", [pm, vp, radius, center, size, bc, m], out);
}

function sat (a1, a2, a3, p1, p2, p3, p4, q1, q2, q3, q4)
{
   add ("sat", [a1, a2, a3, p1, p2, p3, p4, q1, q2, q3, q4], [SAT .isSeparated ([a1, a2, a3], [p1, p2, p3, p4], [q1, q2, q3, q4])]);
}

function bezier (c, steps, arc)
{
   const q = [], k = [], a = [];
   Bezier .quadric (... c .slice (0, 9), steps, q);
   Bezier .cubic (... c, steps, k);
   Bezier .arc (... arc, a);
   add ("bezier", [c, steps, arc], [q .length, q, k .length, k, a .length, a]);
}

function eigenCase (values, order)
{
   const m = order === 3 ? new Matrix3 (... values) : new Matrix4 (... values);
   const result = { values: [], vectors: Array .from ({ length: order }, () => []) };
   eigen (m, result);
   add ("eigen", [order, values], [result .values, result .vectors .flat ()]);
}

function sorts (keys, first, last)
{
   const items = keys .map ((k, i) => ({ k, i })), cmp = (a, b) => a .k < b .k;
   const m = items .slice (); new MergeSort (m, cmp) .sort (first, last);
   const q = items .slice (); new QuickSort (q, cmp) .sort (first, last);
   const mn = keys .slice (); new MergeSort (mn) .sort (first, last);
   const qn = keys .slice (); new QuickSort (qn) .sort (first, last);
   add ("sort", [keys .length, keys, first, last], [m .map (x => x .i), q .map (x => x .i), mn, qn]);
}

const v2c = (x, y) => new Vector2 (x, y), v3c = (x, y, z) => new Vector3 (x, y, z), I3b = new Matrix3 ();

box2 (v2c (-1, -2), v2c (1, 2), I3b, I3b, v2c (0, 0), v2c (2, 4), v2c (1, 1), v2c (0, 0), v2c (1, 1), v2c (-1, 2), v2c (3, 3), v2c (4, 5));
box2 (v2c (0, 0), v2c (0, 0), I3b, I3b, v2c (0, 0), v2c (0, 0), v2c (0, 0), v2c (0, 0), v2c (0, 0), v2c (0, 0), v2c (0, 0), v2c (0, 0));
triangle2 (v2c (0, 0), v2c (1, 0), v2c (0, 1), v2c (0.25, 0.25));
triangle2 (v2c (0, 0), v2c (1, 0), v2c (2, 0), v2c (0.5, 0));
triangle2 (v2c (0, 0), v2c (1, 0), v2c (0, 1), v2c (1, 1));
spheroid3 (6378137, 298.257223563, true);
spheroid3 (6378137, 6356752.3142, false);
spheroid3 (1, 0, true);
box3more (v3c (2, 2, 2), v3c (0, 0, 0), I4, v3c (-1, -1, -1), v3c (1, 1, 1), I4, v3c (0, 0, 0), [1, 2, 3, 4, 5, 6, 7]);
box3more (v3c (2, 0, 2), v3c (0, 0, 0), I4, v3c (0, 0, 0), v3c (0, 0, 0), I4, v3c (5, 5, 5), [0, 0, 0]);   // flat and point boxes
box3more (v3c (0, 0, 0), v3c (1, 1, 1), I4, v3c (3, 3, 3), v3c (4, 4, 4), I4, v3c (1, 1, 1), [1]);         // separated
box3more (v3c (0, 2, 0), v3c (0, 0, 0), I4, v3c (-1, 0, -1), v3c (1, 0, 1), I4, v3c (0, 0, 0), [1, 2]);     // a line, a flat box
camera (-1, 1, -1, 1, 0.1, 100, Math .PI / 4, 800, 600);
camera (-1, 1, -1, 1, 0.1, 100, Math .PI / 4, 600, 800);
camera (1, 1, 1, 1, 0, 0, 0, 0, 0);
{
   const pm = new Matrix4 (); Camera .perspective (0.785398, 0.125, 1e5, 800, 600, pm);
   viewVolumeObject (pm, new Vector4 (0, 0, 800, 600), 1, v3c (0, 0, -10), v3c (2, 2, 2), v3c (0, 0, -10), I4);
   viewVolumeObject (pm, new Vector4 (0, 0, 800, 600), 1, v3c (0, 0, 10), v3c (2, 2, 2), v3c (0, 0, 10), I4);
   const om = new Matrix4 (); Camera .ortho (-2, 2, -2, 2, 0.1, 50, om);
   viewVolumeObject (om, new Vector4 (10, 20, 300, 200), 0.5, v3c (3, 0, -5), v3c (1, 1, 1), v3c (2.4, 0, -5), I4);
}
sat (v3c (1, 0, 0), v3c (0, 1, 0), v3c (0, 0, 1), v3c (0, 0, 0), v3c (1, 0, 0), v3c (0, 1, 0), v3c (0, 0, 1), v3c (1, 0, 0), v3c (2, 0, 0), v3c (1, 1, 0), v3c (1, 0, 1));
sat (v3c (1, 0, 0), v3c (0, 1, 0), v3c (0, 0, 1), v3c (0, 0, 0), v3c (1, 0, 0), v3c (0, 1, 0), v3c (0, 0, 1), v3c (1.5, 0, 0), v3c (2, 0, 0), v3c (2, 1, 0), v3c (2, 0, 1));
bezier ([0, 0, 0, 1, 2, 0, 3, 0, 0, 4, 1, 1], 10, [0, 0, 1, 1, 0, false, true, 1, 1, 16]);
bezier ([0, 0, 0, 1, 2, 0, 3, 0, 0, 4, 1, 1], 1, [0, 0, 1, 2, 0.5, true, true, 1, 1, 16]);
bezier ([0, 0, 0, 1, 2, 0, 3, 0, 0, 4, 1, 1], 2, [1, 1, 1, 1, 0, false, false, 1, 1, 16]);   // same end points
bezier ([0, 0, 0, 1, 2, 0, 3, 0, 0, 4, 1, 1], 3, [0, 0, 0, 1, 0, false, false, 2, 1, 16]);   // zero radius
bezier ([0, 0, 0, 1, 2, 0, 3, 0, 0, 4, 1, 1], 5, [0, 0, 0.1, 0.1, 7, true, false, 5, 3, 64]); // radii too small, rotation > 2 pi
eigenCase ([2, 1, 0, 1, 2, 0, 0, 0, 3], 3);
eigenCase ([1, 0, 0, 0, 1, 0, 0, 0, 1], 3);
eigenCase ([4, 1, 2, 0.5, 1, 3, 0, 1, 2, 0, 5, 1, 0.5, 1, 1, 6], 4);
sorts ([3, 1, 2, 1, 3, 0, 2, 2, 1], 0, 9);
sorts ([5, 5, 5, 5, 5], 0, 5);
sorts ([9, 8, 7, 6, 5, 4, 3, 2, 1, 0], 2, 8);
sorts ([1], 0, 1);
sorts ([2, 1], 0, 0);

for (let i = 0; i < 100; ++ i)
{
   box2 (v2c (r (), r ()), v2c (r (), r ()), m3 (), m3 (), v2c (r (), r ()), v2c (r (0, 4), r (0, 4)), v2c (r (), r ()), v2c (r (), r ()), v2c (r (), r ()), v2c (r (), r ()), v2c (r (), r ()), v2c (r (), r ()));
   triangle2 (v2c (r (), r ()), v2c (r (), r ()), v2c (r (), r ()), v2c (r (), r ()));
   spheroid3 (r (1, 1e7), r (1, 400), rnd () < 0.5);
   box3more (v3c (r (0, 4), r (0, 4), r (0, 4)), v3c (r (), r (), r ()), m4 (), v3 (), v3 (), m4 (), v3 (), Array .from ({ length: 3 + (i % 7) }, () => r ()));
   camera (r (-10, -1), r (1, 10), r (-10, -1), r (1, 10), r (0.01, 1), r (10, 1000), r (0.2, 2), r (100, 2000), r (100, 2000));

   const pm = new Matrix4 ();
   Camera .perspective (r (0.2, 2), r (0.01, 1), r (10, 1e3), r (100, 2000), r (100, 2000), pm);
   viewVolumeObject (pm, new Vector4 (r (0, 50), r (0, 50), r (100, 2000), r (100, 2000)), r (0.1, 5), v3c (r (), r (), r (-50, 0)), v3c (r (0, 4), r (0, 4), r (0, 4)), v3c (r (), r (), r (-50, 0)), m4 ());
   sat (v3 (), v3 (), v3 (), v3 (), v3 (), v3 (), v3 (), v3 (), v3 (), v3 (), v3 ());
   bezier (Array .from ({ length: 12 }, () => r ()), 2 + (i % 9), [r (), r (), r (-3, 3), r (-3, 3), r (-10, 10), rnd () < 0.5, rnd () < 0.5, r (), r (), 4 + (i % 40)]);

   const s = Array .from ({ length: 3 }, () => Array .from ({ length: 3 }, () => r ()));
   eigenCase ([s [0] [0], s [0] [1], s [0] [2], s [0] [1], s [1] [1], s [1] [2], s [0] [2], s [1] [2], s [2] [2]], 3);

   const t = Array .from ({ length: 4 }, () => Array .from ({ length: 4 }, () => r ()));
   eigenCase ([0, 1, 2, 3] .flatMap (a => [0, 1, 2, 3] .map (b => a <= b ? t [a] [b] : t [b] [a])), 4);

   const n = 1 + (i % 17), keys = Array .from ({ length: n }, () => Math .floor (r (0, 6)));
   sorts (keys, i % 3 === 0 ? 0 : Math .min (1, n), i % 5 === 0 ? n : Math .max (0, n - 1));
}

process .stdout .write (JSON .stringify ({ source: "X_ITE 16.5.0 @ 34cdf9f97, standard/Math/Geometry", cases }, null, 0) + "\n");
