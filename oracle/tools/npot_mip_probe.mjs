/**
 * Probe (issue #51, sub:render.core): the mip chain that gl.generateMipmap builds in the headless Chrome of the render
 * oracle (ANGLE on the GPU, flags of x_ite-shot-msaa.mjs), for the RGBA8 textures given as W x H on the command line.
 *
 *   XITE_TESTS=~/Projects/cybertown/X_ITE/x_ite-tests node oracle/tools/npot_mip_probe.mjs OUT.json 301x201 75x50 ...
 *
 * `WxH@x,y` instead fills level 0 with 0 (alpha 255) except one texel (x, y), which is 255 in R, G and B: the footprint of
 * that texel in the levels is the filter. Otherwise level 0 is filled from a fixed integer hash (no random source). Writes {renderer, cases: [{w, h, levels:
 * [{w, h, rgba: base64}]}]}, level 0 first, read back through a framebuffer (readPixels). Nothing here loads X_ITE.
 */
import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const { chromium } = createRequire (join (process .env .XITE_TESTS ?? "", "/")) ("playwright");
const [ out, ...sizes ] = process .argv .slice (2);

const browser = await chromium .launch ({
   executablePath: "/usr/bin/google-chrome", headless: true,
   args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ],
});
const page = await browser .newPage ();

const result = await page .evaluate (sizes => {
   const gl = document .createElement ("canvas") .getContext ("webgl2");
   const ext = gl .getExtension ("WEBGL_debug_renderer_info");
   const b64 = a => { let s = ""; for (const v of a) s += String .fromCharCode (v); return btoa (s); };
   const cases = [ ];

   for (const size of sizes)
   {
      const [ dims, at ] = size .split ("@");
      const [ w, h ] = dims .split ("x") .map (Number);
      const data = new Uint8Array (w * h * 4);

      for (let y = 0; y < h; ++ y) for (let x = 0; x < w; ++ x)
      {
         if (at)
         {
            const [ ax, ay ] = at .split (",") .map (Number), i = (y * w + x) * 4;
            data [i] = data [i + 1] = data [i + 2] = x === ax && y === ay ? 255 : 0; data [i + 3] = 255;
            continue;
         }

         let s = (Math .imul (x, 73856093) ^ Math .imul (y, 19349663) ^ Math .imul (w, 83492791)) >>> 0;
         s = (Math .imul (s, 1103515245) + 12345) & 0x7fffffff;
         const i = (y * w + x) * 4;
         data [i] = (s >> 3) & 255; data [i + 1] = (s >> 11) & 255; data [i + 2] = (s >> 19) & 255; data [i + 3] = 255;
      }

      const tex = gl .createTexture ();
      gl .bindTexture (gl .TEXTURE_2D, tex);
      gl .texImage2D (gl .TEXTURE_2D, 0, gl .RGBA8, w, h, 0, gl .RGBA, gl .UNSIGNED_BYTE, data);
      gl .generateMipmap (gl .TEXTURE_2D);

      const fb = gl .createFramebuffer ();
      gl .bindFramebuffer (gl .FRAMEBUFFER, fb);
      const levels = [ ];

      for (let l = 0, lw = w, lh = h; ; ++ l)
      {
         gl .framebufferTexture2D (gl .FRAMEBUFFER, gl .COLOR_ATTACHMENT0, gl .TEXTURE_2D, tex, l);
         const px = new Uint8Array (lw * lh * 4);
         gl .readPixels (0, 0, lw, lh, gl .RGBA, gl .UNSIGNED_BYTE, px);
         levels .push ({ w: lw, h: lh, rgba: b64 (px) });
         if (lw === 1 && lh === 1) break;
         lw = Math .max (1, lw >> 1); lh = Math .max (1, lh >> 1);
      }

      cases .push ({ w, h, at, levels });
   }

   return { renderer: ext ? gl .getParameter (ext .UNMASKED_RENDERER_WEBGL) : "", cases };
}, sizes);

await browser .close ();
writeFileSync (resolve (out), JSON .stringify (result));
console .log (`${result .renderer}: ${result .cases .length} textures -> ${out}`);
