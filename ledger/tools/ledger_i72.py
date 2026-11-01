#!/usr/bin/env python3
"""Issue #72 ledger update (node:ComposedShader): python3 -I ledger_i72.py check|apply

transformFeedbackVaryings is not a field of ComposedShader in pinned X_ITE 16.5.0: it is an internal property set only by X_ITE's own LineTransform shaders
(wide lines, instancing) and the particle emitter programs, never from scene content. Those uses are compared with X_ITE (rea_b21_wide_lines, the particle suites).
The stale "not ported" text is removed and the entry is COMPLETE. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i72", "ledger.json")
SNAP_SHA256 = "f0693ef96e3f4801dcac7f0ccaeecb4b07727c2ae26bea05c77389e1cc78fc08"

T = ["crates/x3d-viewer/tests/rea_b21_wide_lines.rs (rea_b21_wide_lines_match_x_ite, rea_b21_wide_lines_cpu_collision_match_x_ite: the LineTransform shader that uses transformFeedbackVaryings)"]
FIELDS = {"tests": lambda old: old + T, "missing": ""}
NOTE = "Issue #72 2026-10-31: ComposedShader.js declares no transformFeedbackVaryings field (fieldDefinitions: metadata, activate, isSelected, isValid, language, parts, user fields); setTransformFeedbackVaryings is called only by X3DShadersContext.createShader for the built-in LineTransform shaders (X3DShapeContext) and the particle emitters (X3DParticleEmitterNode), so no scene can set it. The port draws wide lines and particles with its own GPU passes, compared with X_ITE. Includes that need macro context do not link standalone in X_ITE either (the ctx-* worlds of batch 08 link them in context)."
C = {"node:ComposedShader": ("COMPLETE", FIELDS, NOTE)}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, note = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            if note: i["notes"] = (i["notes"] + " | " if i["notes"] else "") + note
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i72.py check|apply")
    if not snap_ok(): sys.exit(1)
    if sys.argv[1:] == ["apply"]:
        d = build()
        try: same = sha256(AUTH) == SNAP_SHA256 or load(AUTH) == d
        except ValueError: same = False
        if not same:
            print("ledger.json is neither the snapshot nor the result of this change set: refused"); sys.exit(1)
        tmp = AUTH + ".tmp"
        with open(tmp, "w") as f:
            json.dump(d, f, indent=1); f.flush(); os.fsync(f.fileno())
        os.replace(tmp, AUTH); sys.exit(0 if check(load(AUTH)) else 1)
    sys.exit(0 if check(load(AUTH)) else 1)
