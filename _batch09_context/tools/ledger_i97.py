#!/usr/bin/env python3
"""Issue #97 ledger update (remaining Browser SAI properties and update control): python3 -I ledger_i97.py check|apply

Starts from ledger/before-i97/ledger.json (sha256 d0dfdf501c0959e5afb3fb424a247221b899e945b991ae2a43dbfc1146bdd3ec). sub:browser.sai-api listed three open items. All three are now compared
with pinned X_ITE 16.5.0 (34cdf9f97), three identical recordings each, by crates/x3d-viewer/tests/rea_batch09_sai.rs:
update-props (every BrowserProperties and RenderingProperties entry through getBrowserProperty / getRenderingProperty; the GPU's
limits by type only), update-speed (currentSpeed and currentFrameRate while the viewer is active) and update-end (a Script calls
Browser.endUpdate (), then Browser.beginUpdate (): the TimeSensor stops and resumes, the Script's prepareEvents keeps running).
Rust defect fixed: Browser.beginUpdate / endUpdate were empty in the Script realm; they now set the live state of the browser.
Nothing else was open, so the entry becomes COMPLETE. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i97", "ledger.json")
SNAP_SHA256 = "d0dfdf501c0959e5afb3fb424a247221b899e945b991ae2a43dbfc1146bdd3ec"
NOTE = ("Issue #97 2026-10-31: update-props, update-speed and update-end (oracle/rea-batch09-sai-baseline, pinned X_ITE 16.5.0, "
        "three identical recordings each). Rust defect fixed: Browser.beginUpdate / endUpdate did nothing in a Script; they set the "
        "live state of the browser (a world that is not live pauses its TimeSensors; Script prepareEvents keeps running, as in X_ITE).")
OLD_MISSING = ("getBrowserProperty / getRenderingProperty only for a missing name and Initialized; currentSpeed / currentFrameRate only in a "
               "frozen scene; beginUpdate / endUpdate only as calls (that rendering stops is not compared)")
TEST = ("crates/x3d-viewer/tests/rea_batch09_sai.rs (update-props, update-speed, update-end: oracle/synthetic-rea-b97-update; "
        "properties, currentSpeed / currentFrameRate, Browser.endUpdate / beginUpdate)")

def fix_missing(old):
    if old != OLD_MISSING: raise SystemExit(f"unexpected missing text: {old!r}")
    return ""

# id: (status or None, {field: value or function of the old value}, note). The verifier reads the first element.
C = {"sub:browser.sai-api": ("COMPLETE", {"missing": fix_missing, "tests": lambda old: old + [TEST]}, NOTE)}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, note = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            i["notes"] = (i["notes"] + " | " if i["notes"] else "") + note
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i97.py check|apply")
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
