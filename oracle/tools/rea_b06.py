#!/usr/bin/env python3
"""REA batch 06 RigidBodyPhysics oracle: pinned X_ITE 16.5.0 (headless Chrome) against x3d-drive.

  python3 -I oracle/tools/rea_b06.py baseline [NAME ...] [--jobs N]
      oracle/synthetic-rea-b06/NAME.x3dv (+ scripts/NAME.json, written by gen_rea_b06.py) ->
      oracle/rea-batch06-baseline/NAME/js.json, from oracle/x_ite-anim.mjs. A baseline is written only from an
      X_ITE run. Every world is run TWICE and the two results are compared in full; the baseline is the first
      run, and when the runs differ the differing paths are printed and stored under "nondeterministic".
  python3 -I oracle/tools/rea_b06.py check [NAME ...] [--target DIR] [--jobs N] [--json OUT] [--verbose]
      x3d-drive (DIR/x3d-drive, default /tmp/fp-rel/release) against the baseline, with the baseline's
      warmupFrames (--warmup-from). Compares:
        * scene text at every dumped frame (and the Inline scene texts) : exact, any difference FAILS
        * "physics" (see gen_rea_b06.py): physics.dt and every number of physics.frames, per quantity
          (TYPE.field, e.g. RigidBody.position; the x/y/z of a vector all count for the same quantity):
          max absolute error per case and quantity is printed; a quantity above TOLERANCES FAILS
        * non-numeric physics values (isActive, DEF names, list lengths, ...) : exact, any difference FAILS
        * the X_ITE console / log : reported ("console differs"), does not fail (FAIL_ON_CONSOLE)
      x3d-drive has to write the same "physics" object as the baseline (script key "physics").

Only worlds whose script has a "physics" key are RigidBodyPhysics cases (the batch 06 particle worlds share the
directory).
"""
import json, os, shutil, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
worlds_dir = os.path.join(root, "oracle", "synthetic-rea-b06")
baseline_dir = os.path.join(root, "oracle", "rea-batch06-baseline")

# Maximum absolute error allowed per quantity ("TYPE.field", "dt", "TYPE.contacts.position", ...). All start at 0:
# nothing has been measured against the Rust side yet. A quantity not listed uses DEFAULT_TOLERANCE.
TOLERANCES = {
    # "RigidBody.position": 0.0,
}
DEFAULT_TOLERANCE = 0.0
FAIL_ON_CONSOLE = False

args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
names = [a for i, a in enumerate(args[1:], 1) if not a.startswith("--") and args[i - 1] not in ("--target", "--jobs", "--json")]
jobs = int(opt("--jobs", "6"))


def paths(name):
    return os.path.join(worlds_dir, name + ".x3dv"), os.path.join(worlds_dir, "scripts", name + ".json")


def all_cases():
    out = []
    sdir = os.path.join(worlds_dir, "scripts")
    for f in sorted(os.listdir(sdir)):
        if f.endswith(".json") and "physics" in json.load(open(os.path.join(sdir, f))):
            out.append(f[:-5])
    return out


names = names or all_cases()


def run_xite(name, tag):
    world, script = paths(name)
    tmp = f"/tmp/rea-b06-baseline/{name}/{tag}"
    shutil.rmtree(tmp, ignore_errors=True)
    os.makedirs(tmp)
    out = os.path.join(tmp, "js.json")
    r = subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world, script, "--out", out], cwd=root, capture_output=True, text=True, timeout=600)
    if not os.path.exists(out):
        return None, "x_ite-anim failed: " + (r.stderr or r.stdout)[-400:]
    js = json.load(open(out))
    if not js.get("ok"):
        return None, "X_ITE run not ok: " + str(js.get("error"))
    return js, None


def diff_paths(a, b, path="", out=None, limit=20):
    out = [] if out is None else out
    if len(out) >= limit:
        return out
    if type(a) != type(b):
        out.append(path or "/")
    elif isinstance(a, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a or k not in b:
                out.append(f"{path}/{k}")
            else:
                diff_paths(a[k], b[k], f"{path}/{k}", out, limit)
    elif isinstance(a, list):
        if len(a) != len(b):
            out.append(f"{path}[len {len(a)} vs {len(b)}]")
        else:
            for i, (x, y) in enumerate(zip(a, b)):
                diff_paths(x, y, f"{path}[{i}]", out, limit)
    elif a != b:
        out.append(path)
    return out[:limit]


def baseline(name):
    js1, err = run_xite(name, "run1")
    if err:
        return name, False, err
    js2, err = run_xite(name, "run2")
    if err:
        return name, False, "second run: " + err
    diffs = diff_paths(js1, js2)
    if diffs:
        js1["nondeterministic"] = diffs
    dest = os.path.join(baseline_dir, name)
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest)
    json.dump(js1, open(os.path.join(dest, "js.json"), "w"), indent=1)
    msg = "baseline written, 2 runs identical" if not diffs else "baseline written, RUNS DIFFER at " + ", ".join(diffs[:6])
    return name, not diffs, msg


def leaves(value, key, out, path=""):
    """Collect (quantity key, path, leaf) for every leaf. Numbers under a list of numbers share the key."""
    if isinstance(value, dict):
        for k, v in value.items():
            leaves(v, f"{key}.{k}" if key else k, out, f"{path}/{k}")
    elif isinstance(value, list):
        for i, v in enumerate(value):
            leaves(v, key, out, f"{path}[{i}]")
    else:
        out.append((key, path, value))


def isnum(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool)


def compare_physics(js, rs):
    """-> (max error per quantity, [(quantity, path)] exact mismatches, shape message or None)"""
    a, b = js.get("physics"), rs.get("physics")
    if a is None:
        return {}, [], "baseline has no physics"
    if b is None:
        return {}, [], "x3d-drive result has no physics"
    errs, bad = {}, []
    # dt log
    la, lb = a.get("dt", []), b.get("dt", [])
    if len(la) != len(lb):
        bad.append(("dt", f"count {len(la)} vs {len(lb)}"))
    for i, (x, y) in enumerate(zip(la, lb)):
        if x.get("frame") != y.get("frame"):
            bad.append(("dt.frame", f"[{i}] {x.get('frame')} vs {y.get('frame')}"))
        e = abs(x["dt"] - y["dt"])
        errs["dt"] = max(errs.get("dt", 0.0), e)
    # frames
    fa, fb = a.get("frames", {}), b.get("frames", {})
    for k in sorted(set(fa) | set(fb), key=int):
        if k not in fa or k not in fb:
            bad.append(("frames", f"frame {k} missing on one side"))
            continue
        for p in sorted(set(fa[k]) | set(fb[k])):
            if p not in fa[k] or p not in fb[k]:
                bad.append(("frames", f"frame {k} {p} missing on one side"))
                continue
            na, nb = fa[k][p], fb[k][p]
            if na is None or nb is None:
                if na != nb:
                    bad.append((p, f"frame {k}: node {'absent' if na is None else 'present'} vs {'absent' if nb is None else 'present'}"))
                continue
            typ = na.get("type", "?")
            la_, lb_ = [], []
            leaves({f: v for f, v in na.items() if f != "type"}, typ, la_, f"frame {k} {p}")
            leaves({f: v for f, v in nb.items() if f != "type"}, typ, lb_, f"frame {k} {p}")
            if na.get("type") != nb.get("type"):
                bad.append((p, f"frame {k}: type {na.get('type')} vs {nb.get('type')}"))
            if [(q, pa) for q, pa, _ in la_] != [(q, pa) for q, pa, _ in lb_]:
                bad.append((typ, f"frame {k} {p}: value structure differs ({len(la_)} vs {len(lb_)} leaves)"))
                continue
            for (q, pa, x), (_, _, y) in zip(la_, lb_):
                if isnum(x) and isnum(y):
                    e = abs(x - y)
                    if e != e:  # NaN on one side
                        bad.append((q, f"{pa}: {x!r} vs {y!r}"))
                        continue
                    errs[q] = max(e, errs.get(q, 0.0))
                elif x != y:
                    bad.append((q, f"{pa}: {x!r} vs {y!r}"))
    return errs, bad, None


def tolerance(q):
    return TOLERANCES.get(q, DEFAULT_TOLERANCE)


def check(name):
    world, script = paths(name)
    base = os.path.join(baseline_dir, name)
    js_path = os.path.join(base, "js.json")
    if not os.path.exists(js_path):
        return name, "FAIL", {"error": "no baseline"}
    js = json.load(open(js_path))
    out = f"/tmp/rea-b06-check/{name}/rs.json"
    os.makedirs(os.path.dirname(out), exist_ok=True)
    if os.path.exists(out):
        os.remove(out)
    target = opt("--target", "/tmp/fp-rel/release")
    exe = os.path.join(target, "x3d-drive")
    if not os.path.exists(exe):
        return name, "FAIL", {"error": f"no {exe}"}
    r = subprocess.run([exe, world, script, "--oracle", os.path.join(root, "oracle"), "--out", out, "--warmup-from", js_path], cwd=root, capture_output=True, text=True, timeout=600)
    if r.returncode != 0 or not os.path.exists(out):
        return name, "FAIL", {"error": "x3d-drive failed: " + (r.stderr or r.stdout)[-300:]}
    rs = json.load(open(out))
    if rs.get("ok") is False:
        return name, "FAIL", {"error": "x3d-drive run not ok: " + str(rs.get("error"))}
    errs, bad, shape = compare_physics(js, rs)
    text_diff = sorted(k for k in set(js.get("vrml", {})) | set(rs.get("vrml", {})) if js.get("vrml", {}).get(k) != rs.get("vrml", {}).get(k))
    inl_diff = sorted(k for k in set(js.get("inlines", {})) | set(rs.get("inlines", {})) if js.get("inlines", {}).get(k) != rs.get("inlines", {}).get(k))
    over = {q: e for q, e in errs.items() if e > tolerance(q)}
    console_equal = js.get("console") == rs.get("console") and js.get("log") == rs.get("log")
    info = {"text_diff_frames": text_diff, "inline_diff_frames": inl_diff, "errs": errs, "over": over, "exact_mismatch": bad[:8], "n_exact_mismatch": len(bad),
            "shape": shape, "console_equal": console_equal, "warmup_equal": js.get("warmupFrames") == rs.get("warmupFrames")}
    ok = not text_diff and not inl_diff and not over and not bad and not shape and (console_equal or not FAIL_ON_CONSOLE)
    return name, "PASS" if ok else "FAIL", info


def show(name, status, info):
    if "error" in info:
        print(f"{status:4} {name}: {info['error']}")
        return
    flags = []
    if info["text_diff_frames"]:
        flags.append(f"scene text differs at frames {info['text_diff_frames'][:6]}")
    if info["inline_diff_frames"]:
        flags.append(f"inline text differs at frames {info['inline_diff_frames'][:6]}")
    if info["shape"]:
        flags.append(info["shape"])
    if info["n_exact_mismatch"]:
        flags.append(f"{info['n_exact_mismatch']} exact mismatches, first: {info['exact_mismatch'][0]}")
    if not info["console_equal"]:
        flags.append("console differs")
    print(f"{status:4} {name}" + ("  [" + "; ".join(flags) + "]" if flags else ""))
    errs = info["errs"]
    shown = sorted(errs.items(), key=lambda kv: -kv[1])
    if not opt("--verbose") and status == "PASS":
        shown = shown[:0]
    for q, e in (shown if opt("--verbose") or status != "PASS" else shown[:0]):
        mark = " > tolerance %g" % tolerance(q) if q in info["over"] else ""
        print(f"       {q:42} max abs err {e:.6e}{mark}")


if args[:1] == ["baseline"]:
    with ThreadPoolExecutor(jobs) as ex:
        results = list(ex.map(baseline, names))
    for n, ok, msg in results:
        print(("OK   " if ok else "FAIL ") + n + ": " + msg)
    bad = [n for n, ok, _ in results if not ok]
    print(f"baselines: {len(results) - len(bad)} of {len(results)} written and identical across two runs")
    sys.exit(1 if bad else 0)
elif args[:1] == ["check"]:
    with ThreadPoolExecutor(jobs) as ex:
        results = list(ex.map(check, names))
    counts = {"PASS": 0, "FAIL": 0}
    for n, status, info in results:
        counts[status] += 1
        show(n, status, info)
    # per-quantity maximum over all cases
    allq = {}
    for _, _, info in results:
        for q, e in info.get("errs", {}).items():
            allq[q] = max(allq.get(q, 0.0), e)
    if allq:
        print("max abs error per quantity over all cases (tolerance default %g):" % DEFAULT_TOLERANCE)
        for q, e in sorted(allq.items()):
            print(f"  {q:44} {e:.6e}")
    if opt("--json"):
        json.dump({n: {"status": s, **i} for n, s, i in results}, open(opt("--json"), "w"), indent=1)
    print(f"suite rea-b06 (RigidBodyPhysics): {counts['PASS']} pass, {counts['FAIL']} fail, of {len(results)} worlds")
    sys.exit(1 if counts["FAIL"] else 0)
else:
    print(__doc__)
    sys.exit(2)
