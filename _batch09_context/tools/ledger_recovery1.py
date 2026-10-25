#!/usr/bin/env python3
"""Recovery ledger update (applied in PR #99, merge 785769b): python3 -I ledger_recovery1.py check|apply

Starts from ledger/before-recovery1/ledger.json (the authoritative ledger at origin/master cdfbffc, sha256 d6d7fd30...6953:
249 COMPLETE, 70 PARTIAL, 25 DIFFERENT, 2 MISSING). Ten entries change. One changes status: sub:browser.keyboard-ui MISSING ->
PARTIAL (PR #15). Nine keep their status and get text or path corrections. No DIFFERENT entry is in the change set. Earlier notes
stay as a prefix. Result: 249 COMPLETE, 71 PARTIAL, 25 DIFFERENT, 1 MISSING.

Revision of the 2026-10-25 recovery proposal (LEDGER_PROPOSAL.diff), after the Phase 2 review:
  - sub:browser.keyboard-ui: missing also lists commandKey / altGrKey and Wayland / macOS validation (PR #15 body, line 61)
  - node:Collision: collideTime is compared within 1e-9, not exactly (rea_b15_particle_collision.rs lines 18, 192)
  - node:ParticleSystem: only the two fixed clauses leave `missing`; the rest stays verbatim
  - sub:io.file-loader: the Script case uses text data: URLs, not base64

The snapshot, this tool and the verify_ledger.py HISTORY step are installed, and ledger.json holds the result (sha256
41cd5e2d...66ef). Both commands refuse a snapshot whose sha256 is not SNAP_SHA256. apply refuses to write over a ledger that
is neither the snapshot nor the result of this change set, so it cannot discard a later ledger change."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-recovery1", "ledger.json")
SNAP_SHA256 = "d6d7fd3083820f7e49aa7b2f4634b449f9f1321598b6f8de7ba0e25dee806953"  # ledger.json at origin/master cdfbffc
NOTE = "Recovery audit 2026-10-25 (origin/master cdfbffc): "
PS_FIXED = ["wide LINE particles are 1 px in the shadow, depth (collision) and pointing passes (PR #29 changed the color pass "
            "only); ",
            "one draw call per particle (instance data through per-draw uniforms) and one pointer ShapeInst per particle per frame; "]
PRELUDE = "rust path crates/x3d-script/src/prelude.js was split into crates/x3d-script/src/prelude/ in 6a1305c."
STALE_06R = ("the Batch 06R gap 'PolylineEmitter, WindPhysicsModel not ported' is stale: node:WindPhysicsModel and "
             "node:PolylineEmitter have their own entries with Rust paths.")

def ps_missing(old):
    for s in PS_FIXED:
        if s not in old: raise SystemExit(f"node:ParticleSystem: expected clause not found: {s!r}")
        old = old.replace(s, "")
    return old

def swap(old, a, b):
    if a not in old: raise SystemExit(f"rust path not found: {a}")
    return [b if r == a else r for r in old]

# id: (status or None, {field: value or function of the old value}, note). The verifier reads the first element.
C = {
 "sub:browser.keyboard-ui": ("PARTIAL", {
    "rust": ["crates/x3d-viewer/src/keys.rs", "crates/x3d-viewer/src/viewer.rs", "crates/x3d-viewer/src/window.rs"],
    "tests": ["crates/x3d-viewer/tests/native_keys.rs (6)",
              "crates/x3d-viewer/src/keys.rs unit tests (10)",
              "oracle/synthetic-rea/key-sensors.wrl (x10_compare.py vs X_ITE 16.5.0: 342 PASS, 0 FAIL)"],
    "missing": "ContextMenu and Notification are not ported; Debug shading keys (Debug-option Ctrl+1..5 / Ctrl+S), Ctrl+Plus "
               "timings and Shift+F8 viewpoint copy are not compared with X_ITE; KeySensor commandKey / altGrKey fields not "
               "compared; Wayland and macOS keyboard input not validated against X_ITE"},
   NOTE + "PR #15 (lane feat/viewer-keyboard-input, merge 3606b2a, 2026-10-23) ported X3DCoreContext key handling: every native "
   "key reaches KeySensor / StringSensor through Viewer3d::key_event, Home / End / PageUp / PageDown change the viewpoint without "
   "a modifier, held keys are released on focus loss. PR #15 did not update the ledger; its body proposed PARTIAL."),
 "node:ParticleSystem": (None, {"missing": ps_missing},
   NOTE + "removed from missing: wide LINE particles in the shadow, depth and pointing passes (B21, PR #35, merge 6264ebd; two "
   "layers B22, PR #36, merge 1005fa5; both compared with pinned X_ITE 16.5.0); one draw call per particle (PR #39, merge "
   "92a76ee); one pointer ShapeInst per particle per frame (PR #40, merge 9762b4e, collision; PR #43, merge d139bf5, pointing). "
   "Status stays PARTIAL."),
 "node:Collision": (None, {"missing": lambda m: m.replace("Collision collideTime path and Billboard orientation not exercised",
                                                         "Billboard orientation not exercised")},
   NOTE + "Collision isActive (exact) and collideTime (within 1e-9) are compared with X_ITE in particle collision scenes in "
   "crates/x3d-viewer/tests/rea_b15_particle_collision.rs (PR #26, merge d2666bd; CPU stand-in PR #32, merge d4cd4d5), so "
   "'collideTime path not exercised' was removed."),
 "node:BoundedPhysicsModel": (None, {}, NOTE + STALE_06R),
 "node:VolumeEmitter": (None, {}, NOTE + STALE_06R),
 "sub:render.fog": (None, {"missing": "no subsystem-level test listed; node:Fog, node:LocalFog and node:FogCoordinate are COMPLETE"},
   NOTE + "'LocalFog, FogCoordinate' removed from missing: node:LocalFog became COMPLETE in b11 and node:FogCoordinate is "
   "COMPLETE."),
 "sub:io.file-loader": (None, {"missing": "non-VRML formats and fetch error messages not compared at this level; no "
                                          "subsystem-level test listed"},
   NOTE + "data: URLs are compared through node:Script (oracle/synthetic-rea-b09-sai/scr-url-order-micro.wrl: data:text/javascript, "
   "data:, data:text/plain) and node:ShaderPart (oracle/synthetic-rea-b08-shaders/data-base64.wrl)."),
 "sub:exec.context-api": (None, {"rust": lambda r: swap(r, "crates/x3d-script/src/prelude.js", "crates/x3d-script/src/prelude/")},
   NOTE + PRELUDE),
 "sub:browser.sai-api": (None, {"rust": lambda r: swap(r, "crates/x3d-script/src/prelude.js", "crates/x3d-script/src/prelude/")},
   NOTE + PRELUDE),
 "sub:physics.engine": (None, {"rust": lambda r: swap(r, "crates/x3d-runtime/src/pick_shapes.rs",
                                                      "crates/x3d-viewer/src/pick_shapes.rs")},
   NOTE + "rust path pick_shapes.rs is crates/x3d-viewer/src/pick_shapes.rs (git history has no crates/x3d-runtime/src/pick_shapes.rs)."),
}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, note = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            i["notes"] = (i["notes"] + " | " if i["notes"] else "") + note
    return d
def check(d):
    bad, snap = [], {i["id"]: i for i in load(SNAP)["items"]}
    cur = {i["id"]: i for i in d["items"]}
    if set(cur) != set(snap) or len(d["items"]) != 346: bad.append("ID set or total changed")
    if [i["id"] for i in d["items"]] != [i["id"] for i in load(SNAP)["items"]]: bad.append("ID order changed")
    want = {i["id"]: i for i in build()["items"]}
    for k, b in snap.items():
        a = cur[k]
        if k not in C and a != b: bad.append(f"unrelated entry changed: {k}")
        if b["status"] == "DIFFERENT" and a != b: bad.append(f"DIFFERENT changed: {k}")
        if k in C and (a["status"] != (C[k][0] or b["status"]) or not a["notes"].startswith(b["notes"])): bad.append(f"{k}: status or notes")
        if k in C and a != want[k]: bad.append(f"{k}: differs from what apply writes")
    c = collections.Counter(i["status"] for i in d["items"]); print(dict(sorted(c.items())))
    print("\n".join(bad) or "OK"); return not bad
def sha256(p):
    with open(p, "rb") as f: return hashlib.sha256(f.read()).hexdigest()
def snap_ok():
    got = sha256(SNAP)
    if got != SNAP_SHA256: print(f"snapshot sha256 {got} is not the expected {SNAP_SHA256}: refused"); return False
    return True
if __name__ == "__main__":
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_recovery1.py check|apply")
    if not snap_ok(): sys.exit(1)
    if sys.argv[1:] == ["apply"]:
        d = build()
        if sha256(AUTH) != SNAP_SHA256 and load(AUTH) != d:
            print("ledger.json is neither the snapshot nor the result of this change set: refused"); sys.exit(1)
        json.dump(d, open(AUTH, "w"), indent=1); sys.exit(0 if check(load(AUTH)) else 1)
    sys.exit(0 if check(load(AUTH)) else 1)
