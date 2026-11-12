#!/usr/bin/env python3
"""Ledger step of the MSAA 4 envelopes (2026-11-11): the transparency worlds whose multisampled X_ITE picture differs between runs.
python3 -I ledger/tools/ledger_oitenv.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-oitenv", "ledger.json")
SNAP_SHA256 = "392ef62951e0da97d5718d2ab91b64fe5789e9976a4570e35212bcd42bc7924d"

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
 "sub:render.transparency-sorting": (None, {
   "tests": append("crates/x3d-render/tests/render_rea_i92_items.rs and render_rea_oit_i92_items.rs (MSAA 4 envelopes: the five shots without a single X_ITE reference against the lowest and highest value of every channel over eight pinned X_ITE 16.5.0 runs, oracle/rea-i92-render-items-baseline/msaa4-envelope and oracle/rea-oit-i92-render-items-baseline/msaa4-envelope, recorder oracle/tools/rea_oit_msaa4_envelope.py; 0 pixels more than 8 off, largest difference 1; negative control with the option off: i92-tr-many 12.0%, i92-tr-nested 11.2%)"),
   "missing": [
     swap("MSAA 4 of tr-many, tr-ortho and fg-transparent has no X_ITE reference (not reproducible);", "MSAA 4 of tr-many, tr-ortho and fg-transparent has no single X_ITE reference (X_ITE's runs differ) and is compared with the envelope of eight runs (at most 1 level apart, in 63, 1 and 0 pixels);"),
     swap("and at MSAA 4 in 6; i92-tr-many and i92-tr-nested have no OIT MSAA 4 reference (X_ITE's three runs differ, msaa4/unstable.json), so MSAA 4 OIT parity is not claimed for them;", "and at MSAA 4 in all 8: 6 against a single reference, i92-tr-many and i92-tr-nested against the envelope of eight runs (1 level apart in 11 pixels each; msaa4/unstable.json still lists them); open: the envelope is a new kind of reference, and the owner has not ruled that it closes the MSAA 4 gap;"),
   ],
  }, "2026-11-11 (MSAA 4 envelopes): the five multisampled shots that X_ITE does not reproduce (option off: i92-fg-transparent, i92-tr-many, i92-tr-ortho; option on: i92-tr-many, i92-tr-nested) were measured with eight runs each: the runs differ by at most 1 level of 255, in 0 to 63 of 262144 pixels. The recorder keeps the lowest and the highest value of every channel (new directories msaa4-envelope; msaa4/unstable.json and every existing reference are unchanged) and refuses a spread over 2 levels or more than 0.1% differing pixels. The tests measure a pixel from the nearest value between the two, with the usual bound. All five shots: 0 pixels more than 8 off, largest difference 1, no Rust change. The first try of the recorder failed because the worktree's x_ite-pin/node_modules link was wrong (no page loaded); the recorder now names a failed run. Status stays PARTIAL until the owner rules on the envelope reference."),
}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, nt = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items():
                for g in (v if isinstance(v, list) else [v]): i[f] = g(i[f]) if callable(g) else g
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_oitenv.py check|apply")
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
