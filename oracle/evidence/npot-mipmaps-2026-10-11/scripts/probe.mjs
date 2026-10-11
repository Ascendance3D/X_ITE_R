// Upload an image like X_ITE, generateMipmap, read back every level as raw RGBA.
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
const RP = "/home/ryan/Projects/cybertown/X_ITE/scratch/rust-port";
const { chromium } = createRequire (RP + "/../../x_ite-tests/") ("playwright");
const [ img, out ] = process .argv .slice (2);
const page_html = `<canvas id=c></canvas><script>
window.run = async () => {
  const im = new Image(); im.src = "/img"; await im.decode();
  const gl = document.getElementById("c").getContext("webgl2");
  const t = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D, t);
  gl.pixelStorei(gl.UNPACK_COLORSPACE_CONVERSION_WEBGL, gl.BROWSER_DEFAULT_WEBGL);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, im.width, im.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, im);
  gl.generateMipmap(gl.TEXTURE_2D);
  const fb = gl.createFramebuffer(); gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
  const levels = []; let w = im.width, h = im.height, l = 0;
  for (;;) {
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, t, l);
    const d = new Uint8Array(w*h*4); gl.readPixels(0,0,w,h,gl.RGBA,gl.UNSIGNED_BYTE,d);
    let s = ""; for (let i = 0; i < d.length; i += 32768) s += String.fromCharCode(...d.subarray(i, i+32768));
    levels.push({ w, h, b64: btoa(s) });
    if (w == 1 && h == 1) break; w = Math.max(1, w>>1); h = Math.max(1, h>>1); l++;
  }
  const dbg = gl.getExtension("WEBGL_debug_renderer_info");
  return { renderer: gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL), levels };
};</script>`;
const data = await readFile (img);
const server = createServer ((q, r) => q .url === "/img" ? r .end (data) : (r .setHeader ("content-type", "text/html"), r .end (page_html)));
await new Promise (r => server .listen (0, "127.0.0.1", r));
const browser = await chromium .launch ({ executablePath: "/usr/bin/google-chrome", headless: true,
  args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ] });
const page = await browser .newPage ();
await page .goto (`http://127.0.0.1:${server .address () .port}/`);
const r = await page .evaluate (() => window .run ());
console .log (r .renderer);
for (const [i, L] of r .levels .entries ()) await writeFile (`${out}.L${i}.${L.w}x${L.h}.rgba`, Buffer .from (L .b64, "base64"));
console .log (r .levels .map (L => `${L.w}x${L.h}`) .join (" "));
await browser .close (); server .close ();
