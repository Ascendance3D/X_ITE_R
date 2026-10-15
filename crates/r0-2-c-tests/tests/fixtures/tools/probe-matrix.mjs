import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
const SRC = "/home/ryan/Projects/cybertown/.worktrees/r0-2-c1-test-hardening/oracle/x_ite-pin/src/standard/Math";
const { chromium } = createRequire ("/home/ryan/Projects/cybertown/x_ite-tests/") ("playwright");
async function compute (Matrix4, Vector3, Rotation4)
{
   const bits = x => { const b = new DataView (new ArrayBuffer (8)); b .setFloat64 (0, x); return "0x" + b .getBigUint64 (0) .toString (16) .padStart (16, "0"); };
   const arr = m => Array .from ({ length: 16 }, (_, i) => bits (m [i]));
   const A = new Matrix4 () .setTransform (new Vector3 (1, 2, 3), new Rotation4 (0, 1, 0, 0.5), new Vector3 (2, 3, 4));
   const B = new Matrix4 () .setTransform (new Vector3 (-1, 0.5, 2), new Rotation4 (1, 0, 0, 0.25));
   const out = { A: arr (A), B: arr (B) };
   out .det = bits (A .determinant ());
   out .inv = arr (A .copy () .inverse ());
   out .AB = arr (A .copy () .multRight (B));
   const t = new Vector3 (), r = new Rotation4 (), s = new Vector3 (), so = new Rotation4 ();
   A .factor (t, r, s, so);
   const rv = r .getAxisAngle ? null : null;
   out .factor = { t: [t .x, t .y, t .z] .map (bits), s: [s .x, s .y, s .z] .map (bits),
                   r: [r .x, r .y, r .z, r .angle] .map (bits), so: [so .x, so .y, so .z, so .angle] .map (bits) };
   return out;
}
const load = async base => [ (await import (base + "/Numbers/Matrix4.js")) .default, (await import (base + "/Numbers/Vector3.js")) .default, (await import (base + "/Numbers/Rotation4.js")) .default ];
const node = await compute (... await load (pathToFileURL (SRC) .href));
const browser = await chromium .launch ({ headless: true });
const page = await browser .newPage ();
await page .route ("http://pin.local/**", async route => {
   const p = SRC + new URL (route .request () .url ()) .pathname;
   route .fulfill ({ body: await readFile (p), contentType: "text/javascript" });
});
await page .route ("http://pin.local/", r => r .fulfill ({ body: "<!doctype html>", contentType: "text/html" }));
await page .goto ("http://pin.local/");
const chrome = await page .evaluate (`(async () => { const L = async p => (await import ("http://pin.local" + p)) .default;
  const compute = ${compute .toString ()};
  return compute (await L ("/Numbers/Matrix4.js"), await L ("/Numbers/Vector3.js"), await L ("/Numbers/Rotation4.js")); })()`);
const version = browser .version ();
await browser .close ();
console .log (JSON .stringify ({ chrome, chromeVersion: version, node, nodeVersion: process .version, same: JSON .stringify (chrome) === JSON .stringify (node) }, null, 1));
