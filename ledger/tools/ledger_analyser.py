#!/usr/bin/env python3
"""Ledger step of Phase 10A and 10B (Analyser data fields from the page's AnalyserNode, PR #575; MicrophoneSource through the page's getUserMedia, PR #576) and of the ShaderPart compile-text fixes of issue #95 (PRs #578, #579, #582, #583, #584), 2026-11-12.
python3 -I ledger/tools/ledger_analyser.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-analyser", "ledger.json")
SNAP_SHA256 = "d9cee3d46c4e6b8b425cf75b5ae195d04b55b98b6c268dae4c1d7490b6d8788d"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f

def swap_in_list(old_text, new_text):
    def f(old):
        assert sum(s.count(old_text) for s in old) == 1, old_text
        return [s.replace(old_text, new_text) for s in old]
    return f

def append(*extra):
    return lambda old: list(old) + list(extra)

C = {
 "node:AudioClip": (None, {
   "missing": lambda old: swap("AnalyserNode data is not read from the page's analyser, the microphone and the StreamAudioDestination MediaStream are not mirrored", 'the StreamAudioDestination MediaStream is not mirrored; on pause a page MicrophoneSource is disconnected and its tracks stay enabled (X_ITE disables them)')(old),
   "tests": append('crates/x3d-web/tests/browser/microphone01.test.mjs (4 cases in headless Chromium with its fake capture device: heard through a Sound node, the getUserMedia request with the mediaDeviceID, stopTime ends the tracks and is silent, startTime asks again; silent without a Sound node, and a replaced world stops the old stream; a refused getUserMedia logs its message and the node stays silent and active; a stream that cannot be connected stops its tracks and logs its message)', "crates/x3d-web/tests/browser/analyser01.test.mjs (2 cases in headless Chromium: the byteFrequencyData and floatFrequencyData peak at the tone's bin and the time-domain swing of a sine, read from the page's AnalyserNode; an fftSize change and a new world)"),
  }, "2026-11-12 (Phase 10A, #575): in the browser the four Analyser data fields are read each frame from the page's AnalyserNode (audio_mirror.mjs readAnalysers, WebBrowser::set_analyser_data), as X_ITE's Analyser.set_time (Analyser.js:60-87) reads them; before, the engine's graph had no signal in a page and the fields were all zero (reproduced: a 1500 Hz sine gave the peak at bin 0, not bin 64). One frame after an fftSize change uses the engine's own data (the page's node gets the new size after the frame)." + ' ' + '2026-11-12 (Phase 10B, #576): in the browser a MicrophoneSource asks the page\'s getUserMedia ({ audio: { deviceId } }) as X_ITE\'s MicrophoneSource.set_start does, and the stream plays through the node\'s audio source; its tracks stop with the node. Before, the page\'s engine had no input device and logged "Requested device not found" (reproduced: no getUserMedia request). On pause the node is disconnected and its tracks stay enabled (X_ITE disables them).' + " Stays PARTIAL: the other gaps above remain."),
 "node:Sound": (None, {
   "missing": lambda old: swap("AnalyserNode data is not read from the page's analyser, the microphone and the StreamAudioDestination MediaStream are not mirrored", 'the StreamAudioDestination MediaStream is not mirrored; on pause a page MicrophoneSource is disconnected and its tracks stay enabled (X_ITE disables them)')(old),
   "tests": append('crates/x3d-web/tests/browser/microphone01.test.mjs (4 cases in headless Chromium with its fake capture device: heard through a Sound node, the getUserMedia request with the mediaDeviceID, stopTime ends the tracks and is silent, startTime asks again; silent without a Sound node, and a replaced world stops the old stream; a refused getUserMedia logs its message and the node stays silent and active; a stream that cannot be connected stops its tracks and logs its message)', "crates/x3d-web/tests/browser/analyser01.test.mjs (2 cases in headless Chromium: the byteFrequencyData and floatFrequencyData peak at the tone's bin and the time-domain swing of a sine, read from the page's AnalyserNode; an fftSize change and a new world)"),
  }, "2026-11-12 (Phase 10A, #575): in the browser the four Analyser data fields are read each frame from the page's AnalyserNode (audio_mirror.mjs readAnalysers, WebBrowser::set_analyser_data), as X_ITE's Analyser.set_time (Analyser.js:60-87) reads them; before, the engine's graph had no signal in a page and the fields were all zero (reproduced: a 1500 Hz sine gave the peak at bin 0, not bin 64). One frame after an fftSize change uses the engine's own data (the page's node gets the new size after the frame)." + ' ' + '2026-11-12 (Phase 10B, #576): in the browser a MicrophoneSource asks the page\'s getUserMedia ({ audio: { deviceId } }) as X_ITE\'s MicrophoneSource.set_start does, and the stream plays through the node\'s audio source; its tracks stop with the node. Before, the page\'s engine had no input device and logged "Requested device not found" (reproduced: no getUserMedia request). On pause the node is disconnected and its tracks stay enabled (X_ITE disables them).' + " Stays PARTIAL: the other gaps above remain."),
 "node:MicrophoneSource": (None, {
   "tests": append('crates/x3d-web/tests/browser/microphone01.test.mjs (4 cases in headless Chromium with its fake capture device: heard through a Sound node, the getUserMedia request with the mediaDeviceID, stopTime ends the tracks and is silent, startTime asks again; silent without a Sound node, and a replaced world stops the old stream; a refused getUserMedia logs its message and the node stays silent and active; a stream that cannot be connected stops its tracks and logs its message)', 'crates/x3d-runtime/tests/audio_ops.rs a_page_engine_records_a_microphone_request_and_keeps_its_node'),
  }, '2026-11-12 (Phase 10B, #576): in the browser a MicrophoneSource asks the page\'s getUserMedia ({ audio: { deviceId } }) as X_ITE\'s MicrophoneSource.set_start does, and the stream plays through the node\'s audio source; its tracks stop with the node. Before, the page\'s engine had no input device and logged "Requested device not found" (reproduced: no getUserMedia request). On pause the node is disconnected and its tracks stay enabled (X_ITE disables them).'),
 "node:Analyser": (None, {
   "tests": append("crates/x3d-web/tests/browser/analyser01.test.mjs (2 cases in headless Chromium: the byteFrequencyData and floatFrequencyData peak at the tone's bin and the time-domain swing of a sine, read from the page's AnalyserNode; an fftSize change and a new world)", 'crates/x3d-web/tests/session.rs the_analyser_data_fields_of_a_page_come_from_the_page_reading'),
  }, "2026-11-12 (Phase 10A, #575): in the browser the four Analyser data fields are read each frame from the page's AnalyserNode (audio_mirror.mjs readAnalysers, WebBrowser::set_analyser_data), as X_ITE's Analyser.set_time (Analyser.js:60-87) reads them; before, the engine's graph had no signal in a page and the fields were all zero (reproduced: a 1500 Hz sine gave the peak at bin 0, not bin 64). One frame after an fftSize change uses the engine's own data (the page's node gets the new size after the frame)."),
 "node:ShaderPart": (None, {
   "missing": lambda old: swap("compile error console text of the includes Fragment, Material, PBR, Unlit, Stipple (ANGLE rejects an #if on an undefined macro; the port treats it as 0), Texture, PointSize, Vertex and of a vertex-stage statement error differs from ANGLE (KNOWN table in rea_b24_shaderpart); type", "type")(old),
   "tests": append("crates/x3d-render/tests/glsl_pp_undefined.rs, glsl_frag_outputs.rs, glsl_array_size.rs, glsl_interpolation_qualifiers.rs, glsl_token_lines.rs (the ANGLE compile checks and texts of #578-#584)"),
  }, "Issue #95 2026-11-12 (PRs #578, #579, #582, #583, #584, quick-wins lane): glsl_compat gives ANGLE's compile errors for an undefined identifier in #if / #elif, several fragment outputs without explicit locations, an undeclared array-size identifier (errors in source order), an interpolation qualifier on a fragment output or vertex input, a missing semicolon at a line end and a return of another type. The KNOWN table of rea_b24_shaderpart went from 9 cases to 0: all 32 stepped cases (16 worlds, MSAA 0 and 4) equal pinned X_ITE in the whole console log. Stays PARTIAL: type is initializeOnly (no stepped type case), the pinned mediump picture difference (max 1 per channel, PINNED table), and the 4 glsl_errors_b08 naga type-check gaps."),
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
    bad += [f"{k}: not in the snapshot" for k in sorted(set(C) - set(snap))]
    built = build()
    if {k: v for k, v in d.items() if k != "items"} != {k: v for k, v in built.items() if k != "items"}: bad.append("top-level fields differ from what apply writes")
    want = {i["id"]: i for i in built["items"]}
    for k, b in snap.items():
        a = cur.get(k)
        if a is None: continue  # reported above: ID set or total changed
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_analyser.py check|apply")
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
