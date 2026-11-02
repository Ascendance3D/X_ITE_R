#!/usr/bin/env python3
"""Ledger step after #318 and #323: python3 -I ledger_i318.py check|apply

node:ParticleSystem and sub:pointer.picking (#53: float32 gaps bounded and enforced, ADR 0011) and node:GaussianSplats (#69: pick under log depth compared, #323). All stay PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i318", "ledger.json")
SNAP_SHA256 = "1401e31a0fed6dcea9ed350e6b72fd62829a8e7589eb50ee7840d0741ae7dcee"

def rep(*pairs):
    def f(old):
        for a, b in pairs:
            assert a in old, a
            old = old.replace(a, b)
        return old
    return f
PS = rep(("KNOWN_GAPS: pc-line frame 45 (a line leaving the collision buffer through its right edge, 1.7e-5 in depth), pp-row-line up to 3 ulps and the point-props cases 1 ulp in gl_PointCoord",
          "bounded float32 gaps, each enforced by its test (ADR 0011, #318): pc-line collision depth within 2e-5 (measured 1.68e-5 at the right-edge pixel of a line; that remainder is not explained), pp-row-line hit points within 3 ulps and the point-props cases within 1 ulp in gl_PointCoord"))
PK = rep(("(pk-lines, KNOWN_GAP)", "(pk-lines, bounded at 398 ulps and enforced by the test, ADR 0011, #318)"),
         ("(pk-points-style, KNOWN_GAP)", "(pk-points-style, bounded at 3 ulps and enforced by the test, ADR 0011, #318)"))
GS = rep((", the pick pass has no logarithmic depth mode", ""))
N_PS = "Issue #53 2026-11-02 (PR #318, ADR 0011): the float32 ulp gaps are decided and enforced as bounds (inside passes, outside fails) instead of KNOWN_GAPS that must keep failing: point sprite hit texCoord 3 ulps (particles 1 ulp), line hit points 3 ulps (particles) and 398 ulps (63 aliased), pc-line collision depth 2e-5. The pc-line translation error was an engine error (line_fragments clamped the interpolation parameter; GL does not) and is fixed. pr-line at MSAA 4 was the mirrored sample pattern (#316). ISO/IEC 19775-1 V4.0 5.3.5 states no accuracy for these results; 19775-2 unverified. Status stays PARTIAL."
N_PK = "Issue #53 2026-11-02 (PR #318, ADR 0011): the line hit point (398 ulps transformed thick lines, 63 aliased) and the PointProperties hit texCoord (3 ulps) gaps are bounded and enforced by pk-lines and pk-points-style; hit ids and every other value stay exact. Status stays PARTIAL."
N_GS = "Issue #69 2026-11-02 (PR #323): picking under the LogarithmicDepthBuffer option is compared with pinned X_ITE (6 worlds, 2646 probes, five identical recordings each): hit or miss identical in all probes, values within 7.1e-5 against the 1e-4 tolerance, so the pointing pass needs no log mode (X_ITE moves values by at most 4.7e-5 between option on and off). MSAA 4 is not recorded for it. Status stays PARTIAL."
C = {
    "node:ParticleSystem": (None, {"missing": PS}, N_PS),
    "sub:pointer.picking": (None, {"missing": PK}, N_PK),
    "node:GaussianSplats": (None, {"missing": GS}, N_GS),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i318.py check|apply")
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
