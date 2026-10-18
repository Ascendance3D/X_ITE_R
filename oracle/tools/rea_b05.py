#!/usr/bin/env python3
"""REA batch 05 Sound oracle: pinned X_ITE (headless Chrome, offline audio) against x3d-drive.

  python3 -I oracle/tools/rea_b05.py baseline [NAME ...]
      oracle/synthetic-rea-b05/NAME.wrl -> oracle/rea-batch05-baseline/NAME/{js.json, js.audio.f32}
      (oracle/x_ite-anim.mjs). A baseline is written only from an X_ITE run.
  python3 -I oracle/tools/rea_b05.py check [NAME ...] [--target DIR] [--jobs N] [--json OUT]
      x3d-drive (DIR/x3d-drive, default /tmp/fp-rel/release) against the baseline, with the
      baseline's warmupFrames. Checks: scene text at every dumped frame, the X_ITE log, the
      Script print lines, the suspend frames, and every audio sample.

Audio tolerance: 0. Every compared world matched sample for sample when the tolerance was set
(REA batch 05 report), so any difference fails. KNOWN lists measured differences that stay open;
they are reported as KNOWN, with their numbers, and do not fail the run.
"""
import array, json, math, os, shutil, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
worlds_dir = os.path.join(root, "oracle", "synthetic-rea-b05")
baseline_dir = os.path.join(root, "oracle", "rea-batch05-baseline")

# Open differences (not approved as DIFFERENT): name -> reason.
KNOWN = {
    "spatial-hrtf": "PannerNode HRTF: web-audio-api's HRIR set, not Chromium's IRCAM Composite data (not ported)",
}

args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
names = [a for i, a in enumerate(args[1:], 1) if not a.startswith("--") and args[i - 1] not in ("--target", "--jobs", "--json")]
names = names or sorted(f[:-4] for f in os.listdir(worlds_dir) if f.endswith(".wrl"))
jobs = int(opt("--jobs", "8"))


def paths(name):
    return os.path.join(worlds_dir, name + ".wrl"), os.path.join(worlds_dir, "scripts", name + ".json")


def load_audio(result, base):
    a = result.get("audio")
    if not a:
        return None
    path = a["file"] if os.path.isabs(a["file"]) else os.path.join(base, a["file"])
    x = array.array("f")
    x.frombytes(open(path, "rb").read())
    n = a["length"]
    return [x[c * n:(c + 1) * n] for c in range(a["channels"])]


def prints(result):
    return [l for l in result.get("console", []) if l.startswith("log: ")]


def baseline(name):
    world, script = paths(name)
    tmp = f"/tmp/rea-b05-baseline/{name}"
    shutil.rmtree(tmp, ignore_errors=True)
    os.makedirs(tmp)
    out = os.path.join(tmp, "js.json")
    r = subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world, script, "--out", out], cwd=root, capture_output=True, text=True, timeout=300)
    if r.returncode != 0 or not os.path.exists(out):
        return name, False, "x_ite-anim failed: " + (r.stderr or r.stdout)[-400:]
    js = json.load(open(out))
    if not js.get("ok"):
        return name, False, "X_ITE run not ok: " + str(js.get("error"))
    dest = os.path.join(baseline_dir, name)
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest)
    if js.get("audio"):
        shutil.copy(js["audio"]["file"], os.path.join(dest, "js.audio.f32"))
        js["audio"]["file"] = "js.audio.f32"
    json.dump(js, open(os.path.join(dest, "js.json"), "w"), indent=1)
    return name, True, "baseline written"


def check(name):
    world, script = paths(name)
    base = os.path.join(baseline_dir, name)
    js_path = os.path.join(base, "js.json")
    if not os.path.exists(js_path):
        return name, "FAIL", {"error": "no baseline"}
    js = json.load(open(js_path))
    out = f"/tmp/rea-b05-check/{name}/rs.json"
    os.makedirs(os.path.dirname(out), exist_ok=True)
    target = opt("--target", "/tmp/fp-rel/release")
    r = subprocess.run([os.path.join(target, "x3d-drive"), world, script, "--oracle", os.path.join(root, "oracle"), "--out", out, "--warmup-from", js_path], cwd=root, capture_output=True, text=True, timeout=300)
    if r.returncode != 0 or not os.path.exists(out):
        return name, "FAIL", {"error": "x3d-drive failed: " + r.stderr[-400:]}
    rs = json.load(open(out))
    info = {
        "text_equal": js.get("vrml") == rs.get("vrml"),
        "log_equal": js.get("log") == rs.get("log"),
        "prints_equal": prints(js) == prints(rs),
        "prints": len(prints(js)),
        "suspend_equal": js.get("audioSuspend") == rs.get("audioSuspend"),
    }
    A, B = load_audio(js, base), load_audio(rs, os.path.dirname(out))
    info["audio_shape_equal"] = A is not None and B is not None and len(A) == len(B) and all(len(a) == len(b) for a, b in zip(A, B))
    channels = []
    if info["audio_shape_equal"]:
        for a, b in zip(A, B):
            d = [abs(p - q) for p, q in zip(a, b)]
            m = max(d)
            channels.append({"peak": max(abs(p) for p in a), "max_abs_diff": m, "rms_diff": math.sqrt(sum(x * x for x in d) / len(d)), "first_diff": next((i for i, x in enumerate(d) if x), None)})
    info["channels"] = channels
    exact = info["audio_shape_equal"] and all(c["max_abs_diff"] == 0 for c in channels)
    others = all(info[k] for k in ("text_equal", "log_equal", "prints_equal", "suspend_equal"))
    status = "PASS" if exact and others else ("KNOWN" if name in KNOWN and others and info["audio_shape_equal"] else "FAIL")
    return name, status, info


if args[:1] == ["baseline"]:
    with ThreadPoolExecutor(jobs) as ex:
        results = list(ex.map(baseline, names))
    bad = [n for n, ok, _ in results if not ok]
    for n, ok, msg in results:
        print(("OK   " if ok else "FAIL ") + n + ": " + msg)
    print(f"baselines: {len(results) - len(bad)} of {len(results)} written")
    sys.exit(1 if bad else 0)
elif args[:1] == ["check"]:
    with ThreadPoolExecutor(jobs) as ex:
        results = list(ex.map(check, names))
    counts = {"PASS": 0, "KNOWN": 0, "FAIL": 0}
    for n, status, info in results:
        counts[status] += 1
        ch = info.get("channels") or []
        audio = " ".join(f"ch{i} peak {c['peak']:.4f} max {c['max_abs_diff']:.3e}" for i, c in enumerate(ch))
        flags = " ".join(k for k in ("text_equal", "log_equal", "prints_equal", "suspend_equal", "audio_shape_equal") if info.get(k) is False)
        print(f"{status:5} {n}: {audio}" + (f" [not equal: {flags}]" if flags else "") + (f" ({KNOWN[n]})" if status == "KNOWN" else "") + (f" {info['error']}" if "error" in info else ""))
    if opt("--json"):
        json.dump({n: {"status": s, **i} for n, s, i in results}, open(opt("--json"), "w"), indent=1)
    print(f"suite rea-b05: {counts['PASS']} pass, {counts['KNOWN']} known difference, {counts['FAIL']} fail, of {len(results)} worlds")
    sys.exit(1 if counts["FAIL"] else 0)
else:
    print(__doc__)
    sys.exit(2)
