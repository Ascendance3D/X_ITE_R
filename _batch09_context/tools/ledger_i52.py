#!/usr/bin/env python3
"""Issue #52 ledger update (node:Inline): python3 -I ledger_i52.py check|apply

A replaced PROTO or EXTERNPROTO body no longer runs shutdown () of its Scripts and nested Inline scenes in the browser build, as in pinned X_ITE 16.5.0 (8 cases, 3 identical recordings).
The native build keeps ISO 4.12.3 shutdown (). The entry stays PARTIAL for the differences listed. The EmbeddedViewer rule is a host API with no scene-observable counterpart in X_ITE.
No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i52", "ledger.json")
SNAP_SHA256 = "fbdda5b8536eedcfe9c33ab03a8409dca343a8ccc02bfc83fd28daf5064a06eb"

T = ["crates/x3d-script/tests/rea_b24_inline_proto.rs (rea_b24_inline_proto_matches_x_ite, replaced_body_shutdown_follows_the_build, disposed_body_inline_load_in_flight; 8 cases c01-c08 in oracle/synthetic-rea-b24-inline-proto: updateInstances, nested Inline, two updates in one tick, instance removed, EXTERNPROTO url swap, load FALSE then TRUE, loadNow)"]
FIELDS = {"tests": lambda old: old + T, "oracle": "pinned X_ITE 16.5.0 (34cdf9f97): oracle/synthetic-rea-b24-inline-proto-baseline.json (8 cases, 3 identical recordings)",
    "missing": "native build keeps shutdown () of a replaced body and its nested Inline scenes (owner-approved, ISO 4.12.3; X_ITE runs none); an Inline load in flight when its body is disposed is cancelled, X_ITE lets it finish and initializes the disposed scene's Scripts (needs an owner decision, #63); root scene loadCount lags by one or two steps in X_ITE after an EXTERNPROTO body loads its Inline (internal field, all other lines agree); initial load logs extra frames in X_ITE and LoadSensor isActive comes before the Inline scene initializes there; plain Inline reload paths (set_internal_scene, inline_abort) not checked for the browser shutdown rule"}
NOTE = "Issue #52 2026-11-01: X_ITE never disposes the nodes of a replaced body one by one (X3DExecutionContext.dispose clears routes and tables), so no Script in the old body or its nested Inline scenes gets shutdown (); the old scenes stop ticking and the new body builds a fresh Inline. instance_dispose_body and inline_close now use dispose_scene_with(.., !replace_skips_shutdown); x3d-web sets the flag, so the browser build matches in every step except the in-flight load. EmbeddedViewer rule: embed.rs is a Rust host API with no X_ITE counterpart; its unload runs shutdown () by the native policy above, and no scene content can reach it. Plain PROTO updateInstances (c01-c04) already matched. Mutations (shutdown restored in either path) are caught."
C = {"node:Inline": (None, FIELDS, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i52.py check|apply")
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
