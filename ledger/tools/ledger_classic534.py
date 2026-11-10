#!/usr/bin/env python3
"""Ledger step of PR #534 (2026-11-10): the classic encoding under ADR 0002 ("Do both") and the C18 mid-transition probe
classified as an upstream defect. python3 -I ledger/tools/ledger_classic534.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-classic534", "ledger.json")
SNAP_SHA256 = "ffcdf5395fcda45a8d1338578d8cc754d4f77d91504021631fc17e591ba60eec"

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
 "sub:parse.vrml-classic": ("COMPLETE", {
   "missing": lambda old: "",
   "tests": append("crates/x3d-parse/tests/classic_annotation.rs (ADR 0002 \"Do both\" in the classic encoding: the recorded V3.3 cases sweep-annotation-1/-2 that the pinned parser rejects are accepted with the component, their nodes unknown by X_ITE's version rule (from 4.0); in V4.0 the six nodes load with the fields of the dormant classes, levels above 2 clamp as X3DBrowser.getComponent; without the component the nodes are unknown as in X_ITE)"),
  }, "PR #534 (2026-11-10): the open Annotation decision is ADR 0002 (Accepted, owner decision 2026-10-17, \"Do both\"); the two known rows sweep-annotation-1/-2 cite it, and classic_annotation.rs pins both sides. Verified at this head: classic_edge (2157 cases, only the 8 known rows differ), classic_edge_facade, classic_edge_runtime (the 6 log rows equal pinned X_ITE in the runtime), classic_annotation 3/3, rea_batch04 (runtime behavior of the Annotation nodes against the dormant classes and the stand-ins), and the corpus tests x0_corpus_matches_x_ite, x0_matches_x_ite and items_parse_only with the private keys found (no evidence skip). REA: analysis ev_bdb28dc35b50b73af2c0a0f9bae3faab1c34b450a2bd853bcfaf96bfa886153e (summary ev_ed4b3ac34d8319f9e34fba3a8f7f7e0c246d369387be082d7aceae25d7d17e3e), partial graph. No other gap is listed: COMPLETE."),
 "sub:nav.collision-gravity": (None, {
   "missing": swap("with one known gap: a probe in the middle of a transition reads X_ITE's stale shapes;", "with one recorded difference: a probe in the middle of a transition reads X_ITE's stale shapes, an upstream defect the port does not copy (X3DRenderObject.getCollisionShape draws the shapes with the view matrix of the last collision pass, getClosestObject with the current eye; the pinned probes at frame 40 equal those at frame 10 although the eye rose 2.2 m);"),
   "tests": append("crates/x3d-viewer/tests/rea_gravity_layers.rs (x_ite_mid_transition_probes_follow_the_old_eye: the pinned recordings show the stale read; trans-skip-probe and trans-layer-probe stay in KNOWN_GAPS so the port keeps fresh shapes)"),
  }, "PR #534 (2026-11-10): C18 mid-transition probe classified. X3DLayerNode.collision returns early while transitionActive (lines 466-467), so collisionShapes keep their model-view matrices of the last pass; getClosestObject (X3DRenderObject.js 470-485, 513-529) multiplies them with a view matrix of the current user position, so the result follows the old eye, not the geometry. getClosestObject is an X_ITE extension; ISO/IEC 19775-1 V4.0 23.2.4 ties collision detection to the objects of the scene. The port keeps its fresh shapes. REA: analysis ev_bdb28dc35b50b73af2c0a0f9bae3faab1c34b450a2bd853bcfaf96bfa886153e, module trace ev_94297619aaada34f6ef3bfb1be12f333446490a64dadb104fc51d808d4934563 (partial graph; the line reading is from the pinned source text). A status change for a kept difference is the owner's: the entry stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_classic534.py check|apply")
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
