#!/usr/bin/env python3
"""Ledger step of PR #530 (2026-11-09): a MovieTexture url with a static PNG or non-PNG data fails instead of hanging.
python3 -I ledger/tools/ledger_movie530.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-movie530", "ledger.json")
SNAP_SHA256 = "3e6c2b976ac3f0a773d6e8527c51bbceb06e7c24acb71f0d32d418b3df8f91fb"

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
 "node:MovieTexture": (None, {
   "tests": append("crates/x3d-web/tests/async_loading.rs (a static PNG then an APNG: warning, then the second url loads; a static PNG or non-PNG alone: FAILED and the load settles; PR #530)",
                   "crates/x3d-viewer/tests/movie_anim.rs KNOWN_DIFFERENCES (movie-load-errors-baseline static, notpng, noframes: the exact extra lines of the deliberate difference, everything else equal to pinned X_ITE)"),
  }, "PR #530 (2026-11-09): pinned MovieTexture.js:92-99 does not await setAPNG, so the error that apng-js returns for a static PNG (Not an animated PNG) or non-PNG data (Not a PNG) becomes an unhandled rejection and the node stays IN_PROGRESS forever; the scene never finishes loading (measured 3 of 3 runs in the pinned page through a glTF EXT_texture_video source). This is a defect, not a compatibility requirement: the port takes loadData's catch path (warning, next url, FAILED_STATE), recorded as ADR 0001 row 14 and in web/API.md. The other gaps stay, so the status stays PARTIAL."),
 "sub:parse.gltf2": (None, {
   "missing": swap("a MovieTexture whose video url is a PNG file leaves createX3DFromURL unsettled in the pinned page (measured once, Rust UNVERIFIED)", "a MovieTexture whose video url is a PNG file leaves createX3DFromURL unsettled in the pinned page (3 of 3 runs; a missing await in MovieTexture.js, a defect); the port fails the texture and settles (deliberate difference, ADR 0001 row 14, PR #530; no pinned glTF baseline can exist for it)"),
  }, "PR #530 (2026-11-09): the unsettled createX3DFromURL of a glTF whose EXT_texture_video source is a PNG is the MovieTexture setAPNG defect; the port does not copy it (see node:MovieTexture). Text only; the status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_movie530.py check|apply")
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
