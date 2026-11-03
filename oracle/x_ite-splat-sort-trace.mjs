/**
 * GaussianSplats sort trace of the pinned X_ITE 16.5.0 for larger splat counts (issue #69, the sort timing decision).
 *
 *   node oracle/x_ite-splat-sort-trace.mjs <out.json> <count> [<count> ...] [--runs 3]
 *
 * For each count, a synthetic world of that many splats (a fixed pseudo-random cloud, written below) is shown, then the
 * Transform is turned by a half turn, and the page records, for the frames that follow, the messages to and from X_ITE's
 * sort worker and its instanced draws of the splat quads (the same trace as crates/x3d-web/tests/browser/splatsort01_case.mjs
 * records for two splats). Written only when every run of every count gives the same trace. Headless only; no network.
 * The page is oracle/x_ite-pin/src/shot.html.
 */
import { writeFile } from "node:fs/promises";
import { checkOutput } from "./check-output.mjs";
import { resolve } from "node:path";

const argv = process .argv .slice (2);
const RUNS = argv .includes ("--runs") ? Number (argv [argv .indexOf ("--runs") + 1]) : 3;
const [ outArg, ... counts ] = argv .filter ((a, i) => a !== "--runs" && argv [i - 1] !== "--runs");

if (!outArg || !counts .length)
   throw new Error ("usage: node oracle/x_ite-splat-sort-trace.mjs <out.json> <count> [<count> ...] [--runs 3]");

checkOutput (resolve (outArg));

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { join, normalize, extname } from "node:path";
import { fileURLToPath } from "node:url";
const ROOT = normalize (join (fileURLToPath (import.meta.url), ".."));
const { chromium } = createRequire (join (process .env .XITE_TESTS ?? join (ROOT, "../../../x_ite-tests"), "/")) ("playwright");
const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".json": "application/json", ".png": "image/png", ".wasm": "application/wasm" };
const server = createServer(async (req, res) => { const p = normalize(join(ROOT, decodeURIComponent(new URL(req.url, "http://x").pathname)));
  try { const b = await readFile(p); res.writeHead(200, {"Content-Type": TYPES[extname(p)] ?? "text/plain", "Cache-Control":"no-store"}); res.end(b);} catch { res.writeHead(404); res.end(); } });
await new Promise(r => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({ executablePath: "/usr/bin/google-chrome", headless: true, args: ["--headless=new","--no-sandbox","--use-gl=angle","--use-angle=gl","--enable-gpu","--ignore-gpu-blocklist"] });
function world(n) {
  let s = 12345, r = () => (s = (s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff;
  const pos = [], ori = [], sc = [], op = [], sh = [];
  for (let i = 0; i < n; ++i) { pos.push(`${((r()-.5)*3).toFixed(3)} ${((r()-.5)*3).toFixed(3)} ${((r()-.5)*3).toFixed(3)}`); ori.push("0 0 0 1"); const q=(0.05+r()*0.15).toFixed(3); sc.push(`${q} ${q} ${q}`); op.push((0.5+r()*0.5).toFixed(2)); sh.push(`${((r()-.5)*3).toFixed(2)} ${((r()-.5)*3).toFixed(2)} ${((r()-.5)*3).toFixed(2)}`); }
  return `#X3D V4.1 utf8\nPROFILE Interchange\nCOMPONENT GaussianSplats:1\nViewpoint { position 0 0 6 }\nBackground { skyColor 0.2 0.2 0.28 }\nDEF T Transform { children GaussianSplats { positions [ ${pos.join(", ")} ] orientations [ ${ori.join(", ")} ] scales [ ${sc.join(", ")} ] opacities [ ${op.join(", ")} ] sphericalHarmonicsDegree0Coef0 [ ${sh.join(", ")} ] } }\n`;
}
const results = { };
let ok = true;
for (const n of counts.map(Number)) {
  const traces = [];
  for (let run = 0; run < RUNS; ++run) {
    const ctx = await browser.newContext({ viewport: { width: 512, height: 512 }, deviceScaleFactor: 1 });
    const page = await ctx.newPage();
    await page.goto(`${base}/x_ite-pin/src/shot.html`);
    await page.waitForFunction(() => window.ready, null, { timeout: 30000 });
    const r = await page.evaluate(async text => {
      const browser = document.getElementById("c").browser;
      window.trace = [];
      const Base = window.Worker;
      window.Worker = class extends Base { constructor(...a) { super(...a); this.addEventListener("message", e => window.trace.push("recv " + e.data.type)); const post = this.postMessage.bind(this); this.postMessage = (m, ...rest) => { window.trace.push("post " + m.type); return post(m, ...rest); }; } };
      const draw = WebGL2RenderingContext.prototype.drawArraysInstanced;
      WebGL2RenderingContext.prototype.drawArraysInstanced = function (mode, first, count, k) { if (count === 6) window.trace.push("draw " + k); return draw.call(this, mode, first, count, k); };
      const X3D = window[Symbol.for("X_ITE.X3D")];
      const frames = async k => { for (let i = 0; i < k; ++i) await new Promise(r => requestAnimationFrame(() => r())); };
      await browser.replaceWorld(await browser.createX3DFromString(text));
      await frames(60);
      window.trace.length = 0;
      browser.currentScene.getNamedNode("T").rotation = new X3D.SFRotation(0, 1, 0, Math.PI);
      await frames(60);
      return window.trace.slice(0, 14);
    }, world(n));
    traces.push(r.join(" | "));
    await ctx.close();
  }
  const same = new Set(traces).size === 1;
  ok &&= same;
  results[n] = same ? traces[0].split(" | ") : traces;
  console.log(n, same ? `identical x${RUNS}` : "DIFFER", "::", traces[0]);
}
await browser.close(); server.close();
if (!ok) { console.error("the runs differ; nothing written"); process.exit(1); }
const version = (await import("node:fs")).readFileSync(join(ROOT, "x_ite-pin/PINNED_COMMIT"), "utf8").trim();
await writeFile(resolve(outArg), JSON.stringify({ pinned: version, chrome: browser.version(), runs: RUNS, view: "Transform turned by a half turn about Y after 60 frames", traces: results }, null, 1) + "\n");
