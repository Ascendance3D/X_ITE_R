#!/usr/bin/env python3
"""Issue #76 ledger update (toString of the math classes): python3 -I ledger_i76.py check|apply

Starts from ledger/before-i76/ledger.json (sha256 c35278e95daa5701da50dae5ab031dd8b3bcd78c0ec974765b83e64b95b20986). sub:fields.math said "toString methods of the math classes are not
ported". Complex, Matrix2 and the geometry classes (Box2, Box3, Cylinder3, Line2, Line3, Plane3, Sphere3, Spheroid3) now have
Display, and crates/x3d-viewer/tests/math_tostring.rs compares all 19 classes of pinned X_ITE 16.5.0 (34cdf9f97) on 3882
cases (oracle/math-tostring-baseline.json, written by oracle/tools/math_tostring_xite.mjs, three identical runs). Nothing else
was open, so the entry becomes COMPLETE. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i76", "ledger.json")
SNAP_SHA256 = "c35278e95daa5701da50dae5ab031dd8b3bcd78c0ec974765b83e64b95b20986"
NOTE = ("Issue #76 2026-10-31: toString of all 19 math classes (Numbers: Complex, Color3, Color4, Matrix2, Matrix3, Matrix4, "
        "Quaternion, Rotation4, Vector2, Vector3, Vector4; Geometry: Box2, Box3, Cylinder3, Line2, Line3, Plane3, Sphere3, "
        "Spheroid3) equals pinned X_ITE 16.5.0 on 3882 cases, three identical recordings.")
OLD_MISSING = "toString methods of the math classes are not ported"
TEST = ("crates/x3d-viewer/tests/math_tostring.rs (oracle/math-tostring-baseline.json from oracle/tools/math_tostring_xite.mjs, "
        "3882 cases, 19 classes)")

def fix_missing(old):
    if old != OLD_MISSING: raise SystemExit(f"unexpected missing text: {old!r}")
    return ""

# id: (status or None, {field: value or function of the old value}, note). The verifier reads the first element.
C = {"sub:fields.math": ("COMPLETE", {"missing": fix_missing, "tests": lambda old: old + [TEST]}, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i76.py check|apply")
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
