#!/usr/bin/env python3
"""FileLoader HTTP oracle baselines (redirects, statuses, content types, headers, the text/html hand-off) (REA ledger entry
sub:io.file-loader; pinned X_ITE in headless Chrome, no rendering).

  python3 -I oracle/tools/file_http.py record [GROUP ...]
      records each group (fh-redirect, fh-status, fh-diff, fh-foreign; default all) THREE times with oracle/x_ite-anim.mjs, requires
      the three results to be identical (every key except `warmupFrames` and `loadFrames`, see oracle/tools/file_loader.py), and
      writes the first as oracle/file-http-baseline/GROUP.json.
  python3 -I oracle/tools/file_http.py xite GROUP
      one X_ITE run into /tmp/file-http/GROUP.json.

The route table and the worlds are made by oracle/tools/gen_file_http.py (oracle/synthetic-rea-file-http). The recorder serves the table on
two local ports (script key `httpRoutes`) and records the hand-off to `foreign` (script key `foreign`). Needs the ignored
oracle/x_ite-pin/node_modules link and XITE_TESTS (see oracle/README.md, Recording from a worktree). A baseline is written only from X_ITE
runs; the Rust result is compared (crates/x3d-viewer/tests/file_http.rs), never copied.
"""
import hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
GROUPS = ["fh-redirect", "fh-status", "fh-diff", "fh-foreign"]
world = lambda g: f"oracle/synthetic-rea-file-http/{g}.wrl"
script = lambda g: f"oracle/synthetic-rea-file-http/{'fh-foreign' if g == 'fh-foreign' else 'fh'}.json"
tmp = "/tmp/file-http"


def run(group, out):
    os.makedirs(os.path.dirname(out), exist_ok=True)
    subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world(group), script(group), "--out", out], check=True, cwd=root, timeout=300)
    return open(out, "rb").read()


mode, args = sys.argv[1], sys.argv[2:]

if mode == "xite":
    run(args[0], f"{tmp}/{args[0]}.json")
elif mode == "record":
    dest = os.path.join(root, "oracle", "file-http-baseline")
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
