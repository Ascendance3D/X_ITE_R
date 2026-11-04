#!/usr/bin/env python3
"""glTF / GLB fetch-failure oracle baselines (REA ledger entries sub:parse.gltf2 and sub:parse.glb2; pinned X_ITE in headless Chrome, no rendering).

  python3 -I oracle/tools/gltf_fetch.py record [GROUP ...]
      records each group (gf-buffer, gf-image, gf-video; default all) THREE times with oracle/x_ite-anim.mjs, requires the
      three results to be identical, and writes the first as oracle/gltf-fetch-baseline/GROUP.json. Identical means every
      key of the result except `warmupFrames` and `loadFrames`: the number of real frames the pinned page needs before the scripted
      frames start depends on the wall clock (10 or 11 in repeated runs of one world), is printed here, and x3d-drive takes it
      from the baseline (`--warmup-from`); the scene text of every dumped frame does not depend on it.
  python3 -I oracle/tools/gltf_fetch.py xite GROUP
      one X_ITE run into /tmp/gltf-fetch/GROUP.json.

The inputs and worlds are made by oracle/tools/gen_gltf_fetch.py (oracle/synthetic-rea-gltf-fetch). Needs the ignored
oracle/x_ite-pin/node_modules link and XITE_TESTS (see oracle/README.md, Recording from a worktree). A baseline is written only
from X_ITE runs; the Rust result is compared (crates/x3d-viewer/tests/gltf_fetch.rs), never copied.
"""
import hashlib, json, os, re, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
GROUPS = ["gf-buffer", "gf-image", "gf-video"]
world = lambda g: f"oracle/synthetic-rea-gltf-fetch/{g}.wrl"
script = "oracle/synthetic-rea-gltf-fetch/gf.json"
tmp = "/tmp/gltf-fetch"


def run(group, out):
    os.makedirs(os.path.dirname(out), exist_ok=True)
    subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world(group), script, "--out", out], check=True, cwd=root, timeout=120)
    return open(out, "rb").read()


mode, args = sys.argv[1], sys.argv[2:]

if mode == "xite":
    run(args[0], f"{tmp}/{args[0]}.json")
elif mode == "record":
    dest = os.path.join(root, "oracle", "gltf-fetch-baseline")
    os.makedirs(dest, exist_ok=True)
    for group in args or GROUPS:
        runs = [run(group, f"{tmp}/{i}/{group}.json") for i in (1, 2, 3)]
        parsed = [json.loads(r) for r in runs]
        # The page names a decoded blob by a random UUID in its error text; the three results are compared with it replaced.
        uuid = lambda t: re.sub(r"blob:http://oracle/[0-9a-f-]+", "blob:http://oracle/UUID", t)
        stable = [uuid(json.dumps({k: v for k, v in p.items() if k not in ("warmupFrames", "loadFrames")}, sort_keys=True)) for p in parsed]
        sums = [hashlib.sha256(x.encode()).hexdigest() for x in stable]
        frames = ",".join(f"{p['warmupFrames']}/{p['loadFrames']}" for p in parsed)
        print(group, " ".join(s[:16] for s in sums), "identical" if len(set(sums)) == 1 else "DIFFERENT", "warmup/load frames", frames)
        if len(set(sums)) != 1:
            sys.exit(f"{group}: the three X_ITE recordings differ")
        open(os.path.join(dest, group + ".json"), "wb").write(runs[0])
    shutil.rmtree(tmp, ignore_errors=True)
