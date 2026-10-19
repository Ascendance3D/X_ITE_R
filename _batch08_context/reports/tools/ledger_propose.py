#!/usr/bin/env python3
"""Batch 08 ledger PROPOSAL (written in the cloud recovery session, 2026-10-18).

Reads ledger/before/ledger.json, changes only the 20 entries of selection/SELECTION.md, keeps every other
entry byte-for-byte, appends (never replaces) notes, and writes ledger/proposed/ledger.json plus
ledger/proposed/CHANGES.md (key-by-key before/after). It does not touch the real ledger.

The statuses here assume the local gates in RECOVERY_REPORT.md section G pass on the final source. If a GPU
gate fails for an entry proposed COMPLETE, that entry is PARTIAL. Entries with a pending owner decision
are proposed at the conservative status; ALT gives the status if the owner accepts.

    python3 -I tools/ledger_propose.py            # from _batch08_context/reports
"""
import json, os, re, sys, collections

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BEFORE = os.path.join(HERE, "ledger", "before", "ledger.json")
OUT = os.path.join(HERE, "ledger", "proposed")
TAG = "REA batch 08 (proposal 2026-10-18, pending local GPU gates):"

# id: (status, rust files, tests, missing, note, alt status if an owner decision goes the other way)
P = {
    "sub:physics.engine": ("PARTIAL",
        ["crates/x3d-runtime/src/physics/bt/", "crates/x3d-runtime/src/bullet.rs", "crates/x3d-runtime/src/pick_shapes.rs"],
        ["crates/x3d-runtime/tests/bullet_b08_engine.rs (37, tolerance 0)", "bullet_dyn_scenes (66)", "physics_b08_lifecycle (5)",
         "physics_b06_baseline/lifecycle", "rea_b03_pick_vectors (4000)", "oracle/tools/rea_b06.py check (47)", "coverage.tsv (109 reached symbols tested)"],
        "picking: 3 of 20000 fresh pick cases (4697 cylinder-mesh, 5480 cylinder-cylinder, 11878 cylinder-heightfield) report a contact where Ammo reports none; a PickSensor event would differ",
        "Ammo coverage table complete for dynamics; broadphase proxy-slot leak fixed; GJK ray rounding fixed.",
        "COMPLETE if the owner accepts the 3/20000 pick residue as a known limit"),
    "node:ComposedShader": ("PARTIAL",
        ["crates/x3d-runtime/src/nodes/shaders.rs", "crates/x3d-render/src/glsl_compat.rs", "crates/x3d-render/src/shader_gpu.rs", "crates/x3d-render/src/traverse_shaders.rs"],
        ["render_rea_b08_shaders (84 worlds x MSAA 0/4 vs X_ITE)", "glsl_compat_b08 (11)", "rea_b08_shaders_reload (3)", "rea_b08_release_tables", "rea_batch08_shaders (stepped, baseline not yet made)"],
        "stepped run-time cases vs X_ITE not run (oracle/rea-batch08-shaders-baseline missing): field changes, part replacement, activate; #include <...> of ShaderRegistry not supported; transformFeedbackVaryings not ported; XML (.x3d) worlds have no render baseline",
        "GLSL ES 1.00/3.00 rewritten to GLSL 450, naga to WGSL; X_ITE built-in uniform surface; user fields as uniforms; per-draw pipelines released after 120 frames.",
        None),
    "node:ShaderPart": ("PARTIAL",
        ["crates/x3d-runtime/src/nodes/shaders.rs", "crates/x3d-render/src/glsl_compat.rs"],
        ["render_rea_b08_shaders (bad-*, file-*, data-base64)", "glsl_compat_b08", "rea_b08_shaders_reload"],
        "stepped url/type change cases vs X_ITE not run; compile error console text not compared with X_ITE (driver/ANGLE log text); #include <...> not supported",
        "url fallback, data: URLs, compile errors with X_ITE's message format, custom options.",
        None),
    "node:ShaderProgram": ("PARTIAL", ["crates/x3d-runtime/src/nodes/shaders.rs"], ["render_rea_b08_shaders stub-nodes.wrl", "rea_b08_shaders_reload stub-nodes.wrl"],
        "no run-time field change compared with X_ITE (contract item 3)", "X_ITE has no behavior (requestImmediateLoad does nothing); node and fields exist, never selected.", None),
    "node:ProgramShader": ("PARTIAL", ["crates/x3d-runtime/src/nodes/shaders.rs"], ["render_rea_b08_shaders stub-nodes.wrl", "rea_b08_shaders_reload stub-nodes.wrl"],
        "no run-time field change compared with X_ITE (contract item 3)", "X_ITE 38-line stub; isValid stays FALSE; the Appearance falls back to default shading.", None),
    "node:PackagedShader": ("PARTIAL", ["crates/x3d-runtime/src/nodes/shaders.rs"], ["render_rea_b08_shaders stub-nodes.wrl", "rea_b08_shaders_reload stub-nodes.wrl"],
        "no run-time field change compared with X_ITE (contract item 3)", "'THIS NODE IS NOT SUPPORTED' in X_ITE; node exists, never selected.", None),
    "sub:parse.gltf2": ("PARTIAL", ["crates/x3d-parse/src/gltf2/"],
        ["gltf_rea (287 inputs, 980 text checks)", "render_rea_b08_gltf (16 x MSAA 0/4)", "gltf_inline_b08 (18)", "rea_b08_gltf_reload (7)", "rea_b08_gltf_viewer_buffers"],
        "KHR_draco_mesh_compression skipped (X_ITE rejects garbage Draco; Rust loads undecoded); KHR/EXT_meshopt_compression stub; KHR_texture_basisu, EXT_texture_avif, EXT_texture_webp no decoder; EXT_texture_video runtime not exercised; buffer-fetch console text differs in 2 Inline cases; stepped animation cases run only by x10_compare.py",
        "Viewer3d top-level glTF now uses the runtime fetcher (integration fix).", None),
    "sub:parse.glb2": ("PARTIAL", ["crates/x3d-parse/src/gltf2/glb.rs"], ["gltf_rea (21 .glb)", "gltf_inline_b08", "rea_b08_gltf_reload"],
        "inherits the glTF decoder gaps (Draco, meshopt, BasisU, AVIF/WebP)", "GLB2Parser complete.", None),
    "node:GaussianSplats": ("PARTIAL", ["crates/x3d-render/src/splats.rs", "crates/x3d-render/src/splats_gpu.rs", "crates/x3d-render/src/traverse_splats.rs"],
        ["render_rea_b08_gs (35 x MSAA 0/4)", "rea_batch08_gs", "rea_b08_gs_reload"],
        "castShadow shadow pass, pointerEvents picking, collision, bboxDisplay, sphere culling, reversed/log depth; sort synchronous (X_ITE async)", "", None),
    "node:VolumeScatterMaterialExtension": ("PARTIAL", ["crates/x3d-render/src/scatter_gpu.rs", "crates/x3d-render/src/traverse.rs", "crates/x3d-render/src/x3d.wgsl"],
        ["render_rea_b08_vs (19 x MSAA 0/4)", "rea_batch08_vs (vs-runtime 16 frames)", "rea_b08_vs_reload"],
        "owner decision pending on the contract item 5 reading for attached nodes",
        "Items 1-4, 6 n/a, 7 have evidence in the lane report; item 5 met under the clarified reading.",
        "COMPLETE if the owner accepts the item-5 reading and the local GPU gates pass"),
    "node:DISEntityManager": ("COMPLETE", ["crates/x3d-render/src/bbox_display.rs"], ["render_rea_b08_net", "rea_batch08_net (dis-*)", "rea_b08_net_reload", "rea_b08_net_unload"],
        "", "Parity with X_ITE 16.5.0: fields, mapping alias, no packet IO (X_ITE has none). Real DIS IO is a separate owner decision.", None),
    "node:DISEntityTypeMapping": ("COMPLETE", ["crates/x3d-runtime/src/nodes/load_sensor.rs"], ["render_rea_b08_net", "rea_batch08_net (dis-*)", "rea_b08_net_unload"],
        "", "Parity: X3DInfoNode/X3DUrlObject with an empty loadData, as X_ITE.", None),
    "node:ReceiverPdu": ("COMPLETE", ["crates/x3d-render/src/bbox_display.rs"], ["render_rea_b08_net", "rea_batch08_net (dis-*, UDP loopback negative control)", "rea_b08_net_reload"],
        "", "Parity: X3DNetworkSensorNode + bbox, no packet IO (X_ITE has none).", None),
    "node:SignalPdu": ("COMPLETE", ["crates/x3d-render/src/bbox_display.rs"], ["render_rea_b08_net", "rea_batch08_net (dis-*, UDP loopback negative control)", "rea_b08_net_reload"],
        "", "Parity: X3DNetworkSensorNode + bbox, no packet IO (X_ITE has none).", None),
    "node:TransmitterPdu": ("COMPLETE", ["crates/x3d-render/src/bbox_display.rs"], ["render_rea_b08_net", "rea_batch08_net (dis-*, UDP loopback negative control)", "rea_b08_net_reload"],
        "", "Parity: X3DNetworkSensorNode + bbox, no packet IO (X_ITE has none).", None),
    "node:InlineGeometry": ("PARTIAL", ["crates/x3d-runtime/src/inline_geometry.rs"], ["render_rea_b08_net (ig-*)", "rea_batch08_net (ig-runtime)", "rea_b08_net_reload", "rea_b08_net_unload"],
        "X_ITE (Cache on) gives a second InlineGeometry with the same url a copy() without routes ('TODO: add routes'); Rust keeps the routes; untested; ig-select reload picture check disabled",
        "", None),
    "node:FontLibrary": ("PARTIAL", ["crates/x3d-runtime/src/font_library.rs", "crates/x3d-parse/src/woff1.rs"], ["render_rea_b08_net (fl-*)", "rea_batch08_net (fl-runtime)", "rea_b08_net_unload", "rea_b08_release_tables"],
        "CFF/OpenType-CFF fonts not loaded (X_ITE's opentype.js reads them); two libraries with one family not pinned (order-dependent in X_ITE)",
        "Per-context font tables released with the context (integration fix, rea_b08_release_tables).", None),
    "sub:parse.svg": ("PARTIAL", ["crates/x3d-parse/src/svg.rs", "crates/x3d-parse/src/svg_canvas.rs", "crates/x3d-parse/src/color.rs"],
        ["svg_rea (1095 checks)", "svg quality options", "render_rea_b08_svg (25 x MSAA 0/4)", "css_colors_b08 (16419)", "svg_b08_lifecycle"],
        "hwb/lab/lch/oklab/oklch/color() not parsed; 43 hsl() values one level off Chrome; one radial-focal gradient pixel one level off; SVG quality is a thread-local not reset for a top-level world",
        "", None),
    "sub:parse.html-support": ("PARTIAL", ["crates/x3d-parse/src/html_support.rs", "crates/x3d-parse/src/html.rs", "crates/x3d-parse/src/xml.rs"],
        ["html_support_rea (325 checks)"],
        "owner decision pending: judged on the parser contract, or a DOM integration layer (live attributes, importDocument) required",
        "Name and field maps equal to X_ITE on 53 HTML inputs and 12 XML twins.",
        "COMPLETE if the owner accepts the parser-contract reading"),
    "sub:browser.multi-viewer-thread": ("PARTIAL", ["crates/x3d-script/src/engine.rs", "crates/x3d-viewer/src/embed.rs", "crates/x3d-render/src/traverse.rs"],
        ["multi_viewer_b08 (17)"],
        "owner decision 2026-10-18: 0.65% limit rejected; x_ite_two_canvases_rigid_bodies_msaa4 f40 is 0.579% (single-viewer edge difference); viewers on different threads, particle pictures, sound, second device not tested; process-global geometry provider",
        "Script host state and browser options per viewer (two bugs fixed).", None),
}


def main():
    before = json.load(open(BEFORE))
    sel = re.findall(r"\| \d+ \| (\S+) \|", open(os.path.join(HERE, "selection", "SELECTION.md")).read())
    if sorted(sel) != sorted(P):
        sys.exit(f"selection mismatch: {sorted(set(sel) ^ set(P))}")

    after = json.loads(json.dumps(before))
    changes = []
    for item in after["items"]:
        if item["id"] not in P:
            continue
        status, rust, tests, missing, note, alt = P[item["id"]]
        old = dict(item)
        item["status"] = status
        item["rust"] = sorted(set(item.get("rust", []) + rust))
        item["tests"] = item.get("tests", []) + tests
        item["missing"] = missing
        add = f"{TAG} {note}".strip() + (f" ALT: {alt}." if alt else "")
        item["notes"] = (item.get("notes", "") + " | " + add) if item.get("notes") else add
        changes.append((item["id"], old, item))

    for a, b in zip(before["items"], after["items"]):
        if a["id"] not in P and a != b:
            sys.exit(f"unrelated entry changed: {a['id']}")

    count = lambda d: collections.Counter(i["status"] for i in d["items"])
    cb, ca = count(before), count(after)
    total = sum(ca.values())
    if total != 346 or sum(cb.values()) != 346:
        sys.exit(f"total {total}")

    os.makedirs(OUT, exist_ok=True)
    json.dump(after, open(os.path.join(OUT, "ledger.json"), "w"), indent=1)
    keys = ["COMPLETE", "PARTIAL", "DIFFERENT", "NOT_TESTED", "MISSING"]
    with open(os.path.join(OUT, "CHANGES.md"), "w") as f:
        f.write("# Batch 08 ledger proposal: key-by-key changes\n\nGenerated by tools/ledger_propose.py from ledger/before/ledger.json. Not applied.\n\n")
        f.write("| status | before | proposed |\n| --- | --- | --- |\n")
        for k in keys:
            f.write(f"| {k} | {cb[k]} | {ca[k]} |\n")
        f.write(f"| total | {sum(cb.values())} | {total} |\n\n")
        f.write(f"Coverage (COMPLETE/346): before {cb['COMPLETE']/346:.2%}, proposed {ca['COMPLETE']/346:.2%}.\n\n")
        f.write("MISSING after: " + ", ".join(i["id"] for i in after["items"] if i["status"] == "MISSING") + "\n\n")
        for id_, old, new in changes:
            f.write(f"## {id_}\n\n")
            for k in ["status", "rust", "tests", "missing", "notes"]:
                if old.get(k) != new.get(k):
                    f.write(f"- `{k}`: {json.dumps(old.get(k))} -> {json.dumps(new.get(k))}\n")
            f.write("\n")
    for k in keys:
        print(f"{k:11} {cb[k]:4} -> {ca[k]:4}")
    print(f"total {sum(cb.values())} -> {total}; coverage {cb['COMPLETE']/346:.2%} -> {ca['COMPLETE']/346:.2%}")


if __name__ == "__main__":
    main()
