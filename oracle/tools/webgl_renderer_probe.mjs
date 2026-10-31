#!/usr/bin/env node
// Prints the WebGL vendor / renderer / version and the depth bits and the reversed depthRange behavior of the Chrome the oracle launches (same GL flags as
// crates/x3d-oracle/src/chrome.rs), so a reference recording names the GPU it ran on.
//   XITE_TESTS=<x_ite-tests> node oracle/tools/webgl_renderer_probe.mjs
import { createRequire } from "node:module";

const { chromium } = createRequire (process .env .XITE_TESTS + "/") ("playwright");
const browser = await chromium .launch ({ executablePath: process .env .X3D_ORACLE_CHROME ?? "/usr/bin/google-chrome",
   args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ] });
const page = await browser .newPage ();
const info = await page .evaluate (() =>
{
   const gl = document .createElement ("canvas") .getContext ("webgl2", { depth: true });
   const e = gl .getExtension ("WEBGL_debug_renderer_info");
   // WebGL rejects depthRange (n, f) with n > f (INVALID_OPERATION) and keeps the old range; desktop GL accepts it.
   gl .depthRange (0.25, 0.75);
   gl .getError ();
   gl .depthRange (1, 0);

   const reversed = { error: gl .getError () === gl .INVALID_OPERATION ? "INVALID_OPERATION" : "none", range: Array .from (gl .getParameter (gl .DEPTH_RANGE)) };

   return { reversedDepthRange: reversed, vendor: gl .getParameter (e .UNMASKED_VENDOR_WEBGL), renderer: gl .getParameter (e .UNMASKED_RENDERER_WEBGL),
            version: gl .getParameter (gl .VERSION), depthBits: gl .getParameter (gl .DEPTH_BITS) };
});
console .log (JSON .stringify ({ ...info, chrome: browser .version () }));
await browser .close ();
