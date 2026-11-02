#!/usr/bin/env python3
"""Ledger step after #301 and #303: python3 -I ledger_i301.py check|apply

node:Script (#301, Script shutdown per lifecycle event) and sub:proto.externproto (#303). ledger_i303.py wrote the #58 note to sub:browser.keyboard-ui by mistake (its C named the wrong entry); notes are append-only, so keyboard-ui gets a correction note and sub:proto.externproto gets the change. All three stay at their status."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i301", "ledger.json")
SNAP_SHA256 = "f48a028ab8927ed376126e65ea5054e6a2b5a89bc4a14be97f36516b7cf8ff33"

ADD = "; browser build: Script shutdown () runs on replaceWorld, on an unloaded Inline scene and on dispose () although X_ITE 16.5.0 runs none (ISO/IEC 14772-1 4.12.3; listed known differences, ADR 0001 group 4); the browser shutdown event on dispose () is sent only while a loadURL is in flight, as in X_ITE (#292)"
NOTE_SCRIPT = "Issue #292 2026-11-02 (PR #301): the shutdown matrix of ISO/IEC 14772-1 4.12.3 was checked per lifecycle event. Equal to X_ITE: root node removed, window unload, Script element removed from the document (shutdown runs), code replaced (shutdown then initialize), never initialized. Differences by the standard: world replaced, Inline unloaded, dispose (suite shutdown01: oracle/web-browser-shutdown-01-baseline, 14 cases, 5 identical runs). The ISO 19775-1 Scripting text is not stored locally and 19775-2 is unverified. Status stays PARTIAL."
OLD = " (extern_remove still cancels, a separate difference)"
NEW = "; removeExternProtoDeclaration only removes the name, as in X_ITE (#303)"
NOTE_EXTERN = "Issue #58 2026-11-02 (PR #303): extern_remove only takes the name out of the context table, as removeExternProtoDeclaration does in X_ITE 16.5.0 (no dispose, no FileLoader abort). A load in flight finishes and rebuilds the instance bodies, and a declaration the page holds can loadNow and newInstance again; compared by oracle/web-browser-externproto-remove-01-baseline (4 cases, 5 identical runs) and the native c20 case, whose known row is now shutdown-only. The Cache browser option reaches the engine. One known difference stays (externRemove.replaced, ADR 0001 row 11). ISO/IEC 19775-1 V4.0 4.4.5 is silent; 19775-2 was not available. Status stays PARTIAL."
NOTE_KEYS = "Correction 2026-11-02: the note that begins 'Issue #58 2026-11-02 (PR #303)' above belongs to sub:proto.externproto, not to this entry (ledger_i303.py named the wrong entry). It is recorded there by ledger_i301.py. Nothing about sub:browser.keyboard-ui changed in #303."
C = {
    "node:Script": (None, {"missing": lambda old: old + ADD}, NOTE_SCRIPT),
    "sub:proto.externproto": (None, {"missing": lambda old: old.replace(OLD, NEW)}, NOTE_EXTERN),
    "sub:browser.keyboard-ui": (None, {}, NOTE_KEYS),
}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i301.py check|apply")
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
