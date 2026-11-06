#!/usr/bin/env python3
"""Summary table of oracle/x_ite-splat-sort-timing.mjs output: python3 -I -B splat_timing_summary.py timing.json"""
import json, sys, statistics as st

d = json.load(open(sys.argv[1]))
i = d["info"]
r0 = next(iter(d["results"].values()))["runs"][0]["env"]
print(f'{i["os"]} {i["cpuModel"]} | chrome {i["chrome"]} | angle {i["angle"]} | {r0["renderer"]}')
print("count | steps*runs | request ms (set->post) med [min-max] | answer ms (post->recv) med [min-max] | stale draws (min/med/max) | draw frame ms med | order same")
for n, res in d["results"].items():
    S = [s for r in res["runs"] for s in r["steps"]]
    f = lambda k: [s[k] for s in S if s[k] is not None]
    def rng(v): return f"{st.median(v):.1f} [{min(v):.1f}-{max(v):.1f}]"
    sd = f("staleDraws")
    fm = [s["drawFrameMs"]["median"] for s in S if s["drawFrameMs"]["median"] is not None]
    print(f'{n} | {len(S)} | {rng(f("requestMs"))} | {rng(f("answerMs"))} | {min(sd)}/{st.median(sd)}/{max(sd)} | {st.median(fm):.1f} | {res["orderIdentical"]} | unanswered {sum(1 for s in S if not s["gotAnswer"])}')
