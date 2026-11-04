#!/usr/bin/env python3
"""Ledger step after PRs #445, #447, #448 (issue #51): python3 -I ledger_i451.py check|apply

Three entries, text only, no status change, no DIFFERENT entry touched. The text is the "Proposed ledger effect" of each merged PR
body, and ADR 0014 section 5 (#447). sub:pointer.picking loses a clause that was stale: DepthMode in the pointing pass was done by
#197 (GPU) and #201 (CPU) and is compared by rea_b23_depthmode_pick (58 cells). #446 and #449 have no ledger effect. Open items stay
visible: the CPU stand-in gaps of node:ParticleSystem, the line and point ulp bounds and touch input of sub:pointer.picking, and the
other Inline differences of ADR 0014.

Counts before and after: 280 COMPLETE / 44 PARTIAL / 21 DIFFERENT / 1 MISSING, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i451", "ledger.json")
SNAP_SHA256 = "fce9dece1c273a49d9460c9d07d8a88f1faff74d43404e6050369460b3a65d61"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f
def chain(*fs):
    def f(old):
        for g in fs: old = g(old)
        return old
    return f
def addtext(text):
    return lambda old: old + "; " + text
def addlist(*texts):
    return lambda old: old + list(texts)
def note(text):
    return lambda old: (old + " | " if old else "") + text

C = {
 "node:ParticleSystem": (None, {
   "missing": swap("POINT particles with PointProperties are 1 px in the collision depth pass (no getPointSize or round mask); ", ""),
   "tests": addlist("crates/x3d-viewer/tests/rea_b15_particle_collision.rs (23 cases, 9 pc-point-props-* for POINT + PointProperties: size, min, max, attenuation, size <= 1, faint (discarded), texture ignored, center outside the view box; GPU and CPU stand-in; 3 identical pinned recordings each, PRs #445, #448)"),
  }, "Issue #51 2026-11-04 (PRs #445, #448): POINT particles with PointProperties are point sprites inside the round point mask in the collision depth pass (getPointSize, Depth2.vs/fs of the pinned page); 9 new pinned cases; also passes on the Apple M1 (Metal) against the NVIDIA recordings. Status stays PARTIAL (the CPU stand-in gaps)."),
 "sub:pointer.picking": (None, {
   "missing": swap("an appearance's DepthMode is not applied in the pointing pass; ", ""),
   "tests": addlist(
     "crates/x3d-viewer/tests/rea_b23_depthmode_pick.rs and rea_b23_depthmode_pick_cpu.rs (58 DepthMode cells: depthTest, depthMask, depthFunction, depthRange, polygonOffset; GPU and CPU pointing; oracle/rea-b23-depthmode-pick-baseline)",
     "crates/x3d-render/tests/pointing_depthmode.rs (7 tests)"),
  }, "Ledger correction 2026-11-04: the clause on DepthMode in the pointing pass was stale; #197 (GPU, afc30fec) and #201 (CPU, bb3c07f5) apply it, compared in 58 of 58 cells. Status stays PARTIAL (line and PointProperties ulp bounds, touch input)."),
 "node:Inline": (None, {
   "missing": addtext("ADR 0014 section 5 (owner-approved 2026-11-04): a request still in flight when a newer request is made is rejected; pinned X_ITE 16.5.0 applies its late response (Inline.loadData aborts nothing: FileLoader.loadDocument returns undefined), X_ITE_R keeps the newer or current scene; an intentional difference, not an exact match (inlinehold01, KNOWN_DIFFERENCES inlineHold.*)"),
   "tests": addlist("crates/x3d-web/tests/browser/inlinehold01.test.mjs (newerFirst, olderFirst, backToA: held fetches against pinned X_ITE 16.5.0, oracle/web-browser-inline-hold-01-baseline, 5 runs with identical values; PR #447)"),
  }, "Issue #51 2026-11-04 (PR #447): three held-fetch browser cases against pinned X_ITE; the stale-response difference is ADR 0014 section 5, owner-approved. Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i451.py check|apply")
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
