// sub:fields.math: toString of every class of the pinned X_ITE's standard/Math (Numbers and Geometry), imported unchanged from
// oracle/x_ite-pin/src. Each case lists its inputs as the 16 hex digits of the IEEE 754 double and the exact string that
// toString returns, so the Rust test (crates/x3d-fields/tests/math_tostring.rs, crates/x3d-viewer/tests/geom_tostring.rs) builds
// the same value from the same bits and compares the strings.
//
//   node oracle/tools/math_tostring_xite.mjs > oracle/math-tostring-baseline.json
//
// Inputs: edge numbers written out (zero, negative zero, NaN, infinities, exponent forms, float noise), then seeded
// pseudo-random combinations of them and of random numbers (mulberry32, seed 0x7057), so a run repeats exactly.

globalThis .libtess = { GluTesselator: class { gluTessCallback () { } gluTessProperty () { } }, gluEnum: { }, windingRule: { } };

const src  = new URL ("../x_ite-pin/src/standard/Math/", import.meta .url);
const load = async p => (await import (new URL (p, src))) .default;

const C = { };
for (const n of ["Complex", "Color3", "Color4", "Matrix2", "Matrix3", "Matrix4", "Quaternion", "Rotation4", "Vector2", "Vector3", "Vector4"])
   C [n] = await load (`Numbers/${n}.js`);
for (const n of ["Box2", "Box3", "Cylinder3", "Line2", "Line3", "Plane3", "Sphere3", "Spheroid3"])
   C [n] = await load (`Geometry/${n}.js`);

const view = new DataView (new ArrayBuffer (8));
const hex = x => { view .setFloat64 (0, Number (x)); return view .getBigUint64 (0) .toString (16) .padStart (16, "0"); };

let seed = 0x7057;
function rnd () { seed |= 0; seed = seed + 0x6D2B79F5 | 0; let t = Math .imul (seed ^ seed >>> 15, 1 | seed); t = t + Math .imul (t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }

const EDGE = [0, -0, 1, -1, 0.5, -0.5, 2, -3.25, 0.1 + 0.2, 1e21, -1e21, 1e-7, -1e-7, 123456789.123456789, 100, 1e20, 123e-20,
   NaN, Infinity, -Infinity, 5e-324, 1.7976931348623157e308, 0.000001, 1e300];
const num = () => rnd () < 0.5 ? EDGE [Math .floor (rnd () * EDGE .length)] : (rnd () - 0.5) * 20 * 10 ** Math .floor ((rnd () - 0.5) * 12);
const nums = n => Array .from ({ length: n }, num);

const cases = [];
const add = (cls, input, make) => { const value = make (input); cases .push ({ cls, in: input .map (hex), out: value .toString () }); };

// A value class takes n numbers; a geometry class takes its parts from consecutive numbers (a vector takes 2 or 3).
const spec =
{
   Complex:   [2, a => new C .Complex (...a)],
   Color3:    [3, a => new C .Color3 (...a)],
   Color4:    [4, a => new C .Color4 (...a)],
   Matrix2:   [4, a => new C .Matrix2 (...a)],
   Matrix3:   [9, a => new C .Matrix3 (...a)],
   Matrix4:   [16, a => new C .Matrix4 (...a)],
   Quaternion:[4, a => new C .Quaternion (...a)],
   Rotation4: [4, a => new C .Rotation4 (...a)],
   Vector2:   [2, a => new C .Vector2 (...a)],
   Vector3:   [3, a => new C .Vector3 (...a)],
   Vector4:   [4, a => new C .Vector4 (...a)],
   Line2:     [4, a => new C .Line2 (new C .Vector2 (a [0], a [1]), new C .Vector2 (a [2], a [3]))],
   Line3:     [6, a => new C .Line3 (new C .Vector3 (a [0], a [1], a [2]), new C .Vector3 (a [3], a [4], a [5]))],
   Plane3:    [6, a => new C .Plane3 (new C .Vector3 (a [0], a [1], a [2]), new C .Vector3 (a [3], a [4], a [5]))],
   Sphere3:   [4, a => new C .Sphere3 (a [0], new C .Vector3 (a [1], a [2], a [3]))],
   Cylinder3: [7, a => new C .Cylinder3 (new C .Line3 (new C .Vector3 (a [0], a [1], a [2]), new C .Vector3 (a [3], a [4], a [5])), a [6])],
   Box2:      [4, a => new C .Box2 (new C .Vector2 (a [0], a [1]), new C .Vector2 (a [2], a [3]))],
   Box3:      [6, a => new C .Box3 (new C .Vector3 (a [0], a [1], a [2]), new C .Vector3 (a [3], a [4], a [5]))],
   Spheroid3: [2, a => new C .Spheroid3 (a [0], a [1], false)],
};

for (const [cls, [n, make]] of Object .entries (spec))
{
   for (const e of EDGE)
      add (cls, Array .from ({ length: n }, () => e), make);

   for (let i = 0; i < 150; ++ i)
      add (cls, nums (n), make);
}

// Complex.toString has four branches (real part, "+", imaginary part, "i"): every pair of the edge numbers.
for (const x of EDGE)
   for (const y of EDGE)
      add ("Complex", [x, y], spec .Complex [1]);

console .log (JSON .stringify ({ pin: "X_ITE 16.5.0 34cdf9f97145b3cd2b8e250f8c88dfdd083c0517", cases }, null, 1));
