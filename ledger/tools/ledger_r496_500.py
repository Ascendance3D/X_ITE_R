#!/usr/bin/env python3
"""Ledger step for PRs #487, #499 and #500: python3 -I ledger_r496_500.py check|apply

Three text-only changes.
- sub:browser.options: PR #499 makes the page and its world one BrowserOptions, as in pinned X_ITE (issue #480): the page
  takes the options a world Script set; the antialiased02 and oit01 todo cases pass on WebGPU and WebGL2.
- node:GaussianSplats: PR #500 tests picking under reversed depth against the pinned standard-depth probes.
- node:TextureCoordinateGenerator: PR #487 (dot in rand1, two re-recorded NVIDIA texgen-params pictures) is merged.

No status changes. No DIFFERENT entry is touched. Counts: 284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING before and
after, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-r496-500", "ledger.json")
SNAP_SHA256 = "4ebd0dabd2d5097c45b3176c34706b8d8750a3b9290392579a27258e5f72fee6"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f

C = {
 "sub:browser.options": (None, {}, "Issue #480 2026-11-07: PR #499 (merged at 3534b792). Pinned X_ITE has one BrowserOptions for the page and its world (X3DCoreContext.js:242, X3DBrowser.js:782-792; REA ev_579d581cde0e, ev_8fb7b33eacd5). A world Script's setBrowserOption is now kept in order (BrowserOptions.set_script_option) and the page takes it (takeScriptOptions, X3DBrowser.#syncWorldOptions) at getBrowserOption, setBrowserOption, getRenderingProperty, loadURL, replaceWorld and after each frame, then runs each option's effect once; the last one set, the page's or a Script's, is in force. Inside a page field callback that runs in the route chain, the page takes them through chainTakeScriptOptions, so the callback reads a Script's option at once, and the effects run at the end of the chain. The antialiased02 B and C and the oit01 next-world cases are no longer todo, and a new oit01 case covers the route-chain read (red 2 of 2, green 2 of 2): before the change 6 of 24 failed on both backends, after it antialiased01, antialiased02, oit01, options01 and reentry01 pass 72 of 72 on WebGPU and WebGL2. Text only: the status does not change."),
 "node:GaussianSplats": (None, {
   "missing": swap(", and picking under reversed depth is not tested (the pointing pass ignores the depth mode for splats)", "; picking under reversed depth equals the pinned standard-depth probes (PR #500; the pointing pass ignores the depth mode for splats)"),
  }, "Issue #69 2026-11-07: PR #500 (test only, merged at 77da741a). b69_gs_pick runs the 2646 probes with DepthMode::Reversed and compares them with the pinned standard-depth recording (oracle/b69-gs-pick-baseline) at the same limits: pass on the RTX 3060 Ti. Equality with the standard probes, not a comparison of reversed depth with X_ITE. Text only: the other gaps of this entry stay, so it stays PARTIAL."),
 "node:TextureCoordinateGenerator": (None, {}, "Issue #469 2026-11-07: PR #487 merged at e0e053ec (owner accepted the two re-recorded NVIDIA texgen-params pictures, MSAA 0 and 4, pinned X_ITE on ANGLE Vulkan). x3d.wgsl rand1 uses dot, which equals pinned X_ITE on the same driver path on the 6 measured paths. The texgen-modes 404 px stays, cause unknown. Text only: the entry stays COMPLETE."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_r496_500.py check|apply")
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
