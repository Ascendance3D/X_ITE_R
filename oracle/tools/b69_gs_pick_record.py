#!/usr/bin/env python3
"""GaussianSplats pointing baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU) for crates/x3d-viewer/tests/b69_gs_pick.rs.

  python3 -I -B oracle/tools/b69_gs_pick_record.py record NAME MSAA [X3D_ORACLE]

runs `x3d-oracle particles` (default target/debug/x3d-oracle) FIVE times on oracle/synthetic-b69-gs-pick/NAME.x3dv with
scripts/NAME.json; MSAA is 0 (the page's default canvas) or 4 (`--aa --ms 4`: the display is multisampled; the pointing buffer
is not, so the picks must not change). `--png-dir` is always passed so the page reshapes its drawing buffer to 512 x 512 (without it
`browser .touch` has a 1 x 1 viewport). The five js.json and js.f32 must be byte-identical; only then is the first kept as
oracle/b69-gs-pick-baseline/msaaMSAA/NAME/. A baseline is never taken from Rust. Staging is /tmp/b69-gs-pick-record.

  X3D_ORACLE_CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" X3D_ORACLE_ANGLE=metal \\
  python3 -I -B oracle/tools/b69_gs_pick_record.py platform PLATFORM --hardware "Apple M1, macOS 27.0.1" [--oracle X3D_ORACLE]

the platform recording (crates/x3d-viewer/tests/common/reference.rs; rules of record_platform_baseline.py), made ON THE TARGET
MACHINE: every case, at MSAA 0 and 4, without and with LOG, recorded THREE times; the three js.json must be byte-identical (else
the run stops and nothing is written). A js.json is kept, as oracle/b69-gs-pick[-log]-PLATFORM-baseline/NAME-msaaMSAA.json (flat,
one directory level as the review gate expects), only where it differs from the NVIDIA recording; js.f32 is empty for these cases
and is not kept. The NVIDIA baselines are never written. LOG=1 in the environment applies to `record` only.
"""
import datetime, filecmp, hashlib, json, os, shutil, subprocess, sys, tempfile

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
CASES = ["gs-pick-basic", "gs-pick-default", "gs-pick-xform", "gs-pick-clip", "gs-pick-mixed", "gs-pick-ortho"]


def run_oracle(oracle, name, msaa, log, out):
    world = os.path.join(root, "oracle", "synthetic-b69-gs-pick", name + ".x3dv")
    script = os.path.join(root, "oracle", "synthetic-b69-gs-pick", "scripts", name + ".json")
    extra = ["--aa", "--ms", "4"] if msaa == "4" else []
    env = dict(os.environ)
    env.pop("X3D_ORACLE_LOG_DEPTH", None)
    if log:
        # LogarithmicDepthBuffer on (X3D_ORACLE_LOG_DEPTH=1 for x3d-oracle); baselines go to b69-gs-pick-log-baseline.
        env["X3D_ORACLE_LOG_DEPTH"] = "1"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    subprocess.run([oracle, "particles", world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "png")] + extra,
                   check=True, cwd=root, timeout=300, stdout=subprocess.DEVNULL, env=env)


def record(args):
    name, msaa = args[0], args[1]
    oracle = args[2] if len(args) > 2 else os.path.join(root, "target", "debug", "x3d-oracle")
    log = os.environ.get("LOG") == "1"
    runs = []
    for i in range(1, 6):
        out = f"/tmp/b69-gs-pick-record/{name}-ms{msaa}{'-log' if log else ''}-{i}"
        run_oracle(oracle, name, msaa, log, out)
        runs.append(out)
    a = runs[0]
    same = all(filecmp.cmp(os.path.join(a, f), os.path.join(b, f), shallow=False) for b in runs[1:] for f in ("js.json", "js.f32"))
    print(f"{name} msaa {msaa}: five X_ITE runs {'identical' if same else 'DIFFER'}")
    if not same:
        sys.exit(1)
    dest = os.path.join(root, "oracle", "b69-gs-pick-log-baseline" if log else "b69-gs-pick-baseline", f"msaa{msaa}", name)
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest)
    for f in ("js.json", "js.f32"):
        shutil.copy(os.path.join(a, f), dest)


def platform(args):
    plat = args[0]
    opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
    hardware, oracle = opt("--hardware", ""), opt("--oracle", os.path.join(root, "target", "release", "x3d-oracle"))
    assert os.environ.get("X3D_ORACLE_CHROME") and os.environ.get("X3D_ORACLE_ANGLE"), "set X3D_ORACLE_CHROME and X3D_ORACLE_ANGLE"
    sha = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest()
    tmp = tempfile.mkdtemp(prefix="b69-gs-pick-platform-")
    keep, same = [], []
    for log in (False, True):
        base = "b69-gs-pick-log-baseline" if log else "b69-gs-pick-baseline"
        for msaa in ("0", "4"):
            for name in CASES:
                runs = []
                for i in (1, 2, 3):
                    out = os.path.join(tmp, f"{base}-{name}-ms{msaa}-{i}")
                    run_oracle(oracle, name, msaa, log, out)
                    runs.append(os.path.join(out, "js.json"))
                if not all(filecmp.cmp(runs[0], b, shallow=False) for b in runs[1:]):
                    sys.exit(f"{base} {name} msaa {msaa}: the three runs differ; nothing written (staging {tmp})")
                if filecmp.cmp(runs[0], os.path.join(root, "oracle", base, f"msaa{msaa}", name, "js.json"), shallow=False):
                    same.append(f"{base} {name}-msaa{msaa}")
                else:
                    keep.append((base, f"{name}-msaa{msaa}", runs[0]))
    pinned = open(os.path.join(root, "oracle", "x_ite-pin", "PINNED_COMMIT")).read().strip()
    chrome = subprocess.run([os.environ["X3D_ORACLE_CHROME"], "--version"], capture_output=True, text=True).stdout.strip()
    for base in sorted({k[0] for k in keep}):
        dest = os.path.join(root, "oracle", f"{base.removesuffix('-baseline')}-{plat}-baseline")
        os.makedirs(dest, exist_ok=True)
        cases = {}
        for b, case, src in keep:
            if b == base:
                shutil.copyfile(src, os.path.join(dest, case + ".json"))
                cases[case] = {"js.json": sha(src)}
        prov = dict(platform=plat, hardware=hardware, chrome=chrome, angle_backend=os.environ["X3D_ORACLE_ANGLE"],
                    pinned_x_ite="16.5.0 " + pinned, recorder="x3d-oracle particles (oracle/tools/b69_gs_pick_record.py platform)",
                    recorded=datetime.date.today().isoformat(), cases=cases,
                    rule="3 byte-identical runs; kept only where the NVIDIA recording differs")
        json.dump(prov, open(os.path.join(dest, "PROVENANCE.json"), "w"), indent=1, sort_keys=True)
    shutil.rmtree(tmp, ignore_errors=True)
    print(f"b69-gs-pick/{plat}: {len(keep) + len(same)} recordings made three times; {len(keep)} kept, {len(same)} equal to the NVIDIA recording")
    for k in keep:
        print("kept", k[0], k[1])


mode, rest = sys.argv[1], sys.argv[2:]
assert mode in ("record", "platform"), "record NAME MSAA [X3D_ORACLE] | platform PLATFORM --hardware TEXT [--oracle X3D_ORACLE]"
(record if mode == "record" else platform)(rest)
