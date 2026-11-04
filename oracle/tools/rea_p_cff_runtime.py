#!/usr/bin/env python3
"""End-to-end CFF font cases (oracle/synthetic-rea-p-cff-runtime, crates/x3d-viewer/tests/rea_p_cff_runtime.rs).

  python3 -I oracle/tools/rea_p_cff_runtime.py record
      Every case of oracle/rea-p-cff-runtime-baseline/cases.txt runs through the pinned X_ITE 16.5.0 (oracle/x_ite-anim.mjs) three times. The
      compared view (scene text at the dumped frame, poses, log, ok, warmupFrames) must be identical in all three; the first run is kept as
      oracle/rea-p-cff-runtime-baseline/<case>.json and the sha256 of each run's view is printed. A baseline is never taken from Rust.
      Needs the ignored link oracle/x_ite-pin/node_modules (see oracle/README.md) and XITE_TESTS for playwright where it is not the sibling.
"""
import hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
assert sys.argv[1:2] == ["record"]
base = os.path.join(root, "oracle", "rea-p-cff-runtime-baseline")
keys = ("vrml", "poses", "log", "ok", "warmupFrames")
cases = [l.split() for l in open(os.path.join(base, "cases.txt")).read().splitlines() if l.strip()]
tmp = f"/tmp/rea-p-cff-runtime-record-{os.getpid()}"
bad = False
for name, world, script in cases:
    views = []
    for i in (1, 2, 3):
        out = os.path.join(tmp, f"{name}-{i}.json")
        os.makedirs(tmp, exist_ok=True)
        subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world, script, "--out", out], check=True, cwd=root, timeout=300)
        full = json.load(open(out))
        views.append({k: v for k, v in full.items() if k in keys})
    text = [json.dumps(v, sort_keys=True) for v in views]
    hashes = [hashlib.sha256(t.encode()).hexdigest() for t in text]
    same = len(set(text)) == 1
    print(f"{name}: three X_ITE runs {'identical' if same else 'DIFFER'} sha256 {' '.join(h[:16] for h in hashes)}")
    if not same:
        bad = True
        continue
    with open(os.path.join(base, name + ".json"), "w") as f:
        json.dump(views[0], f, sort_keys=True, indent=1)
        f.write("\n")
shutil.rmtree(tmp, ignore_errors=True)
sys.exit(1 if bad else 0)
