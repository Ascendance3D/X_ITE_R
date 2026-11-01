#!/usr/bin/env python3
"""Issue #58 ledger note (node:Inline, sub:proto.externproto), second step: python3 -I ledger_i58b.py check|apply

PR #259 lets an Inline or EXTERNPROTO load that is in flight when its context is disposed finish as pinned X_ITE 16.5.0 does. The two `missing` clauses that listed the
cancel as a difference are replaced by the matching behavior; extern_remove still cancels. Status stays PARTIAL for both entries (other items remain in their missing text)."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i58b", "ledger.json")
SNAP_SHA256 = "86f6a4f2ed7d9cb364dca5a52418f64d39fb8b182b621874ce54cf1fa7213fda"

OLD1 = "an Inline load in flight when its body is disposed is cancelled, X_ITE lets it finish and initializes the disposed scene's Scripts (needs an owner decision, #63); "
NEW1 = 'a load in flight when its context is disposed finishes, as in X_ITE 16.5.0 (issue #58): the Scripts of the late scene initialize and the scene is disposed again at the end of the frame, it is never live; '
OLD2 = "disposed-context cancel is a deliberate difference: pinned X_ITE builds the PROTO body (and runs its Script initialize) in a disposed scene when the declaration's load was in flight one tick after the Inline was asked for; Rust cancels it with its context (owner decision pending, #63); "
NEW2 = 'a load in flight when its context is disposed finishes, as in X_ITE 16.5.0 (issue #58): the EXTERNPROTO instance bodies are built and the Scripts of the late scene initialize, then the scene is disposed again at the end of the frame (extern_remove still cancels, a separate difference); '
NOTE = 'Issue #58 2026-11-01 (PR #259): a load in flight when its context is disposed finishes, as in X_ITE 16.5.0. The Scripts of the late scene initialize, EXTERNPROTO instance bodies are built, and the scene is then disposed again at the end of the frame. The scene is never live. The native build still calls Script shutdown() on a replaced body (ISO 14772-1 4.12.3). Status stays PARTIAL; no DIFFERENT entry is in the change set.'
F1 = {"missing": lambda old: old.replace(OLD1, NEW1)}
F2 = {"missing": lambda old: old.replace(OLD2, NEW2)}
C = {"node:Inline": (None, F1, NOTE), "sub:proto.externproto": (None, F2, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i58b.py check|apply")
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
