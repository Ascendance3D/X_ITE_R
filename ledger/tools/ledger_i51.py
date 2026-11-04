#!/usr/bin/env python3
"""Ledger step after PRs #400, #405, #407-#413, #415 and #417 (issues #51, #69): python3 -I ledger_i51.py check|apply

Thirteen entries, text only, no status change, no DIFFERENT entry touched. Every sentence below is taken from the merged PR body
(measured figures and open items are the PR's own). The entries stay PARTIAL (node:PointSet stays COMPLETE) because each keeps
other recorded gaps. #414 and #416 are not part of this step."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i51", "ledger.json")
SNAP_SHA256 = "45c4261d71634b47fc2900bfab7ca167538525b69e0b6666275a82b65b9ffe5a"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f
def addtext(text):
    return lambda old: old + "; " + text
def addlist(text):
    return lambda old: old + [text]

C = {
 "node:GaussianSplats": (None, {
   "tests": addlist("crates/x3d-render/tests/render_b69_gs_reversed.rs (43 splat and shadow worlds with reversed depth, MSAA 0 and 4, against the pinned standard pictures)"),
   "missing": addtext("reversed depth (a Rust-only mode) has no pinned recording: it is compared with the pinned standard pictures, not with a reversed-depth page, and picking under reversed depth is not tested (the pointing pass ignores the depth mode for splats)")},
  "Issue #69 2026-11-03 (PR #400): reversed depth draws 43 splat and shadow worlds at MSAA 0 and 4 equal to the pinned standard pictures, 0 pixels over 8 and a largest channel difference of 2 (render_b69_gs_reversed). This is equality with the standard picture, not a comparison of reversed depth with X_ITE. Status stays PARTIAL."),
 "sub:render.core": (None, {
   "missing": swap("art/rainbow-room and art/djascendance-live exceed the bound (non-power-of-two texture mipmaps, X8 gap)",
     "art/rainbow-room and art/djascendance-live exceed the bound. The non-power-of-two mipmap part is a measured driver limit on synthetic worlds (render_npot_mip): for some textures with one odd and one even side, the pinned NVIDIA gl.generateMipmap shifts the even side by one texel, the Rust chain is the standard one and matches the driver level by level otherwise; the synthetic npot-parity-shifted world is 2.8% over the bound at MSAA 0 and 4 and is pinned to its measured state, not matched. The two private worlds were not re-read, so their cause is not claimed to be proven"),
   "tests": lambda old: old + [
     "crates/x3d-render/tests/render_npot_mip.rs (synthetic NPOT mipmap worlds against the pinned page; nine inside the bound, npot-parity-shifted pinned to its measured state over the bound)",
     "crates/x3d-render/tests/render_point_tie.rs (12 synthetic worlds, MSAA 0+4, exact)"]},
  "Issue #51 2026-11-03 (PRs #405 and #409): NPOT mipmap difference is a measured driver limit on synthetic worlds (not matched, not widened); point sprite pixel-center ties and the 1/256 px position snap match the pinned GL page (oracle/render-point-tie-baseline, oracle/render-msaa4-point-tie-baseline). Status stays PARTIAL."),
 "node:PointSet": (None, {
   "tests": addlist("crates/x3d-render/tests/render_point_tie.rs (12 synthetic worlds, MSAA 0+4, exact)")},
  "Issue #51 2026-11-03 (PR #409): a pixel center exactly on a sprite edge is filled like the pinned GL page (render_point_tie). pk-points-style (KNOWN_GAPS) and the 56 pixels of vis-items-pointset are not closed by this."),
 "node:DepthMode": (None, {
   "missing": swap("the main color pass is not compared for a reversed depthRange (WebGL ignores near > far; only the pointing pass applies that rule)",
     "main color pass: a reversed depthRange (near > far) is ignored like WebGL (INVALID_OPERATION, the range stays the default), measured against the pinned page at MSAA 0 and 4 with 0 pixels over 8 (render_depthmode_reversed)"),
   "tests": addlist("crates/x3d-render/tests/render_depthmode_reversed.rs (reversed depthRange in the main color pass, MSAA 0 and 4, against the pinned page)")},
  "Issue #51 2026-11-03 (PR #407): the main color pass applied a reversed depthRange; WebGL raises INVALID_OPERATION and keeps the default range. Before the fix the reversed world was 29870 pixels over 8 (11.395%) at MSAA 0 and 30076 (11.473%) at MSAA 4, largest delta 229; after it 0 pixels over 8. Status stays PARTIAL."),
 "node:Collision": (None, {
   "missing": swap("only binding / static load / children paths oracle-tested here; Billboard orientation not exercised",
     "only binding / static load / children paths and, since #408, the WALK-avatar collideTime path and Billboard under Collision are oracle-tested here; pointing-buffer hit points 0.27 from a surface differ by 3.05e-7 (float32 depth, not compared)"),
   "tests": addlist("crates/x3d-viewer/tests/rea_collision_time.rs (WALK-avatar Collision isActive and collideTime order and times: wall in/out/in, enabled, visible, proxy, nested, the collisionRadius * sqrt 2 threshold; Billboard axis 0 1 0 and 0 0 0 under Collision and Anchor; MSAA 0 and 4, three identical recordings each)")},
  "Issue #51 2026-11-03 (PR #408): Collision collideTime and Billboard orientation are compared with pinned X_ITE at MSAA 0 and 4. Status stays PARTIAL; the other gaps of the entry were not audited."),
 "node:Anchor": (None, {
   "missing": swap("Collision collideTime path and Billboard orientation not exercised; ",
     ""),
   "tests": addlist("crates/x3d-viewer/tests/rea_collision_time.rs (Billboard under Anchor with pointing probes, pictures and poses; collideTime path; MSAA 0 and 4)")},
  "Issue #51 2026-11-03 (PR #408): the collideTime path and Billboard orientation under an Anchor are compared (collideTime was already compared in rea_b15_particle_collision.rs). The gap about an Anchor to a non-X3D page or another window stays. Status stays PARTIAL."),
 "sub:parse.golden-gate": (None, {
   "missing": swap("dispatch of binary input (GLB2, STLB, PLYB parsers) not compared through createX3DFromURL",
     "binary dispatch (STLB, PLYB little and big endian, GLB2, by content under other extensions, gzipped, no-handler input, URL fallback lists) is compared through createX3DFromURL on 73 inputs (golden_gate_binary, three identical recordings per world): scene text of every dumped frame and the loader's log exact; NaN STL coordinates (the pinned page logs a stack overflow) are not compared; parsers added or removed with GoldenGate.addParsers / removeParsers and PLY Gaussian splats are not covered"),
   "tests": addlist("crates/x3d-viewer/tests/golden_gate_binary.rs (73 binary inputs through createX3DFromURL: scene text of every dumped frame and the loader log exact)")},
  "Issue #51 2026-11-03 (PR #410). Status stays PARTIAL: the entry's behavior and oracle fields are empty, so any status change is the owner's."),
 "sub:nav.collision-gravity": (None, {
   "missing": swap("not compared: active-layer gravity with two Layers (C17); C18 only indirectly through the trans-*-walk poses; ",
     "C17 (active-layer gravity with two Layers) is compared in 7 cases with two Rust defects fixed (collide runs for every drawn layer; the fall speed is per layer); C18 is compared directly in 5 cases (poses, the wall's isActive count, probes after the transition) with one known gap: a probe in the middle of a transition reads X_ITE's stale shapes; "),
   "tests": addlist("crates/x3d-viewer/tests/rea_b24_nav.rs (C17 7 cases and C18 5 cases added by #411; docs/reports/NAV_ITEM_LISTS.md compared 19, missing 0)")},
  "Issue #51 2026-11-03 (PR #411): the pinned page applies the active-layer rule to gravity and to the viewer's constrained moves, not to collide: every drawn layer runs the COLLISION traversal with its own viewpoint, NavigationInfo and Collision nodes. Status stays PARTIAL."),
 "sub:parse.svg": (None, {
   "missing": swap("relative color syntax (lab(from red l a b)) not parsed;",
     "relative color syntax is parsed and evaluated (1,794 values exact at 1e-12 against Chrome; 302 converted values within 5e-6 and 15 values with an srgb-linear origin within 1e-4, both bounds approved by the owner for those groups only; achromatic and exact-zero noise after conversions and var() origins are not reproduced);"),
   "tests": addlist("crates/x3d-parse/tests/rea_css5_relative_color.rs (1,794 exact + 317 converted CSS Color 5 relative colors vs Chrome, also through the scene-loading path; 5e-6 for 302 values, 1e-4 for 15 srgb-linear-origin values)")},
  "Issue #51 2026-11-03 (PR #412): CSS Color 5 relative colors, oracle/css5-relative-colors-baseline and oracle/css5-relative-svg-baseline (13 SVG files, svg_css5_relative_colors_match_x_ite). Status stays PARTIAL: hsl/hwb ties, the radial-focal pixel and the SVG quality thread-local remain."),
 "sub:io.file-loader": (None, {
   "missing": swap("non-VRML formats and fetch error messages not compared at this level; no subsystem-level test listed",
     "text formats, encodings, gzip, url resolution, data: and ecmascript: candidates, fallback order and the failure text are compared with pinned X_ITE 16.5.0 at the createX3DFromURL level (file_loader, 228 lists, three identical recordings; GLB/STL/PLY by golden_gate_binary); still missing: redirects, response headers and the foreign text/html hand-off, the Inline data-url BOM, space-in-host URL parsing"),
   "tests": addlist("crates/x3d-viewer/tests/file_loader.rs (228 url lists through createX3DFromURL; oracle/file-loader-baseline)")},
  "Issue #51 2026-11-03 (PR #413): gunzip takes a single member, no second gunzip, a data: text url is read as a string. Status stays PARTIAL."),
 "sub:nav.walk-fly": (None, {
   "missing": swap("not compared: Rubberband overlay (F15), ContentScale (F17), macOS roll angle (F18)",
     "not compared: macOS roll angle (F18, macOS only), MSAA above 0 and Viewport nodes for the rubber band; F15 (Rubberband overlay) is compared at MSAA 0 in 18 cases with equal pixel sets and a channel difference of 0, F17 (ContentScale 0.5 to 3) shows no difference"),
   "tests": addlist("crates/x3d-viewer/tests/rea_nav_overlay.rs (28 cases, three identical recordings each; oracle/rea-nav-overlay-baseline)")},
  "Issue #51 2026-11-03 (PR #415): the rubber band of FLY and WALK is drawn like the pinned page. Status stays PARTIAL."),
 "node:FontLibrary": (None, {
   "missing": swap("no end-to-end runtime FontLibrary or FontStyle run with a CFF font (Font::parse is compared)",
     "runtime FontLibrary, family-url and ScreenFontStyle runs with the authored CFF font are compared (rea_p_cff_runtime, 5 cases: scene text and log of the pinned page); open: ScreenFontStyle with a FontLibrary CFF font and glyphs whose charstring width differs from hmtx (W, V in the authored font): pinned textBounds 38 1 and lineBounds 36.26667 for WVWV at pointSize 16, Rust 33 1 and 31.57333 (not baselined)"),
   "tests": addlist("crates/x3d-viewer/tests/rea_p_cff_runtime.rs (5 worlds: FontLibrary with 11 Texts, family url, url lists with warnings and errors, dynamic changes, ScreenFontStyle layout; oracle/rea-p-cff-runtime-baseline)")},
  "Issue #51 2026-11-04 (PR #417): text lineBounds are single precision in the pinned page and the port rounded them to f32 in the horizontal and vertical layout (textBounds 3.9999999379724027 against 4 with the CFF font before). Status stays PARTIAL."),
 "sub:text.fonts": (None, {
   "missing": swap("ScreenText with a CFF font not compared (cubics flattened into 16 lines)",
     "ScreenText with a CFF font: layout compared (rea_p_cff_runtime), the raster not compared (cubics flattened into 16 lines); ScreenFontStyle W/V advance with a FontLibrary CFF font is an open difference (see node:FontLibrary)"),
   "tests": addlist("crates/x3d-viewer/tests/rea_p_cff_runtime.rs (5 worlds, scene text and log exact)")},
  "Issue #51 2026-11-04 (PR #417): lineBounds are single precision (fixed). Status stays PARTIAL."),
}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i51.py check|apply")
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
