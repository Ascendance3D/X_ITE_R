#!/usr/bin/env python3
"""Ledger step after #332 (issue #90): python3 -I ledger_i332.py check|apply

One entry, node:MovieTexture: VP9 WebM decoded natively, vis-items-movie matches. Status stays PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i332", "ledger.json")
SNAP_SHA256 = "344f7ee7284c38339177e72177993c7f1a8734cc7831d4a3e296bab30858cc51"

def setm(new):
    return lambda old: new
OLD1 = "frames are not decoded: vis-items-movie is a KNOWN_GAPS world (animated GIF, APNG and WebM urls draw untextured white; x3d-shot: 'not rendered: MovieTexture')"
NEW1 = "frames of VP8, AV1 and MP4 urls are not decoded natively (assumed playable, as before); video sound, seeking and real-time element playback are not reproduced natively; VP9 WebM is 8-bit only and assumes BT.709 whatever the stream's colour tags say"
def fix(old):
    assert old == OLD1, "expected text not found"
    return NEW1
C = {
 "node:MovieTexture": (None, {"missing": fix}, "Issue #90 2026-11-02 (PR #332): animated GIF, APNG (earlier) and VP9 WebM (vp9dec 0.1.1, MIT, plus an in-repo WebM demuxer) are decoded natively. vis-items-movie matches pinned X_ITE 16.5.0: 0 pixels over 8 (max 1) at MSAA 0 and 4, three identical recordings each; it was 5.5% over 8 and the KNOWN_GAPS entry is removed. The pinned page converts the untagged stream with the BT.709 matrix and bilinear chroma interpolation (BT.601: 2.9% over 8; BT.709 replicated chroma: 0.57%). Playback maps elapsed time to frames as GifMedia does. ISO/IEC 19775-1 V4.0 MovieTexture clause number unverified (no local clause text); 19775-2 unverified. Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i332.py check|apply")
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
