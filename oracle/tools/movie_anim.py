#!/usr/bin/env python3
"""MovieTexture GIF / APNG playback baselines (issue #90; pinned X_ITE 16.5.0, headless Chrome on the GPU).

  python3 -I oracle/tools/movie_anim.py record [RUNS]
      Runs x_ite-anim.mjs (MSAA 0) and x_ite-anim-msaa.mjs (MSAA 4) RUNS times (default 5) on each case of
      CASES into /tmp/movie-anim-xite/runN/NAME-msaaM. The baseline oracle/movie-anim-baseline/NAME-msaaM
      (js.json, js/*.png) is written from run 1 only when every run gave the same js.json and the same PNG
      bytes; recordings.json keeps the sha256 of each run's files. A difference between runs writes nothing
      and exits 1.

The worlds and scripts come from oracle/tools/gen_movie_anim.py. A baseline is written only from an X_ITE
run; the Rust result is compared (crates/x3d-viewer/tests/movie_anim.rs), never copied.
"""
import hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "movie-anim-baseline")
CASES = [(n, f"oracle/synthetic-movie-anim/{n}.wrl", f"oracle/synthetic-movie-anim/{n}.json") for n in ("movie-anim",)]
MSAA = ["0", "4"]


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def files(case_dir):
    """sha256 of js.json and of every PNG, by name relative to the case directory."""
    out = {"js.json": sha(os.path.join(case_dir, "js.json"))}
    for f in sorted(os.listdir(os.path.join(case_dir, "js"))):
        out["js/" + f] = sha(os.path.join(case_dir, "js", f))
    return out


def record(runs):
    tmp = "/tmp/movie-anim-xite"
    shutil.rmtree(tmp, ignore_errors=True)
    report, same = {}, True

    for name, world, script in CASES:
        for msaa in MSAA:
            key = f"{name}-msaa{msaa}"
            hashes = []
            for run in range(1, runs + 1):
                d = os.path.join(tmp, f"run{run}", key)
                os.makedirs(d)
                anim = "x_ite-anim.mjs" if msaa == "0" else "x_ite-anim-msaa.mjs"
                cmd = ["node", os.path.join(root, "oracle", anim), world, script,
                       "--out", os.path.join(d, "js.json"), "--png-dir", os.path.join(d, "js")]
                if msaa != "0":
                    cmd += ["--ms", msaa]
                subprocess.run(cmd, check=True, cwd=root, timeout=300)
                hashes.append(files(d))
            agree = all(h == hashes[0] for h in hashes)
            same &= agree
            report[key] = {"runs": hashes, "identical": agree}
            print(f"{key}: {runs} runs, {'identical' if agree else 'DIFFERENT'}")

    if not same:
        json.dump(report, open(os.path.join(tmp, "recordings.json"), "w"), indent=1)
        sys.exit(f"runs differ; nothing written (see {tmp}/recordings.json)")

    os.makedirs(out_dir, exist_ok=True)
    lines = []
    for name, world, script in CASES:
        for msaa in MSAA:
            key = f"{name}-msaa{msaa}"
            dst = os.path.join(out_dir, key)
            if os.path.exists(dst):
                sys.exit(f"{dst} exists; a new recording goes to a new baseline directory")
            shutil.copytree(os.path.join(tmp, "run1", key), dst)
            lines.append(f"{name} {world} {script} {msaa}\n")
    open(os.path.join(out_dir, "cases.txt"), "w").writelines(lines)
    json.dump(report, open(os.path.join(out_dir, "recordings.json"), "w"), indent=1)


args = sys.argv[1:]
if args and args[0] == "record":
    record(int(args[1]) if len(args) > 1 else 5)
else:
    sys.exit(__doc__)
