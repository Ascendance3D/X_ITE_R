// CSS Color 4 values through X3DParser.convertColor in headless Chrome:
//   XITE_TESTS=<x_ite-tests checkout> node oracle/x_ite-css4-colors.mjs out.json
// Each entry is [value, computedColor | null, [r, g, b, a] | null]: the computed `color` string,
// and the Color4 components X_ITE derives from it (`convertColor`, before the division by 255 is
// applied to the alpha); `null` when Chrome rejects the value. NaN is written as the string "NaN".
import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
const { chromium } = createRequire (join (process .env .XITE_TESTS ?? new URL ("../../../x_ite-tests", import.meta.url) .pathname, "/")) ("playwright");
const browser = await chromium .launch ({ headless: true });
const page = await browser .newPage ();
await page .setContent ("<body></body>");

const values = new Set ();
const add = v => values .add (v);
const alphas = [ "", " / 0.5", " / 50%", " / 1", " / 0", " / none", " / 150%", " / 0.333", "/0.25" ];

// hwb: hue (units), whiteness, blackness grid, including the gray rule (W + B >= 100%).
for (let h = 0; h < 360; h += 15) for (const w of [0, 10, 20, 40, 50, 60, 100]) for (const b of [0, 10, 30, 50, 60, 100]) add (`hwb(${h} ${w}% ${b}%)`);
for (let h = 0; h < 360; h += 1) for (const [w, b] of [[0, 0], [20, 30], [10, 10], [25, 25], [5, 45]]) add (`hwb(${h} ${w}% ${b}%)`);
for (const v of [ "hwb(0.5turn 10% 20%)", "hwb(200grad 10% 20%)", "hwb(3.14159rad 10% 20%)", "hwb(-90 10% 20%)", "hwb(450 10% 20%)", "hwb(none 0% 0%)", "hwb(120 none 30%)", "hwb(120 20% none)", "hwb(120 20 30)", "hwb(120 20% 30%,0.5)", "hwb(120,20%,30%)", "hwb(120 20%)", "hwb(120 20% 30% 40%)", "hwb(120 150% 30%)", "hwb(120 -10% 30%)", "HWB(120 20% 30%)", "hwb( 120   20%   30% )" ]) add (v);
for (const a of alphas) add (`hwb(120 20% 30%${a})`);

// lab / lch / oklab / oklch
for (const L of [ "0", "10", "50", "50%", "100", "100%", "120%", "-5", "29.2345", "1.23456789", "none" ]) for (const [a, b] of [ ["0", "0"], ["40", "20"], ["-40", "-20"], ["40%", "-20%"], ["200", "-200"], ["2.3456789", "3.456789"], ["none", "5"] ]) add (`lab(${L} ${a} ${b})`);
for (const L of [ "0", "50", "50%", "100%", "120%", "-5", "none" ]) for (const [c, h] of [ ["0", "0"], ["40", "120"], ["40", "480"], ["40", "-60"], ["-40", "120"], ["40%", "120"], ["150%", "120"], ["40", "0.5turn"], ["40", "200grad"], ["40", "3.14159rad"], ["40", "90deg"], ["none", "none"], ["40", "50%"] ]) add (`lch(${L} ${c} ${h})`);
for (const L of [ "0", "0.6", "60%", "1", "100%", "1.5", "-0.1", "none" ]) for (const [a, b] of [ ["0", "0"], ["0.1", "0.05"], ["-0.1", "-0.05"], ["50%", "-50%"], ["0.5", "-0.5"], ["none", "0.2"] ]) add (`oklab(${L} ${a} ${b})`);
for (const L of [ "0", "0.6", "60%", "1", "1.5", "none" ]) for (const [c, h] of [ ["0", "0"], ["0.15", "40"], ["0.15", "400"], ["0.15", "-40"], ["-0.1", "40"], ["50%", "40"], ["150%", "40"], ["0.15", "0.25turn"], ["none", "none"], ["0.15", "10%"] ]) add (`oklch(${L} ${c} ${h})`);
for (const fn of [ "lab(50 40 20", "lch(50 40 120", "oklab(0.6 0.1 0.05", "oklch(0.6 0.15 40" ]) for (const a of alphas) add (`${fn}${a})`);

// color()
for (const sp of [ "srgb", "srgb-linear", "display-p3", "a98-rgb", "prophoto-rgb", "rec2020", "xyz", "xyz-d50", "xyz-d65", "SRGB", "foo", "" ])
   for (const c of [ "1 0.5 0", "100% 50% 0%", "0 0 0", "2 -1 0.123456789", "none 0.5 0.5", "0.2 0.3 0.4", "1 0.5", "1 0.5 0 0.1", "0.5, 0.5, 0.5", "1e-7 0.5 0.5", "10% 20% 30%" ]) add (`color(${sp} ${c})`);
for (const a of alphas) { add (`color(srgb 1 0.5 0${a})`); add (`color(display-p3 0.1 0.2 0.3${a})`); }
for (const v of [ "color(srgb 1 0.5 0 / 0.5 / 3)", "color()", "color(srgb)", "color(from red srgb r g b)", "lab(from red l a b)", "lab(50 40 20 60)", "lab(50, 40, 20)", "lab(50 40)", "lab()", "lab(50 40 20deg)", "lab(50deg 40 20)", "oklab(0.6 0.1 0.05 0.5)", "oklab(0.6 0.1)", "oklab(0.6, 0.1, 0.05)", "oklab(0.6deg 0.1 0.05)" ]) add (v);

const list = [ ... values ];
const out = await page .evaluate (list =>
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
writeFileSync (process .argv [2], JSON .stringify ({ chrome: await browser .version (), entries: out }));
console .log (out .length, "values;", out .filter (e => e [1] === null) .length, "rejected");
await browser .close ();
