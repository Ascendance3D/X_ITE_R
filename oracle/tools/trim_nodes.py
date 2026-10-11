#!/usr/bin/env python3
"""Trim oracle/x_ite-nodes.json (dumped from X_ITE by oracle/x_ite-nodes.mjs) into the compact
table the x3d-scene crate embeds: crates/x3d-scene/data/x_ite-nodes.json.

Per node and per spec variant ("vrml2" = a #VRML V2.0 scene, "x3d" = a #X3D V4.0 scene):
  fields:  [accessType, name, type, definitionDefault]   (what isDefaultValue compares to)
  initial: {name: value} only where a constructor changed the starting value
  aliases: {alias: fieldName}
"""
import json, sys

src, dst = sys.argv[1], sys.argv[2]
d = json.load(open(src))

def variant(v):
    fields = [[f["accessType"], f["name"], f["type"], f["value"]] for f in v["fields"]]
    defaults = {f["name"]: f["value"] for f in v["fields"]}
    initial = {name: value for name, _type, value in v["instance"] if defaults.get(name) != value}
    return {"udf": v["canUserDefinedFields"], "fields": fields, "initial": initial, "aliases": v["aliases"]}

out = {
    "pinned": d["pinned"],
    "browserName": d["browserName"],
    "browserVersion": d["browserVersion"],
    "vrml2Available": d["vrml2Available"],
    "components": d["components"],
    "profiles": d["profiles"],
    "nodes": [
        {
            "typeName": n["typeName"],
            "component": n["component"],
            "containerField": n["containerField"],
            "from": n["from"],
            "to": n["to"],
            "vrml2": variant(n["vrml2"]),
            "x3d": variant(n["x3d"]),
        }
        for n in d["nodes"]
    ],
}

json.dump(out, open(dst, "w"), separators=(",", ":"))
print(f"{len(out['nodes'])} nodes -> {dst}")
