#!/usr/bin/env python3
"""Issue #293 ledger note (node:AudioClip, node:Sound): python3 -I ledger_i307.py check|apply

PR #307: dispose () no longer closes the page AudioContext, as in X_ITE. Both stay PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i307", "ledger.json")
SNAP_SHA256 = "8029cc2d216de082733d004a7b3da22b275d4df3d791a95d2a9f0e3c6e7c06a1"

OLD = ", dispose () closes the page AudioContext (X_ITE leaves it)"
NEW = ""
NOTE = "Issue #293 2026-11-02 (PR #307): dispose () no longer closes the page's AudioContext, as in X_ITE 16.5.0 (REA found no close in X3DSoundContext.js or X3DBrowser.js); a page that removes many canvases keeps one context per element, as with X_ITE. After replaceWorld the page graph is the destination alone while the new world has no Sound of its own. The page's AudioDestination feeds a MediaStreamAudioDestinationNode that plays in an audio element (ops mediaStream and elementStart); the compared case matches the pinned recordings (oracle/web-browser-sound-03-baseline, oracle/web-browser-sound-01-baseline, 5 identical runs), and soundMore.lifecycle, soundMore.audioDestination and the graph difference of sound.replace left KNOWN_DIFFERENCES. ISO/IEC 19775-1 V4.0 16.2.1 and 16.4.3 are silent on context lifetime; 19775-2 is unverified. Status stays PARTIAL."
C = {"node:AudioClip": (None, {"missing": lambda old: old.replace(OLD, NEW)}, NOTE), "node:Sound": (None, {"missing": lambda old: old.replace(OLD, NEW)}, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i307.py check|apply")
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
