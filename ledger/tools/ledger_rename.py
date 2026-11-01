#!/usr/bin/env python3
"""Folder rename ledger update (no status change): python3 -I ledger_rename.py check|apply

The folders _batch08_context, _batch09_context and oracle/rea-batchNN-* were renamed (docs/renames.md). In every entry that is not
DIFFERENT, the paths in the tests and oracle fields name the new folders. Notes are never rewritten, and the DIFFERENT entries
stay byte for byte, so old paths remain in them; verify_ledger.py resolves those through its RENAMED map."""
import json, os, re, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-rename", "ledger.json")
SNAP_SHA256 = "d428f3a9982977da1d1208006de6ff32c365a3f7171fd93c883a67502829a570"

OLD = re.compile(r"rea-batch(\d)|_batch09_context/tools/|_batch09_context/ledger/before|_batch09_context/ledger/|_batch09_context/|_batch08_context/")
NEW = {"_batch09_context/tools/": "ledger/tools/", "_batch09_context/ledger/before": "ledger/snapshots/before", "_batch09_context/ledger/": "ledger/",
       "_batch09_context/": "ledger/history/b09/", "_batch08_context/": "ledger/history/b08-recovery/"}
FIELDS = ("tests", "oracle")
def fix(v):
    if isinstance(v, str): return OLD.sub(lambda m: "rea-b" + m.group(1) if m.group(1) else NEW[m.group(0)], v)
    if isinstance(v, list): return [fix(x) for x in v]
    return v
def load(p): return json.load(open(p))
def change(i):
    return {f: fix(i[f]) for f in FIELDS if f in i and fix(i[f]) != i[f]} if i["status"] != "DIFFERENT" else {}
C = {i["id"]: (None, None, None) for i in load(SNAP)["items"] if change(i)}

def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C: i.update(change(i))
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
        if k in C and a["status"] != b["status"]: bad.append(f"{k}: status")
        if k in C and a != want[k]: bad.append(f"{k}: differs from what apply writes")
    c = collections.Counter(i["status"] for i in d["items"]); print(dict(sorted(c.items())), len(C), "entries in the change set")
    print("\n".join(bad) or "OK"); return not bad
def sha256(p):
    with open(p, "rb") as f: return hashlib.sha256(f.read()).hexdigest()
def snap_ok():
    got = sha256(SNAP)
    if got != SNAP_SHA256: print(f"snapshot sha256 {got} is not the expected {SNAP_SHA256}: refused"); return False
    return True
if __name__ == "__main__":
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_rename.py check|apply")
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
