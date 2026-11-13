#!/usr/bin/env python3
"""Ledger step of the four glsl_errors_b08 compile-error gaps of issue #95 (PRs #587, #588, #590, #593, quick-wins lane), 2026-11-12:
all 16 b08 compile-error cases equal pinned X_ITE. node:ShaderPart stays PARTIAL. Evidence only: no status changes.
python3 -I ledger/tools/ledger_shaderglsl.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-shaderglsl", "ledger.json")
SNAP_SHA256 = "768cfbde77d2365100393000d8a71eccc55a4bfebe2f70be47680f2119abd59f"

def append(*extra):
    return lambda old: list(old) + list(extra)

C = {
 "node:ShaderPart": (None, {
   "tests": append('crates/x3d-render/tests/glsl_es_int_float.rs, glsl_es_bool_condition.rs, glsl_es_dimension.rs, glsl_es_constructor_args.rs (the GLSL ES type checks of #587, #588, #590, #593)', 'crates/x3d-render/tests/glsl_errors_b08.rs (16 compile-error cases equal to pinned X_ITE, oracle/rea-b08-shaders-baseline errors-runtime; GAPS empty)'),
  }, "Issue #95 2026-11-12 (PRs #587, #588, #590, #593, quick-wins lane): the 4 glsl_errors_b08 gaps are closed: an int literal into a float, an if condition that is not bool, a dimension mismatch and a constructor with too many arguments give ANGLE's compile error; all 16 b08 compile-error cases equal the pinned X_ITE log (GAPS empty). The checks cover the recorded patterns only: not covered are a float loop condition, an int expression (not a literal) into a float, matrix and struct constructors, and a constructor argument of unknown size before the constructor is full; the port has no full GLSL ES type checker. Stays PARTIAL: type is initializeOnly (no stepped type case) and the pinned mediump picture difference (max 1 per channel)."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_shaderglsl.py check|apply")
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
