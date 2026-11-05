// `?backend=webgl2` or `?backend=webgpu` chooses the graphics backend; without it the engine chooses (WebGPU when it can
// start, else WebGL2). index.html runs this module before ../x_ite_r.mjs, so the <x3d-canvas> has the attribute before
// the engine reads it (when its WebAssembly module has loaded).
const backend = new URLSearchParams (location .search) .get ("backend");

if (backend)
   document .getElementById ("canvas") .setAttribute ("backend", backend);
