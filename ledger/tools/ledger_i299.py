#!/usr/bin/env python3
"""Issue #146 ledger step (sub:browser.dom-integration): python3 -I ledger_i299.py check|apply

PRs #291, #297 and #299 port DOMIntegration to the browser build and compare it with pinned X_ITE 16.5.0. DIFFERENT becomes PARTIAL (one listed difference). The native host has no DOM (ADR 0006)."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i299", "ledger.json")
SNAP_SHA256 = "23035c281abb29600f01387f27fb267535e877db01cf2b11e58a2f2d27f3cb70"

RUST = ["crates/x3d-web/web/x_ite_r.mjs", "crates/x3d-web/src/wasm.rs", "crates/x3d-web/tests/dom_mutation.rs"]
TESTS = ["crates/x3d-web/tests/browser/dom01.test.mjs (10)", "crates/x3d-web/tests/browser/dom02.test.mjs (6)", "crates/x3d-web/tests/browser/dom03.test.mjs (5)", "crates/x3d-web/tests/dom_mutation.rs (3)"]
MISSING = "Browser build only: the order of the first load events of the Inline elements differs from X_ITE (X_ITE starts the DOM code while the world's loads run; here all loads are over first), listed in KNOWN_DIFFERENCES (domEvents.inline) and web/API.md. The native host has no DOM (ADR 0006)."
NOTE = "Issues #146, #291, #297, #299 2026-11-02: the browser build ports DOMIntegration (the canvas follows its X3D child, importDocument of an HTML element, attribute, child and removed-node changes, Inline children, load, error and field CustomEvents, user field refresh, disposal of a removed Scene element), each compared with pinned X_ITE 16.5.0. One scheduling difference is listed, so the status is PARTIAL. The native host has no DOM."
C = {"sub:browser.dom-integration": ("PARTIAL", {"rust": RUST, "tests": TESTS, "missing": MISSING}, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i299.py check|apply")
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
