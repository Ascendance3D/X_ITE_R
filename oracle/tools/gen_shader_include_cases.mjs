import { join } from "node:path";
const root = "/home/ryan/Projects/cybertown/.worktrees/rust-port-private/integration-b06-b08";
const { default: ShaderCompiler } = await import (join (root, "oracle/x_ite-pin/src/x_ite/Browser/Shaders/ShaderCompiler.js"));
const { default: ShaderRegistry } = await import (join (root, "oracle/x_ite-pin/src/x_ite/Browser/Shaders/ShaderRegistry.js"));
const c = new ShaderCompiler ({ getVersion: () => 2 });
const cases = [
  "void main () { }\n",
  "a\n#include <Utils>\nb\n",
  "#include <Nope>\nx",
  "  #include   <Fog>  \nmore\n#include <ClipPlanes>",
  "#include <a/b>\nz",
  "#include <>\nz",
  "#include<Perlin>\r\nq",
  "// #include <Utils>\nx",
  "#include <Utils> // c\nx",
];
console.log (JSON.stringify (cases.map (s => ({ in: s, out: c.process (s) }))));
