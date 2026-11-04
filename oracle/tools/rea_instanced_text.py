#!/usr/bin/env python3
"""Text under InstancedShape baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU), three runs per case and MSAA.

  python3 -I -B oracle/tools/rea_instanced_text.py record [NAME ...]
      X_ITE side only (worlds and scripts from oracle/synthetic-instanced-text, made by gen_instanced_text.py; no NAME means
      every world). Each case is recorded at MSAA 0 and at MSAA 4, three times each. The three js.json (without `loadFrames`, the frame
      the load finished in, which follows Chrome's I/O) and every PNG must be identical, or the case is not written. The first run
      is kept as oracle/instanced-text-baseline/NAME-msaaN.json (one file in one directory) and its pictures as
      oracle/instanced-text-baseline/png/NAME-msaaN/FRAME.png. A baseline is never taken from Rust. The SHA-256 of every run is printed.
"""
import filecmp, hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
assert sys.argv[1] == "record"
src = os.path.join(root, "oracle", "synthetic-instanced-text")
names = sys.argv[2:] or sorted(f[:-4] for f in os.listdir(src) if f.endswith(".wrl"))
dest_dir = os.path.join(root, "oracle", "instanced-text-baseline")
os.makedirs(dest_dir, exist_ok=True)
tmp = "/tmp/instanced-text-record"
for name in names:
    for msaa in ("0", "4"):
        runs, outs = [], []
        for i in (1, 2, 3):
            out = f"{tmp}/{name}-msaa{msaa}-{i}"
            shutil.rmtree(out, ignore_errors=True)
            os.makedirs(out)
            anim = "x_ite-anim.mjs" if msaa == "0" else "x_ite-anim-msaa.mjs"
            cmd = ["node", os.path.join(root, "oracle", anim), f"oracle/synthetic-instanced-text/{name}.wrl",
                   f"oracle/synthetic-instanced-text/{name}.json", "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "js")]
            if msaa != "0":
                cmd += ["--ms", msaa]
            subprocess.run(cmd, check=True, cwd=root, timeout=300)
            d = {k: v for k, v in json.load(open(os.path.join(out, "js.json"))).items() if k != "loadFrames"}
            runs.append(d)
            outs.append(out)
            print(f"{name} MSAA {msaa} run {i}: sha256 {hashlib.sha256(json.dumps(d, sort_keys=True).encode()).hexdigest()}")
        pngs = sorted(os.listdir(os.path.join(outs[0], "js")))
        same = (all(r == runs[0] for r in runs[1:])
                and all(pngs == sorted(os.listdir(os.path.join(o, "js"))) and all(filecmp.cmp(os.path.join(outs[0], "js", f), os.path.join(o, "js", f), shallow=False) for f in pngs) for o in outs[1:]))
        print(f"{name} MSAA {msaa}: three X_ITE runs {'identical' if same else 'DIFFER'} ({len(pngs)} pictures)")
        if not same:
            sys.exit(1)
        if not runs[0].get("ok") or runs[0].get("log"):
            print(f"{name} MSAA {msaa}: the world failed or logged an error: {runs[0].get('log')}")
            sys.exit(1)
        json.dump(runs[0], open(os.path.join(dest_dir, f"{name}-msaa{msaa}.json"), "w"), indent=1, sort_keys=True)
        pdir = os.path.join(dest_dir, "png", f"{name}-msaa{msaa}")
        shutil.rmtree(pdir, ignore_errors=True)
        shutil.copytree(os.path.join(outs[0], "js"), pdir)
shutil.rmtree(tmp, ignore_errors=True)
