// CSS Color 5 relative color syntax through X3DParser.convertColor in headless Chrome:
//   XITE_TESTS=<x_ite-tests checkout> node oracle/x_ite-css5-relative-colors.mjs out.json
// Same entries as oracle/x_ite-css4-colors.mjs: [value, computedColor | null, [r, g, b, a] | null],
// the computed `color` string and the Color4 components X_ITE derives from it (`convertColor`);
// null when Chrome rejects the value. NaN is written as the string "NaN".
// Every value is synthetic. The list is deterministic: the same script gives the same file.
import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
const { chromium } = createRequire (join (process .env .XITE_TESTS ?? new URL ("../../../x_ite-tests", import.meta.url) .pathname, "/")) ("playwright");
const browser = await chromium .launch ({ headless: true, executablePath: process .env .CHROME || undefined });
const page = await browser .newPage ();
await page .setContent ("<body></body>");

// `values` are compared exactly. `cross` holds colors that pass through another color space, which
// Chrome computes in 32-bit floats with its own transfer functions: they differ from a 64-bit
// computation in the last printed digit or two (see tests/rea_css5_relative_color.rs).
const values = new Set (), cross = new Set ();
const add = v => values .add (v), addCross = v => cross .add (v);

// Origins that Chrome keeps exactly in the color space of the function that reads them.
const srgbOrigins = [ "red", "white", "black", "gray", "rebeccapurple", "transparent", "currentcolor", "#123456", "#1234", "#12345678", "#abc", "#f0f8", "RED", "#ABCDEF",
   "rgb(10 20 30)", "rgb(10 20 30 / 0.5)", "rgb(10, 20, 30)", "rgba(10, 20, 30, 0.25)", "rgb(100% 50% 25%)", "rgb(10.5 20.4 30.7)", "rgb(300 -5 128)",
   "hsl(120 50% 50%)", "hsl(30deg 80% 40% / 0.7)", "hsl(200, 60%, 30%)", "hwb(200 20% 30%)", "hwb(10 0% 0% / 0.4)", "color(srgb 0.2 0.4 0.6)", "color(srgb 0.2 0.4 0.6 / 0.3)" ];
const labOrigins = [ "lab(50 40 20)", "lab(29.2345 -12.5 60 / 0.4)", "lab(80 -60 -30)", "lab(10% 50% -50%)", "lab(none 20 -20)", "lab(70 none none / none)" ];
const lchOrigins = [ "lch(50 40 120)", "lch(29.2345 70 300 / 0.4)", "lch(80 20 0.25turn)", "lch(10% 50% 200grad)", "lch(40 none 200)", "lch(70 30 none / none)" ];
const oklabOrigins = [ "oklab(0.6 0.1 0.05)", "oklab(0.45 -0.12 0.2 / 0.4)", "oklab(60% 25% -25%)", "oklab(none 0.1 0.1)", "oklab(0.9 none none / none)" ];
const oklchOrigins = [ "oklch(0.6 0.15 40)", "oklch(0.45 0.2 300 / 0.4)", "oklch(70% 50% 0.5turn)", "oklch(0.8 none 120)", "oklch(0.3 0.1 none / none)" ];
const rgbSpaces = [ "srgb", "srgb-linear", "display-p3", "a98-rgb", "rec2020", "prophoto-rgb" ];
const colorOrigins = sp => [ `color(${sp} 0.2 0.4 0.6)`, `color(${sp} 0.7 0.3 0.1 / 0.4)`, `color(${sp} 20% 40% 60%)`, `color(${sp} none 0.5 0.5 / none)` ];
const xyzOrigins = sp => [ `color(${sp} 0.2 0.3 0.4)`, `color(${sp} 0.5 0.4 0.3 / 0.4)` ];

// A. The same color space: identity, every channel keyword, alpha.
for (const o of srgbOrigins) {
   add (`rgb(from ${o} r g b)`); add (`rgb(from ${o} r g b / alpha)`); add (`rgba(from ${o} r g b)`);
   add (`color(from ${o} srgb r g b)`); add (`color(from ${o} srgb r g b / alpha)`);
   add (`hsl(from ${o} h s l)`); add (`hsl(from ${o} h s l / alpha)`); add (`hwb(from ${o} h w b)`); add (`hwb(from ${o} h w b / alpha)`);
   add (`rgb(from ${o} b g r)`); add (`rgb(from ${o} g b r / 0.5)`); add (`rgb(from ${o} r g b / 25%)`);
   add (`hsl(from ${o} h s l / 0.5)`); add (`hsla(from ${o} h s l)`);
}
for (const o of labOrigins) { add (`lab(from ${o} l a b)`); add (`lab(from ${o} l a b / alpha)`); add (`lab(from ${o} b a l)`); add (`lab(from ${o} l a b / 0.5)`); }
for (const o of lchOrigins) { add (`lch(from ${o} l c h)`); add (`lch(from ${o} l c h / alpha)`); add (`lch(from ${o} c l h)`); add (`lch(from ${o} l c h / 50%)`); }
for (const o of oklabOrigins) { add (`oklab(from ${o} l a b)`); add (`oklab(from ${o} l a b / alpha)`); add (`oklab(from ${o} b a l)`); add (`oklab(from ${o} l a b / 0.5)`); }
for (const o of oklchOrigins) { add (`oklch(from ${o} l c h)`); add (`oklch(from ${o} l c h / alpha)`); add (`oklch(from ${o} c l h)`); add (`oklch(from ${o} l c h / 50%)`); }
for (const sp of rgbSpaces) for (const o of colorOrigins (sp)) { add (`color(from ${o} ${sp} r g b)`); add (`color(from ${o} ${sp} r g b / alpha)`); add (`color(from ${o} ${sp} b g r / 0.5)`); add (`color(from ${o} ${sp.toUpperCase ()} r g b)`); }
for (const sp of [ "xyz", "xyz-d50", "xyz-d65" ]) for (const o of xyzOrigins (sp)) {
   const tgt = sp; // xyz computes to xyz-d65
   add (`color(from ${o} ${tgt} x y z)`); add (`color(from ${o} ${tgt} x y z / alpha)`); add (`color(from ${o} ${tgt} z y x / 0.5)`);
}

// B. Direct conversions between related spaces.
for (const o of labOrigins) { add (`lch(from ${o} l c h)`); add (`lch(from ${o} l c calc(h + 10))`); }
for (const o of oklabOrigins) add (`oklch(from ${o} l c h)`);
// Polar to rectangular runs through 32-bit sine and cosine.
for (const o of lchOrigins) { addCross (`lab(from ${o} l a b)`); addCross (`lab(from ${o} l calc(a * 2) calc(b / 2))`); }
for (const o of oklchOrigins) addCross (`oklab(from ${o} l a b)`);

// C. Literal components, percentages, angles and none in every channel.
for (const c of [ "10", "50%", "0", "-10", "300", "none", "0.5", "10.5", "1e1" ]) {
   add (`rgb(from #123456 ${c} g b)`); add (`rgb(from #123456 r ${c} b)`); add (`rgb(from #123456 r g ${c})`);
   add (`color(from #123456 srgb ${c} g b)`); add (`color(from #123456 srgb r g ${c})`);
   add (`hsl(from hsl(120 50% 50%) h ${c} l)`); add (`hsl(from hsl(120 50% 50%) h s ${c})`); add (`hwb(from hwb(120 20% 30%) h ${c} b)`); add (`hwb(from hwb(120 20% 30%) h w ${c})`);
   add (`lab(from lab(50 40 20) ${c} a b)`); add (`lab(from lab(50 40 20) l ${c} b)`); add (`lab(from lab(50 40 20) l a ${c})`);
   add (`lch(from lch(50 40 120) ${c} c h)`); add (`lch(from lch(50 40 120) l ${c} h)`);
   add (`oklab(from oklab(0.6 0.1 0.05) ${c} a b)`); add (`oklab(from oklab(0.6 0.1 0.05) l ${c} b)`); add (`oklab(from oklab(0.6 0.1 0.05) l a ${c})`);
   add (`oklch(from oklch(0.6 0.15 40) ${c} c h)`); add (`oklch(from oklch(0.6 0.15 40) l ${c} h)`);
   add (`color(from color(display-p3 0.2 0.4 0.6) display-p3 ${c} g b)`); add (`color(from color(xyz 0.2 0.3 0.4) xyz ${c} y z)`);
}
for (const h of [ "0", "90", "-90", "450", "none", "0.25turn", "100grad", "1.5rad", "45deg", "50%", "10px", "1e2deg", "-0.5turn" ]) {
   add (`hsl(from hsl(120 50% 50%) ${h} s l)`); add (`hwb(from hwb(120 20% 30%) ${h} w b)`);
   add (`lch(from lch(50 40 120) l c ${h})`); add (`oklch(from oklch(0.6 0.15 40) l c ${h})`);
}
for (const a of [ "0", "1", "0.5", "50%", "150%", "-1", "-20%", "none", "alpha", "0.333", "1e-1", "10deg", "10px", "calc(alpha / 2)", "calc(1 - alpha)", "calc(0.5)" ]) {
   add (`rgb(from rgb(10 20 30 / 0.5) r g b / ${a})`); add (`hsl(from hsl(10 20% 30% / 0.5) h s l / ${a})`); add (`hwb(from hwb(10 20% 30% / 0.5) h w b / ${a})`);
   add (`lab(from lab(50 40 20 / 0.5) l a b / ${a})`); add (`lch(from lch(50 40 20 / 0.5) l c h / ${a})`); add (`oklab(from oklab(0.5 0.1 0.2 / 0.5) l a b / ${a})`);
   add (`oklch(from oklch(0.5 0.1 20 / 0.5) l c h / ${a})`); add (`color(from color(srgb 0.1 0.2 0.3 / 0.5) srgb r g b / ${a})`);
}

// D. Clamping and ranges.
for (const c of [ "-10", "0", "50", "100", "120", "150%", "-5%", "200%" ]) {
   add (`lab(from lab(50 40 20) ${c} a b)`); add (`lab(from lab(50 40 20) l ${c} b)`);
   add (`lch(from lch(50 40 120) ${c} c h)`); add (`lch(from lch(50 40 120) l ${c} h)`);
}
for (const c of [ "-0.1", "0", "0.5", "1", "1.5", "150%", "-5%", "200%" ]) {
   add (`oklab(from oklab(0.6 0.1 0.05) ${c} a b)`); add (`oklab(from oklab(0.6 0.1 0.05) l ${c} b)`);
   add (`oklch(from oklch(0.6 0.15 40) ${c} c h)`); add (`oklch(from oklch(0.6 0.15 40) l ${c} h)`);
}
for (const c of [ "-50", "0", "100", "150", "-10%", "120%" ]) {
   add (`hsl(from hsl(120 50% 50%) h ${c} l)`); add (`hsl(from hsl(120 50% 50%) h s ${c})`); add (`hwb(from hwb(120 20% 30%) h ${c} b)`); add (`hwb(from hwb(120 20% 30%) h w ${c})`);
}
add ("hwb(from hwb(120 20% 30%) h 60 60)"); add ("hwb(from hwb(120 20% 30%) h 100 0)"); add ("hwb(from hwb(120 20% 30%) h 0 100)"); add ("hwb(from hwb(120 20% 30%) h 100 100)");

// E. calc() and the math functions, with channel keywords.
const exprs = [ "calc(r)", "calc(r + 10)", "calc(r - 10)", "calc(r * 2)", "calc(r / 3)", "calc(r + g)", "calc(r * g / 2)", "calc((r + g) / 2)", "calc(r + 10 * 2)", "calc(-r)", "calc(+r)", "calc(r - -10)",
   "calc(pi * 10)", "calc(e * 10)", "calc(r + infinity)", "calc(r - infinity)", "calc(-infinity)", "calc(infinity)", "calc(NaN)", "calc(1 / 0)", "calc(-1 / 0)", "calc(0 / 0)",
   "min(r, 100)", "max(r, 100)", "min(r, g, b, 50)", "clamp(0, r, 100)", "clamp(10, r, 5)", "abs(r - 100)", "sign(r - 100)", "round(r / 3)", "round(up, r / 3)", "round(down, r / 3)", "round(to-zero, r / 3)", "round(nearest, r / 3)", "round(r, 10)", "round(up, r, 10)",
   "mod(r, 7)", "mod(-r, 7)", "rem(r, 7)", "rem(-r, 7)", "sqrt(r)", "pow(r, 2)", "pow(2, 3)", "hypot(r, g)", "hypot(3, 4)", "log(r)", "log(100, 10)", "exp(1)", "sin(90deg)", "cos(0)", "tan(45deg)", "sin(1.5708rad)", "calc(sin(0.5turn) * 100)",
   "calc(asin(1) / 1deg)", "calc(atan2(1, 1) / 1deg)", "calc(10%)", "calc(50% + 10%)", "calc(r + 10%)", "calc(r * 50%)", "calc(50% * 2)", "calc(1deg)", "calc(r + 1deg)", "calc(h)", "calc(r + )", "calc()", "calc(r +10)", "calc(r+10)", "calc( r + 10 )", "CALC(r + 10)", "calc(calc(r) + 1)", "calc((r))", "calc(r r)", "calc(r, 1)", "r + 10", "foo(r)", "calc(foo)",
   "calc(r * 2 / 3 + 1 - 0.5)", "calc(2 * (r + 1))", "calc(1e2 + r)", "calc(.5 + r)", "calc(5. + r)" ];
for (const e of exprs) {
   add (`rgb(from #123456 ${e} g b)`); add (`rgb(from #123456 r ${e} b)`); add (`rgb(from #123456 r g b / ${e})`); add (`color(from #123456 srgb ${e} g b)`);
}
const exprsHue = [ "calc(h)", "calc(h + 30)", "calc(h - 30)", "calc(h * 2)", "calc(h / 2)", "calc(h + 360)", "calc(h - 400)", "calc(h + 30deg)", "calc(30deg)", "calc(0.5turn)", "calc(90deg + 0.25turn)", "calc(h + s)", "calc(h + alpha)", "calc(h * alpha)", "min(h, 100)", "max(h, 100)", "abs(h - 200)", "mod(h + 90, 360)", "calc(100grad)", "calc(1rad)", "calc(360 * 2)", "calc(h + 30%)", "atan2(1, 1)", "calc(atan2(1, 1))", "asin(1)", "sin(90deg)" ];
for (const e of exprsHue) { add (`hsl(from hsl(120 50% 50%) ${e} s l)`); add (`hwb(from hwb(120 20% 30%) ${e} w b)`); add (`lch(from lch(50 40 120) l c ${e})`); add (`oklch(from oklch(0.6 0.15 40) l c ${e})`); }
const exprsLab = [ "calc(l)", "calc(l + 10)", "calc(l * 2)", "calc(l / 2)", "calc(l - 100)", "calc(l + a)", "calc(a * 2)", "calc(-a)", "calc(b / 2)", "calc(l * 50%)", "calc(l + 10%)", "calc(50%)", "calc(25% * 2)", "min(l, 20)", "max(l, 80)", "clamp(10, l, 20)", "abs(a)", "calc(alpha * 100)", "calc(l * alpha)" ];
for (const e of exprsLab) { add (`lab(from lab(50 40 20 / 0.5) ${e} a b)`); add (`lab(from lab(50 40 20 / 0.5) l ${e} b)`); add (`lab(from lab(50 40 20 / 0.5) l a ${e})`); add (`oklab(from oklab(0.6 0.1 0.05 / 0.5) ${e} a b)`); add (`oklab(from oklab(0.6 0.1 0.05 / 0.5) l ${e} b)`); add (`lch(from lch(50 40 120 / 0.5) ${e} c h)`); add (`lch(from lch(50 40 120 / 0.5) l ${e} h)`); add (`oklch(from oklch(0.6 0.15 40 / 0.5) l ${e} h)`); }
for (const e of [ "calc(r)", "calc(r + 0.1)", "calc(r * 2)", "calc(r - g)", "calc(alpha)", "calc(1 - r)", "calc(r * 50%)", "calc(r + 10%)", "calc(50%)", "min(r, 0.5)", "max(r, 0.5)", "clamp(0.1, r, 0.2)" ]) {
   add (`color(from color(display-p3 0.2 0.4 0.6) display-p3 ${e} g b)`); add (`color(from color(rec2020 0.2 0.4 0.6) rec2020 r ${e} b)`); add (`color(from color(srgb-linear 0.2 0.4 0.6) srgb-linear r g ${e})`);
}
for (const e of [ "calc(x)", "calc(x * 2)", "calc(y + z)", "calc(x / y)", "calc(z - 0.1)", "calc(r)", "calc(h)" ]) {
   add (`color(from color(xyz-d50 0.2 0.3 0.4) xyz-d50 ${e} y z)`); add (`color(from color(xyz-d65 0.2 0.3 0.4) xyz-d65 x ${e} z)`); add (`color(from color(xyz 0.2 0.3 0.4) xyz x y ${e})`);
}

// F. Nested relative colors and a computed value as the origin.
for (const v of [
   "rgb(from rgb(from red r g b) r g b)", "rgb(from rgb(from #123456 calc(r + 10) g b) b g r)", "rgb(from hsl(from #123456 h s l) r g b)", "rgb(from hwb(from #123456 h w b) r g b)",
   "lab(from lab(from lab(50 40 20) l a b) l a b)", "lab(from lab(from lab(50 40 20) calc(l + 10) a b) calc(l * 2) a b)", "lch(from lch(from lch(50 40 120) l c calc(h + 10)) l c calc(h + 10))",
   "oklab(from oklab(from oklab(0.6 0.1 0.05) l a b) l a b)", "oklch(from oklch(from oklch(0.6 0.15 40) l c h) l c calc(h + 90))", "color(from color(from color(srgb 0.2 0.4 0.6) srgb r g b) srgb b g r)",
   "color(from color(from color(display-p3 0.2 0.4 0.6) display-p3 r g b / 0.5) display-p3 r g b / alpha)", "rgb(from rgb(from rgb(from red r g b / 0.5) r g b / alpha) r g b / alpha)",
   "rgb(from rgb(from red none g b) r g b)", "lab(from lab(from lab(50 40 20) none a b) l a b)", "rgb(from rgb(from red r g b / none) r g b / alpha)",
] ) add (v);

// G. Syntax: spacing, case, separators, wrong component counts, wrong keywords. Chrome rejects most.
for (const v of [
   "rgb(from red r g b)", "rgb(  from   red   r   g   b  )", "rgb(from red r g b )", "rgb( from red r g b)", "RGB(FROM RED R G B)", "Rgb(From Red r g b)", "rgb(from red R G B)", "rgb(from  RED  r g b / ALPHA)", "rgb(from red r g b/alpha)", "rgb(from red r g b/ 0.5)", "rgb(from red r g b /0.5)", "rgb(from red r g b/0.5)",
   "rgb(from red, r, g, b)", "rgb(from red r, g, b)", "rgb(from red r g b, 0.5)", "rgb(from red r g b 0.5)", "rgb(from red r g b /)", "rgb(from red r g b / 0.5 0.5)", "rgb(from red r g b / 0.5 / 0.5)", "rgb(from red r g)", "rgb(from red r g b b)", "rgb(from red)", "rgb(from)", "rgb(from )", "rgb(from r g b)", "rgb(from red / r g b)",
   "rgb (from red r g b)", "rgb(fromred r g b)", "rgb(From red r g b)", "rgb(from red r g b))", "rgb(from (red) r g b)", "rgb(from rgb(red) r g b)", "rgb(from foo r g b)", "rgb(from #12 r g b)", "rgb(from #12345 r g b)", "rgb(from 123 r g b)",
   "rgb(from red l g b)", "rgb(from red h s l)", "rgb(from red x y z)", "rgb(from red r g r)", "rgb(from red r r r)", "rgb(from red alpha g b)", "rgb(from red r g alpha)", "rgb(from red r g b / r)", "rgb(from red r g b / g)", "rgb(from red r g b / h)", "rgb(from red none none none / none)",
   "hsl(from red r g b)", "hsl(from red h s)", "hwb(from red h s l)", "hsl(from red l s h)", "hsl(from red s h l)", "hwb(from red h w b b)", "lab(from red h c l)", "lab(from red l a b c)", "lab(from red l a)", "lab(from red r g b)", "lab(from red l, a, b)", "lch(from red l a b)", "lch(from red l c a)", "lch(from red l h c)", "oklab(from red l c h)", "oklch(from red l a b)",
   "color(from red srgb)", "color(from red srgb r g)", "color(from red srgb r g b b)", "color(from red r g b)", "color(from red foo r g b)", "color(from red xyz r g b)", "color(from red srgb x y z)", "color(from red srgb h s l)", "color(from red xyz-d50 r g b)", "color(from red display-p3 x y z)", "color(from red srgb, r, g, b)", "color(from red srgb r g b / alpha / alpha)", "color(from red rgb r g b)", "color(from red hsl h s l)", "color(from red lab l a b)", "color(from red)", "color(from)",
   "color(from red srgb r g b)", "color(from red SRGB r g b)", "color(from red Display-P3 r g b)", "color(from red XYZ x y z)", "color(from red xyz x y z)", "color(from red xyz-d65 x y z)", "color(from red xyz-d50 x y z)",
   "lab(from red from red l a b)", "lab(from lab(from) l a b)", "lab(from lab(50 40 20 l a b)", "from red lab(l a b)", "lab(red from l a b)", "lab(l a b from red)",
   "rgb(from currentcolor r g b)", "lab(from transparent l a b)", "lab(from black l a b)", "rgb(from currentcolor r g b / alpha)", "hsl(from currentcolor h s l)",
] ) add (v);

// H. Cross-color-space conversions (the origin is read in a different color space than it was written).
const crossOrigins = [ "#123456", "rgb(200 100 50)", "hsl(30 60% 40%)", "hwb(200 20% 30%)", "color(srgb 0.3 0.5 0.7)", "lab(50 40 20)", "lch(60 50 200)", "oklab(0.6 0.1 0.05)", "oklch(0.7 0.15 150)", "color(display-p3 0.3 0.5 0.7)", "color(a98-rgb 0.3 0.5 0.7)", "color(rec2020 0.3 0.5 0.7)", "color(prophoto-rgb 0.3 0.5 0.7)", "color(srgb-linear 0.3 0.5 0.7)", "color(xyz-d50 0.3 0.4 0.5)", "color(xyz-d65 0.3 0.4 0.5)", "rebeccapurple", "orange", "teal", "crimson" ];
const targets = [ "rgb(from O r g b)", "hsl(from O h s l)", "hwb(from O h w b)", "lab(from O l a b)", "lch(from O l c h)", "oklab(from O l a b)", "oklch(from O l c h)",
   ...rgbSpaces .map (s => `color(from O ${s} r g b)`), "color(from O xyz-d50 x y z)", "color(from O xyz-d65 x y z)" ];
for (const o of crossOrigins) for (const t of targets) addCross (t .replace ("O", o));

const record = async list => page .evaluate (list =>
{
   const wrap = document .createElement ("div"), div = document .createElement ("div");
   wrap .style .display = "none"; wrap .append (div); document .body .append (wrap);
   // X3DParser.convertColor, line for line.
   return list .map (v =>
   {
      wrap .style .color = "white"; div .style .color = ""; div .style .color = v;
      if (!div .style .color) return [v, null, null];
      const rgb = getComputedStyle (div) .color;
      const values = rgb .replace (/^rgba?\(|\)$/g, "") .split (/[\s,]+/) .map (s => parseFloat (s));
      values [0] /= 255; values [1] /= 255; values [2] /= 255;
      if (typeof values [3] !== "number") values [3] = 1;
      return [v, rgb, values .slice (0, 4) .map (n => Number .isNaN (n) ? "NaN" : n)];
   });
}, list);
const out = await record ([ ... values ]), converted = await record ([ ... cross ]);
writeFileSync (process .argv [2], JSON .stringify ({ chrome: await browser .version (), entries: out, converted }));
console .log (out .length, "values;", out .filter (e => e [1] === null) .length, "rejected;", converted .length, "converted");
await browser .close ();
