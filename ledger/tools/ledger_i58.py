#!/usr/bin/env python3
"""Issue #58 ledger update (sub:proto.externproto): python3 -I ledger_i58.py check|apply

Cached EXTERNPROTO live state now equals pinned X_ITE 16.5.0 in every recorded step (rea_b24_externproto_live, 10 cases, 5 identical recordings). The
disposed-context cancel is measured: X_ITE builds the PROTO body into a disposed scene when the load was still in flight, Rust cancels. The entry stays
PARTIAL for that one difference, which waits for an owner decision. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i58", "ledger.json")
SNAP_SHA256 = "8734e1340a03c815f97e882b854433625de87f91428e9211aa957fa31e43c19a"

T = ["crates/x3d-script/tests/rea_b24_externproto_live.rs (rea_b24_externproto_live_matches_x_ite: two declarations sharing one url and cached scene, reload to another file or the cached scene, same-frame reload, loadNow on a cache hit, load FALSE then url change then load TRUE, an interface change; disposed-context cases d1-d4; shared_scene_declarations_stay_independent; disposed_context_cancels_the_load)"]
FIELDS = {"tests": lambda old: old + T,
    "missing": "disposed-context cancel is a deliberate difference: pinned X_ITE builds the PROTO body (and runs its Script initialize) in a disposed scene when the declaration's load was in flight one tick after the Inline was asked for; Rust cancels it with its context (owner decision pending, #63); createX3DFromString with an EXTERNPROTO after load not compared",
    "oracle": "pinned X_ITE 16.5.0 (34cdf9f97) logs: oracle/synthetic-rea-b24-externproto-live-baseline.json (10 cases, 5 identical recordings)"}
NOTE = "Issue #58 2026-10-31: shared-declaration live state matches X_ITE in every recorded step with no KNOWN entry. The d1 disposed-context difference is pinned by two KNOWN entries (d1-dispose-load, d1-dispose-url) and by disposed_context_cancels_the_load; matching X_ITE means dropping the two cancel guards (extern_close, the ctx_dead filter in extern_instances) and the entries."
C = {"sub:proto.externproto": (None, FIELDS, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i58.py check|apply")
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
