#!/usr/bin/env python3
"""Ledger step after PRs #427, #428, #430-#434, #437 (issues #51, #89, #95): python3 -I ledger_i437.py check|apply

Seven entries, text only, no status change, no DIFFERENT entry touched. Every sentence is taken from the "Proposed ledger effect" of
the merged PR body, or from ADR 0014 (#437); #429 and #436 have no ledger effect. Known open items stay visible: the classic-edge
resource-load log differences and the Annotation decision, the ShaderPart compile-error text, the Inline c07 overlap-frames
difference, and the FileLoader redirects / response headers / text/html hand-off (PR #438, not merged)."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i437", "ledger.json")
SNAP_SHA256 = "8aede583e634921e0ec6fba6525da049d5408c4544722035b40560da20cdcc01"

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
 "sub:parse.vrml-classic": (None, {
   "missing": swap("X3D classic edge cases not separately oracle-tested",
                    "X3D classic edge cases are compared with the pinned parser (classic_edge, 2157 authored inputs, PR #427); open: 6 resource-load log differences (runtime side) and the Annotation owner decision"),
   "tests": addlist(
     "crates/x3d-parse/tests/classic_edge.rs (oracle/classic-edge-baseline/classic-edge.json, 2157 cases, three identical pinned recordings)",
     "crates/vrml97/tests/classic_edge_facade.rs (the same cases through the vrml97 facade)"),
  }, "Issue #51 2026-11-04 (PR #427): 2157 authored classic-encoding edge cases against the pinned parser; four parser defects fixed. The status stays PARTIAL; a status change is the owner's while the 6 resource-load log differences and the Annotation decision stand."),
 "node:ShaderPart": (None, {
   "missing": swap("only MSAA 0 recorded for the new cases; ", ""),
   "oracle": swap("(16 cases, MSAA 0, 5 identical recordings)", "(16 cases at MSAA 0, 5 identical recordings, and at MSAA 4, 3 identical recordings)"),
  }, "Issue #95 2026-11-04 (PR #428): the 16 stepped cases are recorded and compared at MSAA 4. Measured mediump half-precision difference, max 1 per channel, pinned. Status stays PARTIAL (KNOWN compile-error text; type is initializeOnly, no stepped type case)."),
 "node:Collision": (None, {
   "missing": swap("; pointing-buffer hit points 0.27 from a surface differ by 3.05e-7 (float32 depth, not compared)", ""),
   "tests": addlist("crates/x3d-viewer/tests/rea_collision_time.rs (ct-wall-pick, ct-nested-pick, ct-bb-ctl-pick: 256 pickProbes each, bit for bit, MSAA 0 and 4, PR #430)"),
  }, "Issue #51 2026-11-04 (PR #430): pointing-buffer hit points 0.27 to 0.38 from a surface equal the pinned page bit for bit. The 3.05e-7 was the avatar's stop position before #423, not float32 depth. Status stays PARTIAL."),
 "node:BoundedPhysicsModel": (None, {
   "missing": swap("Not replayed: a geometry change at a time a TimeSensor-started Script chooses (that Script did not run under the oracle protocol in X_ITE or Rust, so bp-rebuild and bp-replace are driven by a field event). ",
                    "A future TimeSensor startTime in the world file never starts the sensor in X_ITE 16.5.0 (Rust matches; bp-ts-file-rebuild); the start timer is measured for an event at frame 1 only. "),
  }, "Issue #51 2026-11-04 (PR #432): Script-driven geometry changes started by a TimeSensor, 11 cases recorded on the pinned page and replayed exactly (rea_b16_bounded_physics). Status stays PARTIAL (the second-GPU clamp measurement of #89 stays open)."),
 "sub:io.file-loader": (None, {
   "missing": swap("; still missing: redirects, response headers and the foreign text/html hand-off, the Inline data-url BOM, space-in-host URL parsing",
                    "; the Inline data-url BOM and space-in-host URL parsing are compared (PR #434); `*` in a host and `#` / `?` in a `file:` host are pinned differences; still missing: redirects, response headers and the foreign text/html hand-off (they need a Fetcher that returns a response: PR #438, not merged)"),
  }, "Issue #51 2026-11-04 (PR #434): Inline data-URL BOM, space in a URL host and the EXTERNPROTO no-handler text; 3 new pinned groups in file_loader. Status stays PARTIAL."),
 "node:Inline": (None, {
   "missing": swap("; plain Inline reload paths (set_internal_scene, inline_abort) not checked for the browser shutdown rule",
                    "; ADR 0014 (owner decision 2026-11-04): c09, c29 and c35 are kept differences; c07 overlap-frames is an open difference (synthetic-inline-reload-known.tsv)"),
   "tests": addlist(
     "crates/x3d-script/tests/inline_reload_shutdown.rs (38 scenarios: browser build equal to X_ITE, native differs only by the pinned shutdown lines, PR #433)",
     "crates/x3d-script/tests/inline_abort_handlers.rs (oracle/synthetic-inline-abort, 3 identical recordings, PR #433)",
     "crates/x3d-script/tests/inline_reload_safety.rs (the ADR 0014 rules, one overlap test per rule with its negative control)"),
  }, "Issue #51 2026-11-04 (PRs #433, #437): plain Inline reload and abort paths are pinned step by step for the browser shutdown rule. ADR 0014 records c09, c29, c35 as kept differences and c07 as to be fixed. Status stays PARTIAL; no DIFFERENT entry."),
 "sub:render.core": (None, {}, "Issue #89 2026-11-04 (PR #431): rea_b22 two-layer LINE particle shadows on the Apple M1 are a reference variation (ANGLE Metal line width limit 1; the page draws 6 px lines as triangles); Rust equals the M1 recording within 2 pixels per layer; NVIDIA status unchanged."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i437.py check|apply")
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
