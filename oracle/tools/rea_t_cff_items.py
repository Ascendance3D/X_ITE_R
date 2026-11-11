#!/usr/bin/env python3
"""Font-level records of the CFF items that were UNVERIFIED (oracle/synthetic-rea-t-cff-items, crates/x3d-text/tests/rea_t_cff_items.rs).

  python3 -I oracle/tools/rea_t_cff_items.py record
      oracle/x_ite-cff-cid-fonts.mjs (FONT_DIR / FONT_CHARS select the font set) runs three times against the pinned X_ITE 16.5.0 (opentype.js 2.0.0);
      the three results must be identical, the first is oracle/rea-t-cff-items-baseline/fonts.json and the sha256 of each run is printed. The fonts
      are those of oracle/tools/gen_cff_items_fonts.py (run it first). A baseline is never taken from Rust. Needs the ignored link
      oracle/x_ite-pin/node_modules (see oracle/README.md) and XITE_TESTS for playwright where it is not the sibling.
"""
import concurrent.futures, hashlib, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
assert sys.argv[1:2] == ["record"]
tmp = f"/tmp/rea-t-cff-items-record-{os.getpid()}"
os.makedirs(tmp, exist_ok=True)
env = dict(os.environ, FONT_DIR="/synthetic-rea-t-cff-items", FONT_CHARS="ABCDEFGHIOVW ")


def run(i):
    out = os.path.join(tmp, f"fonts-{i}.json")
    subprocess.run(["node", os.path.join(root, "oracle", "x_ite-cff-cid-fonts.mjs"), out], check=True, cwd=root, env=env, stdout=subprocess.DEVNULL)
    return open(out).read()


with concurrent.futures.ThreadPoolExecutor(3) as pool:
    texts = list(pool.map(run, (1, 2, 3)))

print("fonts: three X_ITE runs", "identical" if len(set(texts)) == 1 else "DIFFER", " ".join(hashlib.sha256(t.encode()).hexdigest()[:16] for t in texts))
ok = len(set(texts)) == 1

if ok:
    os.makedirs(os.path.join(root, "oracle", "rea-t-cff-items-baseline"), exist_ok=True)
    open(os.path.join(root, "oracle", "rea-t-cff-items-baseline", "fonts.json"), "w").write(texts[0])

shutil.rmtree(tmp, ignore_errors=True)
sys.exit(0 if ok else 1)
