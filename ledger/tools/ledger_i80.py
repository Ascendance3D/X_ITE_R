#!/usr/bin/env python3
"""Issue #80 ledger note (sub:browser.keyboard-ui): python3 -I ledger_i80.py check|apply

PR #288 compares every native shortcut with pinned X_ITE 16.5.0. Status stays PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i80", "ledger.json")
SNAP_SHA256 = "b59d2c095d7714af90299a5acd72d74ebd171f671e49f84a742198d4d5df8449"

OLD = "browser: Debug shading keys (Ctrl+1..5, Ctrl+S), Shift+F8 for GeoViewpoint and the encoding, and the Loading n files notifications are not compared; native: no Timings panel, no macOS Ctrl+click or touch long-press, x10_compare key-sensors 342/0 not run; Wayland and macOS keyboard input not validated against X_ITE"
NEW = "native: no Timings panel (#286); no macOS Ctrl+click or touch long-press (#287); browser: Shift+F8 for GeoViewpoint and the encoding and the Loading n files notifications are not compared; Wayland and macOS keyboard input not validated against X_ITE (#88)"
NOTE = "Issue #80 2026-11-01: every native shortcut is compared with pinned X_ITE 16.5.0 (#288: native_keys_01, 7 tests; x10_compare key-sensors 342 PASS, 0 FAIL). commandKey and altGrKey are browser flags (getCommandKey, getAltGrKey, key codes 91 and 225), recorded and equal; KeySensor has no such fields. Status stays PARTIAL."
F = {"missing": lambda old: old.replace(OLD, NEW)}
C = {"sub:browser.keyboard-ui": (None, F, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i80.py check|apply")
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
