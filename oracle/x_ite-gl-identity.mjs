// Prints the WebGL renderer of the headless Chrome the oracle scripts launch (same flags as x_ite-shot*.mjs).
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = dirname (fileURLToPath (import.meta .url));
const { chromium } = createRequire (join (ROOT, "../../../x_ite-tests/")) ("playwright");

const browser = await chromium .launch ({ executablePath: "/usr/bin/google-chrome", headless: true,
   args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ] });
const page = await browser .newPage ();
console .log (await page .evaluate (() =>
{
   const gl   = document .createElement ("canvas") .getContext ("webgl2"),
         info = gl .getExtension ("WEBGL_debug_renderer_info");
   return JSON .stringify ({ vendor: gl .getParameter (info .UNMASKED_VENDOR_WEBGL), renderer: gl .getParameter (info .UNMASKED_RENDERER_WEBGL) });
}));
await browser .close ();
