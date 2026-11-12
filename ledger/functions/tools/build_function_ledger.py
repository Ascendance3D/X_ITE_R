#!/usr/bin/env python3
"""The function-level ledger: every function of pinned X_ITE 16.5.0 (34cdf9f97) and the Rust symbol that holds it.

  node ledger/functions/tools/extract_functions.mjs <acorn dir> oracle/x_ite-pin/src > functions.jsonl
  python3 -I -B ledger/functions/tools/build_function_ledger.py functions.jsonl [--check]

It reads the extractor's list, the Rust sources of crates/*/src and crates/*/tests, and ledger/ledger.json (read only), and
writes the six files of ledger/functions/. `--check` writes nothing and fails when a file would change. The output depends only
on those inputs: no date, no machine path.

What a record says, and what it does not say:

- `mapping` is about code presence only.
    cited      a Rust source line names the X_ITE function (`Owner.name`, `Owner .prototype .name`, `Owner.js` with the name);
               `rust` is the Rust function at that line.
    name       no citation; a Rust function of the same name (snake case) is in a file or `impl` of the same class, or in a
               Rust file that the 346-entry ledger lists for the X_ITE file. A candidate, not a verified mapping.
    class      a constructor (the function of the class itself) with no citation; the 346-entry ledger lists the Rust files of
               its class, and `rust` is those files. The class is present; the constructor's own statements are not mapped.
    none       no Rust symbol found by these rules. This is not proof that the behavior is absent: a Rust function can
               hold it under another name without a citation.
- `tests` is about tested behavior, separate from presence.
    direct     a Rust test file or test module names the X_ITE function.
    entry      no direct test; the 346-entry ledger entry of the X_ITE file is COMPLETE and lists tests (the entry as a whole).
    none       neither.
- `ledger` links the record to the 346-entry ledger by the entry's `upstream` file. `approved_difference` is true when a linked
  entry is DIFFERENT; `open_gap` is true when a linked entry is PARTIAL or MISSING.

A function without a name of its own (a callback, an arrow function) is `inline`: it is part of the function around it and has
no mapping of its own."""
import collections, csv, hashlib, io, json, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT = os.path.join(ROOT, "ledger", "functions")
PIN = "X_ITE 16.5.0 @ 34cdf9f97"
REA = [
    "ev_4b19e25b68ef315c508675fd4b3f73b3b165891f5c683cafe92b06a6e1e87313",
    "ev_304fc670a11afcb98229572336aae2872b97e3f40960f15a63a9d2945bd1873a",
]

def scope_of(file):
    """(scope, applicable): which part of the pinned tree a file is, and whether its functions are X_ITE's own engine code."""
    if file.startswith("x_ite/") and re.search(r"\.(fs|vs)\.js$", file):
        return "shader-source", False
    if file.startswith("x_ite/") or file.startswith("standard/") or file == "lib/helper.js":
        return "engine", True
    if file.startswith("assets/shaders/"):
        return "shader-source", False
    if file.startswith("assets/components/"):
        return "component-module", True
    if file.startswith("lib/") or file.startswith("assets/lib/"):
        return "third-party", False
    return "site-or-harness", False

def snake(name):
    s = re.sub(r"__$", "", name)
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", s)
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", s)
    return s.lower()

def names_for(name):
    """The Rust function names that can hold the X_ITE function `name`."""
    s = snake(name)
    out = [s]
    if s.startswith("get_") and len(s) > 4:
        out.append(s[4:])
    if s.startswith("set_") and name.endswith("__") and len(s) > 4:
        out.append(s[4:])
    return out

def stems_for(owner):
    out = {snake(owner)}
    if owner.startswith("X3D") and len(owner) > 3:
        out.add(snake(owner[3:]))
    if owner.endswith("Node") and len(owner) > 4:
        out.add(snake(owner[:-4]))
    return out

FN = re.compile(r"^(\s*)(?:pub(?:\([^)]*\))?\s+)?(?:default\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+\"[^\"]*\"\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)")
IMPL = re.compile(r"^(\s*)(?:unsafe\s+)?impl(?:\s*<[^>]*>)?\s+(?:[\w:]+(?:<[^>]*>)?\s+for\s+)?([\w:]+)")
MOD_TEST = re.compile(r"^(\s*)(?:pub\s+)?mod\s+\w*tests?\w*\s*\{")
CITE = re.compile(r"\b([A-Z][A-Za-z0-9_]*)(?:\.js)?\s?(?:\.|::|#)\s?(?:prototype\s?\.\s?)?([A-Za-z_$][A-Za-z0-9_$]*)")
CITE_FILE = re.compile(r"\b([A-Z][A-Za-z0-9_]*)\.js\b[`'\"]?[ ,:(]+[`'\"]?([A-Za-z_$][A-Za-z0-9_$]*)")
WORD = re.compile(r"[A-Za-z_$][A-Za-z0-9_$]*")

def rust_files():
    for crate in sorted(os.listdir(os.path.join(ROOT, "crates"))):
        for part in ("src", "tests", "benches", "examples"):
            top = os.path.join(ROOT, "crates", crate, part)
            for d, dirs, files in os.walk(top):
                dirs.sort()
                for f in sorted(files):
                    if f.endswith(".rs"):
                        yield crate, part != "src", os.path.relpath(os.path.join(d, f), ROOT).replace(os.sep, "/")

def index_rust(methods_of):
    """(functions by name, citations (owner, name) -> symbols, test citations (owner, name) -> files, type and stem index)."""
    by_name = collections.defaultdict(list)
    cited = collections.defaultdict(list)
    tested = collections.defaultdict(set)
    count = 0
    for crate, test_file, path in rust_files():
        lines = open(os.path.join(ROOT, path), encoding="utf-8", errors="replace").read().split("\n")
        stem = os.path.basename(path)[:-3]
        if stem == "mod":
            stem = os.path.basename(os.path.dirname(path))
        # Pass 1: the functions, with the `impl` type around each and whether a test module holds it.
        fns, impls, test_mod = [], [], None
        for n, line in enumerate(lines, 1):
            m = MOD_TEST.match(line)
            if m and test_mod is None:
                test_mod = len(m.group(1))
            elif test_mod is not None and line.startswith(" " * test_mod + "}") and len(line) - len(line.lstrip()) == test_mod:
                test_mod = None
            m = IMPL.match(line)
            if m:
                indent = len(m.group(1))
                impls = [i for i in impls if i[0] < indent] + [(indent, m.group(2).split("::")[-1])]
            elif line.strip() == "}" :
                indent = len(line) - len(line.lstrip())
                impls = [i for i in impls if i[0] < indent]
            m = FN.match(line)
            if m:
                indent = len(m.group(1))
                owner = next((t for i, t in reversed(impls) if i < indent), None)
                symbol = f"{owner}::{m.group(2)}" if owner else m.group(2)
                fn = {"crate": crate, "file": path, "line": n, "symbol": symbol, "name": m.group(2), "impl": owner, "stem": stem,
                      "test": test_file or test_mod is not None}
                fns.append(fn)
                if not fn["test"]:
                    by_name[m.group(2)].append(fn)
                    count += 1
        # Pass 2: the citations of X_ITE functions, each given to the Rust function at that line.
        starts = [f["line"] for f in fns]
        def at(n, doc):
            import bisect
            i = bisect.bisect_right(starts, n)
            if doc:
                return fns[i] if i < len(fns) else None      # a doc comment belongs to the next function
            return fns[i - 1] if i else None
        owner_now = None
        for n, line in enumerate(lines, 1):
            text = line.strip()
            comment = text.startswith("//")
            if not comment:
                owner_now = None
                if "//" not in line and '"' not in line:
                    continue
            found = []
            for rx in (CITE, CITE_FILE):
                for m in rx.finditer(line):
                    owner, name = m.group(1), m.group(2)
                    if name in methods_of.get(owner, ()):
                        found.append((owner, name))
                        owner_now = owner
            # `Owner.first, second, third`: more functions of the owner named on the comment line.
            if comment and owner_now:
                for w in WORD.findall(text):
                    if len(w) >= 6 and (re.search(r"[A-Z]", w[1:]) or w.endswith("__")) and w in methods_of.get(owner_now, ()):
                        found.append((owner_now, w))
            if not found:
                continue
            module_doc = text.startswith("//!")
            fn = None if module_doc else at(n, text.startswith("///"))
            for key in dict.fromkeys(found):
                if test_file or (fn and fn["test"]):
                    tested[key].add(path)
                elif fn:
                    if fn not in cited[key]:
                        cited[key].append(fn)
                else:
                    ref = {"crate": crate, "file": path, "line": n, "symbol": "(module documentation)", "name": None, "impl": None,
                           "stem": stem, "test": False}
                    if not any(c["file"] == path and c["name"] is None for c in cited[key]):
                        cited[key].append(ref)
    return by_name, cited, tested, count

def load_ledger():
    items = json.load(open(os.path.join(ROOT, "ledger", "ledger.json")))["items"]
    by_file = collections.defaultdict(list)
    for i in items:
        for path in re.findall(r"src/([\w./-]+\.js)", str(i.get("upstream", ""))):
            by_file[path].append(i)
    return items, by_file

def ref(fn):
    return {"crate": fn["crate"], "file": fn["file"], "line": fn["line"], "symbol": fn["symbol"]}

def build(extract):
    files, functions = [], []
    for line in open(extract):
        r = json.loads(line)
        (files if "bytes" in r else functions).append(r)
    # The X_ITE functions of each owner. A function without an owner belongs to its file.
    methods_of = collections.defaultdict(set)
    for f in functions:
        base = os.path.basename(f["file"])[:-3]
        f["owner_shown"] = (f["owner"] or base).split(".")[-1] if f["owner"] not in (None, "this") else base
        if f["name"]:
            methods_of[f["owner_shown"]].add(f["name"])
            methods_of[base].add(f["name"])
    by_name, cited, tested, rust_count = index_rust(methods_of)
    items, ledger_by_file = load_ledger()
    rust_listed = {}
    records = []
    for f in functions:
        scope, applicable = scope_of(f["file"])
        base = os.path.basename(f["file"])[:-3]
        owner = f["owner_shown"]
        inline = f["kind"].startswith("anonymous")
        entries = ledger_by_file.get(f["file"], [])
        rec = {
            "id": f"{f['file']}#{owner}.{f['name'] or 'anonymous'}@{f['line']}",
            "upstream": {"file": "src/" + f["file"], "line": f["line"], "end_line": f["endLine"], "owner": owner, "name": f["name"],
                         "kind": f["kind"], "static": f["static"], "params": f["params"], "enclosing": f["enclosing"],
                         "sha256": f["sha256"]},
            "scope": scope,
            "applicable": applicable and not inline,
            "inline": inline,
            "mapping": "none", "rust": [],
            "tests": "none", "test_files": [],
            "ledger": [{"id": e["id"], "status": e["status"]} for e in entries],
            "approved_difference": any(e["status"] == "DIFFERENT" for e in entries),
            "open_gap": any(e["status"] in ("PARTIAL", "MISSING", "NOT_TESTED") for e in entries),
        }
        if f["name"] and not inline:
            keys = [(owner, f["name"])] + ([(base, f["name"])] if base != owner else [])
            hits = [c for k in keys for c in cited.get(k, [])]
            if hits:
                rec["mapping"] = "cited"
                rec["rust"] = [ref(c) for c in hits[:6]]
            else:
                stems = stems_for(owner) | stems_for(base)
                listed = set()
                for e in entries:
                    listed |= set(re.findall(r"crates/[\w./-]+\.rs", str(e.get("rust", ""))))
                cands = []
                for n in names_for(f["name"]):
                    for fn in by_name.get(n, []):
                        if fn["stem"] in stems or fn["impl"] in (owner, base) or fn["file"] in listed:
                            cands.append(fn)
                if cands:
                    rec["mapping"] = "name"
                    rec["rust"] = [ref(c) for c in cands[:6]]
                elif f["name"] == base and f["kind"] == "constructor-or-function" and listed:
                    rec["mapping"] = "class"
                    rec["rust"] = [{"crate": p.split("/")[1], "file": p, "line": None, "symbol": "(class files of the 346-entry ledger)"}
                                   for p in sorted(listed)[:6]]
            direct = sorted({p for k in keys for p in tested.get(k, ())})
            if direct:
                rec["tests"], rec["test_files"] = "direct", direct[:8]
            elif any(e["status"] == "COMPLETE" and e.get("tests") for e in entries):
                rec["tests"] = "entry"
        records.append(rec)
    return files, records, items, rust_count

def table(rows, head):
    out = ["| " + " | ".join(head) + " |", "| " + " | ".join("---" for _ in head) + " |"]
    out += ["| " + " | ".join(str(c) for c in r) + " |" for r in rows]
    return "\n".join(out)

def area(file):
    p = file.split("/")
    if p[0] == "x_ite" and len(p) > 2 and p[1] in ("Components", "Browser"):
        return "/".join(p[:3])
    return "/".join(p[:2]) if len(p) > 2 else p[0]

# The owner's order of work (directive of 2026-11-11, section 5), by the area of the X_ITE source.
PRIORITY = [
    ("2 rendering and graphics", r"^x_ite/(Rendering|Browser/(Rendering|Shaders|Shape|Texturing|Lighting|EnvironmentalEffects|PointingDeviceSensor|Picking|VolumeRendering|GaussianSplats|CubeMapTexturing|Texturing3D|TextureProjection|Shape)|Components/(Rendering|Shape|Shaders|Texturing|Texturing3D|CubeMapTexturing|Lighting|EnvironmentalEffects|Geometry2D|Geometry3D|NURBS|VolumeRendering|TextureProjection|ParticleSystems|X_ITE|CADGeometry|HAnim|Followers|Interpolation|Grouping|Layering|Layout|Geospatial|RigidBodyPhysics))"),
    ("3 navigation and collision", r"^x_ite/(Browser/(Navigation|Followers|Layering|Layout)|Components/(Navigation|EnvironmentalSensor|PointingDeviceSensor|KeyDeviceSensor|Picking))"),
    ("4 browser and WASM host", r"^x_ite/(Browser/(Core|X3DBrowser|X3DBrowserContext|DOMIntegration|Networking|Time|WebXR|XRInput|Legacy)|X3D|X3DCanvasElement|Configuration|Features|Namespace)"),
    ("6 scene loading and scripting", r"^x_ite/(Parser|InputOutput|Execution|Prototype|Routing|Base|Fields|Browser/Scripting|Components/(Core|Networking|Scripting|EventUtilities|Time|DIS|Annotation))"),
    ("7 text, media and audio", r"^x_ite/(Browser/(Text|Sound)|Components/(Text|Sound))"),
]

def priority(file):
    for name, rx in PRIORITY:
        if re.search(rx, file):
            return name
    if file.startswith("standard/") or file == "lib/helper.js":
        return "6 scene loading and scripting" if "Utility" in file else "2 rendering and graphics"
    return "8 other"

def render(files, records, items, rust_count):
    out = {}
    app = [r for r in records if r["applicable"]]
    total = len(records)
    c_scope = collections.Counter(r["scope"] for r in records)
    c_map = collections.Counter(r["mapping"] for r in app)
    c_test = collections.Counter(r["tests"] for r in app)
    statuses = collections.Counter(i["status"] for i in items)
    meta = {
        "upstream": PIN, "source": "oracle/x_ite-pin/src", "files": len(files),
        "files_with_parse_error": sum(1 for f in files if "parseError" in f),
        "source_sha256": hashlib.sha256("".join(f["file"] + f["sha256"] for f in files).encode()).hexdigest(),
        "functions": total, "inline_functions": sum(1 for r in records if r["inline"]),
        "applicable_functions": len(app), "rust_functions_indexed": rust_count,
        "by_scope": dict(sorted(c_scope.items())), "mapping_of_applicable": dict(sorted(c_map.items())),
        "tests_of_applicable": dict(sorted(c_test.items())),
        "applicable_with_approved_difference": sum(1 for r in app if r["approved_difference"]),
        "applicable_with_open_gap": sum(1 for r in app if r["open_gap"]),
        "applicable_without_ledger_entry": sum(1 for r in app if not r["ledger"]),
        "compatibility_ledger": {"file": "ledger/ledger.json", "entries": len(items), "statuses": dict(sorted(statuses.items()))},
        "rea_evidence": REA,
        "rules": "ledger/functions/tools/build_function_ledger.py (module documentation)",
    }
    # One function on each line, so that a change of one record is a change of one line.
    out["FUNCTION_LEDGER.json"] = ('{"meta": ' + json.dumps(meta, indent=1) + ',\n"functions": [\n'
                                   + ",\n".join(json.dumps(r, separators=(",", ":")) for r in records) + "\n]}\n")

    buf = io.StringIO()
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(["id", "upstream_file", "line", "owner", "name", "kind", "scope", "applicable", "inline", "mapping", "rust_crate", "rust_file",
                "rust_line", "rust_symbol", "more_rust", "tests", "test_files", "ledger_ids", "ledger_statuses", "approved_difference", "open_gap"])
    for r in records:
        u, first = r["upstream"], (r["rust"][0] if r["rust"] else {})
        w.writerow([r["id"], u["file"], u["line"], u["owner"], u["name"] or "", u["kind"], r["scope"], int(r["applicable"]), int(r["inline"]),
                    r["mapping"], first.get("crate", ""), first.get("file", ""), first.get("line") or "", first.get("symbol", ""),
                    max(0, len(r["rust"]) - 1), r["tests"], " ".join(r["test_files"]), " ".join(e["id"] for e in r["ledger"]),
                    " ".join(e["status"] for e in r["ledger"]), int(r["approved_difference"]), int(r["open_gap"])])
    out["FUNCTION_LEDGER.csv"] = buf.getvalue()

    head = f"""Upstream: {PIN} (`oracle/x_ite-pin/src`). Generator: `ledger/functions/tools/` (extractor and builder; the rules are in the
builder's module documentation). This file is generated: do not edit it.

`mapping` is code presence only: `cited` (a Rust source line names the X_ITE function), `name` (a Rust function of the same name
in a file or `impl` of the same class; a candidate), `class` (a constructor whose class has Rust files in the 346-entry ledger), `none` (no symbol found by these rules; not proof of absence). `tests` is
separate: `direct` (a Rust test names the function), `entry` (its entry of the 346-entry ledger is COMPLETE with tests), `none`.
"""
    by_file = collections.defaultdict(list)
    for r in records:
        by_file[r["upstream"]["file"]].append(r)
    md = ["# Function ledger", "", head]
    for file in sorted(by_file):
        rows = by_file[file]
        scope = rows[0]["scope"]
        if scope == "third-party":
            md += [f"## `{file}`", "", f"Third-party code, {len(rows)} functions; not listed (see FUNCTION_LEDGER.json).", ""]
            continue
        entries = ", ".join(f"`{e['id']}` {e['status']}" for e in rows[0]["ledger"]) or "none"
        md += [f"## `{file}`", "", f"Scope: {scope}. Ledger entries: {entries}.", ""]
        body = []
        for r in rows:
            u = r["upstream"]
            if r["inline"]:
                continue
            rust = "; ".join((f"`{x['file']}:{x['line']}` `{x['symbol']}`" if x["line"] else f"`{x['file']}`") for x in r["rust"][:2]) + (f" (+{len(r['rust']) - 2})" if len(r["rust"]) > 2 else "")
            body.append([u["line"], f"`{u['owner']}.{u['name']}`", u["kind"], r["mapping"], rust or "", r["tests"]])
        inline = sum(1 for r in rows if r["inline"])
        md += [table(body, ["line", "function", "kind", "mapping", "Rust", "tests"]) if body else "No named function.", ""]
        if inline:
            md += [f"Inline functions (callbacks, part of the functions above): {inline}.", ""]
    out["FUNCTION_LEDGER.md"] = "\n".join(md)

    def pct(n, d):
        return f"{100 * n / d:.1f}%" if d else "n/a"
    s = ["# Function coverage summary", "", head,
         "## Counts", "",
         table([
             ["JavaScript files read", len(files)],
             ["files that did not parse", meta["files_with_parse_error"]],
             ["functions, all", total],
             ["of these: inline (callbacks and arrow functions without a name; part of the function around them)", meta["inline_functions"]],
             ["of these: named functions", total - meta["inline_functions"]],
             ["applicable functions (named, X_ITE's own engine code)", len(app)],
             ["Rust functions indexed (crates/*/src, test modules left out)", rust_count],
         ], ["item", "count"]), "",
         "## All functions by scope", "",
         table([[k, v, sum(1 for r in records if r["scope"] == k and not r["inline"])] for k, v in sorted(c_scope.items())],
               ["scope", "functions", "named"]), "",
         "Only `engine` and `component-module` functions are applicable. Third-party libraries (`lib/ammojs`, `lib/nurbs`, `lib/libtess.js`, `assets/lib`), shader source strings and the site files are listed in the JSON and CSV, and are not mapped.", "",
         "## Applicable functions: code presence", "",
         table([[k, c_map.get(k, 0), pct(c_map.get(k, 0), len(app))] for k in ("cited", "name", "class", "none")], ["mapping", "functions", "share"]), "",
         "`cited` is the only verified mapping. `name` is a candidate from the name and the class, and `class` says only that the class has Rust files. `none` is not a statement that the behavior is absent.", "",
         "## Applicable functions: tested behavior", "",
         table([[k, c_test.get(k, 0), pct(c_test.get(k, 0), len(app))] for k in ("direct", "entry", "none")], ["tests", "functions", "share"]), "",
         "## Applicable functions and the 346-entry ledger", "",
         table([
             ["linked to an entry by the entry's upstream file", len(app) - meta["applicable_without_ledger_entry"]],
             ["no entry lists the file", meta["applicable_without_ledger_entry"]],
             ["linked entry is DIFFERENT (approved difference)", meta["applicable_with_approved_difference"]],
             ["linked entry is PARTIAL (open gap)", meta["applicable_with_open_gap"]],
         ], ["item", "functions"]), "",
         f"The 346-entry ledger itself: {statuses.get('COMPLETE', 0)} COMPLETE / {statuses.get('PARTIAL', 0)} PARTIAL / {statuses.get('DIFFERENT', 0)} DIFFERENT / {statuses.get('MISSING', 0)} MISSING — total {len(items)}. It is read only here.", "",
         "## By area of the X_ITE source (applicable functions)", ""]
    areas = collections.defaultdict(lambda: collections.Counter())
    for r in app:
        a = areas[area(r["upstream"]["file"][4:])]
        a["n"] += 1; a[r["mapping"]] += 1; a["t_" + r["tests"]] += 1
    s += [table([[f"`{k}`", v["n"], v["cited"], v["name"], v["class"], v["none"], v["t_direct"], v["t_entry"], v["t_none"]] for k, v in sorted(areas.items())],
                ["area", "functions", "cited", "name", "class", "none", "tests direct", "tests entry", "tests none"]), "",
          "## REA evidence", "",
          "Static analysis of the whole pinned source (`analyze_javascript_application`, detail `summary`): parent `" + REA[0] + "`, summary `" + REA[1] + "`. REA parsed 765 JavaScript files (1793281 AST nodes, 0 parse failures); its graph is truncated and partial, it executed nothing, and it did not analyze Rust.", "",
          "REA is the independent check of the parse (file count, no parse failure). The function list itself comes from the extractor in `tools/`, which is deterministic; the Rust mapping comes from the Rust sources and is not a REA result.", ""]
    out["FUNCTION_COVERAGE_SUMMARY.md"] = "\n".join(s)

    un = [r for r in app if r["mapping"] == "none"]
    u_md = ["# Unmapped functions", "", head,
            f"Applicable functions with no Rust symbol found: **{len(un)}** of {len(app)}. Each is a function to map by hand (a Rust function holds it without a citation), to port, or to mark as not needed in Rust with a reason.", ""]
    grouped = collections.defaultdict(list)
    for r in un:
        grouped[r["upstream"]["file"]].append(r)
    for file in sorted(grouped):
        rows = grouped[file]
        entries = ", ".join(f"`{e['id']}` {e['status']}" for e in rows[0]["ledger"]) or "no ledger entry"
        u_md += [f"## `{file}` ({len(rows)}; {entries})", "",
                 ", ".join(f"`{r['upstream']['name']}` ({r['upstream']['line']})" for r in rows), ""]
    out["UNMAPPED_FUNCTIONS.md"] = "\n".join(u_md)

    b = ["# Port completion backlog", "", head,
         "One row for each X_ITE source file with applicable functions that have no Rust symbol, or whose ledger entry has an open gap. The order is the owner's order of work (directive of 2026-11-11), then the number of unmapped functions.", "",
         "A row is closed when each of its functions has a `cited` mapping and a test, or a recorded reason why Rust does not need it. The backlog does not change the 346-entry ledger.", ""]
    rows_by_file = collections.defaultdict(list)
    for r in app:
        rows_by_file[r["upstream"]["file"]].append(r)
    groups = collections.defaultdict(list)
    for file, rows in rows_by_file.items():
        none = sum(1 for r in rows if r["mapping"] == "none")
        name = sum(1 for r in rows if r["mapping"] == "name")
        untested = sum(1 for r in rows if r["tests"] == "none")
        gap = rows[0]["open_gap"]
        if none or gap:
            entries = ", ".join(f"{e['id']} {e['status']}" for e in rows[0]["ledger"]) or "none"
            groups[priority(file[4:])].append((-none, file, [f"`{file}`", len(rows), none, name, untested, entries]))
    grand = 0
    for g in sorted(groups):
        rows = sorted(groups[g])
        n = -sum(r[0] for r in rows)
        grand += n
        b += [f"## Priority {g}", "", f"{len(rows)} files, {n} functions without a Rust symbol.", "",
              table([r[2] for r in rows], ["X_ITE file", "functions", "no Rust symbol", "name candidate only", "no test", "ledger entries"]), ""]
    b += [f"Total: {grand} functions without a Rust symbol in {sum(len(v) for v in groups.values())} files.", "",
          "Priority 1 of the directive (missing function mappings) is this whole file. Priority 5 (WebGL2 and WebGPU) is a property of the Rust renderer backends and has no X_ITE source area of its own.", ""]
    out["PORT_COMPLETION_BACKLOG.md"] = "\n".join(b)
    return out, meta

def main():
    args = [a for a in sys.argv[1:] if a != "--check"]
    if len(args) != 1:
        sys.exit(__doc__)
    out, meta = render(*build(args[0]))
    bad = []
    for name, text in out.items():
        path = os.path.join(OUT, name)
        if "--check" in sys.argv:
            if not os.path.exists(path) or open(path, encoding="utf-8").read() != text:
                bad.append(name)
        else:
            with open(path, "w", encoding="utf-8") as f:
                f.write(text)
    print(json.dumps({k: v for k, v in meta.items() if k not in ("rules",)}, indent=1))
    if bad:
        sys.exit("would change: " + ", ".join(bad))

main()
