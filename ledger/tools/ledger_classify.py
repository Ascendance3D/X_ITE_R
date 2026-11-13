#!/usr/bin/env python3
"""Ledger step: the classification of the 21 DIFFERENT entries (owner decision 2026-11-12, schema in verify_ledger.py), and the
evidence of four of them (sub:browser.dom-integration, node:SplinePositionInterpolator, node:SplinePositionInterpolator2D,
node:IconAnnotation). No status changes: the step names the 21 entries in its evidence set, so verify_ledger.py lets it change
only their EVIDENCE_FIELDS.
python3 -I ledger/tools/ledger_classify.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-classify", "ledger.json")
SNAP_SHA256 = "289c7600e7021720bc55be35b210871f16baed46d9f4d9bdf124fb6fb33a356b"
REVIEWED = "2026-11-12"

def append(*extra):
    return lambda old: list(old) + list(extra)

NURBS = ["node:NurbsCurve", "node:NurbsCurve2D", "node:NurbsOrientationInterpolator", "node:NurbsPatchSurface", "node:NurbsPositionInterpolator",
         "node:NurbsSet", "node:NurbsSurfaceInterpolator", "node:NurbsSweptSurface", "node:NurbsSwungSurface", "node:NurbsTextureCoordinate",
         "node:NurbsTrimmedSurface"]
ANNOTATION_REGISTERED = ["node:AnnotationTarget", "node:GroupAnnotation", "node:TextAnnotation", "node:URLAnnotation"]
ISO_ANNOTATION = {"state": "UNVERIFIED"}  # the local ISO/IEC 19775-1:2023 capture has no file of clause 42 (Annotation)

SPLINE_ISO = {
    "state": "VERIFIED",
    "clauses": ["19775-1:2023 19.2.3", "19775-1:2023 19.2.4"],
    "source": "ISO/IEC 19775-1:2023 (X3D V4.0) Part 1, components/interpolators.md of the owner's local standards capture; both clauses read in full on 2026-11-12",
}
SPLINE = {
    "class": "UPSTREAM_DEFECT_CORRECTED",
    "iso": SPLINE_ISO,
    "decision": {"ref": "owner decision 2026-10-16 (keep the math); PR #581 (the 19.2.3 and 19.2.4 rules)", "reason": "X_ITE 16.5.0 throws; ISO 19775-1 gives the rules"},
    "upstream_defects": [{
        "summary": "normalizeVelocity TRUE with a non-empty keyVelocity throws 'value.norm is not a function' (SFVec fields have no norm), and X_ITE fails the scene load",
        "evidence": "oracle/synthetic-rea/interp-ext.wrl",
        "rust_test": "crates/x3d-runtime/tests/spline_normalize_velocity.rs",
    }],
    "impact": "equal to X_ITE on its working paths; SP3 of interp-ext.wrl (keyVelocity of size 2) differs from X_ITE on purpose in value and keyVelocity text in all 130 frames (19.2.4: first and last velocity)",
    "reviewed": REVIEWED,
}
SPLINE_NOTE = ("2026-11-12 (PRs #577, #581; evidence only, status unchanged): ISO/IEC 19775-1:2023 19.2.3 and 19.2.4 apply: the final velocity "
               "is normalized, the keyVelocity size rule (equal to keyValue, or 2 for the first and last key, else ignored), and closed is ignored "
               "when the velocities of the first and last key are given or their keyValues differ. oracle/synthetic-rea/interp-ext.wrl SP3 now "
               "differs from X_ITE on purpose (keyVelocity of size 2): value and keyVelocity text, all 130 frames. SplineScalarInterpolator is unchanged.")

DOM_RUST = ["crates/x3d-web/web/x_ite_r.mjs", "crates/x3d-runtime/src/dom.rs", "crates/x3d-web/src/session.rs", "crates/x3d-web/src/wasm.rs",
            "crates/x3d-web/src/chain.rs"]
DOM_TESTS = ["crates/x3d-web/tests/browser/dom01.test.mjs", "crates/x3d-web/tests/browser/dom02.test.mjs", "crates/x3d-web/tests/browser/dom03.test.mjs",
             "crates/x3d-web/tests/dom_mutation.rs"]
DOM_NOTE = ("2026-11-12 (owner decision: status stays DIFFERENT; evidence only): the browser build has the DOM integration: web/x_ite_r.mjs (the "
            "MutationObserver of the canvas and its scene elements), x3d-runtime dom.rs, x3d-web session.rs (xml_targets), wasm.rs (xmlChild) and "
            "chain.rs (the Inline load state that DOMIntegration listens to). Tests dom01, dom02, dom03 (oracle/web-browser-dom-01/02/03-baseline) "
            "and dom_mutation.rs. Remaining difference: the first-load Inline event order (ADR 0001, issue #146). The native host has no DOM by design.")

C = {}
for k in NURBS:
    C[k] = (None, {"classification": {
        "class": "DOCUMENTED_DIFFERENCE",
        "iso": {"state": "UNVERIFIED"},  # clause 27 was not read again in this review; the decision names it
        "decision": {"ref": "owner decision 2026-10-17 'ISO everywhere' (REA batch 04)", "reason": "follow ISO/IEC 19775-1 clause 27 where X_ITE 16.5.0 differs"},
        "impact": "equal to X_ITE for unweighted input; differs where the notes name the X_ITE 16.5.0 departures (I1-I8)",
        "reviewed": REVIEWED}}, None)
for k in ANNOTATION_REGISTERED:
    C[k] = (None, {"classification": {
        "class": "DOCUMENTED_DIFFERENCE",
        "iso": ISO_ANNOTATION,
        "decision": {"ref": "ADR 0002; owner decision 2026-10-17 'Do both'", "reason": "X_ITE 16.5.0 ships the Annotation classes but does not register the component"},
        "impact": "COMPONENT Annotation fails in X_ITE 16.5.0; Rust registers the component and gives the node its fields",
        "reviewed": REVIEWED}}, None)
C["node:AnnotationLayer"] = (None, {"classification": {
    "class": "UNVERIFIED",
    "iso": ISO_ANNOTATION,
    "decision": {"ref": "ADR 0002; owner decision 2026-10-17 'Do both'"},
    "upstream_defects": [{"summary": "the X_ITE class cannot initialize: X3DLayerNode reads _hidden, _visible and _children that it lacks",
                          "evidence": "REA batch 04 probe (notes); crates/x3d-viewer/tests/rea_batch04.rs against the stand-in references"}],
    "impact": "Rust gives an empty layer; ISO leaves the presentation of annotations to the browser",
    "reviewed": REVIEWED}}, None)
C["node:IconAnnotation"] = (None, {"classification": {
    "class": "UNVERIFIED",
    "iso": ISO_ANNOTATION,
    "decision": {"ref": "ADR 0002 and its amendment; owner decision 2026-10-17 'Do both'"},
    "upstream_defects": [{"summary": "the X_ITE class cannot initialize: X3DUrlObject reads _load that it lacks",
                          "evidence": "REA batch 04 probe (notes); crates/x3d-viewer/tests/rea_batch04.rs against the stand-in references"}],
    "impact": "Rust loads the url with the candidates of ImageTexture (beyond X_ITE, whose class cannot initialize); no default icon and no display",
    "reviewed": REVIEWED},
    "tests": append("crates/x3d-runtime/tests/icon_annotation_load.rs (5 tests: PNG and JPEG complete, the fallback skips missing and non-image candidates, the first image ends it, an empty url does not start, the candidates are the ones of ImageTexture)")},
    "2026-11-12 (PR #585; evidence only, status unchanged): IconAnnotation loads its url with the candidates of ImageTexture (ADR 0002, amendment "
    "2026-11-12); the stand-in of oracle/synthetic-rea-b04/icon-annotation.wrl is now ImageTexture, no longer GeoMetadata (rea_batch04.rs, MSAA 0 "
    "and 4). Beyond X_ITE, whose class cannot initialize; no default icon and no display. No ISO claim: the local 2023 capture has no Annotation clause.")
C["node:SplinePositionInterpolator"] = (None, {"classification": SPLINE,
    "tests": append("crates/x3d-runtime/tests/spline_normalize_velocity.rs")}, SPLINE_NOTE)
C["node:SplinePositionInterpolator2D"] = (None, {"classification": SPLINE,
    "tests": append("crates/x3d-runtime/tests/spline_normalize_velocity.rs")}, SPLINE_NOTE)
C["node:SpatialSound"] = (None, {"classification": {
    "class": "DOCUMENTED_DIFFERENCE",
    "iso": {"state": "NOT_APPLICABLE"},  # the HRIR data set is no ISO requirement
    "decision": {"ref": "ADR 0005; owner decision 2026-10-17", "reason": "Chromium's IRCAM Composite HRTF data has no stated license and is not redistributed"},
    "impact": "only enableHRTF TRUE: measured maximum difference 0.126 / 0.141 (spatial-hrtf.wrl); the rights of the web-audio-api HRIR data are not verified at the primary source (THIRD_PARTY_NOTICES.md)",
    "reviewed": REVIEWED}}, None)
C["sub:browser.dom-integration"] = (None, {"classification": {
    "class": "DOCUMENTED_DIFFERENCE",
    "iso": {"state": "NOT_APPLICABLE"},
    "decision": {"ref": "ADR 0001; owner decision 2026-11-12 (status stays DIFFERENT)", "reason": "a native host has no DOM"},
    "impact": "native builds have no DOM integration; in the browser the first-load Inline event order differs (ADR 0001, issue #146)",
    "reviewed": REVIEWED},
    "rust": append(*DOM_RUST), "tests": append(*DOM_TESTS)}, DOM_NOTE)

E = set(C)  # the evidence set of this step (verify_ledger.py HISTORY)

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
    from ledger_classification import classification_errors
    bad, snap = [], {i["id"]: i for i in load(SNAP)["items"]}
    cur = {i["id"]: i for i in d["items"]}
    if set(cur) != set(snap) or len(d["items"]) != 346: bad.append("ID set or total changed")
    if [i["id"] for i in d["items"]] != [i["id"] for i in load(SNAP)["items"]]: bad.append("ID order changed")
    bad += [f"{k}: not in the snapshot" for k in sorted(set(C) - set(snap))]
    want = {i["id"]: i for i in build()["items"]}
    for k, b in snap.items():
        a = cur.get(k)
        if a is None: continue
        if k not in C and a != b: bad.append(f"unrelated entry changed: {k}")
        if k in C and b["status"] != "DIFFERENT": bad.append(f"{k}: not a DIFFERENT entry")
        if a["status"] != b["status"]: bad.append(f"{k}: status changed")
        if k in C and a != want[k]: bad.append(f"{k}: differs from what apply writes")
        bad += classification_errors(a)
    c = collections.Counter(i["status"] for i in d["items"]); print(dict(sorted(c.items())))
    if dict(c) != {"COMPLETE": 289, "PARTIAL": 36, "DIFFERENT": 21}: bad.append(f"totals changed: {dict(c)}")
    print("\n".join(bad) or "OK"); return not bad
def sha256(p):
    with open(p, "rb") as f: return hashlib.sha256(f.read()).hexdigest()
if __name__ == "__main__":
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_classify.py check|apply")
    if sha256(SNAP) != SNAP_SHA256: print(f"snapshot sha256 {sha256(SNAP)} is not the expected {SNAP_SHA256}: refused"); sys.exit(1)
    if sys.argv[1:] == ["apply"]:
        d = build()
        try: same = sha256(AUTH) == SNAP_SHA256 or load(AUTH) == d
        except ValueError: same = False
        if not same: print("ledger.json is neither the snapshot nor the result of this change set: refused"); sys.exit(1)
        tmp = AUTH + ".tmp"
        with open(tmp, "w") as f:
            json.dump(d, f, indent=1); f.flush(); os.fsync(f.fileno())
        os.replace(tmp, AUTH); sys.exit(0 if check(load(AUTH)) else 1)
    sys.exit(0 if check(load(AUTH)) else 1)
