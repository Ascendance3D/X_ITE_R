#!/usr/bin/env python3
"""Ledger text step for PR #438 and PR #441 (issue #51): python3 -I ledger_i441.py check|apply

Text only. No status changes. Both PRs are merged on master 31499a74, and two entries still said otherwise.

- sub:io.file-loader: the gap "redirects, response headers and the foreign text/html hand-off (PR #438, not merged)" is closed.
  file_http compares them with pinned X_ITE 16.5.0 (4 groups, three identical recordings); the 4 cases where the port differs
  are pinned by known_differences_are_pinned. The status stays PARTIAL: the pinned differences remain and the owner has
  not approved them as differences of the platform.
- sub:parse.golden-gate: addParsers / removeParsers / getParsers are covered by golden_gate_registry (PR #441). That test
  checks the pinned source, not a recorded run. The status stays PARTIAL: PLY Gaussian splats and NaN STL stay open.

Counts do not change: 282 COMPLETE / 42 PARTIAL / 21 DIFFERENT / 1 MISSING, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i441", "ledger.json")
SNAP_SHA256 = "fc985c42ecf06c197bb3336bff41a5bc8384bdad933d566c6b81a714e3879e14"

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
 "sub:io.file-loader": (None, {
   "missing": swap("still missing: redirects, response headers and the foreign text/html hand-off (they need a Fetcher that returns a response: PR #438, not merged)", "redirects (301, 302, 303, 307, 308, chains, a loop, 20 and 21 hops), failed statuses, content types, response headers, Content-Encoding, a cut body and the foreign text/html hand-off are compared with the pinned page by file_http (PR #438, 4 groups, three identical recordings); pinned differences (known_differences_are_pinned): a redirect to another origin without Access-Control-Allow-Origin (the port has no origin) and the reason phrase of a status (the port keeps the canonical phrase)"),
   "tests": addlist("crates/x3d-viewer/tests/file_http.rs (4 groups; oracle/file-http-baseline)", "crates/x3d-http/tests/http_fetcher.rs"),
  }, "Issue #51 2026-11-05 (PR #438, merged 31499a74): redirects, response headers and the foreign hand-off are no longer missing. Re-run on master 31499a74: file_http (5), file_loader (8), golden_gate_binary (5) and http_fetcher (29) pass, 0 skipped. Status stays PARTIAL."),
 "sub:parse.golden-gate": (None, {
   "missing": swap("parsers added or removed with GoldenGate.addParsers / removeParsers and PLY Gaussian splats are not covered", "parsers added or removed with GoldenGate.addParsers / removeParsers / getParsers are covered by golden_gate_registry against the pinned source, not a recorded run (PR #441); PLY Gaussian splats are not covered"),
   "tests": addlist("crates/x3d-parse/tests/golden_gate_registry.rs (pinned GoldenGate.js list semantics)"),
  }, "Issue #51 2026-11-05 (PR #441, merged 31499a74): the parser registry gap is closed. Re-run on master 31499a74: golden_gate_registry passes (16 tests), 0 skipped. Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i452.py check|apply")
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
