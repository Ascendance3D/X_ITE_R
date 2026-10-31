#!/usr/bin/env python3
"""Issue #85 ledger update (fill the empty rust fields): python3 -I ledger_i85.py check|apply

Starts from ledger/before-i85/ledger.json (sha256 3321b7e5fbf8bf267df2bb33daa914938c715cb2bbe1887f5d2ee9c58ccd9550). node:AcousticProperties and sub:render.clip-planes are COMPLETE
with tests, but their rust field was empty. AcousticProperties has no Rust file of its own: the node is generated from the node
table crates/x3d-scene/data/x_ite-nodes.json. ClipPlane is read by the render traversal (traverse.rs), the pointing walk
(walk.rs), the depth pass (depth_gpu.rs) and the shader include (01-ClipPlanes.glsl). Only the rust field changes: status and
notes are unchanged. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i85", "ledger.json")
SNAP_SHA256 = "3321b7e5fbf8bf267df2bb33daa914938c715cb2bbe1887f5d2ee9c58ccd9550"

def fill(paths):
    def f(old):
        if old != []: raise SystemExit(f"rust field is not empty: {old!r}")
        return paths
    return f

# id: (status or None, {field: value or function of the old value}, note or None). The verifier reads the first element.
C = {
 "node:AcousticProperties": (None, {"rust": fill(["crates/x3d-scene/data/x_ite-nodes.json"])}, None),
 "sub:render.clip-planes": (None, {"rust": fill(["crates/x3d-render/src/traverse.rs", "crates/x3d-viewer/src/walk.rs",
    "crates/x3d-render/src/depth_gpu.rs", "crates/x3d-runtime/src/nodes/shader_includes/01-ClipPlanes.glsl"])}, None),
}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i85.py check|apply")
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
