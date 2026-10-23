import sys, re
root = sys.argv[1]; out = sys.argv[2]
files = {"R06": "x_ite_particles.mjs", "EV": "x_ite_particles_events.mjs", "B07": "x_ite_particles_b07.mjs"}
def fn_between(src, start, end):
    i = src.index(start); j = src.index(end, i)
    return src[i:j]
consts = []
for tag, f in files.items():
    src = open(f"{root}/oracle/tools/{f}").read()
    init = fn_between(src, "([ s, date0 ]) =>\n{", "}, [ seed, t0 * 1000 ]);") + "}"
    norm = fn_between(src, "([name, s, t0]) =>\n   {", "}, [ nodeName, seed, t0 ]);") + "}"
    step_start = "([tt, name]) =>\n      {" if tag == "R06" else "([tt, name, events]) =>\n      {"
    step_end = "}, [ t, nodeName ]);" if tag == "R06" else "}, [ t, nodeName, (script .events ?? []) .filter (e => e .frame === k) ]);"
    step = fn_between(src, step_start, step_end) + "}"
    dump = fn_between(src, "born =>\n         {", "}, script .filter === \"born\");") + "}"
    geom = fn_between(src, "() =>\n   {\n      const ps = window .__ps, en = ps .emitterNode, arr", "});\n") + "}"
    info = fn_between(src, "() =>\n   {\n      const ps = window .__ps;\n      return { geometryType", "});\n") + "}"
    for name, s in [("INIT", init), ("NORMALIZE", norm), ("STEP", step), ("DUMP", dump), ("GEOMETRY", geom), ("INFO", info)]:
        assert src.count(s) == 1, (tag, name)
        consts.append((f"{name}_{tag}", s))
    for s in ["() => window .getLog ?.() ?? [ ]", "() => window .__calls", "() => window .ready",
              "([u, t, d, w]) => window .load (u, t, d, w)", "d => window .setDt (d)"]:
        assert s in src, (tag, s)
consts += [("GET_LOG_OPT", "() => window .getLog ?.() ?? [ ]"), ("CALLS", "() => window .__calls")]
lines = ["//! Page-side JavaScript of oracle/tools/x_ite_particles.mjs (R06), x_ite_particles_events.mjs (EV) and",
         "//! x_ite_particles_b07.mjs (B07), byte for byte. Generated from those files; the unit test",
         "//! `particles_js_is_verbatim` checks every constant still occurs in its source.", ""]
for n, s in consts:
    assert '"###' not in s
    lines.append(f'pub const {n}: &str = r###"{s}"###;\n')
open(out, "w").write("\n".join(lines))
print(len(consts))
