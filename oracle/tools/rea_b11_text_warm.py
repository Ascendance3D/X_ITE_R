#!/usr/bin/env python3
"""REA batch 11 lane b11-2-text: how repeatable is X_ITE's Text during the oracle's warm-up frames? (documented open, never baselined)

  python3 -I oracle/tools/rea_b11_text_warm.py WORLD N [--load K]
      Runs the pinned X_ITE N times on WORLD (oracle/synthetic-rea-b11-text/warm-stamp*.wrl: a Script stamps every origin / textBounds /
      lineBounds event of a Text with the Script's frame count) and prints, per run, `loadFrames` (frames the oracle traversed in real time
      while the world loaded) and the frames the events arrived in, then the distinct patterns. --load K runs K busy loops meanwhile.
The browser clock is not touched. Nothing is written into the repository.
"""
import json, os, re, subprocess, sys, tempfile, collections

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
world, n = sys.argv[1], int(sys.argv[2])
load = int(sys.argv[sys.argv.index("--load") + 1]) if "--load" in sys.argv else 0
script = os.path.splitext(world)[0] + ".json"
busy = [subprocess.Popen(["sh", "-c", "while :; do :; done"]) for _ in range(load)]
patterns = collections.Counter()
try:
    for i in range(1, n + 1):
        out = os.path.join(tempfile.mkdtemp(prefix="rea-b11-warm-"), "js.json")
        subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world, script, "--out", out], check=True, cwd=root, timeout=120,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        d = json.load(open(out))
        text = d["vrml"][sorted(d["vrml"], key=int)[-1]]
        ev = [e for e in re.findall(r'"(f\d+ [^"]*)"', text) if not e.startswith("f0")]
        sig = f"loadFrames={d['loadFrames']}: " + " ".join(f"{e.split()[0]}:{e.split()[1].split('=')[0]}" for e in ev)
        patterns[sig] += 1
        print(i, sig)
finally:
    for b in busy:
        b.kill()
print(len(patterns), "distinct patterns")
for sig, c in patterns.most_common():
    print(c, sig)
