#!/usr/bin/env python3
"""Ledger step for the InlineGeometry second instance (issue #68, ADR 0012): python3 -I ledger_ig2.py check|apply

One text-only change. node:InlineGeometry listed the decided difference of ADR 0012 (X_ITE, Cache on, gives a second
InlineGeometry of the same url a copy() without routes; Rust keeps them) as untested. rea_batch08_net case ig-second now
compares it with pinned X_ITE 16.5.0 (oracle/rea-b08-net-baseline/ig-second-msaa0, three identical recordings) and asserts
the difference in its region. The ig-select reload picture check stays listed, so the entry stays PARTIAL.

No status changes. No DIFFERENT entry is touched. Counts: 284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING before and
after, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-ig2", "ledger.json")
SNAP_SHA256 = "21926ac64f7feb137ee7c9faabf70eeb576fedc6f87bab1672fbcbf229deb33b"

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
 "node:InlineGeometry": (None, {
   "missing": swap("X_ITE (Cache on) gives a second InlineGeometry with the same url a copy() without routes ('TODO: add routes' in InlineGeometry.js); Rust keeps the routes; a decided difference (ADR 0012) that is untested: no second-instance recording exists; ig-select reload picture check disabled", "ig-select reload picture check disabled"),
   "tests": addlist("crates/x3d-viewer/tests/rea_batch08_net.rs (rea_batch08_net_cases_match_x_ite, case ig-second --known-diff=ig-second: three InlineGeometry of ig/routes.x3dv and three of ig/routes-extrusion.x3dv; everything outside the decided region equal to X_ITE; in the region X_ITE's second and third copies stay still and Rust's move, ADR 0012)"),
   "oracle": lambda old: (old + "; " if old else "") + "pinned X_ITE 16.5.0 (34cdf9f97): oracle/rea-b08-net-baseline/ig-second-msaa0 (three identical recordings, rea_b08_net.py pin)",
  }, "Issue #68 2026-11-06: the decided difference of ADR 0012 has a test now. oracle/rea-b08-net-baseline/ig-second-msaa0 (pinned X_ITE 16.5.0, three identical recordings, js.json sha256 0022e835ec69ba45bd39a645783f8d2c57a375e7a08125270107d0ac018b5db9) shows X_ITE's copies of the Extrusion with routes to set_spine still and the Coordinate copies moving (the copy shares the routed Coordinate node); rea_batch08_net compares everything else and asserts the difference in the region (red checks: without the mask 1.5 to 4.4% of the pixels over 8; a mask on a moving X_ITE cell fails). Text only: the ig-select reload picture check stays open, so the entry stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_ig2.py check|apply")
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
