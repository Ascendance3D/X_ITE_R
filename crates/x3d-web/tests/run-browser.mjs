// Portable test discovery for `npm test`: Node finds the files, not the shell, so cmd.exe and sh behave the same
// (cmd.exe gave the quoted glob to node --test as a literal name: 0 tests, exit 0).
//
//   npm test                                    every tests/browser/*.test.mjs
//   npm test -- --test-name-pattern=Anchor      extra arguments go to node --test, before the file list
//
// Exit 1 when no test file is found, so a broken discovery never reports a pass.
import { readdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const dir   = fileURLToPath (new URL ("./browser/", import.meta.url));
// readdirSync, not fs.globSync (Node 22 and later), as tests/browser/shards.mjs.
const files = readdirSync (dir) .filter (f => f .endsWith (".test.mjs")) .sort () .map (f => dir + f);

if (files .length === 0)
{
   console .error (`run-browser: no test files found in ${dir}`);
   process .exit (1);
}

console .error (`run-browser: ${files .length} test files`);

const r = spawnSync (process .execPath, [ "--test", "--test-concurrency=1", ... process .argv .slice (2), ... files ], { stdio: "inherit" });

if (r .error)
   console .error (`run-browser: ${r .error}`);

process .exit (r .status ?? 1);
