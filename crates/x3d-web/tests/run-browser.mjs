// Portable test discovery: no shell glob, so cmd.exe and sh behave the same. Exit 1 when no test file is found.
import { globSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
const dir = fileURLToPath(new URL("./browser/", import.meta.url));
const files = globSync("*.test.mjs", { cwd: dir }).sort().map(f => dir + f);
if (files.length === 0) { console.error("run-browser: no test files found in " + dir); process.exit(1); }
const r = spawnSync(process.execPath, ["--test", "--test-concurrency=1", ...files], { stdio: "inherit" });
process.exit(r.status ?? 1);
