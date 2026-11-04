#!/usr/bin/env python3
"""Ledger step after PRs #416, #418, #420-#425 (issues #51, #69, #89): python3 -I ledger_i425.py check|apply

Eight entries, text only, no status change, no DIFFERENT entry touched. Every sentence is taken from the merged PR body (measured
figures, known differences and open items are the PR's own). Entries stay as they are in status (PARTIAL; COMPLETE ones are not
touched). Known unresolved differences stay visible: pp-row-point-props, the nine tie sprites, the CFF2-without-VariationStore
loading difference."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i425", "ledger.json")
SNAP_SHA256 = "3d1dee4006428d03b6716a7570c2429f3b8aa4a91cb3e4acaf14d688e611f5c4"

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
 "node:ParticleSystem": (None, {
   "missing": chain(
     swap("shadow-pass culling of the particle bbox (the shadow pass draws every particle, including off-screen systems); ", ""),
     swap("particle records of a system inside an Inline are not compared (images and seeds are: pr-inline, pr-inline-transform; the dumper looks up root-scene nodes only); ", ""),
     swap("a texCoord ramp on POINT or LINE systems is not modeled or recorded; ", "")),
   "tests": addlist(
     "crates/x3d-viewer/tests/rea_b25_particle_shadow_cull.rs (11 cases: a system inside or outside the light view volume of a SpotLight, a PointLight and a DirectionalLight control, bboxSize and bboxCenter, transform; MSAA 0 and 4, at most 8 pixels per image, records compared; oracle/rea-b25-particle-shadow-cull-baseline, 3 identical pinned runs per pass; not yet in the GPU_TESTS list of the workflows, so the hosted CPU runner records an allowed no-gpu-adapter skip)",
     "crates/x3d-viewer/tests/rea_b13_particles.rs (pr-inline-records, pr-inline-records-nested: the records of a ParticleSystem inside an Inline, node path INLINE/NAME; pr-point-texture, pr-point-texramp, pr-point-texramp-props, pr-line-texture, pr-line-texramp: a texCoord ramp on POINT and LINE, which the pinned page ignores)"),
   "oracle": addtext("oracle/rea-b25-particle-shadow-cull-baseline (shadow-pass culling of the particle bbox) and 7 more cases in oracle/rea-b13-particles-baseline"),
  },
  "Issue #51 2026-11-04 (PR #425): shadow-pass culling is fixed. Pinned X3DRenderObject.addDepthShape keeps a shape in the shadow pass only if its bbox sphere meets the light view volume; a ParticleSystem bbox is the emitter box (or bboxSize), so a system outside a SpotLight cone or outside five of the six PointLight faces casts no shadow there. Rust drew every particle in every shadow view and now applies the same sphere test (traverse.rs shadow_display, thin-line path too). With the fix reverted, 24 image comparisons fail in 6 cases (up to 5.4% of the pixels); the 5 control cases pass either way. Records of a system inside an Inline are dumped by a script node path INLINE/NAME (nested too) in both the oracle and the Rust driver. A texCoord ramp on POINT and LINE: pinned X_ITE ignores it (texCoordCount 0) and Rust already matched; the LINE negative control fails, the POINT case has no discriminating control (Rust POINT does not read vertex texture coordinates). UNVERIFIED: Shape, InstancedShape and GaussianSplats with an explicit bboxSize are culled the same way in the pinned shadow pass and not yet in Rust (not measured); the DEPTH traversal (RenderedTexture depthMap) with a particle system; BoundedPhysicsModel bboxes in a shadow pass. Status stays PARTIAL."),
 "node:FontLibrary": (None, {
   "missing": chain(
     swap("open: ScreenFontStyle with a FontLibrary CFF font and glyphs whose charstring width differs from hmtx (W, V in the authored font): pinned textBounds 38 1 and lineBounds 36.26667 for WVWV at pointSize 16, Rust 33 1 and 31.57333 (not baselined); two libraries",
          "ScreenFontStyle with the authored CFF font, FontLibrary and family-url routes, W/V charstring widths, maxExtent and late font arrival are compared (rea_q_screen_cff, 4 cases, Text output events, PR #420); open: the canvas raster of CFF glyphs (cubics flattened into 16 lines); two libraries"),
     swap("CID-keyed CFF and CFF2 not recorded",
          "CID-keyed CFF and CFF2 are loaded and compared (rea_t_cff_cid, PR #422, 11 authored fonts); known difference: a CFF2 without VariationStore never completes loading in X_ITE, Rust finishes with empty glyphs; UNVERIFIED: variable CFF2 fonts (fvar, avar, HVAR, blend with coordinates), seac inside a CID-keyed font, subroutine depth over 10, subroutine bias for 1240 or more subrs")),
   "tests": addlist(
     "crates/x3d-viewer/tests/rea_q_screen_cff.rs (4 cases: ScreenFontStyle with the authored CFF font, FontLibrary and family-url routes, W/V widths, maxExtent, late arrival; oracle/rea-q-screen-cff-baseline)",
     "crates/x3d-text/tests/rea_t_cff_cid.rs and crates/x3d-viewer/tests/rea_t_cff_cid.rs (CID-keyed CFF with FDSelect 0/3 and 2 or 3 FDs, CFF2 with FDSelect 3/4/none, VariationStore, vsindex, blend; 11 newly authored fonts from oracle/tools/gen_cff_cid_fonts.py; opentype.js 2.0.0 quirks reproduced: INDEX objectOffset, header endOffset, unsigned DICT operand 28, advance-width rules)"),
  },
  "Issue #51 2026-11-04 (PRs #420, #422): the double ceil of the screen text box and the wait for FontStyle loads in the native world load are fixed (#420). The CFF parser and charstring interpreter are a port of opentype.js 2.0.0 including its quirks, and malformed fonts are rejected with the opentype.js message (#422). The charstring width replaces the hmtx advance only when the charstring has a width argument or a stem hint; in CFF2 a stem hint sets haveWidth with defaultWidthX 0 so the advance becomes 0. Status stays PARTIAL."),
 "sub:text.fonts": (None, {
   "missing": swap("ScreenFontStyle W/V advance with a FontLibrary CFF font is an open difference (see node:FontLibrary)",
                    "ScreenFontStyle W/V advance with a FontLibrary CFF font is compared (rea_q_screen_cff, PR #420); CID-keyed CFF and CFF2 are compared (PR #422), with the open items of node:FontLibrary"),
  }, None),
 "node:InstancedShape": (None, {
   "missing": swap("Volume picking (PickableGroup) of instances is not compared (X_ITE uses the base geometry)",
                    "pick sensors (Line GEOMETRY/BOUNDS, Primitive, Volume, Point) over InstancedShape targets and TransformSensor.targetObject = InstancedShape are compared with pinned X_ITE (instanced_pick, oracle/instanced-pick-baseline/ip-sweep.json); X_ITE collects only the base geometry and does not accept an InstancedShape as pickTarget"),
   "tests": addlist("crates/x3d-viewer/tests/instanced_pick.rs (oracle/instanced-pick-baseline/ip-sweep.json: pick sensors over InstancedShape targets, TransformSensor.targetObject = InstancedShape)"),
  }, "Issue #51 2026-11-04 (PR #418): pick sensors over InstancedShape are compared with the pinned page, which collects only the base geometry. Status stays PARTIAL (Text under InstancedShape stays untested)."),
 "sub:parse.gltf2": (None, {
   "missing": chain(
     swap("EXT_texture_video runtime not exercised; buffer-fetch console text differs in 2 Inline cases;",
          "EXT_texture_video: scene construction and load-failure diagnostics are compared (gltf_fetch gf-video), playback frames of a glTF video are not compared; the Inline buffer-fetch console text is compared and passes, only the leaked loading object (fallback.gltfFailed) differs; a MovieTexture whose video url is a PNG file leaves createX3DFromURL unsettled in the pinned page (measured once, Rust UNVERIFIED);")),
   "tests": addlist("crates/x3d-viewer/tests/gltf_fetch.rs (oracle/gltf-fetch-baseline: gf-buffer 38, gf-image 26, gf-video 14 URL lists through Browser.createX3DFromURL; newly authored glTF/GLB files, 3 identical pinned recordings each)"),
  }, "Issue #51 2026-11-04 (PR #424): createX3DFromURL of a glTF or GLB whose buffer fails now follows the Inline buffer-failure path (the candidate FileLoader logs nothing and the attempt is not counted); an image without uri or bufferView gets url \"undefined\" as in the page. Known difference, pinned and not passed: the pinned page names a decoded image that fails by a random blob URL, the engine names the file or data URL itself (KNOWN_UNBLOBBED lists the 6 exact names). Status stays PARTIAL."),
 "sub:parse.glb2": (None, {
   "tests": addlist("crates/x3d-viewer/tests/gltf_fetch.rs (GLB external buffers and GLB image in the BIN chunk; oracle/gltf-fetch-baseline)"),
  }, None),
 "sub:pointer.picking": (None, {}, "Issue #69 2026-11-04 (PRs #416, #421, Apple M1, Metal): on ANGLE Metal the aliased line width range is [1, 1] and the page draws 1 px lines as Metal-native line primitives; Rust now draws them the same way (gpu::metal_native_lines, PointingPrimitive::Lines), so pk-events and pp-slant-line match exactly on the M1 (negative-controlled). Known unresolved differences, pinned and not passed: pp-row-point-props (6 probes, 3 ulps, M1_KNOWN_DIFFS) and the nine tie sprites ml-sprite-low, ml-sprite-pos and ml-sprite-size (TIE_DIFFS in metal_coverage.rs, measured errors up to 1.33e-3, no platform tolerance). Lines drawn by the page as triangles (width above 1) still differ by up to 243 ulps in pk-lines (existing allowance 398). Validated on one machine (Apple M1, macOS 27.0.1, Chrome 156). See docs/metal-reference-variation.md."),
 "sub:nav.walk-fly": (None, {}, "Issue #89 2026-11-04 (PR #423): the fly viewer takes dt from two millisecond Date.now() numbers as the page does (the seconds subtraction rounded differently, 4.8e-8 at frame 100). Frames 35, 60 and 80 are compared at 1e-13 on every runner; frame 100 (stairs, height from the depth pass) needs a GPU adapter and was validated on the physical M1 (walk-steps, walk-instanced-steps)."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i425.py check|apply")
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
