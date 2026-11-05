#!/usr/bin/env python3
"""Ledger step after PR #454 (X_ITE's SLOW_SCROLL, issue #51): python3 -I ledger_i454.py check|apply

Two promotions, PARTIAL to COMPLETE, and two text-only changes. PR #454 (60561425) added five cases to
oracle/rea-b24-nav-baseline (pinned X_ITE 16.5.0, macOS user agent, three identical recordings): plane-wheel-mac,
lookat-cor-wheel-mac, examine-cor-wheel-mac, fly-wheel-roll-mac, walk-wheel-roll-mac. They were run on master 60561425
with rea_b24_nav_shard0, shard1, shard2, slow_scroll_user_agent and viewer: 16 passed, 0 skipped (KNOWN_GAPS is empty).

- sub:nav.plane: the only listed gap, "macOS scroll factor (P10)", is closed (plane-wheel-mac). COMPLETE.
- sub:nav.lookat: the only listed gap, "macOS scroll factor (L07)", is closed (lookat-cor-wheel-mac). COMPLETE.
- sub:nav.examine: E17 is closed (examine-cor-wheel-mac). Text only. E14 and E18 stay listed, so it stays PARTIAL.
- sub:nav.walk-fly: F18 is closed (fly-wheel-roll-mac, walk-wheel-roll-mac). Text only. MSAA above 0 and Viewport nodes
  for the rubber band stay listed, so it stays PARTIAL.

No DIFFERENT entry is touched. Counts: 282 COMPLETE / 42 PARTIAL / 21 DIFFERENT / 1 MISSING before,
284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING after, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i454", "ledger.json")
SNAP_SHA256 = "c5301b979f9df16d28b666659ef1f03edfbcc5d25b4670c10c1b4ec6f977e4b2"

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
 "sub:nav.plane": ("COMPLETE", {
   "missing": "",
   "tests": lambda old: [t.replace("40 cases", "45 cases") for t in old],
   "oracle": lambda old: old.replace("rea-b24-nav-baseline (40 cases", "rea-b24-nav-baseline (45 cases"),
  }, "Issue #51 2026-11-05: PR #454 (60561425) added the macOS-user-agent cases to oracle/rea-b24-nav-baseline; rea_b24_nav shard0, shard1, shard2 (45 cases), slow_scroll_user_agent and viewer re-run on master 60561425: 16 passed, 0 skipped. Promoted PARTIAL to COMPLETE: the only listed gap, the macOS scroll factor (P10), is closed by plane-wheel-mac."),
 "sub:nav.lookat": ("COMPLETE", {
   "missing": "",
   "tests": lambda old: [t.replace("40 cases", "45 cases") for t in old],
   "oracle": lambda old: old.replace("rea-b24-nav-baseline (40 cases", "rea-b24-nav-baseline (45 cases"),
  }, "Issue #51 2026-11-05: PR #454 (60561425) added the macOS-user-agent cases to oracle/rea-b24-nav-baseline; rea_b24_nav shard0, shard1, shard2 (45 cases), slow_scroll_user_agent and viewer re-run on master 60561425: 16 passed, 0 skipped. Promoted PARTIAL to COMPLETE: the only listed gap, the macOS scroll factor (L07), is closed by lookat-cor-wheel-mac."),
 "sub:nav.examine": (None, {
   "missing": swap("not compared: the macOS scroll factor (E17); E14", "not compared: E14"),
   "tests": lambda old: [t.replace("40 cases", "45 cases") for t in old],
   "oracle": lambda old: old.replace("rea-b24-nav-baseline (40 cases", "rea-b24-nav-baseline (45 cases"),
  }, "Issue #51 2026-11-05: PR #454 (60561425) added the macOS-user-agent cases to oracle/rea-b24-nav-baseline; rea_b24_nav shard0, shard1, shard2 (45 cases), slow_scroll_user_agent and viewer re-run on master 60561425: 16 passed, 0 skipped. Text only: E17 is closed by examine-cor-wheel-mac; E14 and E18 stay open, so the entry stays PARTIAL."),
 "sub:nav.walk-fly": (None, {
   "missing": swap("not compared: macOS roll angle (F18, macOS only), MSAA above 0", "not compared: MSAA above 0"),
   "tests": lambda old: [t.replace("40 cases", "45 cases") for t in old],
   "oracle": lambda old: old.replace("rea-b24-nav-baseline (40 cases", "rea-b24-nav-baseline (45 cases"),
  }, "Issue #51 2026-11-05: PR #454 (60561425) added the macOS-user-agent cases to oracle/rea-b24-nav-baseline; rea_b24_nav shard0, shard1, shard2 (45 cases), slow_scroll_user_agent and viewer re-run on master 60561425: 16 passed, 0 skipped. Text only: F18 is closed by fly-wheel-roll-mac and walk-wheel-roll-mac; MSAA above 0 and the Viewport rubber band stay open, so the entry stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i454.py check|apply")
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
