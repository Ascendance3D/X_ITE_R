#!/usr/bin/env python3
"""Ledger step of sub:exec.context-api, 2026-11-13 (compatibility closure report): the `missing` field still said the
port's scene objects are plain objects (brx-obj-open). Issue #77 (closed 2026-10-31) found the class chain already ported
by commit 5ec9ae2b (X3DScene > X3DExecutionContext > X3DBaseNode > X3DChildObject > X3DObject, prelude/rest.js), and the
recorded cases scx-api, scx-members and scx-print of rea_batch09_sai compare it with pinned X_ITE in CI. Text and evidence
only: the entry stays PARTIAL by the owner decision of 2026-10-20.
python3 -I ledger/tools/ledger_ctxapi.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-ctxapi", "ledger.json")
SNAP_SHA256 = "9b584c7c9d661882619851ddc15333e5b0132e12c581e126577f7848f12b03c5"

def append(*extra):
    return lambda old: list(old) + list(extra)

C = {
 "sub:exec.context-api": (None, {
   "missing": "kept PARTIAL by owner decision 2026-10-20 until its full REA requirements pass; that decision does not list the remaining requirements, so a promotion needs owner review. The X3DExecutionContext / X3DScene class chain (brx-obj-open, named here before) is ported (commit 5ec9ae2b, issue #77) and compared (scx-api, scx-members, scx-print)",
   "tests": append("crates/x3d-viewer/tests/rea_batch09_sai.rs scx-api scx-members (instanceof, prototype chain, member owners, typeName and toString of Browser.currentScene, getExecutionContext () and created scenes; oracle/rea-b09-sai-baseline, issue #77)"),
  }, "2026-11-13 (compatibility closure report): `missing` no longer says the scene objects are plain objects; issue #77 found the class chain ported by commit 5ec9ae2b (crates/x3d-script/src/prelude/rest.js X3DExecutionContextClass, X3DSceneClass) and compared by scx-api, scx-members and scx-print. Text and evidence only; the entry stays PARTIAL (owner decision 2026-10-20)."),
}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, nt = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            if nt: i["notes"] = (i["notes"] + " | " if i["notes"] else "") + nt
    return d
def check(d):
    bad, snap = [], {i["id"]: i for i in load(SNAP)["items"]}
    cur = {i["id"]: i for i in d["items"]}
    if set(cur) != set(snap) or len(d["items"]) != 346: bad.append("ID set or total changed")
    if [i["id"] for i in d["items"]] != [i["id"] for i in load(SNAP)["items"]]: bad.append("ID order changed")
    bad += [f"{k}: not in the snapshot" for k in sorted(set(C) - set(snap))]
    built = build()
    if {k: v for k, v in d.items() if k != "items"} != {k: v for k, v in built.items() if k != "items"}: bad.append("top-level fields differ from what apply writes")
    want = {i["id"]: i for i in built["items"]}
    for k, b in snap.items():
        a = cur.get(k)
        if a is None: continue  # reported above: ID set or total changed
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_ctxapi.py check|apply")
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
