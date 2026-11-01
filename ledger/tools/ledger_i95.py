#!/usr/bin/env python3
"""Issue #95 ledger update (node:ShaderPart, node:ComposedShader): python3 -I ledger_i95.py check|apply

Stepped url and compile-error cases are now compared with pinned X_ITE 16.5.0 (rea_b24_shaderpart: 16 worlds, 5 identical recordings, MSAA 0), and an error
inside an #include now names the include file and its line like X_ITE. Both entries stay PARTIAL: the compile error console text of eight includes and one
vertex-stage case still differs (ANGLE against naga), and transformFeedbackVaryings is not ported. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i95", "ledger.json")
SNAP_SHA256 = "e91124e4b16907939e66720c9aa84a720a3e4314e49fbd14eb737c0fad3915f7"

T = ["crates/x3d-viewer/tests/rea_b24_shaderpart.rs rea_b24_shaderpart_cases_match_x_ite (16 worlds: url fallback and steps, load switch, shader selection, vertex stage, include errors; log, scene text, poses and pictures; KNOWN table for the compile text)",
     "crates/x3d-render/tests/rea_b24_shaderpart.rs (include_compile_errors_name_the_include_file_and_line_like_x_ite, remap_log_follows_the_line_directives)"]
ORACLE = "pinned X_ITE 16.5.0 (34cdf9f97): oracle/rea-batch24-shaderpart-baseline (16 cases, MSAA 0, 5 identical recordings), oracle/synthetic-rea-b24-shaderpart"
SP = {"tests": lambda old: old + T, "oracle": ORACLE,
    "missing": "compile error console text of the includes Fragment, Material, PBR, Unlit, Stipple (ANGLE rejects an #if on an undefined macro; the port treats it as 0), Texture, PointSize, Vertex and of a vertex-stage statement error differs from ANGLE (KNOWN table in rea_b24_shaderpart); only MSAA 0 recorded for the new cases; type is initializeOnly so no stepped type case exists (parts-runtime and the bad-type worlds cover it)"}
CS = {"tests": lambda old: old + T[:1], "oracle": ORACLE,
    "missing": "transformFeedbackVaryings not ported; includes that need macro context do not link standalone in X_ITE either (ctx-* worlds of batch 08 link them in context); MSAA runtime cases pass"}
NOTE_SP = "Issue #95 2026-10-31: url fallback, empty and bad urls, url = url, load switch, shader selection and vertex-stage steps equal X_ITE. An error inside an include now reports the include file and its line like X_ITE (remap_log through the #line directives). Mutations on the line delta, the include name, ignored compile errors and the stage are caught."
NOTE_CS = "Issue #95 2026-10-31: the batch 24 shader selection and url-step worlds equal X_ITE for ComposedShader validity."
C = {"node:ShaderPart": (None, SP, NOTE_SP), "node:ComposedShader": (None, CS, NOTE_CS)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i95.py check|apply")
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
