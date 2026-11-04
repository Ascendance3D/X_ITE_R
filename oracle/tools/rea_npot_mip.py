#!/usr/bin/env python3
"""NPOT mipmap baselines (issue #51, sub:render.core): the pinned X_ITE 16.5.0 pictures of oracle/synthetic-npot-mip.

  XITE_TESTS=~/Projects/cybertown/X_ITE/x_ite-tests python3 -I -B oracle/tools/rea_npot_mip.py record

For MSAA 0 and MSAA 4 runs `node oracle/x_ite-shot-msaa.mjs oracle/synthetic-npot-mip OUT --ms N` three times (headless
Chrome, ANGLE on the GPU; `--ms 0` gives the page of x_ite-shot.mjs, byte for byte the same pictures). The three runs must
give the same pixels (the PNG bytes and the rgbaSha256 of every world). The first run is kept as
oracle/render-npot-mip-baseline/msaa0 and .../msaa4. A baseline is never taken from Rust.
"""
import filecmp, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))


def main():
    assert sys.argv[1:2] == ["record"]
    for ms in (0, 4):
        runs = []
        for i in (1, 2, 3):
            out = f"/tmp/npot-mip-record/msaa{ms}-{i}"
            shutil.rmtree(out, ignore_errors=True)
            os.makedirs(os.path.dirname(out), exist_ok=True)
            subprocess.run(["node", os.path.join(root, "oracle", "x_ite-shot-msaa.mjs"),
                            os.path.join(root, "oracle", "synthetic-npot-mip"), out, "--ms", str(ms), "--jobs", "3"],
                           check=True, cwd=root, timeout=300, stdout=subprocess.DEVNULL)
            runs.append(out)
        a = runs[0]
        pngs = sorted(f for f in os.listdir(a) if f.endswith(".png"))
        hashes = lambda d: {r["file"]: r["rgbaSha256"] for r in json.load(open(os.path.join(d, "index.json")))["results"]}
        same = bool(pngs) and all(
            sorted(f for f in os.listdir(b) if f.endswith(".png")) == pngs
            and all(filecmp.cmp(os.path.join(a, f), os.path.join(b, f), shallow=False) for f in pngs)
            and hashes(a) == hashes(b) for b in runs[1:])
        print(f"MSAA {ms}: three X_ITE runs {'identical' if same else 'DIFFER'} ({len(pngs)} images)")
        if not same:
            sys.exit(1)
        for f, h in sorted(hashes(a).items()):
            print(f"  {f} rgbaSha256 {h[:16]}")
        dest = os.path.join(root, "oracle", "render-npot-mip-baseline", f"msaa{ms}")
        shutil.rmtree(dest, ignore_errors=True)
        shutil.copytree(a, dest)


if __name__ == "__main__":
    main()
