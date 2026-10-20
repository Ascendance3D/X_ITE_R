#!/usr/bin/env python3
"""Batch 09 cloud ledger update: targeted edits of the authoritative ledger (_batch09_context/ledger/ledger.json).

  python3 -I ledger_b09_cloud.py check     # verify only (exit 1 on a problem)
  python3 -I ledger_b09_cloud.py apply     # apply C to the authoritative ledger, then verify

ledger_b09.py is not used: its `apply` rebuilds from ledger/before/ledger.json and writes to another tree's path. This tool starts
from the authoritative ledger itself, and ledger/before-cloud/ledger.json is the snapshot it was taken from at the start of the
cloud session (231 COMPLETE, 55 PARTIAL, 25 DIFFERENT, 33 NOT_TESTED, 2 MISSING). `apply` is idempotent: each entry of C is
rebuilt from the snapshot and C.

Each entry of C: (new status, tests appended, missing text, note appended). Earlier notes are kept as a prefix.
`verify` checks: the 346 IDs of the snapshot, every entry outside C equal to the snapshot, no DIFFERENT entry in C or changed, and
for each entry of C its status, its earlier notes kept and its earlier tests kept.
"""
import collections, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-cloud", "ledger.json")

# id: (status, tests, missing, note)
C = {
    "sub:parse.golden-gate": (
        "PARTIAL",
        ["crates/x3d-viewer/tests/rea_batch09_sai.rs parse-formats parse-xml-mfstring parse-xml-unknown"],
        "dispatch of binary input (GLB2, STLB, PLYB parsers) not compared through createX3DFromURL",
        "b09 cloud 4b4daa1: parse-formats (50 createX3DFromString inputs: XML, VRML, JSON, glTF JSON, OBJ, STL ASCII, PLY ASCII, SVG, "
        "HTML, unknown, empty and whitespace input; the parser chosen, the scene and the rejection text) baselined from X_ITE "
        "(3 identical recordings), exact",
    ),
    "sub:exec.context-api": (
        "PARTIAL",
        ["crates/x3d-viewer/tests/rea_batch09_sai.rs ctxev-* cfg-scene cfg-comp"],
        "X3DExecutionContext / X3DScene class chain (brx-obj-open: the port's scene objects are plain objects); kept PARTIAL by "
        "owner decision 2026-10-20 until its full REA requirements pass",
        "b09 cloud: timing differences A-C fixed (createX3DFromString parse as a microtask, SFNode creation on a stream's first "
        "read, Inline change events after the frame); component state of a scene exact (cfg-comp)",
    ),
    "sub:exec.import-export": (
        "COMPLETE",
        ["crates/x3d-viewer/tests/rea_batch09_sai.rs ie-chain ie-rename ie-direct ie-err-* ie-err-string ie-err-string-frames"],
        "",
        "b09 cloud e62be31+: IMPORT -> local node -> EXPORT chains, renameImportedNode / updateImportedNode / removeImportedNode / "
        "addImportedNode from a Script, duplicate and missing names, exports of imported nodes, ROUTE before load, failed load, "
        "self import, createX3DFromString with IMPORT/EXPORT (incl. the frame its promise settles): 14 cases, each baselined from "
        "X_ITE (3 identical recordings), exact (scene text every frame, sorted log)",
    ),
    "sub:exec.bindable-stacks": (
        "COMPLETE",
        ["crates/x3d-viewer/tests/rea_batch09_sai.rs bind-*"],
        "",
        "b09 cloud e62be31+: bind-{Viewpoint,OrthoViewpoint,GeoViewpoint,NavigationInfo,Background,TextureBackground,Fog}, "
        "bind-route, bind-inline, bind-read, bind-browser, bind-switch, bind-layers (isBound / bindTime of every node routed to a "
        "Script, per layer, activeLayer changes, layers removed and added): 13 cases baselined from X_ITE (3 identical "
        "recordings), exact; poses 1e-4",
    ),
    "sub:config.components-profiles": (
        "COMPLETE",
        ["crates/x3d-viewer/tests/rea_batch09_sai.rs cfg-browser cfg-scene cfg-comp cfg-parse-*"],
        "",
        "b09 cloud: supported profiles and components (title, providerURL, external, dependencies, aliases), getProfile / "
        "getComponent / loadComponents incl. errors, the add/update/removeSupported* calls, scene profile and components "
        "(cfg-comp: any name or level a Script gives is kept and printed as X_ITE does; missing, null and unknown arguments), "
        "PROFILE/COMPONENT statements in VRML, XML and JSON: 8 cases baselined from X_ITE (3 identical recordings), exact. Kept "
        "documented (cfg-scene-odd): the text of the TypeError addComponent () throws is the JavaScript engine's (QuickJS words "
        "it differently from V8); the thrown class and the state are compared",
    ),
    "node:MetadataSet": (
        "COMPLETE",
        ["crates/x3d-viewer/tests/rea_batch09_sai.rs meta-parse meta-events meta-nodes", "crates/x3d-parse/tests/gltf_rea.rs"],
        "",
        "b09 cloud 6a1305c+: meta-nodes (55 probes: all six metadata nodes' values, events, nesting, cycles, USE, VRML/XML/JSON "
        "printing, getNodeType) baselined from X_ITE (3 identical recordings) and exact; meta-parse, meta-events exact; gltf_rea 287 "
        "X_ITE results incl. KHR_materials_variants MetadataSet. X3DNode getMetaData/setMetaData are internal (not reachable from a "
        "Script); their only output, the glTF variants metadata, is compared. Report: _batch09_context/step3-final/REPORT.md",
    ),
    "sub:fields.types": (
        "PARTIAL",
        ["crates/x3d-viewer/tests/rea_batch09_sai.rs fld-* fldx-* fcb-* scr-field-access scr-field-defaults",
         "crates/x3d-script/tests/script.rs", "crates/x3d-script/tests/weak_set_engine.rs"],
        "X3DConstants of external-component node types (X_ITE order varies run to run); IterableWeakSet members held strongly "
        "(QuickJS: X_ITE's own class holds them as strongly in this engine, weak_set_engine.rs); engine-level Intl/Error.stack; the number "
        "of toString calls on a raw component (fldx-print-count-open: X_ITE calls it once per print of it, the port takes the texts "
        "of a host's raw values when its Scripts have run and before a Script prints)",
        "b09 cloud: Script field objects are X_ITE's own classes (prelude generated from the pin). Owner decision 2026-10-20 "
        "(approved safety difference, kept within this entry): a length or index near 2^32 is a RangeError and the field keeps its "
        "prior value; X_ITE allocates and runs out of memory (script.rs huge_mf_length_or_index_is_a_range_error_not_an_allocation, "
        "huge_length_on_a_node_field_keeps_its_prior_value). Owner decision 2026-10-20 (Decision 2, no string-to-NaN difference): "
        "fldx-internal (strings, numeric strings, '', null, undefined, NaN, true, objects, '1e3' written into Vector3 / Color3 "
        "through getValue (), the setters, constructors, Rotation4, MFFloat, MFVec3f elements, SFFloat; node fields with routes and "
        "Script fields) baselined from X_ITE (3 identical recordings), exact: raw components kept with the value and along routes, "
        "never default, printed String (raw) without a unit and ToNumber / factor with one. Checkpoint after b2e5b9f: fldx-print "
        "(VRML, XML, JSON of node, field and scene; units; a component whose text changes after assignment), fldx-units (a node's or "
        "field's own toXXXString converts no unit: X3DObject pushes a null execution context), fldx-cross (a raw value one Script "
        "writes, read by another), each 3 identical X_ITE recordings, exact; weak_set_engine.rs: the bundled QuickJS keeps a "
        "FinalizationRegistry target that is its own token alive, and freeing a cycle with a WeakRef to such a target corrupts its "
        "heap (reproduced without port code), so the WeakRef stays replaced; unreachable members are still freed (memory flat)",
    ),
    "node:Script": (
        "PARTIAL",
        ["crates/x3d-script/tests/script.rs a_throwing_field_callback_ends_its_event_and_the_runtime_goes_on",
         "crates/x3d-viewer/tests/rea_batch09_sai.rs scr-globals scr-classes scr-url-order-sync scr-url-order-micro scr-image-events"],
        "scr-classes-open: the page's own globals (window, document, XMLHttpRequest, fetch) are not provided (owner decision "
        "2026-10-20, Decision 1: no DOM, no stand-in objects); "
        "scr-url-order-open: the frame a fetched Script candidate (a file, a base64 data URL) completes in, and that Chrome completes "
        "a base64 data URL's fetch after the files' (network completion in the oracle page; the microtask order now matches, "
        "scr-url-order-micro); "
        "scr-browser-open: X3DConstants of external-component node types (X_ITE's load order)",
        "b09 cloud: owner decision 2026-10-20: a field callback that throws is reported at error level and ends the callbacks of "
        "that event (as X_ITE's processEvent loop); the runtime goes on (X_ITE's browser stops), value and routes kept. Checkpoint "
        "after b2e5b9f: Script URL candidates step as X_ITE's FileLoader does (a throwing callback reaches the next candidate three "
        "microtask ticks later, a URL fetch rejects at once two ticks later, an unparsable URL one, a fetched one after its fetch); "
        "scr-url-order-micro, 3 identical X_ITE recordings, exact",
    ),
    "sub:fields.math": (
        "PARTIAL",
        ["crates/x3d-viewer/tests/geom_math.rs (oracle/geom-math-baseline.json from oracle/tools/geom_xite.mjs)"],
        "the X_ITE geometry the port has no counterpart of is not compared: Box2, Triangle2, Spheroid3, Camera.frustum / "
        "perspective, the ViewVolume instance (set, getEdges, intersectsSphere, intersectsBox), Box3 beyond fromExtents / multRight "
        "/ getExtents, Triangle3.triangulatePolygon (libtess), Algorithms (Bezier, SAT, eigen_decomposition, sorts)",
        "b09 cloud checkpoint after b2e5b9f: the pinned X_ITE's own standard/Math/Geometry modules (node, three identical runs) on "
        "2434 cases (degenerate ones and seeded random ones): Line3 (fromPoints, closest point to a point and a line, perpendicular "
        "vectors, intersectsTriangle, multMatrixLine / multLineMatrix), Line2, Plane3 (distance, closest point, intersectsLine, "
        "multLeft / multRight), Sphere3 and Cylinder3 intersectsLine, Triangle3 area / normal / quadNormal, ViewVolume "
        "unProjectPoint / unProjectRay / projectPoint / projectLine, Camera.ortho, Box3 fromExtents / multRight / getExtents: the "
        "port's geom.rs and lookat.rs equal it bit for bit",
    ),
}


# The NOT_TESTED nodes (checkpoint after b2e5b9f): field and event evidence from oracle/tools/gen_rea_b09_nodes_cloud.py. Each stays
# PARTIAL: no visual evidence is compared in the cloud (no GPU).
NODE_WORLDS = {
    "node-rendering": ["Color", "ColorRGBA", "Coordinate", "CoordinateDouble", "IndexedLineSet", "Normal", "PointSet"],
    "node-shape": ["Appearance", "Material", "Shape"],
    "node-lighting": ["DirectionalLight", "PointLight", "SpotLight"],
    "node-texturing": ["ImageTexture", "PixelTexture", "TextureTransform", "MovieTexture"],
    "node-texturing3d": ["TextureCoordinate3D", "TextureCoordinate4D"],
    "node-text": ["FontStyle", "Text"],
    "node-layout": ["LayoutGroup", "ScreenFontStyle", "ScreenGroup"],
    "node-environment": ["LocalFog", "TextureBackground"],
    "node-navigation": ["OrthoViewpoint"],
    "node-hanim": ["HAnimJoint", "HAnimSegment", "HAnimSite"],
    "node-sound": ["AudioClip", "Sound"],
    "node-dis": ["EspduTransform"],
}
for _world, _types in NODE_WORLDS.items():
    for _t in _types:
        _missing = "visual evidence (rendering) not compared: needs the desktop GPU run"
        if _t == "Text":
            _missing += "; the layout outputs origin, textBounds, lineBounds are not sent by the port's runtime (node-text-outputs-open: " \
                        "X_ITE sends them when it rebuilds the text, after its default font has loaded)"
        C["node:" + _t] = (
            "PARTIAL",
            [f"crates/x3d-viewer/tests/rea_batch09_sai.rs {_world}"],
            _missing,
            f"b09 cloud checkpoint after b2e5b9f: {_world} (3 identical X_ITE recordings, exact): field definitions (name, access "
            "type, data type, default value), a created node's VRML / XML / JSON text, and on a live node a callback on every field "
            "that sends events, every inputOnly / inputOutput field (not node fields) set to a sample value, the callbacks in order "
            "with their values, the node's text after"
            + (" (OrthoViewpoint's fieldOfView now printed as X_ITE's SFVec4f of any length)" if _t == "OrthoViewpoint" else ""),
        )


def apply():
    # Each entry of C is rebuilt from the snapshot (its state when the cloud session began) and C: running apply again, or after C
    # changed, gives the same entry (no note is appended twice, a changed `missing` text is taken).
    d = json.load(open(AUTH))
    snap = {i["id"]: i for i in json.load(open(SNAP))["items"]}
    for i in d["items"]:
        p = C.get(i["id"])
        if not p:
            continue
        st, tests, missing, note = p
        b = snap[i["id"]]
        i["status"] = st
        i["tests"] = list(b.get("tests") or []) + [t for t in tests if t not in (b.get("tests") or [])]
        i["missing"] = missing
        i["notes"] = (b.get("notes", "") + " | " if b.get("notes") else "") + note
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
        if k not in C:
            if a != b:
                bad.append(f"unrelated entry changed: {k}")
            continue
        if b["status"] == "DIFFERENT":
            bad.append(f"DIFFERENT entry in C: {k}")
        if a["status"] != C[k][0]:
            bad.append(f"{k}: {a['status']} != {C[k][0]}")
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
