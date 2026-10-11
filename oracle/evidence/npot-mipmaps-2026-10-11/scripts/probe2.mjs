// Random RGBA textures of many sizes: upload, generateMipmap, read every level.
import { createServer } from "node:http";
import { writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
const RP = "/home/ryan/Projects/cybertown/X_ITE/scratch/rust-port";
const { chromium } = createRequire (RP + "/../../x_ite-tests/") ("playwright");
const sizes = JSON.parse(process.argv[2]);
const html = `<canvas id=c></canvas><script>
window.run = (sizes) => {
  const gl = document.getElementById("c").getContext("webgl2"); const out = [];
  let seed = 12345; const rnd = () => (seed = (seed * 1103515245 + 12345) >>> 0) >>> 24;
  for (const [W,H] of sizes) {
    const src = new Uint8Array(W*H*4); for (let i=0;i<src.length;i++) src[i] = (i%4==3) ? 255 : rnd();
    const t = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D, t);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, W, H, 0, gl.RGBA, gl.UNSIGNED_BYTE, src);
    gl.generateMipmap(gl.TEXTURE_2D);
    const fb = gl.createFramebuffer(); gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    let w=W,h=H,l=0; const lv=[];
    for (;;) { gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, t, l);
      const d = new Uint8Array(w*h*4); gl.readPixels(0,0,w,h,gl.RGBA,gl.UNSIGNED_BYTE,d); lv.push(Array.from(d));
      if (w==1&&h==1) break; w=Math.max(1,w>>1); h=Math.max(1,h>>1); l++; }
    out.push({W,H,lv}); gl.deleteTexture(t); gl.deleteFramebuffer(fb);
  }
  return out; };</script>`;
const server = createServer ((q, r) => (r .setHeader ("content-type", "text/html"), r .end (html)));
await new Promise (r => server .listen (0, "127.0.0.1", r));
const browser = await chromium .launch ({ executablePath: "/usr/bin/google-chrome", headless: true,
  args: [ "--headless=new", "--no-sandbox", "--use-gl=angle", "--use-angle=gl", "--enable-gpu", "--ignore-gpu-blocklist" ] });
const page = await browser .newPage ();
await page .goto (`http://127.0.0.1:${server .address () .port}/`);
await writeFile ("synth.json", JSON.stringify (await page .evaluate (s => window .run (s), sizes)));
await browser .close (); server .close ();
