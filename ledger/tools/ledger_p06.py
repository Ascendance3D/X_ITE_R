#!/usr/bin/env python3
"""Ledger step for addUserDefinedField on an EXTERNPROTO (issue #78, case p06): python3 -I ledger_p06.py check|apply

One text-only change. sub:proto.proto listed "addUserDefinedField on an EXTERNPROTO not probed". Case p06 of
crates/x3d-script/tests/rea_b24_proto_sai.rs compares it with pinned X_ITE 16.5.0 (three identical recordings); the port
now matches. The other listed gaps stay, so the entry stays PARTIAL.

No status changes. No DIFFERENT entry is touched. Counts: 284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING before and
after, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-p06", "ledger.json")
SNAP_SHA256 = "21926ac64f7feb137ee7c9faabf70eeb576fedc6f87bab1672fbcbf229deb33b"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f
def chain(*fs):
    def f(old):
        for g in fs: old = g(old)
        return old
    return f
def addtext(text):
    return lambda old: old + "; " + text
def addlist(*texts):
    return lambda old: old + list(texts)
def note(text):
    return lambda old: (old + " | " if old else "") + text

C = {
 "sub:proto.proto": (None, {
   "missing": swap("addUserDefinedField on an EXTERNPROTO not probed; ", ""),
   "tests": lambda old: [t.replace("field add/replace/remove, late and held instances", "field add/replace/remove, late and held instances, field add/remove on an EXTERNPROTO before and after its load (p06)") if t.startswith("crates/x3d-script/tests/rea_b24_proto_sai.rs") else t for t in old],
  }, "Issue #78 2026-11-06: addUserDefinedField and removeUserDefinedField on an EXTERNPROTO probed (case p06, pinned X_ITE 16.5.0, three identical recordings, sha256 71a6c7e67171c0d8169e860ebb70836acdef1290702f7bd456df166f2dfb3b34). The port threw 'node does not support this' and ignored the remove; now the EXTERNPROTO's own interface changes, its instances share it until updateInstances, a load assigns the loaded PROTO's interface (fields added before the load are gone) and the loaded PROTO keeps its own. Text only: the other listed gaps stay, so the entry stays PARTIAL."),
}



def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, nt = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            if nt: i["notes"] = (i["notes"] + " | " if i["notes"] else "") + nt
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_p06.py check|apply")
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
