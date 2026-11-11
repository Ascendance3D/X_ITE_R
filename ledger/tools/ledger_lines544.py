#!/usr/bin/env python3
"""Ledger step of PR #544 (2026-11-10): ExplosionEmitter with LINE, POINT and QUAD particles compared with pinned X_ITE; the
observed 0.738% LINE gap is 1 px line rasterization, not the emitter.
python3 -I ledger/tools/ledger_lines544.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-lines544", "ledger.json")
SNAP_SHA256 = "a078608a78c1bfbd5c56f28e54644a8354976c6f4e02487291ee74c7dda198db"

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
 "node:ExplosionEmitter": (None, {
   "tests": append("crates/x3d-viewer/tests/rea_b18_particle_lines.rs ex-line, ex-line-plain, ex-point, ex-quad, cone-line (oracle/rea-b18-particle-lines-baseline, three identical recordings; PR #544)"),
  }, "PR #544 (2026-11-10): a draft multi-viewer world (ExplosionEmitter, LINE particles, 96 x 72) differed from pinned X_ITE by 0.738% at MSAA 0. Five 512 x 512 cases isolate it: every particle record equals X_ITE within 1e-5 at frames 5, 20 and 40, POINT and QUAD are pixel-identical at MSAA 0 and 4, LINE is identical at MSAA 4 (1 pixel in ex-line-plain at frame 40); at MSAA 0 isolated line-end pixels differ (ex-line 49, 103, 187; the ConeEmitter control 3, 9, 20; the existing particle-free inst-line-dashed 119), so the cause is 1 px line rasterization (wgpu LineList against ANGLE GL lines), not the emitter. All cases are under 0.08% of the image and pass the unchanged 0.5% bound; the measurement is recorded in the ADR 0011 addendum. No Rust change. Status stays COMPLETE."),
 "sub:parse.gltf2": (None, {
   "missing": swap("gltf_anim_step, 32 cases, MSAA 0 and 4, PR #538", "gltf_anim_step, 36 cases: the 30 of oracle/rea-gltf-anim-baseline and the 6 driver lines of oracle/rea-b08-gltf-baseline, MSAA 0 and 4, PR #538"),
  }, "PR #544 (2026-11-10): correction of the PR #538 text: gltf_anim_step checks 36 cases, not 32 (review of #538). In a known STEP-weights case the Inline text must equal the pinned text apart from its numbers (PR #538 review fix). Status stays PARTIAL."),
 "sub:browser.multi-viewer-thread": (None, {
   "missing": swap("observed but not tracked here: an ExplosionEmitter with LINE particles differs from pinned X_ITE by 0.738% at MSAA 0 with one viewer (node:ParticleSystem)", "the ExplosionEmitter LINE draft's 0.738% is 1 px line rasterization in a small image, not a multi-viewer or emitter gap (node:ExplosionEmitter, PR #544)"),
  }, "PR #544 (2026-11-10): the ExplosionEmitter observation of PR #542 is resolved as 1 px line rasterization (node:ExplosionEmitter). Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_lines544.py check|apply")
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
