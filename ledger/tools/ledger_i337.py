#!/usr/bin/env python3
"""Ledger step after #337 (issue #69): python3 -I ledger_i337.py check|apply

One entry, node:GaussianSplats: MSAA 4 evidence for the castShadow event and log-depth pick worlds. Status stays PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i337", "ledger.json")
SNAP_SHA256 = "10e86021b446208ec8ffa7b1443cfca7166311729bf03a3daafefbbff7d0ed94"

def setm(new):
    return lambda old: new
C = {
 "node:GaussianSplats": (None, {}, "Issue #69 2026-11-02 (PR #337): MSAA 4 is compared for the castShadow event worlds (pinned X_ITE 16.5.0, three identical recordings each; render_b69_gs_shadow, 16 shots, 0 pixels over 8, MSAA 4 max channel difference 1) and for the log-depth pick worlds (six worlds, five identical recordings each; the MSAA 4 picks equal the MSAA 0 picks, 2646 probes, 0 differ, worst value difference 7.06e-5 against the 1e-4 tolerance). Still open for #69: no CPU stand-in exists for splat pointing and collision (undecided); collision at MSAA 4 is judged not applicable because the pass is offscreen (not tested); every GPU result is Vulkan on the RTX 3060 Ti, and the issue's macOS Metal backend needs a Mac. Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i337.py check|apply")
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
