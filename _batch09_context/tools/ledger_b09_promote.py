#!/usr/bin/env python3
"""Batch 09 promotion review: 7 of the 13 reviewed node entries become COMPLETE (owner-approved review, 2026-10-20).

  python3 -I ledger_b09_promote.py check     # verify only (exit 1 on a problem)
  python3 -I ledger_b09_promote.py apply     # rebuild the entries of P from the snapshot, then verify

ledger/before-promotion/ledger.json is the authoritative ledger as it was at 0023b6b (235 COMPLETE, 84 PARTIAL, 25 DIFFERENT, 2 MISSING).
Every entry outside P must equal the snapshot, the 25 DIFFERENT entries included."""
import collections, json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-promotion", "ledger.json")
VIS = "crates/x3d-render/tests/render_rea_b09_visual.rs::rea_b09_visual_renders_match_x_ite"
ST = "crates/x3d-viewer/tests/rea_batch09_inputs.rs::rea_batch09_inputs_cases_match_x_ite"
NOTE = "Promotion review 2026-10-20 (combined 0023b6b): "
P = {
 "node:Color": [VIS + " (vis-rendering, MSAA 0/4)"],
 "node:Coordinate": [VIS + " (vis-rendering, MSAA 0/4)"],
 "node:TextureTransform": [VIS + " (vis-texturing, MSAA 0/4)"],
 "node:TextureCoordinate3D": [VIS + " (vis-texturing3d, MSAA 0/4)"],
 "node:TextureCoordinate4D": [VIS + " (vis-texturing3d, MSAA 0/4)"],
 "node:FontStyle": [VIS + " (vis-text-font, vis-text-layout, MSAA 0/4)"],
 "node:TextureBackground": [VIS + " (vis-texturebackground-front/-corner/-sky/-partial, MSAA 0/4)", ST + " (background-runtime, MSAA 0/4)"],
}
WHY = {
 "node:Color": "per-vertex with colorIndex, per-face, PointSet and IndexedLineSet per-vertex all compared; field and event evidence exact (node-rendering). 0.019% at MSAA 0 (49 aliased line px).",
 "node:Coordinate": "IndexedFaceSet, IndexedLineSet and PointSet consumers compared; field and event evidence exact (node-rendering).",
 "node:TextureTransform": "translation, rotation with center, scale, and all three together compared (vis-texturing); field and event evidence exact (node-texturing).",
 "node:TextureCoordinate3D": "3-component texture coordinates into a PixelTexture3D compared; field and event evidence exact (node-texturing3d).",
 "node:TextureCoordinate4D": "4-component texture coordinates with w != 1 into a PixelTexture3D compared; field and event evidence exact (node-texturing3d).",
 "node:FontStyle": "family list and fallback, style, size, spacing, justify (BEGIN/FIRST/MIDDLE/END), horizontal, leftToRight, topToBottom compared; field and event evidence exact (node-text, text-fontstyle, text-family, text-events).",
 "node:TextureBackground": "six textures, partial faces, transparency, sky and ground colors with angles, viewpoint rotation compared; run-time bind and a replaced frontTexture node compared (background-runtime); field and event evidence exact (node-environment).",
}

def apply():
    d = json.load(open(AUTH))
    snap = {i["id"]: i for i in json.load(open(SNAP))["items"]}
    for k, i in enumerate(d["items"]):
        if i["id"] not in P:
            d["items"][k] = snap[i["id"]]  # an entry taken out of P goes back to the snapshot
            continue
        b = snap[i["id"]]
        i["status"] = "COMPLETE"
        i["tests"] = list(b.get("tests") or []) + P[i["id"]]
        i["missing"] = ""
        i["notes"] = (b.get("notes", "") + " | " if b.get("notes") else "") + NOTE + WHY[i["id"]]
    json.dump(d, open(AUTH, "w"), indent=1)

def verify():
    after = {i["id"]: i for i in json.load(open(AUTH))["items"]}
    before = {i["id"]: i for i in json.load(open(SNAP))["items"]}
    bad = []
    if set(before) != set(after):
        bad.append(f"ID set differs: {sorted(set(before) ^ set(after))}")
    for k, b in before.items():
        a = after.get(k)
        if a is None:
            continue
        if k not in P:
            if a != b:
                bad.append(f"unrelated entry changed: {k}")
            continue
        if b["status"] != "PARTIAL":
            bad.append(f"{k}: was {b['status']}, not PARTIAL")
        if a["status"] != "COMPLETE" or a["missing"]:
            bad.append(f"{k}: not promoted cleanly")
        if b.get("notes") and not a["notes"].startswith(b["notes"]):
            bad.append(f"{k}: earlier notes lost")
        if any(t not in a.get("tests", []) for t in b.get("tests", [])):
            bad.append(f"{k}: earlier tests lost")
    c = collections.Counter(i["status"] for i in after.values())
    print(dict(sorted(c.items())), "total", sum(c.values()))
    if sum(c.values()) != 346:
        bad.append("total is not 346")
    if c["DIFFERENT"] != 25:
        bad.append("DIFFERENT count is not 25")
    print("\n".join(bad) or "OK")
    return not bad

if __name__ == "__main__":
    if sys.argv[1:] == ["apply"]:
        apply()
    sys.exit(0 if verify() else 1)
