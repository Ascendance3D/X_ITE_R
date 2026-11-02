#!/usr/bin/env python3
"""Ledger step after #324 (ADR 0012, issue #68): python3 -I ledger_i324.py check|apply

Seven entries. Four are promoted to COMPLETE (node:Text, node:VolumeScatterMaterialExtension, sub:parse.html-support, node:MicrophoneSource): their only listed gap was the pending decision. Three stay PARTIAL with the decided difference in the text (node:BlendedVolumeStyle, node:ShadedVolumeStyle, node:InlineGeometry)."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i324", "ledger.json")
SNAP_SHA256 = "84aab72f2c043518e8067a953cd91cbe5b3c2aae20c6f1fead5b6697f45a143f"

def setm(new):
    return lambda old: new
ADR = "ADR 0012 (Accepted 2026-11-02, PR #324)"
C = {
 "node:Text": ("COMPLETE", {"missing": setm("")}, "Issue #68 2026-11-02 (" + ADR + "): origin, textBounds and lineBounds in the oracle's warm-up frames are not compared, because pinned X_ITE itself is not repeatable there (one or two default-font rebuilds depending on load); an engineering decision from the recordings, no ISO clause is involved. Promoted PARTIAL to COMPLETE: no other gap was listed."),
 "node:VolumeScatterMaterialExtension": ("COMPLETE", {"missing": setm("")}, "Issue #68 2026-11-02 (" + ADR + "): the contract item 5 reading for attached nodes is the clarified reading already used for the eight COMPLETE volume styles (attached-node removal and Script-created events); the contract's own wording is not in the repository. Promoted PARTIAL to COMPLETE: no other gap was listed."),
 "sub:parse.html-support": ("COMPLETE", {"missing": setm("")}, "Issue #68 2026-11-02 (" + ADR + "): judged on the parser contract (name and field maps equal X_ITE on 53 HTML inputs and 12 XML twins, 325 checks). The part that needs a DOM (live attributes, importDocument) is not part of this entry: ADR 0006 and issue #146; sub:browser.dom-integration stays DIFFERENT. Promoted PARTIAL to COMPLETE: no other gap was listed."),
 "node:MicrophoneSource": ("COMPLETE", {"missing": setm("")}, "Issue #68 2026-11-02 (" + ADR + "): the oracle host has no input device, so X_ITE raises no events there and neither does Rust (synthetic-rea-b05/microphone.wrl, offline co-simulation, tolerance 0). Live capture from a device (getUserMedia) is NOT compared and cannot be on this host; the decision accepts that. ISO/IEC 19775-1 V4.0 16.4.14 defines live input and gives no result for a host without an input device. Promoted PARTIAL to COMPLETE on that decision only."),
 "node:BlendedVolumeStyle": (None, {"missing": setm("weightFunction ALPHA1 and ONE_MINUS_ALPHA1 hang pinned X_ITE (it writes blendColor_ <id> .a, invalid GLSL), so no X_ITE baseline exists; X_ITE also uses ALPHA0 and ALPHA1 where ISO/IEC 19775-1 V4.0 41.4.1 Table 41.3 has ALPHA1 and ALPHA2 and draws ALPHA2 as CONSTANT. Rust follows the standard for ALPHA1, ALPHA2 and their ONE_MINUS forms (render_b68_owner::blended_alpha_weight_functions_follow_the_standard) and keeps ALPHA0 because X_ITE draws it; a difference from X_ITE by the standard, decided in ADR 0012")}, "Issue #68 2026-11-02 (" + ADR + "): ALPHA1 and ALPHA2 follow ISO/IEC 19775-1 V4.0 41.4.1 Table 41.3, pinned by a regression test; worlds and X_ITE recordings in oracle/synthetic-rea-b68-owner (5 identical runs). Status stays PARTIAL: no X_ITE baseline exists for the weight functions in question."),
 "node:ShadedVolumeStyle": (None, {"missing": setm("A non-Material X3DMaterialNode (PhysicalMaterial) as material: pinned X_ITE throws and never draws (5 of 5 runs); Rust draws the volume and logs the node as unsupported; a decided difference (ADR 0012), pinned by render_b68_owner")}, "Issue #68 2026-11-02 (" + ADR + "): the difference is decided and tested (worlds sh-material and sh-physicalmaterial, X_ITE recordings 5 identical runs). Status stays PARTIAL."),
 "node:InlineGeometry": (None, {"missing": setm("X_ITE (Cache on) gives a second InlineGeometry with the same url a copy() without routes ('TODO: add routes' in InlineGeometry.js); Rust keeps the routes; a decided difference (ADR 0012) that is untested: no second-instance recording exists; ig-select reload picture check disabled")}, "Issue #68 2026-11-02 (" + ADR + "): decided as an approved difference. ISO/IEC 19775-1 V4.0 has no InlineGeometry node (it is an X_ITE node), so no clause decides it. Status stays PARTIAL because the difference has no test."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i324.py check|apply")
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
