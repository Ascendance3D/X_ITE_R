#!/usr/bin/env python3
"""Ledger step of the README and ledger reconciliation, 2026-11-13: the `missing` field of two PARTIAL entries is brought
in line with their own notes. Text only: no status changes, no new evidence.
  - node:ShaderPart: `missing` named one gap; its last note (PRs #587, #588, #590, #593) names three.
  - sub:parse.gltf2: `missing` said MP4 sources are "not decoded"; since ADR 0015 (node:MovieTexture) MP4 is decoded by the
    platform backends of non-default features. The glTF MP4 case is still not compared.
python3 -I ledger/tools/ledger_missingsync.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-missingsync", "ledger.json")
SNAP_SHA256 = "1f9080cd8a993589fd7469ef927762a5847cf6e7d45a51348d46225b1ac07e1b"

def replace(old, new):
    def f(v):
        if v.count(old) != 1: raise SystemExit(f"expected one occurrence of {old!r}")
        return v.replace(old, new)
    return f

C = {
 "node:ShaderPart": (None, {
   "missing": "type is initializeOnly so no stepped type case exists (parts-runtime and the bad-type worlds cover it); the pinned mediump picture difference (max 1 per channel, PINNED table of rea_b24_shaderpart); GLSL ES type checks cover the recorded patterns only (no full GLSL ES type checker): not covered are a float loop condition, an int expression (not a literal) into a float, matrix and struct constructors, and a constructor argument of unknown size before the constructor is full",
  }, "2026-11-13 (README and ledger reconciliation): `missing` now lists the three gaps that the note of PRs #587, #588, #590, #593 names; text only, no new evidence."),
 "sub:parse.gltf2": (None, {
   "missing": replace("MP4 sources (not decoded, node:MovieTexture)", "MP4 sources (decoded only by the platform backends of non-default features, ADR 0015, node:MovieTexture; no glTF MP4 case is compared)"),
  }, "2026-11-13 (README and ledger reconciliation): the MP4 clause of `missing` follows node:MovieTexture (ADR 0015); text only, no new evidence."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_missingsync.py check|apply")
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
