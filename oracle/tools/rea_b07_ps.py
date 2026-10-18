#!/usr/bin/env python3
"""REA batch 07 (particles part) oracle: ForcePhysicsModel, WindPhysicsModel and PolylineEmitter in pinned X_ITE
(headless Chrome, transform-feedback readback) against the Rust port's particle driver. Adapted from
rea_b06_particles.py (the 06 tool is unchanged); read its docstring for the record format and the tolerances.

  python3 -I oracle/tools/gen_rea_b07_ps.py                       # (re)write the worlds and scripts
  python3 -I oracle/tools/rea_b07_ps.py baseline [NAME ...] [--jobs N]
      oracle/synthetic-rea-b07-ps/NAME.x3dv + scripts/NAME.json  ->  oracle/rea-batch07-ps-baseline/NAME/
      (oracle/tools/x_ite_particles_b07.mjs). Every case is run twice with the same Math.random seed; the bit
      identity of the two runs (steps, forceDraws, particle floats) is recorded under "determinism". A baseline is
      written only from an X_ITE run, never from the Rust driver.
  python3 -I oracle/tools/rea_b07_ps.py check [NAME ...] [--driver CMD] [--json OUT] [--jobs N]
      Runs the Rust driver (default /tmp/b07-ps-target/release/x3d-particles-drive) with the baseline's recorded
      Math.random stream (per-frame randomSeed and the draws of the Wind models) and compares per particle,
      per frame; also per step (numParticles, createParticles, isActive, maxParticles, numForces), the
      polylines texture, the scene text and the console log.

Every case is exact, because everything X_ITE randomizes is Math.random and the baseline records it. The `polyline-big`
and `wind-big` cases (20000 particles) are exact too; for `polyline-big` the report adds the distribution of the
born particles over the three segments against the length proportion (chi-square, a sanity check on the baseline
itself and on the port).

Driver contract: <driver> <world.x3dv> <script.json> --seeds-from <baseline js.json> --out <rs.json>.
Tolerances are the 06R ones (TOL below), measured as abs + rel * |reference|.
"""
import json, math, os, shutil, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

import numpy as np

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
tools_dir = os.path.join(root, "oracle", "tools")
worlds_dir = os.path.join(root, "oracle", "synthetic-rea-b07-ps")
scripts_dir = os.path.join(worlds_dir, "scripts")
baseline_dir = os.path.join(root, "oracle", "rea-batch07-ps-baseline")

import importlib.util
_spec = importlib.util.spec_from_file_location("gen_rea_b07_ps", os.path.join(tools_dir, "gen_rea_b07_ps.py"))
G = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(G)

STRIDE = 28
GROUPS = {"life": [0], "lifetime": [1], "elapsed": [2], "color": [4, 5, 6, 7], "velocity": [8, 9, 10], "position": [24, 25, 26, 27]}
TOL = {"life": (0.0, 0.0), "lifetime": (0.0, 0.0), "elapsed": (0.0, 0.0), "color": (0.0, 0.0), "velocity": (3e-6, 1e-6), "position": (5e-6, 2e-6)}
# Explicitly relaxed velocity / position tolerance of the cases generated with tol="strong" (a Wind force of several units
# per frame): the GPU's turbulence normal (sin / cos / sqrt of the shader on the RTX) deviates from plain float32 by
# about 2e-7 relative per step, which the 06R tolerance (measured on speeds <= 4) does not cover once the force is
# larger than ~1 per frame. Measured and reasoned in REPORT.md; the Rust step itself equals a plain float32 emulation.
TOL_STRONG = {"velocity": (1e-4, 2e-5), "position": (1e-4, 2e-5)}
TOL_STEP = {"deltaTime": (0.0, 1e-12), "creationTime": (1e-9, 0.0)}
CHI2_CRIT_DOF2 = 18.42   # alpha 1e-4

KNOWN = {}

args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
names = [a for i, a in enumerate(args[1:], 1) if not a.startswith("--") and args[i - 1] not in ("--driver", "--jobs", "--json", "--out")]
jobs = int(opt("--jobs", "8"))


def all_names():
    return sorted(f[:-5] for f in os.listdir(scripts_dir) if f.startswith("ps-") and f.endswith(".json"))


def load_run(json_path):
    js = json.load(open(json_path))
    f32 = os.path.join(os.path.dirname(json_path), js["fileF32"]) if js.get("fileF32") else None
    data = np.fromfile(f32, dtype="<f4") if f32 and os.path.exists(f32) else np.zeros(0, "<f4")
    dumps = {}
    for k, d in js.get("dumps", {}).items():
        n = d["numParticles"]
        dumps[int(k)] = data[d["offset"]:d["offset"] + n * STRIDE].reshape(n, STRIDE)
    return js, dumps


def run_xite(name, out_json):
    r = subprocess.run(["node", os.path.join(tools_dir, "x_ite_particles_b07.mjs"), os.path.join(worlds_dir, name + ".x3dv"), os.path.join(scripts_dir, name + ".json"), "--out", out_json],
                       cwd=root, capture_output=True, text=True, timeout=900)
    if r.returncode != 0 or not os.path.exists(out_json):
        raise RuntimeError("x_ite_particles_b07 failed: " + (r.stderr or r.stdout)[-400:])
    return load_run(out_json)


def baseline(name):
    try:
        tmp = "/tmp/rea-b07-ps-baseline/" + name
        shutil.rmtree(tmp, ignore_errors=True); os.makedirs(tmp)
        ja, da = run_xite(name, tmp + "/a.json")
        jb, db = run_xite(name, tmp + "/b.json")
        fa, fb = open(tmp + "/a.f32", "rb").read(), open(tmp + "/b.f32", "rb").read()
        strip = lambda j: {k: v for k, v in j.items() if k not in ("fileF32",)}
        same_json = strip(ja) == strip(jb)
        det = {"runs": 2, "same_Math_random_seed": ja["seed"], "bit_identical_f32": fa == fb, "identical_json_incl_steps_forceDraws_geometry_scene_text": same_json, "f32_bytes": len(fa)}
        if ja.get("log") and any(l.startswith("frame ") or l.startswith("animate called") for l in ja["log"]):
            raise RuntimeError("harness log reports a nondeterminism source: %s" % ja["log"][:3])
        dest = os.path.join(baseline_dir, name)
        shutil.rmtree(dest, ignore_errors=True); os.makedirs(dest)
        out = strip(ja)
        out["determinism"] = det
        out["world"] = "oracle/synthetic-rea-b07-ps/%s.x3dv" % name
        shutil.copy(tmp + "/a.f32", os.path.join(dest, "js.f32"))
        out["fileF32"] = "js.f32"
        json.dump(out, open(os.path.join(dest, "js.json"), "w"), indent=1)
        ok = det["bit_identical_f32"] and same_json
        return name, ok, "baseline written; two runs " + ("bit-identical" if ok else "DIFFER %s" % det)
    except Exception as ex:
        return name, False, "error: %s" % ex


def cmp_group(ref, got, cols, tol):
    a = ref[:, cols].astype("f8"); b = got[:, cols].astype("f8")
    nan_a, nan_b = np.isnan(a), np.isnan(b)
    nan_mismatch = int((nan_a != nan_b).sum())
    ok = ~(nan_a | nan_b)
    d = np.abs(a - b)[ok]
    allow = (tol[0] + tol[1] * np.abs(a))[ok]
    return {"max_abs": float(d.max()) if d.size else 0.0, "n_fail": int((d - allow > 0).sum()) + nan_mismatch, "nan_mismatch": nan_mismatch}


def check_polyline(base, rs, fails, info):
    bp = base.get("geometry", {}).get("polyline")
    rp = rs.get("geometry", {}).get("polyline")
    if bp is None and rp is None:
        return
    if (bp is None) != (rp is None):
        fails.append("polylines texture: %s in X_ITE, %s in Rust" % ("present" if bp else "absent", "present" if rp else "absent"))
        return
    if bp["verticesIndex"] != rp["verticesIndex"]:
        fails.append("polylines verticesIndex %s vs %s" % (bp["verticesIndex"], rp["verticesIndex"]))
        return
    n = len(bp["array"])
    a = np.array(bp["array"], "f8"); b = np.array(rp["array"][:n], "f8")
    if len(b) < n:
        fails.append("polylines texture: %d floats in Rust, X_ITE sampled %d" % (len(rp["array"]), n))
        return
    # X_ITE reuses its Float32Array between rebuilds without clearing it, so the unused y z w channels of the length
    # texels (texels 0 .. verticesIndex - 1; the shader reads only .x there) can hold stale vertex data: compare x only.
    mask = np.ones((n // 4, 4), bool)
    mask[:bp["verticesIndex"], 1:] = False
    d = float(np.abs(a - b).reshape(-1, 4)[mask].max()) if n else 0.0
    info["polyline"] = {"verticesIndex": bp["verticesIndex"], "floats": n, "max_abs": d}
    if d > 0:
        fails.append("polylines texture differs, max abs %.3g" % d)


def check_exact(name, base, bd, rs, rd):
    info = {"steps": {}, "groups": {}}
    tol = dict(TOL)
    if G.CASES[name[len("ps-"):]].get("tol") == "strong":
        tol.update(TOL_STRONG)
        info["tolerance"] = "strong (relaxed velocity / position, see TOL_STRONG)"
    fails = []
    bs, rs_ = base["steps"], rs.get("steps", [])
    if len(bs) != len(rs_):
        fails.append("steps: %d vs %d" % (len(bs), len(rs_)))
    for key in ("numParticles", "isActive", "maxParticles", "numForces"):
        bad = [s["frame"] for s, r in zip(bs, rs_) if key in s and s[key] != r.get(key)]
        info["steps"][key + "_bad_frames"] = bad[:5]
        if bad:
            fails.append("%s differs at frames %s" % (key, bad[:5]))
    bad_cp = [s["frame"] for s, r in zip(bs, rs_) if bool(s["createParticles"]) != bool(r["createParticles"])]
    if bad_cp: fails.append("createParticles differs at frames %s" % bad_cp[:5])
    bad_fd = [s["frame"] for s, r in zip(bs, rs_) if len(s.get("forceDraws", [])) != r.get("forceDrawsUsed")]
    if bad_fd: fails.append("force draws used differ at frames %s" % bad_fd[:5])
    # the force texture itself (CPU doubles rounded to float32 by both sides): exact, null = non-finite
    bad_f = [s["frame"] for s, r in zip(bs, rs_) if s.get("forces") is not None and [None if x is None else float(np.float32(x)) for x in s["forces"]] != [None if x is None else float(np.float32(x)) for x in r.get("forces", [])]]
    info["steps"]["forces_bad_frames"] = bad_f[:5]
    if bad_f: fails.append("force texture differs at frames %s" % bad_f[:5])
    info["steps"]["forceDraws_total"] = sum(len(s.get("forceDraws", [])) for s in bs)
    dt_err = max((abs(s["deltaTime"] - r["deltaTime"]) / max(abs(s["deltaTime"]), 1e-300) for s, r in zip(bs, rs_)), default=0.0)
    ct_err = max((abs(s["creationTime"] - r["creationTime"]) for s, r in zip(bs, rs_)), default=0.0)
    info["steps"]["deltaTime_max_rel"] = dt_err; info["steps"]["creationTime_max_abs"] = ct_err
    if dt_err > TOL_STEP["deltaTime"][1]: fails.append("deltaTime rel err %.3g" % dt_err)
    if ct_err > TOL_STEP["creationTime"][0]: fails.append("creationTime abs err %.3g" % ct_err)
    for g in GROUPS:
        info["groups"][g] = {"max_abs": 0.0, "n_fail": 0, "frames_bad": []}
    for k in sorted(bd):
        if k not in rd or len(rd[k]) != len(bd[k]):
            fails.append("dump %d: %s records vs %d" % (k, len(rd[k]) if k in rd else "no", len(bd[k])))
            continue
        for g, cols in GROUPS.items():
            r = cmp_group(bd[k], rd[k], cols, tol[g])
            gi = info["groups"][g]
            gi["max_abs"] = max(gi["max_abs"], r["max_abs"]); gi["n_fail"] += r["n_fail"]
            if r["n_fail"] and len(gi["frames_bad"]) < 5:
                gi["frames_bad"].append(k)
    for g, gi in info["groups"].items():
        if gi["n_fail"]:
            fails.append("%s: %d values over tolerance (max abs %.3g, first bad frames %s)" % (g, gi["n_fail"], gi["max_abs"], gi["frames_bad"]))
    check_polyline(base, rs, fails, info)
    if base.get("vrmlLoad") != rs.get("vrmlLoad"):
        fails.append("scene text after load differs")
    if base.get("vrmlEnd") != rs.get("vrmlEnd"):
        fails.append("scene text at the end differs")
    if sorted(base.get("console", [])) != sorted(rs.get("console", [])):
        fails.append("console log differs: X_ITE %s, Rust %s" % (base.get("console", [])[:3], rs.get("console", [])[:3]))
    info["console_lines"] = len(base.get("console", []))
    return fails, info


def segment_shares(base, dumps):
    """Born particles (elapsed 0, visible) of the polyline-big case: the share per segment against the length proportion."""
    poly = base["geometry"]["polyline"]
    nv = int(poly["array"].__len__() / 4 - poly["verticesIndex"])
    arr = np.array(poly["array"], "f8").reshape(-1, 4)
    v = arr[poly["verticesIndex"]:poly["verticesIndex"] + nv, :3]
    seg = [(v[2 * i], v[2 * i + 1]) for i in range(nv // 2)]
    length = np.array([np.linalg.norm(b - a) for a, b in seg])
    counts = np.zeros(len(seg))
    for d in dumps.values():
        born = d[(d[:, 2] == 0) & ~np.isnan(d[:, 24])]
        p = born[:, 24:27].astype("f8")
        dist = []
        for a, b in seg:
            t = np.clip(((p - a) @ (b - a)) / ((b - a) @ (b - a)), 0, 1)
            dist.append(np.linalg.norm(p - (a + t[:, None] * (b - a)), axis=1))
        counts += np.bincount(np.argmin(np.array(dist), axis=0), minlength=len(seg))
    exp = length / length.sum() * counts.sum()
    chi2 = float(((counts - exp) ** 2 / exp).sum())
    return {"segment_lengths": length.tolist(), "observed": counts.tolist(), "expected": exp.tolist(), "chi2": chi2, "dof": len(seg) - 1, "crit_alpha_1e-4": CHI2_CRIT_DOF2 if len(seg) == 3 else None}


def run_driver(name, outdir):
    os.makedirs(outdir, exist_ok=True)
    drv = (opt("--driver") or "/tmp/b07-ps-target/release/x3d-particles-drive").split()
    out = os.path.join(outdir, "rs.json")
    if os.path.exists(out):
        os.remove(out)
    r = subprocess.run(drv + [os.path.join(worlds_dir, name + ".x3dv"), os.path.join(scripts_dir, name + ".json"), "--seeds-from", os.path.join(baseline_dir, name, "js.json"), "--out", out],
                       cwd=root, capture_output=True, text=True, timeout=900)
    if r.returncode != 0 or not os.path.exists(out):
        raise RuntimeError("driver failed: " + (r.stderr or r.stdout or "no output")[-300:])
    return out


def check(name):
    try:
        bpath = os.path.join(baseline_dir, name, "js.json")
        if not os.path.exists(bpath):
            return name, "FAIL", {"error": "no baseline"}, ["no baseline"]
        base, bd = load_run(bpath)
        rs, rd = load_run(run_driver(name, "/tmp/rea-b07-ps-check/" + name))
        fails, info = check_exact(name, base, bd, rs, rd)
        if name == "ps-polyline-big":
            sb, sr = segment_shares(base, bd), segment_shares(base, rd)
            info["segment_chi2"] = {"xite": sb["chi2"], "rust": sr["chi2"], "observed_xite": sb["observed"], "observed_rust": sr["observed"], "expected": sb["expected"]}
            for who, v in (("X_ITE", sb["chi2"]), ("Rust", sr["chi2"])):
                if v > CHI2_CRIT_DOF2:
                    fails.append("segment shares of %s: chi2 %.2f > %.2f" % (who, v, CHI2_CRIT_DOF2))
        status = "PASS" if not fails else ("KNOWN" if name in KNOWN else "FAIL")
        return name, status, info, fails
    except Exception as ex:
        return name, "FAIL", {"error": str(ex)}, [str(ex)]


def report(results):
    counts = {"PASS": 0, "KNOWN": 0, "FAIL": 0}
    for n, status, info, fails in results:
        counts[status] += 1
        if "groups" in info:
            line = " ".join("%s %.1e" % (g, gi["max_abs"]) for g, gi in info["groups"].items())
            line += " | draws %d" % info["steps"]["forceDraws_total"]
            if "polyline" in info: line += " | polylines %d floats max %.1e" % (info["polyline"]["floats"], info["polyline"]["max_abs"])
            if "segment_chi2" in info: line += " | chi2 xite %.2f rust %.2f" % (info["segment_chi2"]["xite"], info["segment_chi2"]["rust"])
        else:
            line = info.get("error", "")
        print("%-5s %-34s %s" % (status, n, line))
        for f in fails[:6]:
            print("        - " + f)
    print("suite rea-b07-ps: %d pass, %d known, %d fail, of %d cases" % (counts["PASS"], counts["KNOWN"], counts["FAIL"], len(results)))
    return counts


if __name__ == "__main__":
    cmd = args[:1]
    sel = names or all_names()
    if cmd == ["baseline"]:
        with ThreadPoolExecutor(jobs) as ex:
            results = list(ex.map(baseline, sel))
        for n, ok, msg in results:
            print(("OK   " if ok else "FAIL ") + n + ": " + msg)
        bad = [n for n, ok, _ in results if not ok]
        print("baselines: %d of %d written, %d with non-identical repeat runs or errors" % (len(results) - sum(1 for _, _, m in results if m.startswith("error")), len(results), len(bad)))
        sys.exit(1 if bad else 0)
    elif cmd == ["check"]:
        with ThreadPoolExecutor(jobs) as ex:
            results = list(ex.map(check, sel))
        counts = report(results)
        if opt("--json"):
            json.dump({n: {"status": s, "fails": f, **i} for n, s, i, f in results}, open(opt("--json"), "w"), indent=1, default=str)
        sys.exit(1 if counts["FAIL"] else 0)
    else:
        print(__doc__)
        sys.exit(2)
