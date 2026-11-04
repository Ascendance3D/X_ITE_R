#!/usr/bin/env python3
"""InstancedShape picking baseline (pinned X_ITE 16.5.0, headless Chrome), three runs.

  python3 -I -B oracle/tools/rea_instanced_pick.py record [NAME]
      X_ITE side only (NAME defaults to ip-sweep; world and script from oracle/synthetic-instanced-pick, made by
      gen_instanced_pick.py). The case is run three times; all three js.json (without `loadFrames`, the frame the load finished in,
      which follows Chrome's I/O) must be identical. The first is kept as oracle/instanced-pick-baseline/NAME.json, one file in one
      directory. A baseline is never taken from Rust. The SHA-256 of each run is printed.
"""
import hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
assert sys.argv[1] == "record"
name = sys.argv[2] if len(sys.argv) > 2 else "ip-sweep"
dest_dir = os.path.join(root, "oracle", "instanced-pick-baseline")
os.makedirs(dest_dir, exist_ok=True)
runs, hashes = [], []
for i in (1, 2, 3):
    out = f"/tmp/instanced-pick-record/{name}-{i}"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"),
                    f"oracle/synthetic-instanced-pick/{name}.wrl", f"oracle/synthetic-instanced-pick/{name}.json",
                    "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "js")], check=True, cwd=root, timeout=300)
    d = {k: v for k, v in json.load(open(os.path.join(out, "js.json"))).items() if k != "loadFrames"}
    runs.append(d)
    hashes.append(hashlib.sha256(json.dumps(d, sort_keys=True).encode()).hexdigest())
    print(f"run {i}: sha256 {hashes[-1]}")
same = all(r == runs[0] for r in runs[1:])
print(f"{name}: three X_ITE runs {'identical' if same else 'DIFFER'}")
if not same:
    sys.exit(1)
json.dump(runs[0], open(os.path.join(dest_dir, f"{name}.json"), "w"), indent=1, sort_keys=True)
shutil.rmtree("/tmp/instanced-pick-record", ignore_errors=True)
