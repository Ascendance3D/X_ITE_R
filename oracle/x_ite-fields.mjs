/**
 * X1 oracle: field parsing/printing, number formatting and math results computed by the
 * pinned X_ITE modules in node (no browser needed: src/x_ite/Fields and src/standard/Math are
 * plain ES modules).
 *
 *   node oracle/x_ite-fields.mjs oracle/x_ite-fields.json
 *
 * Every case is synthetic. Numbers are written as JSON (shortest round-trip), non-finite
 * numbers as the strings "NaN", "Infinity", "-Infinity".
 */
import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const SRC = join (dirname (fileURLToPath (import.meta.url)), "x_ite-pin/src/");
const imp = async p => (await import (join (SRC, p))) .default;

// Minimal globals so the parser's module graph loads outside a browser (nothing here is used
// by field parsing; X_ITE reads them at import time). Import order avoids a module cycle.
globalThis .document = { currentScript: { src: "http://localhost/x_ite.js" }, location: { href: "http://localhost/" }, querySelector () { return null; } };
globalThis .libtess  = { GluTesselator: function () { return { gluTessCallback () { }, gluTessProperty () { } }; }, gluEnum: { }, windingRule: { } };
await import (join (SRC, "x_ite/Execution/X3DScene.js"));

const Fields        = await imp ("x_ite/Fields.js");
await import (join (SRC, "x_ite/Parser/VRMLParser.js")); // installs X3DField.prototype.fromVRMLString
const VRMLGenerator = await imp ("x_ite/InputOutput/VRMLGenerator.js");
const Vector2       = await imp ("standard/Math/Numbers/Vector2.js");
const Vector3       = await imp ("standard/Math/Numbers/Vector3.js");
const Vector4       = await imp ("standard/Math/Numbers/Vector4.js");
const Rotation4     = await imp ("standard/Math/Numbers/Rotation4.js");
const Quaternion    = await imp ("standard/Math/Numbers/Quaternion.js");
const Matrix3       = await imp ("standard/Math/Numbers/Matrix3.js");
const Matrix4       = await imp ("standard/Math/Numbers/Matrix4.js");
const Color3        = await imp ("standard/Math/Numbers/Color3.js");
const Color4        = await imp ("standard/Math/Numbers/Color4.js");

const [ outArg ] = process .argv .slice (2);

const num  = v => Object .is (v, -0) ? "-0" : Number .isFinite (v) ? v : String (v);
const nums = a => Array .from (a, num);

// --- 1. number formatting ---------------------------------------------------------------

// Deterministic pseudo-random doubles over many magnitudes (xorshift, fixed seed).
let seed = 0x2545F491;
const rand = () => { seed ^= seed << 13; seed >>>= 0; seed ^= seed >>> 17; seed ^= seed << 5; seed >>>= 0; return seed / 4294967296; };

const numbers = [ 0, -0, 1, -1, 0.1, 0.5, 1.5, 2.5, 0.15, 0.25, 0.35, 1e-4, 9.99999e-5, 1e-5, 0.00012345678, 123456.7, 1234567, 12345678,
   9999999.5, 99999995, 999999.95, 0.000099999995, 1e21, 1e-7, 123456789012345680, 0.7853981633974483, Math .PI, Math .E, 1 / 3, 2 / 3,
   946684800.123, 1e300, -1e-300, 5e-324, Number .MAX_VALUE, NaN, Infinity, -Infinity, 0.30000000000000004, 1.0000001, 1.00000005,
   Math .fround (0.1), Math .fround (0.7), Math .fround (1 / 3), Math .fround (1e-5), Math .fround (33.1), 0.599696, -0.39834, 1.75 ];

for (let i = 0; i < 3000; ++ i)
{
   const e = Math .floor (rand () * 40) - 20;
   const v = (rand () - 0.5) * 2 * Math .pow (10, e);
   numbers .push (i % 3 === 0 ? Math .fround (v) : v);
}

const generator = new VRMLGenerator ();

const format = numbers .map (v => [ num (v), generator .FloatFormat (v), generator .DoubleFormat (v) ]);

// --- 2. field parse + print -------------------------------------------------------------

const fieldCases = [
   ["SFBool", "TRUE"], ["SFBool", "FALSE"], ["SFBool", "true"],
   ["SFColor", "1 0.5 0"], ["SFColor", "1.5 -1 0.25"], ["SFColor", "0.13725 0.13725 0.13725"],
   ["SFColorRGBA", "1 0.5 0 0.25"], ["SFColorRGBA", "2 0 0 -1"],
   ["SFDouble", "3.141592653589793"], ["SFDouble", "1e-20"], ["SFDouble", "PI"], ["SFDouble", "-INF"], ["SFDouble", "nan"], ["SFDouble", "pi3_4"],
   ["SFFloat", "0.1"], ["SFFloat", "123456789"], ["SFFloat", "1e-5"], ["SFFloat", "0.00012345678"], ["SFFloat", "9999999.5"],
   ["SFFloat", "-0"], ["SFFloat", "+.5"], ["SFFloat", "5."], ["SFFloat", "1.e3"], ["SFFloat", "-SQRT1_2"], ["SFFloat", "1E+2"],
   ["SFImage", "2 2 3 0xFF0000 0x00FF00 0x0000FF 0xFFFFFF"], ["SFImage", "1 1 1 -1"], ["SFImage", "0 0 0"], ["SFImage", "3 1 4 1 2 3"],
   ["SFInt32", "42"], ["SFInt32", "-7"], ["SFInt32", "0x1F"], ["SFInt32", "2147483648"], ["SFInt32", "99999999999"], ["SFInt32", "+3"],
   ["SFMatrix3d", "1 2 3 4 5 6 7 8 9.5"], ["SFMatrix3f", "1 0 0 0 1 0 0 0 1"], ["SFMatrix3f", "0.1 0.2 0.3 0.4 0.5 0.6 0.7 0.8 0.9"],
   ["SFMatrix4d", "1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16.25"], ["SFMatrix4f", "0.1 0 0 0 0 0.2 0 0 0 0 0.3 0 1 2 3 1"],
   ["SFQuaternion", "0 0 0 1"], ["SFQuaternion", "0.1 0.2 0.3 0.9"],
   ["SFRotation", "0 1 0 3.14159"], ["SFRotation", "1 1 0 PI2"], ["SFRotation", "0 0 0 0"], ["SFRotation", "0 1 0 0"], ["SFRotation", "0.015 -0.081 0.997 3.096"],
   ["SFString", "\"hello\""], ["SFString", "\"a \\\"quoted\\\" \\\\ back\""], ["SFString", "\"multi\nline\""], ["SFString", "\"\""], ["SFString", "\"tab\\t x\""],
   ["SFTime", "0"], ["SFTime", "-1"], ["SFTime", "946684800.123456789"],
   ["SFVec2d", "1.123456789012345678 2"], ["SFVec2f", "0.5 -0.25"], ["SFVec2f", "1e10 1e-10"],
   ["SFVec3d", "1 2 3"], ["SFVec3f", "0.39834 -0.599696 -0.499488"], ["SFVec3f", "-1.21571 0.15692 0.7894"],
   ["SFVec4d", "1 2 3 4"], ["SFVec4f", "0.1 0.2 0.3 0.4"],
   ["MFBool", "[]"], ["MFBool", "TRUE"], ["MFBool", "[TRUE FALSE true]"],
   ["MFColor", "[]"], ["MFColor", "1 0 0"], ["MFColor", "[1.5 0 0, 0 1 0]"], ["MFColor", "[0.1 0.2 0.3 0.4]"],
   ["MFColorRGBA", "[1 0 0 1, 0 1 0 0.5]"], ["MFColorRGBA", "2 0 0 1"],
   ["MFDouble", "[1.1 2.2 3.3]"], ["MFDouble", "5"],
   ["MFFloat", "[0.1, 0.2, 0.3]"], ["MFFloat", "0.7"], ["MFFloat", "[]"],
   ["MFImage", "[1 1 1 255 2 1 2 1 2]"], ["MFImage", "1 1 3 0xFFFFFF"],
   ["MFInt32", "[1, -1, 0xFF]"], ["MFInt32", "7"], ["MFInt32", "[3 2 1 0 -1]"],
   ["MFMatrix3d", "[1 0 0 0 1 0 0 0 1, 2 0 0 0 2 0 0 0 2]"], ["MFMatrix3f", "[0.1 0 0 0 1 0 0 0 1]"],
   ["MFMatrix4d", "1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1"], ["MFMatrix4f", "[0.5 0 0 0 0 0.5 0 0 0 0 0.5 0 0 0 0 1, 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1]"],
   ["MFQuaternion", "[0 0 0 1, 0.1 0.2 0.3 0.9]"], ["MFQuaternion", "[0 0 0 1 0.5]"],
   ["MFRotation", "[0 1 0 1, 1 0 0 2]"], ["MFRotation", "0 0 1 0"], ["MFRotation", "[0 1 0 1 1 0 0]"],
   ["MFString", "[\"a\" \"b\"]"], ["MFString", "\"single\""], ["MFString", "[]"], ["MFString", "[\"x\\\"y\", \"z\\\\\"]"],
   ["MFTime", "[0 1 2.5]"],
   ["MFVec2d", "[1 2, 3 4]"], ["MFVec2f", "[0 1, 1 1, 1 0]"], ["MFVec2f", "[1 2 3]"], ["MFVec2f", "0.5 0.5"],
   ["MFVec3d", "[1 2 3, 4 5 6]"], ["MFVec3f", "[0.39834 -0.599696 -0.499488, -0.39834 -1.75 -0.499488]"], ["MFVec3f", "[]"], ["MFVec3f", "1 2 3"],
   ["MFVec4d", "[1 2 3 4]"], ["MFVec4f", "[0.1 0.2 0.3 0.4, 1 1 1 1]"],
];

function raw (f)
{
   const t = f .getTypeName ();

   switch (t)
   {
      case "SFBool":   return f .getValue ();
      case "SFString": return f .getValue ();
      case "SFInt32":
      case "SFFloat":
      case "SFDouble":
      case "SFTime":   return num (f .getValue ());
      case "SFImage":  return [f .width, f .height, f .comp, ... Array .from (f .array)];
      case "MFImage":  return Array .from (f, i => [i .width, i .height, i .comp, ... Array .from (i .array)]);
      case "MFString": return Array .from (f);
      case "MFBool":   return Array .from (f, v => !! v);
   }

   if (t .startsWith ("SF"))
      return nums (f);

   return Array .from (f, v => typeof v === "object" ? nums (v) : num (v));
}

const fields = fieldCases .map (([type, input]) =>
{
   const field = new Fields [type] ();

   try
   {
      field .fromVRMLString (input);

      const fresh = new Fields [type] ();

      return { type, input, ok: true, vrml: field .toVRMLString (), raw: raw (field), isDefault: field .isDefaultValue (), equalsDefault: field .equals (fresh) };
   }
   catch (error)
   {
      return { type, input, ok: false, error: String (error .message) };
   }
});

// --- 3. math ----------------------------------------------------------------------------

const math = { };

{
   const a = new Vector3 (1.5, -2.25, 3.125), b = new Vector3 (-0.5, 4, 0.75);

   math .vector3 = {
      add: nums (a .copy () .add (b)), subtract: nums (a .copy () .subtract (b)), multiply: nums (a .copy () .multiply (1.7)),
      divide: nums (a .copy () .divide (3)), multVec: nums (a .copy () .multVec (b)), divVec: nums (a .copy () .divVec (b)),
      cross: nums (a .copy () .cross (b)), dot: num (a .dot (b)), norm: num (a .norm ()), squaredNorm: num (a .squaredNorm ()),
      normalize: nums (a .copy () .normalize ()), distance: num (a .distance (b)), lerp: nums (a .copy () .lerp (b, 0.3)),
      negate: nums (a .copy () .negate ()), inverse: nums (a .copy () .inverse ()), reflect: nums (a .copy () .reflect (b .copy () .normalize ())),
      min: nums (a .copy () .min (b)), max: nums (a .copy () .max (b)), abs: nums (a .copy () .abs ()),
      hypot3: num (Math .hypot (1e200, 1e200, 3)), hypotTiny: num (Math .hypot (3e-200, 4e-200, 0)),
   };
}

{
   const a = new Vector2 (0.3, -0.7), b = new Vector2 (2, 5);

   math .vector2 = {
      add: nums (a .copy () .add (b)), dot: num (a .dot (b)), norm: num (a .norm ()), normalize: nums (a .copy () .normalize ()),
      lerp: nums (a .copy () .lerp (b, 0.25)), distance: num (a .distance (b)), reflect: nums (a .copy () .reflect (b .copy () .normalize ())),
   };
}

{
   const a = new Vector4 (1, 2, 3, 4), b = new Vector4 (-1, 0.5, 0.25, 2);

   math .vector4 = {
      add: nums (a .copy () .add (b)), dot: num (a .dot (b)), norm: num (a .norm ()), normalize: nums (a .copy () .normalize ()),
      lerp: nums (a .copy () .lerp (b, 0.6)), distance: num (a .distance (b)),
   };
}

{
   const r1 = new Rotation4 (0, 1, 0, 1.2), r2 = new Rotation4 (1, 1, 0, 0.7), v = new Vector3 (1, 2, 3);

   math .rotation4 = {
      quaternion1: nums (r1 .getQuaternion ()),
      quaternion2: nums (r2 .getQuaternion ()),
      raw2: nums (r2),
      multLeft: nums (r1 .copy () .multLeft (r2)),
      multRight: nums (r1 .copy () .multRight (r2)),
      inverse: nums (r2 .copy () .inverse ()),
      matrix: nums (r2 .getMatrix ()),
      multVecRot: nums (r2 .multVecRot (v .copy ())),
      multRotVec: nums (r2 .multRotVec (v .copy ())),
      slerp: nums (r1 .copy () .slerp (r2, 0.35)),
      setVectors: nums (new Rotation4 () .setVectors (new Vector3 (1, 0, 0), new Vector3 (0, 1, 1))),
      setVectorsOpposite: nums (new Rotation4 () .setVectors (new Vector3 (1, 0, 0), new Vector3 (-1, 0, 0))),
      fromMatrix: nums (new Rotation4 () .setMatrix (r2 .getMatrix ())),
      bigAngle: nums (new Rotation4 (0, 0, 1, 7) .getQuaternion ()),
      identityEquals: new Rotation4 (0, 1, 0, 0) .equals (Rotation4 .IDENTITY),
      twoPiEquals: new Rotation4 (0, 1, 0, 2 * Math .PI) .equals (Rotation4 .IDENTITY),
      euler: nums (new Rotation4 () .setEuler (0.1, 0.2, 0.3)),
   };
}

{
   const q1 = new Quaternion (0.1, 0.2, 0.3, 0.9), q2 = new Quaternion (-0.3, 0.1, 0.5, 0.8);

   math .quaternion = {
      multLeft: nums (q1 .copy () .multLeft (q2)), multRight: nums (q1 .copy () .multRight (q2)),
      inverse: nums (q1 .copy () .inverse ()), normalize: nums (q1 .copy () .normalize ()), norm: num (q1 .norm ()),
      matrix: nums (q1 .copy () .normalize () .getMatrix ()), slerp: nums (q1 .copy () .normalize () .slerp (q2 .copy () .normalize (), 0.4)),
      log: nums (q1 .copy () .normalize () .log ()), exp: nums (q1 .copy () .exp ()),
      setMatrix: nums (new Quaternion () .setMatrix (new Matrix3 (0, 1, 0, -1, 0, 0, 0, 0, 1))),
   };
}

{
   const m = new Matrix3 (2, 0.5, 0, -1, 3, 0.25, 4, 5, 1), n = new Matrix3 (1, 2, 3, 0, 1, 4, 5, 6, 0);

   math .matrix3 = {
      determinant: num (m .determinant ()), inverse: nums (m .copy () .inverse ()), transpose: nums (m .copy () .transpose ()),
      multLeft: nums (m .copy () .multLeft (n)), multRight: nums (m .copy () .multRight (n)),
      multVecMatrix2: nums (m .multVecMatrix (new Vector2 (1, 2))), multVecMatrix3: nums (m .multVecMatrix (new Vector3 (1, 2, 3))),
      multMatrixVec3: nums (m .multMatrixVec (new Vector3 (1, 2, 3))),
      rotate: nums (m .copy () .rotate (0.4)), scale: nums (m .copy () .scale (new Vector2 (2, 3))), translate: nums (m .copy () .translate (new Vector2 (5, -1))),
      singularInverse: nums (new Matrix3 (1, 2, 3, 2, 4, 6, 0, 0, 0) .inverse ()),
   };
}

{
   const m = new Matrix4 (2, 0.5, 0, 0.1, -1, 3, 0.25, 0, 4, 5, 1, 0.2, 1, 2, 3, 1);
   const n = new Matrix4 (1, 0, 2, 0, 0, 1, 0, 3, 4, 0, 1, 0, 0, 5, 0, 1);

   math .matrix4 = {
      determinant: num (m .determinant ()), inverse: nums (m .copy () .inverse ()), transpose: nums (m .copy () .transpose ()),
      multLeft: nums (m .copy () .multLeft (n)), multRight: nums (m .copy () .multRight (n)),
      multVecMatrix: nums (m .multVecMatrix (new Vector3 (1, 2, 3))), multMatrixVec: nums (m .multMatrixVec (new Vector3 (1, 2, 3))),
      multDirMatrix: nums (m .multDirMatrix (new Vector3 (1, 2, 3))), multMatrixDir: nums (m .multMatrixDir (new Vector3 (1, 2, 3))),
      multVecMatrix4: nums (m .multVecMatrix (new Vector4 (1, 2, 3, 4))),
      setTransform: nums (new Matrix4 () .setTransform (new Vector3 (1, 2, 3), new Rotation4 (0, 1, 0, 0.5), new Vector3 (2, 2, 3),
                                                       new Rotation4 (1, 0, 0, 0.3), new Vector3 (0.5, 0, -1))),
      translateRotateScale: nums (new Matrix4 () .translate (new Vector3 (1, 0, 0)) .rotate (new Rotation4 (0, 0, 1, 1)) .scale (new Vector3 (2, 3, 4))),
   };
}

{
   const c = new Color3 (0.2, 0.6, 0.4), d = new Color4 (0.9, 0.1, 0.3, 0.5);

   math .color = {
      clamp3: nums (new Color3 (1.5, -0.5, 0.5)), clamp4: nums (new Color4 (2, -1, 0.5, 3)),
      hsv: nums (c .getHSV ([ ])), fromHsv: nums (new Color3 () .setHSV (... c .getHSV ([ ]))),
      hsva: nums (d .getHSVA ([ ])), fromHsva: nums (new Color4 () .setHSVA (... d .getHSVA ([ ]))),
      hsvGrey: nums (new Color3 (0.5, 0.5, 0.5) .getHSV ([ ])), fromHsvWrap: nums (new Color3 () .setHSV (7.5, 0.5, 0.8)),
   };
}

// --- 4. Math.* functions (V8) on deterministic inputs -------------------------------------

const jsmathInputs = [ 0, -0, 0.05, 0.1, 0.15, 0.3, 0.5, 0.7, 0.785, 1, 1.5, Math .PI / 2, Math .PI, 2, 3, 6.28, 7, 100, 1e5, 1e-9, -0.4, -2.5 ];

for (let i = 0; i < 2000; ++ i)
{
   const e = Math .floor (rand () * 12) - 6;
   jsmathInputs .push ((rand () - 0.5) * 2 * Math .pow (10, e));
}

const unit = jsmathInputs .map (v => v - Math .trunc (v)); // in (-1, 1) for acos/asin

const jsmath = {
   inputs: nums (jsmathInputs),
   unit: nums (unit),
   sin: nums (jsmathInputs .map (Math .sin)),
   cos: nums (jsmathInputs .map (Math .cos)),
   tan: nums (jsmathInputs .map (Math .tan)),
   exp: nums (jsmathInputs .map (v => Math .exp (v / 100))),
   log: nums (jsmathInputs .map (v => Math .log (Math .abs (v)))),
   log10: nums (jsmathInputs .map (v => Math .log10 (Math .abs (v)))),
   acos: nums (unit .map (Math .acos)),
   asin: nums (unit .map (Math .asin)),
   atan2: nums (jsmathInputs .map ((v, i) => Math .atan2 (v, unit [i]))),
   hypot: nums (jsmathInputs .map ((v, i) => Math .hypot (v, unit [i], 0.5))),
};

await writeFile (outArg, JSON .stringify ({ format, fields, math, jsmath }, null, 1));
console .log (`${format .length} numbers, ${fields .length} field cases, ${Object .keys (math) .length} math groups`);
