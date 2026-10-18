#!/usr/bin/env python3
"""REA batch 06R ParticleSystems oracle: pinned X_ITE (headless Chrome, transform-feedback readback)
against the Rust port's particle driver.

  python3 -I oracle/tools/rea_b06_particles.py baseline [NAME ...] [--jobs N]
      oracle/synthetic-rea-b06/NAME.x3dv + scripts/NAME.json  ->  oracle/rea-batch06-baseline/NAME/
      (oracle/tools/x_ite_particles.mjs). Every case is run twice with the same Math.random seed; the
      bit-identity of the two runs is recorded in js.json ("determinism"). Statistical cases (-stat)
      are run a third time with another seed to measure the null Kolmogorov-Smirnov distances. A
      baseline is written only from an X_ITE run.
  python3 -I oracle/tools/rea_b06_particles.py check [NAME ...] [--driver CMD | --rs-dir DIR] [--jobs N] [--json OUT]
      Runs the Rust driver (default /tmp/fp-rel/release/x3d-particles-drive, which does not exist yet)
      or reads DIR/NAME/rs.json, and compares it with the baseline: per-particle state for the exact
      cases, two-sample distribution tests for the -stat cases. Prints a per-quantity max error / KS
      statistic report and a PASS / FAIL per case.
  python3 -I oracle/tools/rea_b06_particles.py emulate [NAME ...] [--out DIR]
      A numpy float32 emulation of the pinned GLSL (X3DParticleEmitterNode.js and the emitter files)
      that writes DIR/NAME/rs.json in the driver format from a baseline's recorded seeds. It is the
      stand-in for the Rust driver: `emulate` then `check --rs-dir DIR` exercises the whole check path
      and measures how close an IEEE float32 implementation of the same math gets to the GPU.
  python3 -I oracle/tools/rea_b06_particles.py nullcheck [NAME ...]
      Runs X_ITE once more with a different Math.random seed and passes it through the statistical check
      (it must PASS: the false-positive test of the KS / category / mean thresholds).
  python3 -I oracle/tools/rea_b06_particles.py selftest
      Compares two fresh X_ITE runs of a few cases (the noise floor of the check).

Driver contract (what the Rust side must write)
-----------------------------------------------
    <driver> <world.x3dv> <script.json> --seeds-from <baseline js.json> --out <rs.json>

reads the same script.json x_ite_particles.mjs reads (dt, t0, frames, warmup, node, dump | dumpAll,
filter) and the per-frame GLSL seeds steps[k-1].randomSeed from the baseline, and writes rs.json plus
the sidecar file named by "fileF32" (little-endian float32), in the baseline's format:

  { "ok": true, "stride": 28, "fileF32": "rs.f32",
    "steps": [ { "frame": 1, "numParticles": 0, "deltaTime": 0.00222.., "creationTime": 946684800.0,
                 "createParticles": true }, ... one per frame 1..frames ],
    "dumps": { "<frame>": { "numParticles": n, "offset": <float index of the first record in the .f32> } } }

One record = 28 float32 in X_ITE's order (only the groups below are compared):
  [0] life  [1] lifetime  [2] elapsedTime  [3] texCoordIndex0 (-1)  [4..7] color rgba
  [8..10] velocity xyz (11 = 0)  [12..23] scale/rotation rows (any value)  [24..27] position xyzw
Hidden particles carry position (NaN, NaN, NaN, NaN), color 0, velocity 0. A dump lists the system's
particles 0 .. numParticles-1 in buffer order after that frame's animate; with "filter": "born" only the
records whose elapsedTime is exactly 0 (created that frame), in buffer order. The initial state is: no
particles, deltaTime 0, creationTime = t0 (0 for ExplosionEmitter), all-zero buffers.

Tolerances
----------
TOL gives, per quantity, abs + rel * |reference|. The X_ITE-to-X_ITE noise floor measured on this
machine is exactly 0 (two runs with the same Math.random seed are bit-identical; see "determinism" in
every js.json), so the exact cases could be held to 0. The values below are what an IEEE float32
re-implementation of the same GLSL reaches against the GPU (see `emulate`): integer-valued and
arithmetic-only quantities are held to 0, quantities that go through sin / cos / sqrt / pow / normalize
carry the GPU's transcendental error. KS_C is the two-sample Kolmogorov-Smirnov critical constant
(D_crit = KS_C * sqrt ((n + m) / (n m))); 2.5 is an alpha of about 1e-5 per test for independent samples; X_ITE's own births are a renewal process
(a slot is reborn after its previous lifetime), and two X_ITE runs with different seeds reach up to 1.07 x the
plain alpha = 0.0007 constant (2.0), so 2.5 leaves ~17% headroom over that measured null (see null_ks in stats.json).
"""
import array, importlib.util, json, math, os, shutil, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

import numpy as np

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
tools_dir = os.path.join(root, "oracle", "tools")
worlds_dir = os.path.join(root, "oracle", "synthetic-rea-b06")
scripts_dir = os.path.join(worlds_dir, "scripts")
baseline_dir = os.path.join(root, "oracle", "rea-batch06-baseline")

_spec = importlib.util.spec_from_file_location("gen_rea_b06_particles", os.path.join(tools_dir, "gen_rea_b06_particles.py"))
G = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(G)

STRIDE = 28
GROUPS = {"life": [0], "lifetime": [1], "elapsed": [2], "color": [4, 5, 6, 7], "velocity": [8, 9, 10], "position": [24, 25, 26, 27]}

# Per-quantity tolerance: abs + rel * |reference|, applied to every component of the group.
TOL = {
    "life":     (0.0, 0.0),
    "lifetime": (0.0, 0.0),
    "elapsed":  (0.0, 0.0),
    "color":    (0.0, 0.0),
    "velocity": (3e-6, 1e-6),     # measured float32-emulation vs GPU: max 2.4e-6 abs at |v| ~ 4
    "position": (5e-6, 2e-6),     # measured: max 4.2e-6 abs (bounded-box, 120 frames of accumulated velocity error)
}
TOL_STEP = {"deltaTime": (0.0, 1e-12), "creationTime": (1e-9, 0.0)}
KS_C = 2.5
CHI_SIGMA = 4.0
MEAN_SIGMA = 4.5
SAMPLE_CAP = 20000
COLS = ["frame", "lifetime", "r", "g", "b", "a", "vx", "vy", "vz", "px", "py", "pz", "pw"]

# Cases whose difference to X_ITE is understood and stays open: name -> reason (reported, not failed).
KNOWN = {}

args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
names = [a for i, a in enumerate(args[1:], 1) if not a.startswith("--") and args[i - 1] not in ("--driver", "--rs-dir", "--jobs", "--json", "--out")]
jobs = int(opt("--jobs", "8"))


def all_names():
    return sorted(f[:-5] for f in os.listdir(scripts_dir) if f.startswith("particles-") and f.endswith(".json"))


def case_of(name):
    return G.CASES[name[len("particles-"):]]


# ---------------------------------------------------------------- IO

def load_run(json_path):
    js = json.load(open(json_path))
    f32 = os.path.join(os.path.dirname(json_path), js["fileF32"]) if js.get("fileF32") else None
    data = np.fromfile(f32, dtype="<f4") if f32 and os.path.exists(f32) else np.zeros(0, "<f4")
    dumps = {}
    for k, d in js.get("dumps", {}).items():
        n = d["numParticles"]
        dumps[int(k)] = data[d["offset"]:d["offset"] + n * STRIDE].reshape(n, STRIDE)
    return js, dumps


def run_xite(name, out_json, seed=None):
    script = os.path.join(scripts_dir, name + ".json")
    if seed is not None:
        s = json.load(open(script))
        s["seed"] = seed
        script = out_json + ".script.json"
        json.dump(s, open(script, "w"))
    r = subprocess.run(["node", os.path.join(tools_dir, "x_ite_particles.mjs"), os.path.join(worlds_dir, name + ".x3dv"), script, "--out", out_json],
                       cwd=root, capture_output=True, text=True, timeout=600)
    if r.returncode != 0 or not os.path.exists(out_json):
        raise RuntimeError("x_ite_particles failed: " + (r.stderr or r.stdout)[-400:])
    return load_run(out_json)


def compact(dumps, cap=SAMPLE_CAP):
    """All dumped (born) records, as the COLS columns, subsampled evenly to at most cap rows."""
    rows = []
    for k in sorted(dumps):
        d = dumps[k]
        if len(d):
            rows.append(np.column_stack([np.full(len(d), k, "<f4"), d[:, 1], d[:, 4:8], d[:, 8:11], d[:, 24:28]]))
    if not rows:
        return np.zeros((0, len(COLS)), "<f4")
    a = np.concatenate(rows)
    if len(a) > cap:
        a = a[np.linspace(0, len(a) - 1, cap).astype(int)]
    return a.astype("<f4")


# ---------------------------------------------------------------- statistics

def ks2(a, b):
    a = np.sort(np.asarray(a, "f8")); b = np.sort(np.asarray(b, "f8"))
    if not len(a) or not len(b):
        return float("nan")
    allv = np.concatenate([a, b])
    return float(np.max(np.abs(np.searchsorted(a, allv, side="right") / len(a) - np.searchsorted(b, allv, side="right") / len(b))))


def ks_crit(n, m):
    return KS_C * math.sqrt((n + m) / (n * m))


def ks1(a, cdf):
    a = np.sort(np.asarray(a, "f8")); n = len(a)
    if not n:
        return float("nan")
    c = cdf(a)
    return float(max(np.max(np.arange(1, n + 1) / n - c), np.max(c - np.arange(0, n) / n)))


def axis_basis(axis):
    z = np.asarray(axis, "f8"); z = z / np.linalg.norm(z)
    e = np.eye(3)[np.argmin(np.abs(z))]
    x = np.cross(z, e); x /= np.linalg.norm(x)
    return x, np.cross(z, x), z


def features(name, s, geom):
    """Named 1-D features of a compact sample set (visible particles only), plus discrete categories."""
    c = case_of(name); e = c["E"]
    vis = ~np.isnan(s[:, 9])
    s = s[vis]
    v = s[:, 6:9].astype("f8"); p = s[:, 9:12].astype("f8")
    speed = np.linalg.norm(v, axis=1)
    f = {"lifetime": s[:, 1].astype("f8"), "speed": speed, "vx": v[:, 0], "vy": v[:, 1], "vz": v[:, 2], "px": p[:, 0], "py": p[:, 1], "pz": p[:, 2]}
    cat = {}
    if c["axis"] is not None:
        bx, by, bz = axis_basis(c["axis"])
        u = v / np.maximum(speed, 1e-30)[:, None]
        f["cos_axis"] = u @ bz
        f["azimuth"] = np.arctan2(u @ by, u @ bx)
    t = e["type"]
    if t == "SurfaceEmitter" and geom.get("surface", {}).get("verticesIndex", -1) >= 0:
        tri = surface_triangles(geom["surface"])
        fid, bary = classify_points(p, tri)
        nrm = np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0])
        # face category = triangle, side = sign of velocity against the triangle's geometric normal
        side = np.sign(np.einsum("ij,ij->i", v, nrm[fid]))
        cat["tri"] = fid
        cat["tri_side"] = fid * 3 + (side + 1).astype(int)
        f["bary0"] = bary[:, 0]; f["bary1"] = bary[:, 1]; f["bary2"] = bary[:, 2]
        f["plane_dist"] = np.abs(np.einsum("ij,ij->i", p - tri[fid, 0], nrm[fid] / np.linalg.norm(nrm[fid], axis=1)[:, None]))
    if t == "VolumeEmitter" and len(volume_triangles(geom["volume"])):
        tri = volume_triangles(geom["volume"])
        lo, hi = tri.reshape(-1, 3).min(0), tri.reshape(-1, 3).max(0)
        cell = np.clip(((p - lo) / (hi - lo) * 4).astype(int), 0, 3)
        cat["cell"] = cell[:, 0] * 16 + cell[:, 1] * 4 + cell[:, 2]
        cat["inside"] = inside_mesh(p, tri).astype(int)
        cat["origin"] = (np.all(p == 0, axis=1) & (s[:, 12] == 0)).astype(int)
        f["depth"] = np.min(np.abs(np.stack([p - lo, hi - p], 0)), axis=(0, 2))
    return f, cat, vis


def tex(arr, i):
    return np.asarray(arr[4 * i:4 * i + 4], "f4")


def surface_triangles(geom):
    arr = geom["array"]; vi = geom["verticesIndex"]; ni = geom["normalsIndex"]
    n = ni - vi
    return np.array([[tex(arr, vi + 3 * t + j)[:3] for j in range(3)] for t in range(n // 3)], "f8")


def volume_triangles(geom):
    return surface_triangles(geom)


def classify_points(p, tri):
    """For each point the triangle it lies in (nearest, by plane distance among those containing it) and its barycentric coordinates."""
    a, b, c = tri[:, 0], tri[:, 1], tri[:, 2]
    ab, ac = b - a, c - a
    n = np.cross(ab, ac)
    d00 = np.einsum("tj,tj->t", ab, ab); d01 = np.einsum("tj,tj->t", ab, ac); d11 = np.einsum("tj,tj->t", ac, ac)
    den = d00 * d11 - d01 * d01
    nn = n / np.linalg.norm(n, axis=1)[:, None]
    best = np.zeros(len(p), int); bary = np.zeros((len(p), 3)); score = np.full(len(p), np.inf)
    for t in range(len(tri)):
        w = p - a[t]
        d20 = w @ ab[t]; d21 = w @ ac[t]
        v = (d11[t] * d20 - d01[t] * d21) / den[t]
        u = (d00[t] * d21 - d01[t] * d20) / den[t]
        wv = 1 - u - v
        out = np.maximum(0, np.maximum(-v, np.maximum(-u, -wv)))     # how far outside the triangle (barycentric)
        sc = np.abs(w @ nn[t]) + out
        m = sc < score
        score[m] = sc[m]; best[m] = t
        bary[m] = np.column_stack([wv[m], v[m], u[m]])
    return best, bary


def inside_mesh(p, tri):
    """Even-odd ray test along a skewed direction against all triangles."""
    d = np.array([0.5773, 0.4411, 0.6877]); d /= np.linalg.norm(d)
    cnt = np.zeros(len(p), int)
    for a, b, c in tri:
        e1, e2 = b - a, c - a
        pv = np.cross(d, e2); det = e1 @ pv
        if abs(det) < 1e-12:
            continue
        tv = p - a
        u = (tv @ pv) / det
        qv = np.cross(tv, e1)
        v = (qv @ d) / det
        t = (qv @ e2) / det
        cnt += ((u >= 0) & (v >= 0) & (u + v <= 1) & (t > 0))
    return cnt % 2 == 1


def hist(x, bins, rng=None):
    h, e = np.histogram(x, bins=bins, range=rng)
    return {"counts": h.tolist(), "edges": [float(v) for v in e]}


def moments(a):
    a = np.asarray(a, "f8")
    return {"mean": a.mean(0).tolist(), "cov": np.cov(a.T).tolist()} if len(a) > 1 else {}


def stats_of(name, s, geom):
    c = case_of(name)
    f, cat, vis = features(name, s, geom)
    sv = s[vis]
    st = {"n": int(len(s)), "n_hidden": int((~vis).sum()), "frames": sorted(set(int(x) for x in s[:, 0])) if len(s) else [],
          "position": moments(sv[:, 9:12]), "velocity": moments(sv[:, 6:9])}
    st["quantiles"] = {k: np.quantile(v, [0, .01, .05, .25, .5, .75, .95, .99, 1]).tolist() for k, v in f.items() if len(v)}
    st["hist"] = {k: hist(v, 20) for k, v in f.items() if len(v)}
    st["categories"] = {k: np.bincount(v).tolist() for k, v in cat.items()}
    ana = {}
    e = c["E"]
    if "cos_axis" in f:
        ang = e.get("angle", math.pi) if e["type"] == "ConeEmitter" and any(e.get("direction", (0, 0, 0))) else math.pi
        lo = math.cos(ang)
        ana["cos_axis_uniform_lo_1"] = {"lo": lo, "ks1": ks1(f["cos_axis"], lambda x: np.clip((x - lo) / (1 - lo), 0, 1))}
        ana["azimuth_uniform"] = {"ks1": ks1(f["azimuth"], lambda x: np.clip((x + math.pi) / (2 * math.pi), 0, 1))}
    if "bary0" in f:   # a point drawn uniformly in a triangle has Beta (1, 2) marginals: CDF 1 - (1 - x)^2
        ana["bary_beta12_ks1"] = {k: ks1(f[k], lambda x: 1 - (1 - np.clip(x, 0, 1)) ** 2) for k in ("bary0", "bary1", "bary2")}
    if "tri" in cat and e["type"] == "SurfaceEmitter":
        tri = surface_triangles(geom["surface"])
        area = 0.5 * np.linalg.norm(np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0]), axis=1)
        cnt = np.bincount(cat["tri"], minlength=len(tri)).astype(float)
        exp = area / area.sum() * cnt.sum()
        ana["tri_area_chi2"] = {"chi2": float(((cnt - exp) ** 2 / exp).sum()), "dof": len(tri) - 1, "expected": exp.tolist(), "observed": cnt.tolist()}
    if "cell" in cat:
        tri = volume_triangles(geom["volume"])
        lo, hi = tri.reshape(-1, 3).min(0), tri.reshape(-1, 3).max(0)
        ana["inside_fraction"] = float(cat["inside"].mean())
        ana["origin_fallback_fraction"] = float(cat["origin"].mean())
        cnt = np.bincount(cat["cell"], minlength=64).astype(float)
        ana["cell_counts_4x4x4"] = cnt.tolist()
        ana["bbox"] = [lo.tolist(), hi.tolist()]
    st["analytic"] = ana
    return st


# ---------------------------------------------------------------- baseline

def strip_runtime(js):
    return {k: v for k, v in js.items() if k not in ("fileF32",)}


def baseline(name):
    try:
        c = case_of(name)
        tmp = "/tmp/rea-b06-particles-baseline/" + name
        shutil.rmtree(tmp, ignore_errors=True); os.makedirs(tmp)
        ja, da = run_xite(name, tmp + "/a.json")
        jb, db = run_xite(name, tmp + "/b.json")
        fa, fb = open(tmp + "/a.f32", "rb").read(), open(tmp + "/b.f32", "rb").read()
        same_steps = ja["steps"] == jb["steps"] and ja["dumps"] == jb["dumps"]
        det = {"runs": 2, "same_Math_random_seed": ja["seed"], "bit_identical_f32": fa == fb, "identical_steps_and_dump_index": same_steps, "f32_bytes": len(fa)}
        if not det["bit_identical_f32"] and len(fa) == len(fb):
            xa = np.frombuffer(fa, "<f4"); xb = np.frombuffer(fb, "<f4")
            diff = ~((xa == xb) | (np.isnan(xa) & np.isnan(xb)))
            det["differing_floats"] = int(diff.sum())
            det["max_abs_diff"] = float(np.nanmax(np.abs(xa[diff] - xb[diff]))) if diff.any() else 0.0
        dest = os.path.join(baseline_dir, name)
        shutil.rmtree(dest, ignore_errors=True); os.makedirs(dest)
        out = strip_runtime(ja)
        out["determinism"] = det
        out["world"] = "oracle/synthetic-rea-b06/%s.x3dv" % name
        if c["stat"]:
            jc, dc = run_xite(name, tmp + "/c.json", seed=(ja["seed"] ^ 0x5bd1e995) & 0x7fffffff)
            sa, sc = compact(da), compact(dc)
            geom = ja["geometry"]
            st = stats_of(name, sa, geom)
            fa_, ca_, _ = features(name, sa, geom); fc_, cc_, _ = features(name, sc, geom)
            st["null_ks"] = {k: ks2(fa_[k], fc_[k]) for k in fa_ if k in fc_ and len(fa_[k]) and len(fc_[k])}
            st["null_ks_crit"] = ks_crit(len(sa), len(sc))
            st["null_seed2"] = jc["seed"]
            st["compact_columns"] = COLS
            st["per_frame_births"] = {str(k): int(len(v)) for k, v in sorted(da.items())}
            sa.tofile(os.path.join(dest, "samples.f32"))
            json.dump(st, open(os.path.join(dest, "stats.json"), "w"), indent=1)
            out["samples"] = {"file": "samples.f32", "columns": COLS, "rows": int(len(sa)), "cap": SAMPLE_CAP}
            out["dumps"] = {k: {kk: vv for kk, vv in v.items() if kk != "offset"} for k, v in out["dumps"].items()}
        else:
            shutil.copy(tmp + "/a.f32", os.path.join(dest, "js.f32"))
            out["fileF32"] = "js.f32"
        json.dump(out, open(os.path.join(dest, "js.json"), "w"), indent=1)
        return name, det["bit_identical_f32"] and same_steps, "baseline written; two runs " + ("bit-identical" if det["bit_identical_f32"] and same_steps else "DIFFER %s" % det)
    except Exception as ex:
        return name, False, "error: %s" % ex


# ---------------------------------------------------------------- check

def cmp_group(ref, got, cols, tol):
    a = ref[:, cols].astype("f8"); b = got[:, cols].astype("f8")
    nan_a, nan_b = np.isnan(a), np.isnan(b)
    nan_mismatch = int((nan_a != nan_b).sum())
    ok = ~(nan_a | nan_b)
    d = np.abs(a - b)[ok]
    allow = (tol[0] + tol[1] * np.abs(a))[ok]
    excess = d - allow
    return {"max_abs": float(d.max()) if d.size else 0.0, "max_rel": float((d / np.maximum(np.abs(a[ok]), 1e-30)).max()) if d.size else 0.0,
            "n_fail": int((excess > 0).sum()) + nan_mismatch, "nan_mismatch": nan_mismatch, "first_bad": next(iter(zip(*np.nonzero((np.abs(a - b) > (tol[0] + tol[1] * np.abs(a))) & ok))), None)}


def check_exact(name, base, bd, rs, rd):
    info = {"steps": {}, "groups": {}}
    fails = []
    bs, rs_ = base["steps"], rs.get("steps", [])
    if len(bs) != len(rs_):
        fails.append("steps: %d vs %d" % (len(bs), len(rs_)))
    bad_np = [s["frame"] for s, r in zip(bs, rs_) if s["numParticles"] != r["numParticles"]]
    bad_cp = [s["frame"] for s, r in zip(bs, rs_) if bool(s["createParticles"]) != bool(r["createParticles"])]
    dt_err = max((abs(s["deltaTime"] - r["deltaTime"]) / max(abs(s["deltaTime"]), 1e-300) for s, r in zip(bs, rs_)), default=0.0)
    ct_err = max((abs(s["creationTime"] - r["creationTime"]) for s, r in zip(bs, rs_)), default=0.0)
    info["steps"] = {"numParticles_bad_frames": bad_np[:5], "createParticles_bad_frames": bad_cp[:5], "deltaTime_max_rel": dt_err, "creationTime_max_abs": ct_err}
    if bad_np: fails.append("numParticles differs at frames %s" % bad_np[:5])
    if bad_cp: fails.append("createParticles differs at frames %s" % bad_cp[:5])
    if dt_err > TOL_STEP["deltaTime"][1]: fails.append("deltaTime rel err %.3g" % dt_err)
    if ct_err > TOL_STEP["creationTime"][0]: fails.append("creationTime abs err %.3g" % ct_err)
    for g in GROUPS:
        info["groups"][g] = {"max_abs": 0.0, "max_rel": 0.0, "n_fail": 0, "frames_bad": []}
    for k in sorted(bd):
        if k not in rd or len(rd[k]) != len(bd[k]):
            fails.append("dump %d: %s records vs %d" % (k, len(rd[k]) if k in rd else "no", len(bd[k])))
            continue
        for g, cols in GROUPS.items():
            r = cmp_group(bd[k], rd[k], cols, TOL[g])
            gi = info["groups"][g]
            gi["max_abs"] = max(gi["max_abs"], r["max_abs"]); gi["max_rel"] = max(gi["max_rel"], r["max_rel"]); gi["n_fail"] += r["n_fail"]
            if r["n_fail"] and len(gi["frames_bad"]) < 5:
                gi["frames_bad"].append(k)
    for g, gi in info["groups"].items():
        if gi["n_fail"]:
            fails.append("%s: %d values over tolerance (max abs %.3g, first bad frames %s)" % (g, gi["n_fail"], gi["max_abs"], gi["frames_bad"]))
    return fails, info


def check_stat(name, base, sd_base, rs, rd):
    fails, info = [], {"tests": {}}
    st = json.load(open(os.path.join(baseline_dir, name, "stats.json")))
    sb = np.fromfile(os.path.join(baseline_dir, name, "samples.f32"), "<f4").reshape(-1, len(COLS))
    sr = compact(rd)
    geom = base["geometry"]
    births_b = {int(k): v["numParticles"] for k, v in base["dumps"].items()}
    births_r = {k: len(v) for k, v in rd.items()}
    if set(births_b) != set(births_r):
        fails.append("dumped frames differ: %s vs %s" % (sorted(births_b)[:5], sorted(births_r)[:5]))
    else:   # how many particles are created in a frame is random (lifetimes); a Poisson-like 6 sigma band
        bad = [k for k in births_b if abs(births_b[k] - births_r[k]) > 6 * math.sqrt(max(births_b[k], births_r[k], 1)) + 3]
        if bad:
            fails.append("born-record counts differ at frames %s (e.g. %s vs %s)" % (bad[:5], [births_b[k] for k in bad[:3]], [births_r[k] for k in bad[:3]]))
    bs, rs_ = base["steps"], rs.get("steps", [])
    if [(s["numParticles"], bool(s["createParticles"])) for s in bs] != [(r["numParticles"], bool(r["createParticles"])) for r in rs_]:
        fails.append("numParticles / createParticles schedule differs")
    fb, cb, vb = features(name, sb, geom)
    fr, cr, vr = features(name, sr, geom)
    info["n"] = [int(len(sb)), int(len(sr))]
    info["hidden_fraction"] = [float(1 - vb.mean()) if len(vb) else 0, float(1 - vr.mean()) if len(vr) else 0]
    ph = (info["hidden_fraction"][0] * len(vb) + info["hidden_fraction"][1] * len(vr)) / max(len(vb) + len(vr), 1)
    zh = abs(info["hidden_fraction"][0] - info["hidden_fraction"][1]) / math.sqrt(max(ph * (1 - ph), 1e-12) * (1 / max(len(vb), 1) + 1 / max(len(vr), 1)))
    if (ph in (0.0, 1.0) and info["hidden_fraction"][0] != info["hidden_fraction"][1]) or (0 < ph < 1 and zh > CHI_SIGMA):
        fails.append("hidden fraction %.4g vs %.4g (z %.1f)" % (info["hidden_fraction"][0], info["hidden_fraction"][1], zh))
    for k in fb:
        if k not in fr or not len(fr[k]):
            fails.append("feature %s missing" % k); continue
        n, m = len(fb[k]), len(fr[k])
        if np.ptp(fb[k]) < 1e-6 and abs(np.mean(fb[k])) < 1e30:      # a constant feature: the value must match
            ok = abs(np.mean(fb[k]) - np.mean(fr[k])) < 1e-5 + 1e-5 * abs(np.mean(fb[k])) and np.ptp(fr[k]) < 1e-5
            info["tests"][k] = {"constant": float(np.mean(fb[k])), "rust_mean": float(np.mean(fr[k])), "ok": bool(ok)}
            if not ok: fails.append("%s: constant %.6g vs %.6g" % (k, np.mean(fb[k]), np.mean(fr[k])))
            continue
        d = ks2(fb[k], fr[k]); crit = ks_crit(n, m)
        info["tests"][k] = {"ks": d, "crit": crit, "null_ks": st["null_ks"].get(k), "ok": bool(d <= crit)}
        if d > crit: fails.append("%s: KS %.4f > %.4f" % (k, d, crit))
    for k in cb:
        cnt_b = np.bincount(cb[k], minlength=max(cb[k].max(), cr[k].max() if len(cr.get(k, [])) else 0) + 1).astype(float)
        cnt_r = np.bincount(cr[k], minlength=len(cnt_b)).astype(float)
        cnt_b = np.pad(cnt_b, (0, len(cnt_r) - len(cnt_b)))
        pb, pr = cnt_b / cnt_b.sum(), cnt_r / cnt_r.sum()
        p = (cnt_b + cnt_r) / (cnt_b.sum() + cnt_r.sum())
        z = np.abs(pb - pr) / np.sqrt(np.maximum(p * (1 - p), 1e-12) * (1 / cnt_b.sum() + 1 / cnt_r.sum()))
        z = np.where(p * (1 - p) < 1e-9, np.where(np.abs(pb - pr) < 1e-9, 0, 99), z)
        info["tests"][k] = {"max_z": float(z.max()), "sigma": CHI_SIGMA, "ok": bool(z.max() <= CHI_SIGMA)}
        if z.max() > CHI_SIGMA: fails.append("%s: category proportion z %.2f > %.1f (category %d)" % (k, z.max(), CHI_SIGMA, int(np.argmax(z))))
    # means with standard-error test (catches shifts the KS test of a near-constant feature would not)
    for g, cols in (("position", slice(9, 12)), ("velocity", slice(6, 9))):
        a, b = sb[vb][:, cols].astype("f8"), sr[vr][:, cols].astype("f8") if len(vr) else np.zeros((0, 3))
        if len(a) > 1 and len(b) > 1:
            se = np.sqrt(a.var(0, ddof=1) / len(a) + b.var(0, ddof=1) / len(b))
            zz = np.abs(a.mean(0) - b.mean(0)) / np.maximum(se, 1e-12)
            zz = np.where(se < 1e-9, np.where(np.abs(a.mean(0) - b.mean(0)) < 1e-6, 0, 99), zz)
            info["tests"][g + "_mean"] = {"max_z": float(zz.max()), "sigma": MEAN_SIGMA, "ok": bool(zz.max() <= MEAN_SIGMA)}
            if zz.max() > MEAN_SIGMA: fails.append("%s mean z %.2f" % (g, zz.max()))
    return fails, info


def run_driver(name, outdir):
    os.makedirs(outdir, exist_ok=True)
    rs_dir = opt("--rs-dir")
    if rs_dir:
        p = os.path.join(rs_dir, name, "rs.json")
        if not os.path.exists(p):
            raise RuntimeError("no " + p)
        return p
    drv = (opt("--driver") or "/tmp/fp-rel/release/x3d-particles-drive").split()
    out = os.path.join(outdir, "rs.json")
    r = subprocess.run(drv + [os.path.join(worlds_dir, name + ".x3dv"), os.path.join(scripts_dir, name + ".json"), "--seeds-from",
                       os.path.join(baseline_dir, name, "js.json"), "--out", out], cwd=root, capture_output=True, text=True, timeout=900)
    if r.returncode != 0 or not os.path.exists(out):
        raise RuntimeError("driver failed: " + (r.stderr or r.stdout or "no output")[-300:])
    return out


def check(name):
    try:
        bpath = os.path.join(baseline_dir, name, "js.json")
        if not os.path.exists(bpath):
            return name, "FAIL", {"error": "no baseline"}, ["no baseline"]
        base = json.load(open(bpath))
        rs, rd = load_run(run_driver(name, "/tmp/rea-b06-particles-check/" + name))
        if not rs.get("ok", True):
            return name, "FAIL", {"error": "driver not ok"}, ["driver reported ok=false: %s" % rs.get("error")]
        if case_of(name)["stat"]:
            fails, info = check_stat(name, base, None, rs, rd)
        else:
            _, bd = load_run(bpath)
            fails, info = check_exact(name, base, bd, rs, rd)
        status = "PASS" if not fails else ("KNOWN" if name in KNOWN else "FAIL")
        return name, status, info, fails
    except Exception as ex:
        return name, "FAIL", {"error": str(ex)}, [str(ex)]


def report(results):
    counts = {"PASS": 0, "KNOWN": 0, "FAIL": 0}
    for n, status, info, fails in results:
        counts[status] += 1
        if "groups" in info:
            line = " ".join("%s %.2e" % (g, gi["max_abs"]) for g, gi in info["groups"].items())
            line += " | dt rel %.1e" % info["steps"]["deltaTime_max_rel"]
        elif "tests" in info:
            ks = [(k, t["ks"] / t["crit"]) for k, t in info["tests"].items() if "ks" in t]
            line = "KS/crit max %s %.2f over %d features" % (max(ks, key=lambda x: x[1])[0] if ks else "-", max((x[1] for x in ks), default=0), len(ks))
            zs = [t["max_z"] for t in info["tests"].values() if "max_z" in t]
            if zs: line += ", max category/mean z %.2f" % max(zs)
        else:
            line = info.get("error", "")
        print("%-5s %-40s %s" % (status, n, line))
        for f in fails[:6]:
            print("        - " + f)
    print("suite rea-b06-particles: %d pass, %d known, %d fail, of %d cases" % (counts["PASS"], counts["KNOWN"], counts["FAIL"], len(results)))
    return counts


# ---------------------------------------------------------------- reference emulation (numpy float32 of the GLSL)

F = np.float32
M_PI = F(3.141592653589793)
V3 = lambda *a: np.array(a, dtype="f4")


def f_normalize(v):
    return (v / F(np.sqrt(np.dot(v, v)))).astype("f4")


def f_cross(a, b):
    return np.array([a[1] * b[2] - b[1] * a[2], a[2] * b[0] - b[2] * a[0], a[0] * b[1] - b[0] * a[1]], dtype="f4")


def f_dot(a, b):
    return F(a[0] * b[0] + a[1] * b[1] + a[2] * b[2])


def f_quat(frm, to):
    f = f_normalize(frm); t = f_normalize(to)
    cos_a = f_dot(f, t); cv = f_cross(f, t); cl = F(np.sqrt(f_dot(cv, cv)))
    if cl == 0:
        if cos_a > 0:
            return V3(0, 0, 0, 1)
        tt = f_cross(f, V3(1, 0, 0))
        if f_dot(tt, tt) == 0:
            tt = f_cross(f, V3(0, 1, 0))
        return np.append(f_normalize(tt), F(0)).astype("f4")
    s = F(np.sqrt(abs(F(1) - cos_a) * F(0.5)))
    cv = f_normalize(cv)
    return np.append(cv * s, F(np.sqrt(abs(F(1) + cos_a) * F(0.5)))).astype("f4")


def f_mulq(v, q):
    return (v + F(2) * f_cross(q[:3], f_cross(q[:3], v) + q[3] * v)).astype("f4")


def f_reflect(i, n):
    return (i - F(2) * f_dot(n, i) * n).astype("f4")


def f_tri(line_p, line_d, a, b, c):
    e1, e2 = (b - a).astype("f4"), (c - a).astype("f4")
    pv = f_cross(line_d, e2); det = f_dot(e1, pv)
    if det == 0:
        return None
    inv = F(1) / det
    tv = (line_p - a).astype("f4")
    u = f_dot(tv, pv) * inv
    if u < 0 or u > 1:
        return None
    qv = f_cross(tv, e1)
    v = f_dot(line_d, qv) * inv
    if v < 0 or u + v > 1:
        return None
    return V3(u, v, F(1) - u - v)


class Emu:
    def __init__(self, name, js):
        self.c = case_of(name); self.e = self.c["E"]; self.js = js; self.geom = js.get("geometry", {})
        e = self.e
        self.speed = F(max(e.get("speed", 0), 0)); self.variation = F(max(e.get("variation", 0.25), 0))
        self.pos = V3(*e.get("position", (0, 0, 0)))
        d = np.array(e.get("direction", (0, 1, 0)), "f8")
        if e["type"] in ("PointEmitter", "VolumeEmitter"):
            n = np.linalg.norm(d); d = d / n if n else d
        self.dir = d.astype("f4")
        self.angle = F(e.get("angle", 0.785398))
        self.solid = True
        if e["type"] == "SurfaceEmitter" and "surface" in e:
            self.solid = bool(e["surface"][3]) if e["surface"][0] == "ifs" else True
        self.PL = F(self.c["life"]); self.LV = F(max(self.c["var"], 0))
        self.seed = 1

    # --- GLSL random
    def srand(self, v):
        self.seed = v & 0xffffffff

    def rnd(self):
        self.seed = (self.seed * 1103515245 + 12345) & 0xffffffff
        return F(F(self.seed) / F(4294967295.0))

    def rv(self, lo, hi):
        return F(lo + self.rnd() * (hi - lo))

    def rlife(self):
        v = self.PL * self.LV
        return self.rv(max(F(0), self.PL - v), self.PL + v)

    def rspeed(self):
        v = self.speed * self.variation
        return self.rv(max(F(0), self.speed - v), self.speed + v)

    def rnormal(self):
        theta = self.rv(-M_PI, M_PI); cphi = self.rv(F(-1), F(1)); r = F(np.sqrt(F(1) - cphi * cphi))
        return V3(F(np.sin(theta)) * r, F(np.cos(theta)) * r, cphi)

    def rnormal_angle(self, angle):
        theta = self.rv(-M_PI, M_PI); cphi = self.rv(F(np.cos(angle)), F(1)); r = F(np.sqrt(F(1) - cphi * cphi))
        return V3(F(np.sin(theta)) * r, F(np.cos(theta)) * r, cphi)

    def rnormal_dir_angle(self, d, angle):
        q = f_quat(V3(0, 0, 1), d)
        return f_mulq(self.rnormal_angle(angle), q)

    def rsurface_normal(self, d):
        theta = self.rv(-M_PI, M_PI); cphi = F(np.power(self.rnd(), F(1) / F(3))); r = F(np.sqrt(F(1) - cphi * cphi))
        n = V3(F(np.sin(theta)) * r, F(np.cos(theta)) * r, cphi)
        return f_mulq(n, f_quat(V3(0, 0, 1), d))

    def rspherical(self):
        n = self.rnormal(); s = self.rspeed()
        return (n * s).astype("f4")

    # --- geometry textures
    def tex_geom(self, key):
        g = self.geom[key]; arr = g["array"]
        return arr, g

    def point_on_surface(self, key):
        arr, g = self.tex_geom(key)
        vi, ni = g["verticesIndex"], g["normalsIndex"]
        tx = lambda i: np.asarray(arr[4 * i:4 * i + 4], "f4")
        last = tx(vi - 1)[0]
        fraction = self.rnd() * last
        count = vi
        if count == 1 or fraction <= tx(0)[0]:
            i0 = 0
        elif fraction >= tx(count - 1)[0]:
            i0 = count - 2
        else:
            idx = 0
            lo, c = 0, count
            while c > 0:
                step = c >> 1; mid = lo + step
                if fraction < tx(mid)[0]:
                    c = step
                else:
                    lo = mid + 1; c -= step + 1
            idx = lo
            i0 = idx - 1 if idx < count else 0
        i0 *= 3
        v = [tx(vi + i0 + j) for j in range(3)]; n = [tx(ni + i0 + j)[:3] for j in range(3)]
        u = self.rnd(); w = self.rnd()
        if u + w > 1:
            u = F(1) - u; w = F(1) - w
        t = F(1) - u - w
        r = V3(t, u, w)
        position = (r[2] * v[0] + r[0] * v[1] + r[1] * v[2]).astype("f4")
        normal = f_normalize(r[2] * n[0] + r[0] * n[1] + r[1] * n[2])
        return position, normal

    def volume_points(self, line_p, line_d):
        """Line vs triangles in the order of X_ITE's BVH traversal (BVH.glsl.js getIntersections), read from the dumped hierarchy."""
        arr, g = self.tex_geom("volume"); vi = g["verticesIndex"]; hI = g["hierarchyIndex"]; root = g["hierarchyRoot"]
        tx = lambda i: np.asarray(arr[4 * i:4 * i + 4], "f4")

        def face(pl_n, pl_d, axes, lo, hi):
            theta = f_dot(line_d, pl_n)
            if theta == 0:
                return False
            t = (pl_d - f_dot(pl_n, line_p)) / theta
            x = (line_p + line_d * t).astype("f4")
            a, b = axes
            return bool(x[a] >= lo[a] and x[b] >= lo[b] and hi[a] >= x[a] and hi[b] >= x[b])

        def box_hit(lo, hi):          # Box3.glsl.js: five of the six faces are tested (no "left")
            return (face(V3(0, 0, 1), F(hi[2]), (0, 1), lo, hi) or face(V3(0, 0, -1), F(-lo[2]), (0, 1), lo, hi)
                    or face(V3(0, 1, 0), F(hi[1]), (0, 2), lo, hi) or face(V3(0, -1, 0), F(-lo[1]), (0, 2), lo, hi)
                    or face(V3(1, 0, 0), F(hi[0]), (1, 2), lo, hi))

        pts, stack, cur = [], [], int(tx(root)[0]) + hI
        while stack or cur >= 0:
            if cur >= 0:
                node = tx(cur)
                if int(node[0]) == 0:
                    if box_hit(tx(cur + 1)[:3], tx(cur + 2)[:3]):
                        stack.append(cur); cur = int(node[1]) + hI
                    else:
                        cur = -1
                else:
                    t = int(node[1])
                    a, b, c = tx(vi + t)[:3], tx(vi + t + 1)[:3], tx(vi + t + 2)[:3]
                    r = f_tri(line_p, line_d, a, b, c)
                    if r is not None:
                        pts.append(np.append(r[2] * a + r[0] * b + r[1] * c, F(1)).astype("f4"))
                    cur = -1
            else:
                cur = int(tx(stack.pop())[2]) + hI
        return pts

    # --- emitters
    def velocity_position(self):
        t = self.e["type"]
        if t == "PointEmitter":
            vel = self.rspherical() if not self.dir.any() else (self.dir * self.rspeed()).astype("f4")
            return vel, np.append(self.pos, F(1)).astype("f4")
        if t == "ConeEmitter":
            if not self.dir.any():
                vel = self.rspherical()
            else:
                n = self.rnormal_dir_angle(self.dir, self.angle); s = self.rspeed(); vel = (n * s).astype("f4")
            return vel, np.append(self.pos, F(1)).astype("f4")
        if t == "ExplosionEmitter":
            return self.rspherical(), np.append(self.pos, F(1)).astype("f4")
        if t == "SurfaceEmitter":
            if self.geom.get("surface", {}).get("verticesIndex", -1) < 0:
                return V3(0, 0, 0), V3(np.nan, np.nan, np.nan, np.nan)
            position, normal = self.point_on_surface("surface")
            if not self.solid and self.rnd() > F(0.5):
                normal = -normal
            return (normal * self.rspeed()).astype("f4"), position
        if t == "VolumeEmitter":
            g = self.geom["volume"]
            vel = V3(0, 0, 0) if g["hierarchyRoot"] < 0 else (self.rspherical() if not self.dir.any() else (self.dir * self.rspeed()).astype("f4"))
            if not len(volume_triangles(g)):          # empty mesh: fraction, two barycentric draws, theta and cphi are still drawn
                for _ in range(5): self.rnd()
                return vel, V3(0, 0, 0, 0)
            point, normal = self.point_on_surface("volume")
            line_p = point[:3]; line_d = self.rsurface_normal(normal)
            pts = self.volume_points(line_p, line_d)
            n = len(pts); n -= n % 2
            if n == 0:
                return vel, V3(0, 0, 0, 0)
            pts = pts[:n]
            if n != 2:
                dist = lambda p: F(f_dot(p[:3], line_d) - f_dot(line_d, line_p))
                gap = n; ex = True
                while ex:
                    gap = int(F(gap) * F(1 / 1.3))
                    if gap <= 1:
                        ex = False; gap = 1
                    for i in range(n - gap):
                        j = gap + i
                        if dist(pts[i]) > dist(pts[j]):
                            pts[i], pts[j] = pts[j], pts[i]; ex = True
            idx = int(F(math.modf(float(self.rnd()))[0]) * F(n // 2)) * 2
            a = self.rnd()
            return vel, (pts[idx] * (F(1) - a) + pts[idx + 1] * a).astype("f4")
        raise ValueError(t)

    def bounce(self, frm, to, vel):
        b = self.geom.get("bounded")
        if not b:
            return to, vel
        verts, norms = b["vertices"], b["normals"]
        line_p = frm[:3]; line_d = f_normalize(vel)
        cands = []
        for t in range(0, len(verts) // 4, 3):
            a, bb, c = [np.asarray(verts[4 * (t + j):4 * (t + j) + 3], "f4") for j in range(3)]
            r = f_tri(line_p, line_d, a, bb, c)
            if r is not None:
                n = [np.asarray(norms[3 * (t + j):3 * (t + j) + 3], "f4") for j in range(3)]
                cands.append((np.append(r[2] * a + r[0] * bb + r[1] * c, F(1)).astype("f4"), (r[2] * n[0] + r[0] * n[1] + r[1] * n[2]).astype("f4")))
        if not cands:
            return to, vel
        d0 = f_dot(line_d, line_p)
        index, dist = -1, F(1000000)
        for i, (p, _) in enumerate(cands):
            d = F(f_dot(p[:3], line_d) - d0)
            if d >= 0 and d < dist:
                dist, index = d, i
        if index == -1:
            return to, vel
        point, nrm = cands[index][0][:3], cands[index][1]
        normal = f_normalize(nrm)
        pd = F(f_dot(normal, point))
        if np.sign(F(f_dot(frm[:3], normal) - pd)) == np.sign(F(f_dot(to[:3], normal) - pd)):
            return to, vel
        damping = F(np.sqrt(f_dot(nrm, nrm)))
        vel = f_reflect(vel, normal)
        to = np.append(point + f_normalize(vel) * F(0.0001), F(1)).astype("f4")
        return to, (vel * damping).astype("f4")

    # --- run
    def run(self):
        c, e, js = self.c, self.e, self.js
        n_max = c["n"]; explosive = e["type"] == "ExplosionEmitter"
        life = np.zeros(n_max, "f4"); lifetime = np.zeros(n_max, "f4"); elapsed = np.zeros(n_max, "f4")
        color = np.zeros((n_max, 4), "f4"); vel = np.zeros((n_max, 3), "f4"); pos = np.zeros((n_max, 4), "f4")
        t0 = js["t0"]; DT = js["dt"]
        RATE = 1 / max(DT, 1e-9)
        ps_dt = 0.0; creation = 0.0 if explosive else float(t0); num = 0
        forces = [p for p in c["P"] if p[0] == "force"]
        mass = max(e.get("mass", 0), 0)
        steps, dumps = [], {}
        dump_all = js.get("dumpAll", False) or True
        script = json.load(open(os.path.join(scripts_dir, "particles-" + js["_case"] + ".json")))
        want = set(range(1, js["frames"] + 1)) if script.get("dumpAll") else set(script.get("dump", []))
        born_only = script.get("filter") == "born"
        on = e.get("on", True)
        for st in js["steps"]:
            k = st["frame"]; t = st["time"]; now = (t * 1000) / 1000
            dtv = 1 / max(RATE, 10)
            ps_dt = ((15 - 1) * ps_dt + dtv) / 15
            delta = ps_dt
            if explosive:
                if now - creation > c["life"] + c["life"] * max(c["var"], 0):
                    creation = now; num = n_max; create = bool(c["create"]); delta = math.inf
                else:
                    create = False
            else:
                create = bool(c["create"])
                if num < n_max:
                    new = max(0, math.floor((now - creation) * n_max / c["life"]))
                    if new:
                        creation = now
                    num = min(n_max, num + new)
            nforce = 0; fvec = []
            if mass and forces:
                tbm = delta / mass
                for p in forces:
                    fvec.append(V3(*[F(x * tbm) for x in p[1]]))
                nforce = len(fvec)
            rs = int(st["randomSeed"]) % (1 << 32)
            d32 = F(delta)
            for i in range(num):
                self.srand(((i + rs) * rs) & 0xffffffff)
                li = int(life[i]); lt = lifetime[i]; el = F(elapsed[i] + d32)
                if el > lt:
                    lt = self.rlife()
                    life[i] = max(li + 1, 1); lifetime[i] = lt; elapsed[i] = 0
                    if create and on:
                        color[i] = 1
                        vel[i], pos[i] = self.velocity_position()
                    else:
                        color[i] = 0; vel[i] = 0; pos[i] = np.nan
                else:
                    v = vel[i].copy(); p = pos[i].copy(); old = p.copy()
                    for fv in fvec:
                        force = np.append(fv, F(0))
                        nrm = self.rnormal_dir_angle(force[:3], force[3])
                        v = (v + nrm * F(np.sqrt(f_dot(force[:3], force[:3])))).astype("f4")
                    p[:3] = (p[:3] + v * d32).astype("f4")
                    if "bounded" in self.geom:
                        p, v = self.bounce(old, p, v)
                    life[i] = li; lifetime[i] = lt; elapsed[i] = el; color[i] = 1; vel[i] = v; pos[i] = p
            steps.append({"frame": k, "time": t, "randomSeed": st["randomSeed"], "numParticles": num, "deltaTime": ps_dt, "creationTime": creation, "createParticles": create})
            if k in want:
                idx = [i for i in range(num) if (not born_only or elapsed[i] == 0)]
                rec = np.zeros((len(idx), STRIDE), "f4")
                for j, i in enumerate(idx):
                    rec[j, 0] = life[i]; rec[j, 1] = lifetime[i]; rec[j, 2] = elapsed[i]; rec[j, 3] = -1
                    rec[j, 4:8] = color[i]; rec[j, 8:11] = vel[i]; rec[j, 24:28] = pos[i]
                dumps[k] = rec
        return steps, dumps


def emulate(name):
    try:
        bpath = os.path.join(baseline_dir, name, "js.json")
        js = json.load(open(bpath)); js["_case"] = name[len("particles-"):]
        steps, dumps = Emu(name, js).run()
        outdir = os.path.join(opt("--out", "/tmp/rea-b06-emulate"), name)
        os.makedirs(outdir, exist_ok=True)
        chunks, off, dd = [], 0, {}
        for k in sorted(dumps):
            dd[str(k)] = {"numParticles": int(len(dumps[k])), "offset": off}
            chunks.append(dumps[k].reshape(-1)); off += dumps[k].size
        (np.concatenate(chunks) if chunks else np.zeros(0, "f4")).astype("<f4").tofile(os.path.join(outdir, "rs.f32"))
        json.dump({"ok": True, "stride": STRIDE, "fileF32": "rs.f32", "steps": steps, "dumps": dd, "emulator": "numpy float32"}, open(os.path.join(outdir, "rs.json"), "w"))
        return name, True, outdir
    except Exception as ex:
        import traceback
        return name, False, traceback.format_exc()[-600:]


# ---------------------------------------------------------------- main

if __name__ == "__main__":
    cmd = args[:1]
    sel = names or all_names()
    if cmd == ["baseline"]:
        with ThreadPoolExecutor(jobs) as ex:
            results = list(ex.map(baseline, sel))
        bad = [n for n, ok, _ in results if not ok]
        for n, ok, msg in results:
            print(("OK   " if ok else "FAIL ") + n + ": " + msg)
        print("baselines: %d of %d written, %d with non-identical repeat runs or errors" % (len(results) - sum(1 for _, _, m in results if m.startswith("error")), len(results), len(bad)))
        sys.exit(1 if bad else 0)
    elif cmd == ["emulate"]:
        sel = [n for n in sel if not case_of(n)["stat"]] if not names else sel
        with ThreadPoolExecutor(jobs) as ex:
            results = list(ex.map(emulate, sel))
        for n, ok, msg in results:
            print(("OK   " if ok else "FAIL ") + n + ": " + msg)
        sys.exit(0 if all(ok for _, ok, _ in results) else 1)
    elif cmd == ["check"]:
        with ThreadPoolExecutor(jobs) as ex:
            results = list(ex.map(check, sel))
        counts = report(results)
        if opt("--json"):
            json.dump({n: {"status": s, "fails": f, **i} for n, s, i, f in results}, open(opt("--json"), "w"), indent=1, default=str)
        sys.exit(1 if counts["FAIL"] else 0)
    elif cmd == ["nullcheck"]:
        # A third X_ITE run with another Math.random seed through the statistical check: must PASS (false-positive test).
        sel = [n for n in sel if case_of(n)["stat"]]
        results = []
        for n in sel:
            tmp = "/tmp/rea-b06-nullcheck/" + n
            shutil.rmtree(tmp, ignore_errors=True); os.makedirs(tmp)
            base = json.load(open(os.path.join(baseline_dir, n, "js.json")))
            rs, rd = run_xite(n, tmp + "/r.json", seed=(base["seed"] ^ 0x2c1b3c6d) & 0x7fffffff)
            fails, info = check_stat(n, base, None, rs, rd)
            results.append((n, "PASS" if not fails else "FAIL", info, fails))
        counts = report(results)
        sys.exit(1 if counts["FAIL"] else 0)
    elif cmd == ["selftest"]:
        sel = names or ["particles-point-dir", "particles-cone-basic", "particles-surface-box", "particles-volume-cube", "particles-bounded-floor"]
        for n in sel:
            tmp = "/tmp/rea-b06-selftest/" + n
            shutil.rmtree(tmp, ignore_errors=True); os.makedirs(tmp)
            ja, da = run_xite(n, tmp + "/a.json"); jb, db = run_xite(n, tmp + "/b.json")
            print(n, "bit-identical" if open(tmp + "/a.f32", "rb").read() == open(tmp + "/b.f32", "rb").read() else "DIFFERENT")
    else:
        print(__doc__)
        sys.exit(2)
