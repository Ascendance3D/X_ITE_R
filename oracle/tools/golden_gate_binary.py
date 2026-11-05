#!/usr/bin/env python3
"""GoldenGate binary dispatch oracle baselines (REA ledger entry sub:parse.golden-gate; pinned X_ITE in headless Chrome, no rendering).

  python3 -I oracle/tools/golden_gate_binary.py record [GROUP ...]
      records each group (gg-stl, gg-ply, gg-ply-splat, gg-glb, gg-dispatch; default all) THREE times with oracle/x_ite-anim.mjs, requires the
      three results to be identical, and writes the first as oracle/golden-gate-binary-baseline/GROUP.json. Identical means every
      key of the result except `warmupFrames` and `loadFrames`: the number of real frames the pinned page needs before the scripted
      frames start depends on the wall clock (10 or 11 in repeated runs of one world), is printed here, and x3d-drive takes it
      from the baseline (`--warmup-from`); the scene text of every dumped frame does not depend on it.
  python3 -I oracle/tools/golden_gate_binary.py xite GROUP
      one X_ITE run into /tmp/golden-gate-binary/GROUP.json.

The inputs and worlds are made by oracle/tools/gen_golden_gate_binary.py (oracle/synthetic-rea-golden-gate-binary). Needs the ignored
oracle/x_ite-pin/node_modules link and XITE_TESTS (see oracle/README.md, Recording from a worktree). A baseline is written only
from X_ITE runs; the Rust result is compared (crates/x3d-viewer/tests/golden_gate_binary.rs), never copied.
"""
import hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
GROUPS = ["gg-stl", "gg-ply", "gg-ply-splat", "gg-glb", "gg-dispatch"]
world = lambda g: f"oracle/synthetic-rea-golden-gate-binary/{g}.wrl"
script = "oracle/synthetic-rea-golden-gate-binary/gg.json"
tmp = "/tmp/golden-gate-binary"


def run(group, out):
    os.makedirs(os.path.dirname(out), exist_ok=True)
    # The splat group starts the GaussianSplats sort Worker, whose fetch Chromium reports twice (see x_ite-anim.mjs).
    env = dict(os.environ, **({"XITE_ANIM_DEDUP_PENDING": "1"} if group == "gg-ply-splat" else {}))
    subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world(group), script, "--out", out], check=True, cwd=root, timeout=120, env=env)
    return open(out, "rb").read()


mode, args = sys.argv[1], sys.argv[2:]

if mode == "xite":
    run(args[0], f"{tmp}/{args[0]}.json")
elif mode == "record":
    dest = os.path.join(root, "oracle", "golden-gate-binary-baseline")
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
