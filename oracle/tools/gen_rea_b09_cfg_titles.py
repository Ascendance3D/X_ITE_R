#!/usr/bin/env python3
"""Add the `title` of every supported component and profile (Configuration/SupportedComponents.js, SupportedProfiles.js of the pin) to
crates/x3d-scene/data/x_ite-nodes.json and x_ite-nodes-annotation.json (REA batch 09, step 3, item 4). Idempotent. The level / external /
dependencies already in the data are checked against the pinned source, and so are the profile components. `Annotation` is the component
oracle/x_ite-pin/src/anim.html registers (title "Annotation", level 2).

  python3 -I oracle/tools/gen_rea_b09_cfg_titles.py
"""
import json, os, re, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
conf = os.path.join(root, "oracle", "x_ite-pin", "src", "x_ite", "Configuration")
data = os.path.join(root, "crates", "x3d-scene", "data")

components = open(os.path.join(conf, "SupportedComponents.js")).read()
profiles = open(os.path.join(conf, "SupportedProfiles.js")).read()

ctitles = {}

for m in re.finditer(r'SupportedComponents \.add \("([^"]+)",\s*\{(.*?)\n\}\);', components, re.S):
    body = m.group(2)
    title = re.search(r'title: "([^"]*)"', body).group(1)
    level = int(re.search(r'level: (\d+)', body).group(1))
    external = "external: true" in body
    deps = re.findall(r'"([^"]+)"', (re.search(r'dependencies: \[(.*?)\]', body) or [None, ""])[1])
    ctitles[m.group(1)] = (title, level, external, deps)

ptitles = {}

for m in re.finditer(r'SupportedProfiles \.add \("([^"]+)",\s*\{\s*title: "([^"]*)",\s*components: \[(.*?)\],\s*\}\);', profiles, re.S):
    comps = [(a, int(b)) for a, b in re.findall(r'getComponent \("([^"]+)", (\d+)\)', re.sub(r'//.*', '', m.group(3)))]
    ptitles[m.group(1)] = (m.group(2), comps)

for f in ("x_ite-nodes.json", "x_ite-nodes-annotation.json"):
    p = os.path.join(data, f)
    d = json.load(open(p))

    for c in d["components"]:
        name = c["name"]

        if name == "Annotation":
            title = "Annotation"
        else:
            title, level, external, deps = ctitles[name]
            assert (c["level"], c["external"], c["dependencies"]) == (level, external, deps), name

        new = {"name": name, "title": title}
        new.update({k: v for k, v in c.items() if k not in ("name", "title")})
        c.clear()
        c.update(new)

    for pr in d.get("profiles", []):
        title, comps = ptitles[pr["name"]]
        assert [(x["name"], x["level"]) for x in pr["components"]] == comps, pr["name"]
        new = {"name": pr["name"], "title": title}
        new.update({k: v for k, v in pr.items() if k not in ("name", "title")})
        pr.clear()
        pr.update(new)

    open(p, "w").write(json.dumps(d, separators=(",", ":"), ensure_ascii=False))

print(len(ctitles), "components,", len(ptitles), "profiles")
