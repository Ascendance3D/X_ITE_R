#!/usr/bin/env python3
"""Issue #71 ledger update (node:TextureCoordinate): python3 -I ledger_i71.py check|apply

The renderer already selects the coordinate set by mapping name (traverse.rs mapping_channel, coordinate_mappings, transform_mappings; x3d.wgsl): the
stale "mapping field not used by the renderer" text is removed and the evidence is named. render_rea_b09_inputs (94 shots at MSAA 0 and 4,
including texcoord-mapping and phong-textures-mapping) passes at master 0533072e. Mutation: mapping_channel returning 0 fails all four mapping shots
(13-14.5% of pixels over 8). Status changes to COMPLETE. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i71", "ledger.json")
SNAP_SHA256 = "de7740290ff6ffdf157f18b3a489c8750753805b396987a0dd9b56a9fdb58a77"

T = ["crates/x3d-render/tests/render_rea_b09_inputs.rs (texcoord-mapping, phong-textures-mapping: PhysicalMaterial and Material texture mappings against MultiTextureCoordinate, TextureCoordinateGenerator and MultiTextureTransform mappings, an unmatched mapping, a repeated mapping; MSAA 0 and 4)"]
FIELDS = {"tests": lambda old: old + T, "missing": "",
    "oracle": "pinned X_ITE 16.5.0 (34cdf9f97) pictures at MSAA 0 and 4: oracle/render-rea-b09-inputs-baseline and render-msaa4-rea-b09-inputs-baseline (texcoord-mapping.wrl, phong-textures-mapping.wrl)"}
NOTE = "Issue #71 2026-10-31: no renderer change was needed; the mapping worlds recorded in batch 09 already match. Mutation: mapping_channel always 0 fails all four mapping shots. Not pinned: first-match against last-match of a repeated mapping name (the repeated-name case renders the same either way)."
C = {"node:TextureCoordinate": ("COMPLETE", FIELDS, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i71.py check|apply")
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
