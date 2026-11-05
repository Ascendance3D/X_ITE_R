#!/usr/bin/env python3
"""Ledger step after PR #458 (EXTERNPROTO scene-cache hit timing, issue #51): python3 -I ledger_i458.py check|apply

One text-only change, no status change. sub:proto.externproto stays PARTIAL (issue #58 and #303 remain).
The gap "createX3DFromString with an EXTERNPROTO after load not compared" is closed: PR #458 adds the case
externproto-after-load to oracle/rea-b09-sai-baseline (pinned X_ITE 16.5.0, three identical recordings). Without the fix
the case failed at frames 11, 17 and 20 (the port resolved a Ready scene-cache hit one frame early). With it,
rea_batch09_sai_cases_match_x_ite passes; extern, proto, inline, b09, sai and browser suites: 46 passed, 0 skipped.
The failed-URL cache difference (X_ITE caches a failure, the port fetches again) is not changed and not tested by the case.

No DIFFERENT entry is touched. Counts stay 284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i458", "ledger.json")
SNAP_SHA256 = "34b53f2b7ed39a5401fcca1e2294c5254f1ec790ddf376c4939144913f08de7f"

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
 "sub:proto.externproto": (None, {
   "missing": lambda old: old[:old.index("; createX3DFromString with an EXTERNPROTO after load is measured")] + "; the failed-URL scene cache differs by design (X_ITE caches a failed URL and throws a setWorldURL TypeError on the next request of it; the port fetches again), so createX3DFromString with an EXTERNPROTO after load is compared (case externproto-after-load, oracle/synthetic-rea-b09-sai/externproto-after-load.wrl, 6 strings with their own urls: scene text, poses and log equal at 60 frames) without asking a cached failed url twice",
   "tests": addlist("crates/x3d-viewer/tests/rea_batch09_sai.rs (rea_batch09_sai_cases_match_x_ite, case externproto-after-load: createX3DFromString with an EXTERNPROTO after load, a data: url, two urls, a hash, a missing url; scene-cache hits resolve at the pinned frame; three identical recordings)"),
  }, "Issue #51 2026-11-05: PR #458. A Ready scene-cache hit in a createX3DFromString scene is handed over after the frame, as in X_ITE (case externproto-after-load, first red at frame 11, now equal at all 60 frames). Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i458.py check|apply")
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
