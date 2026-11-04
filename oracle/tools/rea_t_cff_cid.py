#!/usr/bin/env python3
"""CID-keyed CFF and CFF2 font cases (oracle/synthetic-rea-t-cff-cid, crates/x3d-viewer/tests/rea_t_cff_cid.rs, crates/x3d-text/tests/rea_t_cff_cid.rs).

  python3 -I oracle/tools/rea_t_cff_cid.py record
      fonts.json: oracle/x_ite-cff-cid-fonts.mjs runs three times against the pinned X_ITE 16.5.0 (opentype.js 2.0.0); the three results must be identical.
      Every case of oracle/rea-t-cff-cid-baseline/cases.txt runs through oracle/x_ite-anim.mjs three times; the recorded view (scene text at the dumped
      frame, poses, log, ok, warmupFrames) must be identical in all three. The first run is kept as oracle/rea-t-cff-cid-baseline/<case>.json and the
      sha256 of each run's view is printed.
      Every case of known.txt (a world the pinned X_ITE never finishes loading) runs three times with a 60 s limit; the baseline <case>.json records
      the three timeouts (`{"timeout": 60, "runs": 3}`) and is written only if all three time out. A timeout is no evidence of anything else.
      A baseline is never taken from Rust. Needs the ignored link oracle/x_ite-pin/node_modules (see oracle/README.md) and XITE_TESTS for playwright
      where it is not the sibling.
"""
import concurrent.futures, hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
assert sys.argv[1:2] == ["record"]
base = os.path.join(root, "oracle", "rea-t-cff-cid-baseline")
keys = ("vrml", "poses", "log", "ok", "warmupFrames")
tmp = f"/tmp/rea-t-cff-cid-record-{os.getpid()}"
os.makedirs(tmp, exist_ok=True)
bad = False


def lines(name):
    return [l.split() for l in open(os.path.join(base, name)).read().splitlines() if l.strip()]


def run_anim(name, world, script, i, limit):
    out = os.path.join(tmp, f"{name}-{i}.json")
    try:
        subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world, script, "--out", out], check=True, cwd=root,
                       timeout=limit, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except subprocess.TimeoutExpired:
        return None
    full = json.load(open(out))
    return {k: v for k, v in full.items() if k in keys}


def run_fonts(i):
    out = os.path.join(tmp, f"fonts-{i}.json")
    subprocess.run(["node", os.path.join(root, "oracle", "x_ite-cff-cid-fonts.mjs"), out], check=True, cwd=root, stdout=subprocess.DEVNULL)
    return open(out).read()


def digest(texts):
    h = [hashlib.sha256(t.encode()).hexdigest() for t in texts]
    return len(set(texts)) == 1, " ".join(x[:16] for x in h)


with concurrent.futures.ThreadPoolExecutor(6) as pool:
    texts = list(pool.map(run_fonts, (1, 2, 3)))
    same, h = digest(texts)
    print(f"fonts: three X_ITE runs {'identical' if same else 'DIFFER'} sha256 {h}")
    if same:
        open(os.path.join(base, "fonts.json"), "w").write(texts[0])
    else:
        bad = True

    for name, world, script in lines("cases.txt"):
        views = list(pool.map(lambda i: run_anim(name, world, script, i, 300), (1, 2, 3)))
        text = [json.dumps(v, sort_keys=True) for v in views]
        same, h = digest(text)
        print(f"{name}: three X_ITE runs {'identical' if same else 'DIFFER'} sha256 {h}")
        if not same:
            bad = True
            continue
        with open(os.path.join(base, name + ".json"), "w") as f:
            json.dump(views[0], f, sort_keys=True, indent=1)
            f.write("\n")

    for name, world, script in lines("known.txt"):
        views = list(pool.map(lambda i: run_anim(name, world, script, i, 60), (1, 2, 3)))
        print(f"{name}: timeouts {[v is None for v in views]}")
        if any(v is not None for v in views):
            bad = True
            continue
        with open(os.path.join(base, name + ".json"), "w") as f:
            json.dump({"timeout": 60, "runs": 3}, f, sort_keys=True, indent=1)
            f.write("\n")

shutil.rmtree(tmp, ignore_errors=True)
sys.exit(1 if bad else 0)
