#!/usr/bin/env python3
"""Ledger step of PR #539 (2026-11-10): Text output fields under InstancedShape and non-finite STL values compared with pinned
X_ITE; node:InstancedShape has no open item left.
python3 -I ledger/tools/ledger_stl539.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-stl539", "ledger.json")
SNAP_SHA256 = "adbd3e6f918704fb6fef8135fba295e121b109085174a97ff5b5e74eb06c1b9c"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f

def swap_in_list(old_text, new_text):
    def f(old):
        assert sum(s.count(old_text) for s in old) == 1, old_text
        return [s.replace(old_text, new_text) for s in old]
    return f

def append(*extra):
    return lambda old: list(old) + list(extra)

C = {
 "node:InstancedShape": ("COMPLETE", {
   "missing": swap("; Text under InstancedShape untested (X_ITE headless shot draws no glyphs).", "; Text under InstancedShape is compared: scene text, log, poses, glyph pictures and a 2601-probe pick grid at MSAA 0 and 4 (instanced_text, PR #435), and the Text output fields textBounds, lineBounds, origin, length and maxExtent of 7 InstancedShape Text nodes and a plain Shape control, before and after a run-time change of string and size (instanced_text it-fields, PR #539). Not reachable: a bounding box of an instanced Text, because the pinned page has no scriptable bbox getter."),
   "tests": append("crates/x3d-viewer/tests/instanced_text.rs it-fields (oracle/instanced-text-baseline/it-fields-msaa0/4, pinned X_ITE 16.5.0, three identical recordings; PR #539)"),
  }, "PR #539 (2026-11-10): the open item (Text under InstancedShape) was stale: instanced_text.rs (PR #435) already compares glyph pictures, poses, log and picking. The one uncompared part, the Text output fields, is now compared (case it-fields: FontStyle on and off, several lines, length and maxExtent, vertical, right to left, an empty string, instance translations, scales and rotations, a run-time change). The port equals the pinned page with no Rust change. Tests: instanced_text 2/2, instanced_pick 1/1. REA: analysis of InstancedShape.js, Text.js, X3DGeometryNode.js, STLAParser.js and STLBParser.js ev_fd1fa7bc0687fa835013dc481220d41352e28bb763284b2ec847d2329344de33 (summary ev_b138924854b4c100f350b97603caa6a0a86aad66b5abd465de1f903679c8377e), partial graph. No other gap is listed: COMPLETE."),
 "sub:parse.golden-gate": (None, {
   "missing": swap("NaN STL coordinates (the pinned page logs a stack overflow) are not compared;", "NaN, +Inf and -Inf in STL coordinates and normals are compared (golden_gate_binary gg-stl-nonfinite, binary and ASCII, PR #539): scene text exact; the pinned ASCII parser accepts only a signed nan or inf constant, as the port does; the pinned stack overflow comes from MikkTSpace.generateTangents (X3DGeometryNode.generateTangents), not from a parser, and is logged only in a DEVELOPMENT build, so those log lines are a named upstream-defect difference (is_pinned_wasm_crash: the pinned log must still hold them and the port must log none);"),
   "tests": append("crates/x3d-viewer/tests/golden_gate_binary.rs golden_gate_stl_nonfinite_matches_x_ite (gg-stl-nonfinite: 34 synthetic STL files, three identical recordings; PR #539)"),
  }, "PR #539 (2026-11-10): the NaN STL gap is closed with no Rust change. The pinned tangent generator (WebAssembly MikkTSpace) throws RangeError: Maximum call stack size exceeded for a NaN coordinate and RuntimeError: unreachable for an infinite one, then memory access out of bounds for later shapes; the geometry keeps no tangents and the scene loads. A crash of the tangent generator is not required behavior, so the port does not copy it. Tests: golden_gate_binary 7/7. REA: ev_fd1fa7bc0687fa835013dc481220d41352e28bb763284b2ec847d2329344de33 (summary ev_b138924854b4c100f350b97603caa6a0a86aad66b5abd465de1f903679c8377e). Status stays PARTIAL: the entry's behavior and oracle fields are empty, so the status change is the owner's."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_stl539.py check|apply")
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
