#!/usr/bin/env python3
"""FileLoader oracle baselines (text input, url resolution, diagnostics) (REA ledger entry sub:io.file-loader; pinned X_ITE in headless Chrome, no rendering).

  python3 -I oracle/tools/file_loader.py record [GROUP ...]
      records each group (ff-text, ff-url, ff-inline, ff-host, ff-inline-data, ff-host-nodes; default all) THREE times with oracle/x_ite-anim.mjs, requires the
      three results to be identical, and writes the first as oracle/file-loader-baseline/GROUP.json. Identical means every
      key of the result except `warmupFrames` and `loadFrames`: the number of real frames the pinned page needs before the scripted
      frames start depends on the wall clock (10 or 11 in repeated runs of one world), is printed here, and x3d-drive takes it
      from the baseline (`--warmup-from`); the scene text of every dumped frame does not depend on it.
  python3 -I oracle/tools/file_loader.py xite GROUP
      one X_ITE run into /tmp/file-loader/GROUP.json.

The inputs and worlds are made by oracle/tools/gen_file_loader.py (oracle/synthetic-rea-file-loader). Needs the ignored
oracle/x_ite-pin/node_modules link and XITE_TESTS (see oracle/README.md, Recording from a worktree). A baseline is written only
from X_ITE runs; the Rust result is compared (crates/x3d-viewer/tests/file_loader.rs), never copied.
"""
import hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
GROUPS = ["ff-text", "ff-url", "ff-inline", "ff-host", "ff-inline-data", "ff-host-nodes"]
world = lambda g: f"oracle/synthetic-rea-file-loader/{g}.wrl"
# a group with a script key of its own (`dumpInlines`) has a script file of its name
script = lambda g: f"oracle/synthetic-rea-file-loader/{g}.json" if os.path.exists(os.path.join(root, f"oracle/synthetic-rea-file-loader/{g}.json")) else "oracle/synthetic-rea-file-loader/ff.json"
tmp = os.environ.get("XITE_FILE_LOADER_TMP", "/tmp/file-loader")


def run(group, out):
    os.makedirs(os.path.dirname(out), exist_ok=True)
    subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world(group), script(group), "--out", out], check=True, cwd=root, timeout=300)
    return open(out, "rb").read()


mode, args = sys.argv[1], sys.argv[2:]

if mode == "xite":
    run(args[0], f"{tmp}/{args[0]}.json")
elif mode == "record":
    dest = os.path.join(root, "oracle", "file-loader-baseline")
    os.makedirs(dest, exist_ok=True)
    for group in args or GROUPS:
        runs = [run(group, f"{tmp}/{i}/{group}.json") for i in (1, 2, 3)]
        parsed = [json.loads(r) for r in runs]
        stable = [json.dumps({k: v for k, v in p.items() if k not in ("warmupFrames", "loadFrames")}, sort_keys=True) for p in parsed]
        sums = [hashlib.sha256(x.encode()).hexdigest() for x in stable]
        frames = ",".join(f"{p['warmupFrames']}/{p['loadFrames']}" for p in parsed)
        print(group, " ".join(s[:16] for s in sums), "identical" if len(set(sums)) == 1 else "DIFFERENT", "warmup/load frames", frames)
        if len(set(sums)) != 1:
            sys.exit(f"{group}: the three X_ITE recordings differ")
        open(os.path.join(dest, group + ".json"), "wb").write(runs[0])
    shutil.rmtree(tmp, ignore_errors=True)
