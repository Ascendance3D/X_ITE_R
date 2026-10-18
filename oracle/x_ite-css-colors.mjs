// Computed CSS colors from headless Chrome for a grid of values: node css-colors.mjs out.json
import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";
const { chromium } = createRequire (new URL ("../../../../X_ITE/x_ite-tests/", import.meta.url) .pathname) ("playwright");
const browser = await chromium .launch ({ headless: true });
const page = await browser .newPage ();
await page .setContent ("<body></body>");
const values = [ ];
for (let h = 0; h < 360; h += 1) for (const s of [0, 20, 50, 80, 100]) for (const l of [0, 10, 25, 40, 50, 60, 75, 90, 100]) values .push (`hsl(${h},${s}%,${l}%)`);
for (let p = 0; p <= 100; p += 0.5) values .push (`rgb(${p}%,${p}%,${p}%)`);
for (const v of [ "rgb(127.5,127.5,127.5)", "rgb(0.5,1.5,2.5)", "rgb(300,-5,2.6)", "rgba(10,20,30,0.5)", "rgba(10,20,30,0.333)", "rgba(10,20,30,.7)", "rgb(10 20 30 / 40%)", "hsla(240,100%,25%,0.5)", "hsl(120deg 50% 50%)", "hsl(0.5turn,50%,50%)", "currentcolor", "inherit", "transparent", "rgb(1,2)", "hwb(120 20% 30%)", "color(srgb 1 0 0)", "lab(50% 40 59)", "oklch(60% 0.2 30)" ]) values .push (v);
const out = await page .evaluate (values => {
   const wrap = document .createElement ("div"), div = document .createElement ("div");
   wrap .style .color = "rgb(1, 2, 3)"; wrap .append (div); document .body .append (wrap);
   return values .map (v => { div .style .color = ""; div .style .color = v; return [v, div .style .color ? getComputedStyle (div) .color : null]; });
}, values);
writeFileSync (process .argv [2], JSON .stringify (out));
await browser .close ();
console .log (out .length);
