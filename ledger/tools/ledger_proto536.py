#!/usr/bin/env python3
"""Ledger step of PR #536 (2026-11-10): PROTO declaration fields are live, held field objects follow a layout change,
the body context reports its declaration as outer node, and createVrmlFromURL / loadURL with an EXTERNPROTO after load are
recorded. python3 -I ledger/tools/ledger_proto536.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-proto536", "ledger.json")
SNAP_SHA256 = "b491079238f9bba4d28e19754c60cad5cdfee875a6551f7c3568de390277e16c"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f

def swap_in_list(old_text, new_text):
    def f(old):
        assert sum(s.count(old_text) for s in old) == 1, old_text
        return [s.replace(old_text, new_text) for s in old]
    return f

def append(*extra):
    return lambda old: list(old) + list(extra)

C = {
 "sub:proto.proto": (None, {
   "missing": swap("an update that disposes and rebuilds a body logs initialize once (X_ITE twice); getBody on an EXTERNPROTO, createInstance with the non-public setup argument and live X3DField objects from getUserDefinedFields not ported; field objects a Script holds across a layout change stop refreshing;", "with it, when an update disposes a body before its Script's load finished, the native build logs that Script's initialize once where X_ITE logs it twice (Script.dispose only unloads; case p08, the browser mode equals X_ITE); createInstance with the non-public setup argument not ported;"),
   "tests": append("crates/x3d-script/tests/rea_b24_proto_sai.rs (cases p07-p10, every case in the browser and the native runtime mode; p07 update paths, p08 initialize after an update before the Script load (KNOWN: native only), p09 live declaration fields and held instance field objects across remove/add/updateInstances, p10 getBody on PROTO and EXTERNPROTO and the body's outer node; pinned X_ITE 16.5.0, three identical recordings; PR #536)"),
  }, "PR #536 (2026-11-10): fixed: getUserDefinedFields and fields of a declaration give its own live X3DField objects (setting one changes the default; first red p09: Object,Object for SFVec3f,SFInt32), the field objects a Script holds follow their fields when an instance's layout changes (the runtime records the remap; a removed field's object keeps its last value), and getBody ().getOuterNode () is the declaration (p10: null before). Not a gap: getBody on an EXTERNPROTO, which X_ITE does not have (pinned: TypeError e.getBody is not a function; the port throws TypeError: not a function, console text only). Recorded, the kept shutdown difference: p08 native. REA: analysis ev_1c8117c6ce9b8248684ad46b18927318cedefa6aaf73287bb01629096cfbfdaa (summary ev_4101060cc97346def4112b43bbae08368541651afcbf7551549ee433bdb20ca1) of Prototype/X3DProtoDeclaration.js, X3DExternProtoDeclaration.js, X3DProtoDeclarationNode.js, Core/X3DPrototypeInstance.js, Scripting/Script.js and Execution/X3DExecutionContext.js; partial graph. Kept differences remain: PARTIAL."),
 "sub:proto.externproto": (None, {
   "missing": swap("createVrmlFromURL and loadURL with an EXTERNPROTO after load are not recorded (only createX3DFromURL, case externproto-from-url)", "createVrmlFromURL and loadURL with an EXTERNPROTO after load are compared (cases externproto-createvrml-url and externproto-loadurl, PR #536)"),
   "tests": append("crates/x3d-viewer/tests/rea_batch09_sai.rs (cases externproto-createvrml-url and externproto-loadurl: createVrmlFromURL and loadURL of worlds with an EXTERNPROTO after load, a missing EXTERNPROTO file; pinned X_ITE 16.5.0, three identical recordings; equal at first run; PR #536)"),
  }, "PR #536 (2026-11-10): createVrmlFromURL and loadURL with an EXTERNPROTO after load recorded on the pinned page (three identical runs) and compared: equal, no engine change. REA: analysis ev_1c8117c6ce9b8248684ad46b18927318cedefa6aaf73287bb01629096cfbfdaa (summary ev_4101060cc97346def4112b43bbae08368541651afcbf7551549ee433bdb20ca1) of Prototype/X3DProtoDeclaration.js, X3DExternProtoDeclaration.js, X3DProtoDeclarationNode.js, Core/X3DPrototypeInstance.js, Scripting/Script.js and Execution/X3DExecutionContext.js; partial graph. The failed-URL scene cache difference stays: PARTIAL."),
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
    bad += [f"{k}: not in the snapshot" for k in sorted(set(C) - set(snap))]
    built = build()
    if {k: v for k, v in d.items() if k != "items"} != {k: v for k, v in built.items() if k != "items"}: bad.append("top-level fields differ from what apply writes")
    want = {i["id"]: i for i in built["items"]}
    for k, b in snap.items():
        a = cur.get(k)
        if a is None: continue  # reported above: ID set or total changed
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_proto536.py check|apply")
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
