/**
 * Writes the include files of the pinned X_ITE's ShaderRegistry (`#include <Name>` in a ShaderPart,
 * ShaderCompiler.process) to crates/x3d-runtime/src/nodes/shader_includes/NN-Name.glsl, NN being the
 * source file number X_ITE gives the file (its position in the registry, from 1).
 *
 *   node oracle/tools/gen_shader_includes.mjs
 */
import { writeFile, mkdir, rm } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join (dirname (fileURLToPath (import.meta.url)), "../..");
const { default: ShaderRegistry } = await import (join (root, "oracle/x_ite-pin/src/x_ite/Browser/Shaders/ShaderRegistry.js"));
const out = join (root, "crates/x3d-runtime/src/nodes/shader_includes");

await rm (out, { recursive: true, force: true });
await mkdir (out, { recursive: true });

const names = Object .getOwnPropertyNames (ShaderRegistry .includes [2]);
const list = [];

for (const [i, name] of names .entries ())
{
   const file = `${String (i + 1) .padStart (2, "0")}-${name}.glsl`;
   await writeFile (join (out, file), ShaderRegistry .includes [2] [name] ());
   list .push (file);
}

await writeFile (join (out, "list.txt"), list .join ("\n") + "\n");
console .log (`${names .length} include files`);
