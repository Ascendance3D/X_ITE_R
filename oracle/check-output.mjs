// Refuses a recorder output that could write protected evidence (issue #65): the external store, a key path
// `oracle/NAME`, or a link to either. The check is `tools/answer_keys.py check-output`; it fails closed, so a
// recorder that cannot run it writes nothing. Write a regenerated key to a staging directory (oracle/README.md).

import { spawnSync } from "node:child_process";
import { join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const TOOL = normalize (join (fileURLToPath (import.meta.url), "..", "..", "tools", "answer_keys.py"));

export function checkOutput (path)
{
   const r = spawnSync ("python3", [ "-I", "-B", TOOL, "check-output", resolve (path) ], { stdio: "inherit" });

   if (r .status !== 0)
   {
      console .error (`check-output: ${resolve (path)} refused or not checked${r .error ? ` (${r .error .message})` : ""}; nothing written`);
      process .exit (r .status || 1);
   }
}
