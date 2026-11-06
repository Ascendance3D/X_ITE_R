#!/usr/bin/env python3
"""Ledger step for PRs #481, #482, #483 and #484: python3 -I ledger_r481_484.py check|apply

Four text-only changes.
- sub:render.core: PR #481 gives the browser renderer the point sprite snap rule that the page's own WebGL2 rasterizer
  measures (#470).
- sub:nav.examine: PR #482 finds the cause of one of the two touchsnapshot01 drifts (#472) in the test's touch delivery and
  holds the page's frames until a two-finger step has arrived whole; the other drift has no proven cause and the CI
  failure rate is not measured, so #472 stays open.
- sub:browser.options: PR #483 gives a world the page's Antialiased and Multisampling options (#480); the page does not
  yet see a world Script's option change (two todo cases).
- sub:render.transparency-sorting: PR #484 sends the page's OrderIndependentTransparency option to the renderer (#479).

No status changes. No DIFFERENT entry is touched. Counts: 284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING before and
after, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-r481-484", "ledger.json")
SNAP_SHA256 = "181d9f6b605660282f0b9d92069fcac15b074350ffb5f23d9d600ee5275bb946"

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
 "sub:render.core": (None, {
   "tests": addlist("crates/x3d-web/tests/browser/pointtie01.test.mjs (the page's WebGL2 point measurement against the test's own, three measurement faults that keep round-to-even, and with XR_POINT_TIE_REFERENCE=nvidia or apple-m1 the 12 point-tie worlds against the pinned pictures; WebGPU and WebGL2)"),
  }, "Issue #470 2026-11-06: PR #481. In the browser the point sprite snap rule (x3d.wgsl frame.layer.z) followed wgpu::Backend::Metal, which the browser never reports, so on the M1 the browser renderer rounded half 1/256 pixel steps to even where X_ITE's ANGLE Metal rounds them up (430 pixels in the 12 point-tie worlds). The page now measures its own WebGL2 rasterizer once (four 1-pixel points) and gives the rule to the renderer; no OS, browser or vendor name selects it. 17 of 17 cases per backend on the physical M1 and on Linux NVIDIA. Safari not tested; without a reference the picture cases are not tested. Text only: the entry stays PARTIAL."),
 "sub:nav.examine": (None, {
   "missing": swap("open: touchsnapshot01 has an intermittent pinch drift under CPU load (#472, 1 of 20 runs with 24 busy loops, 0 of 20 without)", "open: one touchsnapshot01 pinch drift (#472, line 66, x = -0.0036) has no proven cause, and the CI failure rate is not measured after PR #482"),
  }, "Issue #472 2026-11-06: PR #482 (test only, merged at 17a0d66e). Chromium sometimes delivers a two-point DevTools touch event as two DOM events about 28 ms apart; a frame between the halves draws the half step of the pinch. Sent as two halves with frames between, the last step gives x = -0.003346939 every time (the PR #468 failure, line 88). The test now holds the page's frames until a DOM touch event has every finger at its new position, and waits one frame first for a callback asked for before the hold; the tolerance is unchanged, with no retry or skip. Forced splits with the hold: 0 failures; without it: 4 of 4 at the last step, 3 of 3 at the 150/250 step (x = -0.01423814), 0 of 3 at the 160/240 step. The PR #466 failure (line 66, x = -0.0036) has no proven cause. At 17a0d66e: 60 of 60 loaded runs pass (18 busy loops on 24 vcores), the input shard 80 of 80; the CI failure rate is not measured, so #472 stays open. No engine change. Text only: the entry stays PARTIAL."),
 "sub:browser.options": (None, {
   "tests": addlist("crates/x3d-web/tests/browser/antialiased02.test.mjs (the page's antialiased attribute and a world Script's Multisampling and Antialiased in five orders, pixels and sample count against pinned X_ITE 16.5.0 measured in Chrome; WebGPU and WebGL2)", "crates/x3d-runtime/src/browser_options.rs (script_sampling_reads_the_page_options)"),
  }, "Issue #480 2026-11-06: PR #483. The world's Antialiased option stayed true under the page's antialiased=\"false\", so a world Script's Multisampling 4 drew 4 samples where X_ITE draws none (BrowserOptions.js set_Multisampling__). A world now takes the page's Antialiased and Multisampling options without an event. Not fixed: the page's getBrowserOption and getRenderingProperty do not see a world Script's option change (two todo cases in antialiased02; issue draft waits for the owner). Text only: the status does not change."),
 "sub:render.transparency-sorting": (None, {
   "missing": swap("OIT is not compared", "OIT is not compared in the reference worlds; the page's OIT option is compared with pinned X_ITE on one synthetic world (oit01, PR #484), and a world Script's option in the next world differs from X_ITE (two todo cases)"),
   "tests": addlist("crates/x3d-web/tests/browser/oit01.test.mjs (the page's OrderIndependentTransparency option before a world loads, on the live world, after loadURL and replaceWorld, and in last-writer order with a world Script, and a todo case for a world Script's option in the next world; WebGPU and WebGL2)"),
  }, "Issue #479 2026-11-06: PR #484. The page's OrderIndependentTransparency option did not reach the browser renderer; only a world Script's did. It is now one of the page's engine options. Against pinned X_ITE 16.5.0 in Chrome (400x300, two overlapping transparent quads): 2 pixels over 8 with OIT on and with it off, pinned on against off 6489. Not fixed: a world Script's option does not stay for the next world, as it does in pinned X_ITE (BrowserOptions.reset runs only at start; two todo cases in oit01). Text only: the MSAA 4 reference gaps stay, so the entry stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_r481_484.py check|apply")
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
