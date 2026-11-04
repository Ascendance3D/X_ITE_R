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


# Cases that get no platform file even when they differ (owner decision 2026-11-04, issue #69): gs-pick-ortho's hit distance
# follows the driver's GLSL inverse () (docs/metal-reference-variation.md); it stays on the NVIDIA recording.
NOT_KEPT = {"gs-pick-ortho": "driver inverse () of the projection matrix (owner decision 2026-11-04, issue #69)"}
PLATFORMS = ("apple-m1",)


def platform(args):
    plat = args[0] if args else ""
    opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
    hardware, oracle = opt("--hardware", ""), opt("--oracle", os.path.join(root, "target", "release", "x3d-oracle"))
    # Explicit checks, not assert: they must also hold under python3 -O.
    if plat not in PLATFORMS:
        sys.exit(f"platform must be one of {PLATFORMS}")
    if not hardware.strip():
        sys.exit("--hardware TEXT is required (the machine, GPU and OS of the recording)")
    if not (os.environ.get("X3D_ORACLE_CHROME") and os.environ.get("X3D_ORACLE_ANGLE")):
        sys.exit("set X3D_ORACLE_CHROME and X3D_ORACLE_ANGLE (x3d-oracle starts that Chrome with that ANGLE backend: crates/x3d-oracle/src/main.rs, chrome.rs)")
    # The platform must be the machine that records: apple-m1 is macOS on arm64 with ANGLE Metal.
    import platform as host
    if plat == "apple-m1":
        chip = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True).stdout.strip() if sys.platform == "darwin" else ""
        # "Apple M1", "Apple M1 Pro", ... are M1 chips; "Apple M10" would not be.
        if not (sys.platform == "darwin" and host.machine() == "arm64" and (chip == "Apple M1" or chip.startswith("Apple M1 "))
                and os.environ["X3D_ORACLE_ANGLE"] == "metal"):
            sys.exit(f"apple-m1 needs an Apple M1 (sysctl machdep.cpu.brand_string), macOS arm64 and X3D_ORACLE_ANGLE=metal "
                     f"(this host: {sys.platform} {host.machine()} {chip or 'no chip name'}, ANGLE {os.environ['X3D_ORACLE_ANGLE']})")
    def sha(p):
        with open(p, "rb") as f:
            return hashlib.sha256(f.read()).hexdigest()
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
                    f32 = os.path.join(out, "js.f32")
                    if os.path.exists(f32) and os.path.getsize(f32) != 0:
                        sys.exit(f"{f32}: js.f32 is expected empty for the pick cases; nothing written")
                if not all(filecmp.cmp(runs[0], b, shallow=False) for b in runs[1:]):
                    sys.exit(f"{base} {name} msaa {msaa}: the three runs differ; nothing written (staging {tmp})")
                if name in NOT_KEPT or filecmp.cmp(runs[0], os.path.join(root, "oracle", base, f"msaa{msaa}", name, "js.json"), shallow=False):
                    same.append(f"{base} {name}-msaa{msaa}")
                else:
                    keep.append((base, f"{name}-msaa{msaa}", runs[0]))
    # A pick does not depend on the multisampling (b69_gs_pick.rs asserts MSAA 0 == MSAA 4), so a case is kept at both levels
    # or at neither: the test takes one platform flag for the two.
    kept = {(b, c.rsplit("-msaa", 1)[0], c.rsplit("-msaa", 1)[1]) for b, c, _ in keep}
    for b, n, _ in kept:
        if {m for b2, n2, m in kept if (b2, n2) == (b, n)} != {"0", "4"}:
            sys.exit(f"{b} {n}: kept at one MSAA level only; nothing written")
    with open(os.path.join(root, "oracle", "x_ite-pin", "PINNED_COMMIT")) as f:
        pinned = f.read().strip()
    chrome = subprocess.run([os.environ["X3D_ORACLE_CHROME"], "--version"], capture_output=True, text=True).stdout.strip()
    # Each destination is staged whole and then replaces the old one, so no file of an earlier recording survives (a case
    # now equal to the NVIDIA recording loses its platform file) and an interrupted run leaves the old directory intact.
    for base in ("b69-gs-pick-baseline", "b69-gs-pick-log-baseline"):
        final = os.path.join(root, "oracle", f"{base.removesuffix('-baseline')}-{plat}-baseline")
        dest = final + ".staging"
        shutil.rmtree(dest, ignore_errors=True)
        if not any(k[0] == base for k in keep):
            shutil.rmtree(final, ignore_errors=True)
            continue
        os.makedirs(dest)
        cases = {}
        for b, case, src in keep:
            if b == base:
                shutil.copyfile(src, os.path.join(dest, case + ".json"))
                cases[case] = {"js.json": sha(src)}
        prov = dict(platform=plat, hardware=hardware, chrome=chrome, angle_backend=os.environ["X3D_ORACLE_ANGLE"],
                    pinned_x_ite="16.5.0 " + pinned, recorder="x3d-oracle particles (oracle/tools/b69_gs_pick_record.py platform)",
                    recorded=datetime.date.today().isoformat(), cases=cases,
                    rule="3 byte-identical runs; kept only where the NVIDIA recording differs",
                    not_kept={n: why for n, why in NOT_KEPT.items()},
                    equal_to_nvidia=sorted(c.split(" ", 1)[1] for c in same if c.startswith(base + " ")
                                           and c.split(" ", 1)[1].rsplit("-msaa", 1)[0] not in NOT_KEPT))
        with open(os.path.join(dest, "PROVENANCE.json"), "w") as f:
            json.dump(prov, f, indent=1, sort_keys=True)
        old = final + ".old"
        # An interrupted earlier run can have left only <final>.old: put it back before the swap.
        if not os.path.exists(final) and os.path.exists(old):
            os.rename(old, final)
        shutil.rmtree(old, ignore_errors=True)
        if os.path.exists(final):
            os.rename(final, old)
        try:
            os.rename(dest, final)
        except BaseException:
            if os.path.exists(old) and not os.path.exists(final):
                os.rename(old, final)
            raise
        shutil.rmtree(old, ignore_errors=True)
    shutil.rmtree(tmp, ignore_errors=True)
    print(f"b69-gs-pick/{plat}: {len(keep) + len(same)} recordings made three times; {len(keep)} kept, {len(same)} equal to the NVIDIA recording")
    for k in keep:
        print("kept", k[0], k[1])


mode, rest = sys.argv[1], sys.argv[2:]
if mode not in ("record", "platform"):
    sys.exit("record NAME MSAA [X3D_ORACLE] | platform PLATFORM --hardware TEXT [--oracle X3D_ORACLE]")
(record if mode == "record" else platform)(rest)
