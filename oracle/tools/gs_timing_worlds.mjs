// Writes the static twins of two states of the camera path of oracle/x_ite-splat-sort-timing.mjs (same cloud, the
// Transform rotation of that step set at load): node oracle/tools/gs_timing_worlds.mjs <out_dir>
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { checkOutput } from "../check-output.mjs";

const out = process .argv [2];
if (!out) throw new Error ("usage: node oracle/tools/gs_timing_worlds.mjs <out_dir>");
checkOutput (resolve (out));
mkdirSync (out, { recursive: true });

function world (n, rotation)
{
   let s = 12345, r = () => (s = (s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff;
   const pos = [], ori = [], sc = [], op = [], sh = [];
   for (let i = 0; i < n; ++i) { pos .push (`${((r () - .5) * 3) .toFixed (3)} ${((r () - .5) * 3) .toFixed (3)} ${((r () - .5) * 3) .toFixed (3)}`); ori .push ("0 0 0 1"); const q = (0.05 + r () * 0.15) .toFixed (3); sc .push (`${q} ${q} ${q}`); op .push ((0.5 + r () * 0.5) .toFixed (2)); sh .push (`${((r () - .5) * 3) .toFixed (2)} ${((r () - .5) * 3) .toFixed (2)} ${((r () - .5) * 3) .toFixed (2)}`); }
   return `#X3D V4.1 utf8\nPROFILE Interchange\nCOMPONENT GaussianSplats:1\nViewpoint { position 0 0 6 }\nBackground { skyColor 0.2 0.2 0.28 }\nDEF T Transform { rotation ${rotation} children GaussianSplats { positions [ ${pos .join (", ")} ] orientations [ ${ori .join (", ")} ] scales [ ${sc .join (", ")} ] opacities [ ${op .join (", ")} ] sphericalHarmonicsDegree0Coef0 [ ${sh .join (", ")} ] } }\n`;
}

writeFileSync (join (out, "gs-w441-flipY.wrl"), world (441, `0 1 0 ${Math .PI}`));
writeFileSync (join (out, "gs-w3969-turnX.wrl"), world (3969, `1 0 0 ${Math .PI / 2}`));
