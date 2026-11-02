#!/usr/bin/env python3
"""Issue #141 and #80 ledger note (sub:browser.keyboard-ui): python3 -I ledger_i141.py check|apply

PR #274 ports the browser context menu, notification, timings panel and Shift+F8 and compares them with pinned X_ITE 16.5.0 (ui01, 5 identical recordings). PR #282 adds the native context menu, notification overlay, World Info dialog, Ctrl+Plus, debug keys and Shift+F8 behind native_ui. Status stays PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i141", "ledger.json")
SNAP_SHA256 = "0fd6bbb89e22f633668da6c2912f44e7ee0c0b8ecd102d77b9a61cf77f6bb282"

OLD = "ContextMenu and Notification are not ported; Debug shading keys (Debug-option Ctrl+1..5 / Ctrl+S), Ctrl+Plus timings and Shift+F8 viewpoint copy are not compared with X_ITE; KeySensor commandKey / altGrKey fields not compared; Wayland and macOS keyboard input not validated against X_ITE"
NEW = "browser: Debug shading keys (Ctrl+1..5, Ctrl+S), Shift+F8 for GeoViewpoint and the encoding, and the Loading n files notifications are not compared; native: no Timings panel, no macOS Ctrl+click or touch long-press, x10_compare key-sensors 342/0 not run; Wayland and macOS keyboard input not validated against X_ITE"
NOTE = "Issues #141 and #80 2026-11-01: the browser context menu, notification, timings panel and Shift+F8 (#274, ui01, 5 identical recordings against pinned X_ITE 16.5.0) and the native context menu, notification overlay, World Info dialog, Ctrl+Plus, debug keys and Shift+F8 (#282, 19 tests with real mouse and key events) exist. commandKey and altGrKey are browser flags (key codes 91 and 225), not KeySensor fields, so the earlier KeySensor wording was wrong. Status stays PARTIAL."
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i141.py check|apply")
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
