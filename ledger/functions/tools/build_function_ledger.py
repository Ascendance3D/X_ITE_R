#!/usr/bin/env python3
"""The function-level ledger: one record for every function of pinned X_ITE 16.5.0 (34cdf9f97), with its disposition.

  node ledger/functions/tools/extract_functions.mjs <acorn dir> oracle/x_ite-pin/src > functions.jsonl
  python3 -I -B ledger/functions/tools/build_function_ledger.py functions.jsonl --large-dir DIR [--check] [--hints DIR]
  python3 -I -B ledger/functions/tools/build_function_ledger.py --verify-dir DIR

The inventory depends only on the pinned source (the extractor's list), ledger/functions/dispositions.jsonl and the
`upstream` file names of ledger/ledger.json (read only). It does not depend on the Rust sources, so a Rust change does not
change it. No date, no machine path.

Files:
- Tracked in Git (ledger/functions/): FUNCTION_COVERAGE_SUMMARY.md, PORT_COMPLETION_BACKLOG.md and
  FUNCTION_LEDGER.manifest.json (for each large file: bytes, records, sha256).
- Not tracked (GitHub gives no patch for them, so no review can read them): FUNCTION_LEDGER.json, FUNCTION_LEDGER.csv,
  FUNCTION_LEDGER.md and UNMAPPED_FUNCTIONS.md. The builder writes them to --large-dir. The manifest is their identity.

Modes:
- default: write the tracked files and the manifest to ledger/functions/, and the four large files to --large-dir.
- --check: write the four large files to --large-dir only; fail when a tracked file or the manifest would change.
- --verify-dir DIR: read the four large files of DIR; fail when one is missing or its bytes, records or sha256 differ
  from the tracked manifest. No other input.

The manifest proves that the data is the generator's output for the pinned source. It does not prove that a mapping is
correct.

Status of a record:
    IMPLEMENTED_AND_VERIFIED   a Rust symbol holds the behavior, and a named test exercises the equivalent behavior
    IMPLEMENTED_NOT_VERIFIED   a Rust symbol holds the behavior; no such test is recorded
    IMPLEMENTED_DIFFERENT      Rust holds a different behavior on purpose (an approved difference)
    PARTIALLY_IMPLEMENTED      a Rust symbol holds part of the behavior
    MISSING_IMPLEMENTATION     inspection found no Rust code for a behavior that the engine needs
    REPLACED_BY_RUST_DESIGN    Rust, or a Rust dependency, makes the function unnecessary
    PLATFORM_SPECIFIC          browser or JavaScript glue; the host integration holds it, not the engine
    NOT_APPLICABLE             not X_ITE's own engine code (third-party library, shader source text, site file, inline callback)
    UNRESOLVED_MAPPING         nobody has inspected the function yet. This is the default. It is not "missing".

A status other than the default comes only from a record of dispositions.jsonl, written by someone who read the X_ITE
function and the Rust code. A source comment, a symbol name or a test name never sets a status. The builder checks that a
record is complete and that its Rust and test symbols exist; it cannot check that a test exercises the equivalent behavior.
That claim (`evidence`) is checked in the review of the pull request that adds the record.

A record of dispositions.jsonl (one JSON object on each line):
    {"id": "<record id>", "status": "...", "rust": [{"file": "crates/.../x.rs", "symbol": "Type::function"}],
     "tests": [{"file": "crates/.../t.rs", "name": "test_function"}], "evidence": "...", "reason": "...", "rea": ["ev_..."]}
- `rust` is required for the IMPLEMENTED_* and PARTIALLY_IMPLEMENTED statuses. Many X_ITE functions can name one symbol.
- `tests` and `evidence` (what the test does and why that is the equivalent behavior) are required for
  IMPLEMENTED_AND_VERIFIED.
- `reason` is required for every other status.
- {"file": "src/...js", ...} in place of `id` is a rule for all applicable functions of that X_ITE file. It is valid only for
  REPLACED_BY_RUST_DESIGN, PLATFORM_SPECIFIC and NOT_APPLICABLE. A record with `id` wins over a rule.

Hints (--hints DIR, not tracked, not in the manifest): MAPPING_HINTS.csv and MAPPING_HINTS_SUMMARY.md, a scan of the Rust
sources for candidates. `cited`: a Rust source line names the X_ITE function. `name`: a Rust function of the same name in a
file or `impl` of the same class. `class`: the 346-entry ledger lists Rust files of the constructor's class. Test hints:
`direct` (a Rust test names the function), `entry` (the linked ledger entry is COMPLETE with tests). A hint is where to look.
It is not a status, and it changes when the Rust sources change.

A function without a name of its own (a callback, an arrow function) is `inline`: it is part of the function around it."""
import collections, csv, hashlib, io, json, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT = os.path.join(ROOT, "ledger", "functions")
PIN = "X_ITE 16.5.0 @ 34cdf9f97"
REA = [
    "ev_4b19e25b68ef315c508675fd4b3f73b3b165891f5c683cafe92b06a6e1e87313",
    "ev_304fc670a11afcb98229572336aae2872b97e3f40960f15a63a9d2945bd1873a",
]
STATUSES = ("IMPLEMENTED_AND_VERIFIED", "IMPLEMENTED_NOT_VERIFIED", "IMPLEMENTED_DIFFERENT", "PARTIALLY_IMPLEMENTED",
            "MISSING_IMPLEMENTATION", "REPLACED_BY_RUST_DESIGN", "PLATFORM_SPECIFIC", "NOT_APPLICABLE", "UNRESOLVED_MAPPING")
NEEDS_RUST = STATUSES[:4]
RULE_STATUSES = ("REPLACED_BY_RUST_DESIGN", "PLATFORM_SPECIFIC", "NOT_APPLICABLE")
LARGE = ("FUNCTION_LEDGER.csv", "FUNCTION_LEDGER.json", "FUNCTION_LEDGER.md", "UNMAPPED_FUNCTIONS.md")
MANIFEST = "FUNCTION_LEDGER.manifest.json"
DISPOSITIONS = os.path.join(OUT, "dispositions.jsonl")

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

def has_fn(path, name):
    """Whether the Rust file `path` (under the repository) defines a function `name` (the last segment of a symbol)."""
    full = os.path.join(ROOT, path)
    if not path.startswith("crates/") or ".." in path or not os.path.isfile(full):
        return False
    return re.search(r"\bfn\s+" + re.escape(name.split("::")[-1]) + r"\b", open(full, encoding="utf-8", errors="replace").read()) is not None

def load_dispositions(ids, files):
    """(by id, by file) of dispositions.jsonl, after the checks of the module documentation. Exits on a bad record."""
    by_id, by_file, bad = {}, {}, []
    if not os.path.exists(DISPOSITIONS):
        return by_id, by_file
    for n, line in enumerate(open(DISPOSITIONS, encoding="utf-8"), 1):
        if not line.strip():
            continue
        def err(text):
            bad.append(f"dispositions.jsonl:{n}: {text}")
        try:
            d = json.loads(line)
        except ValueError as e:
            err(f"not JSON ({e})"); continue
        if not isinstance(d, dict) or ("id" in d) == ("file" in d):
            err("needs exactly one of `id` and `file`"); continue
        unknown = set(d) - {"id", "file", "status", "rust", "tests", "evidence", "reason", "rea"}
        if unknown:
            err(f"unknown keys {sorted(unknown)}")
        status = d.get("status")
        if status not in STATUSES or status == "UNRESOLVED_MAPPING":
            err(f"status {status!r} is not a disposition"); continue
        rust, tests = d.get("rust", []), d.get("tests", [])
        if "file" in d:
            if status not in RULE_STATUSES:
                err(f"a file rule cannot set {status}")
            if d["file"] not in files:
                err(f"no applicable function in {d['file']}")
            if d["file"] in by_file:
                err(f"second rule for {d['file']}")
            by_file[d["file"]] = d
        else:
            if d["id"] not in ids:
                err(f"no applicable record {d['id']}")
            if d["id"] in by_id:
                err(f"second record for {d['id']}")
            by_id[d["id"]] = d
        if status in NEEDS_RUST:
            if not rust:
                err(f"{status} needs `rust`")
            for r in rust:
                if not (isinstance(r, dict) and set(r) == {"file", "symbol"} and has_fn(r["file"], r["symbol"])):
                    err(f"Rust symbol not found: {r}")
        elif rust or tests:
            err(f"{status} takes no `rust` and no `tests`")
        if status == "IMPLEMENTED_AND_VERIFIED":
            if not tests or len(str(d.get("evidence", "")).strip()) < 40:
                err("IMPLEMENTED_AND_VERIFIED needs `tests` and `evidence` (what the test does and why that is the equivalent behavior)")
        elif len(str(d.get("reason", "")).strip()) < 20:
            err(f"{status} needs `reason`")
        for t in tests:
            if not (isinstance(t, dict) and set(t) == {"file", "name"} and has_fn(t["file"], t["name"])):
                err(f"test not found: {t}")
    if bad:
        sys.exit("\n".join(bad))
    return by_id, by_file

def build(extract, with_hints):
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
    items, ledger_by_file = load_ledger()
    records = []
    for f in functions:
        scope, applicable = scope_of(f["file"])
        owner = f["owner_shown"]
        inline = f["kind"].startswith("anonymous")
        applicable = applicable and not inline
        records.append({
            "id": f"{f['file']}#{owner}.{f['name'] or 'anonymous'}@{f['line']}",
            "upstream": {"file": "src/" + f["file"], "line": f["line"], "end_line": f["endLine"], "owner": owner, "name": f["name"],
                         "kind": f["kind"], "static": f["static"], "params": f["params"], "enclosing": f["enclosing"],
                         "sha256": f["sha256"]},
            "scope": scope,
            "applicable": applicable,
            "inline": inline,
            "status": "UNRESOLVED_MAPPING" if applicable else "NOT_APPLICABLE",
            "disposition": None,
            "ledger": [e["id"] for e in ledger_by_file.get(f["file"], [])],
        })
    by_id, by_file = load_dispositions({r["id"] for r in records if r["applicable"]},
                                       {r["upstream"]["file"] for r in records if r["applicable"]})
    for r in records:
        d = by_id.get(r["id"]) or (by_file.get(r["upstream"]["file"]) if r["applicable"] else None)
        if d:
            r["status"] = d["status"]
            r["disposition"] = {k: d[k] for k in ("rust", "tests", "evidence", "reason", "rea") if d.get(k)}
            if "file" in d:
                r["disposition"]["rule"] = d["file"]
    return files, records, items, (hints(functions, records, ledger_by_file, methods_of) if with_hints else None)

def hints(functions, records, ledger_by_file, methods_of):
    """The scan of the Rust sources for candidates (see the module documentation). Not part of the inventory."""
    by_name, cited, tested, rust_count = index_rust(methods_of)
    rows = []
    for f, rec in zip(functions, records):
        if not rec["applicable"]:
            continue
        base, owner = os.path.basename(f["file"])[:-3], f["owner_shown"]
        entries = ledger_by_file.get(f["file"], [])
        h = {"id": rec["id"], "mapping": "none", "rust": [], "tests": "none", "test_files": [],
             "ledger": [{"id": e["id"], "status": e["status"]} for e in entries]}
        keys = [(owner, f["name"])] + ([(base, f["name"])] if base != owner else [])
        hit = [c for k in keys for c in cited.get(k, [])]
        if hit:
            h["mapping"], h["rust"] = "cited", [ref(c) for c in hit[:6]]
        else:
            stems = stems_for(owner) | stems_for(base)
            listed = set()
            for e in entries:
                listed |= set(re.findall(r"crates/[\w./-]+\.rs", str(e.get("rust", ""))))
            cands = [fn for n in names_for(f["name"]) for fn in by_name.get(n, [])
                     if fn["stem"] in stems or fn["impl"] in (owner, base) or fn["file"] in listed]
            if cands:
                h["mapping"], h["rust"] = "name", [ref(c) for c in cands[:6]]
            elif f["name"] == base and f["kind"] == "constructor-or-function" and listed:
                h["mapping"] = "class"
                h["rust"] = [{"crate": p.split("/")[1], "file": p, "line": None, "symbol": "(class files of the 346-entry ledger)"}
                             for p in sorted(listed)[:6]]
        direct = sorted({p for k in keys for p in tested.get(k, ())})
        if direct:
            h["tests"], h["test_files"] = "direct", direct[:8]
        elif any(e["status"] == "COMPLETE" and e.get("tests") for e in entries):
            h["tests"] = "entry"
        rows.append(h)
    return rows, rust_count

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

OPEN = ("UNRESOLVED_MAPPING", "MISSING_IMPLEMENTATION", "PARTIALLY_IMPLEMENTED")
HEAD = f"""Upstream: {PIN} (`oracle/x_ite-pin/src`). Generator: `ledger/functions/tools/` (extractor and builder; the rules are in the
builder's module documentation). This file is generated: do not edit it.

A status other than `UNRESOLVED_MAPPING` comes only from an inspected record of `ledger/functions/dispositions.jsonl`.
`UNRESOLVED_MAPPING` means that nobody has inspected the function. It does not mean that the implementation is missing.
"""

def symbols(d, key, a, b):
    return " ".join(f"{x[a]}::{x[b]}" for x in (d or {}).get(key, []))

def render(files, records, items):
    """(tracked files, large files, manifest) as text."""
    small, large = {}, {}
    app = [r for r in records if r["applicable"]]
    total = len(records)
    c_scope = collections.Counter(r["scope"] for r in records)
    c_status = collections.Counter(r["status"] for r in app)
    source_sha = hashlib.sha256("".join(f["file"] + f["sha256"] for f in files).encode()).hexdigest()
    meta = {
        "upstream": PIN, "source": "oracle/x_ite-pin/src", "files": len(files),
        "files_with_parse_error": sum(1 for f in files if "parseError" in f),
        "source_files_sha256": source_sha,
        "functions": total, "inline_functions": sum(1 for r in records if r["inline"]),
        "applicable_functions": len(app),
        "by_scope": dict(sorted(c_scope.items())),
        "status_of_applicable": {s: c_status.get(s, 0) for s in STATUSES},
        "applicable_without_ledger_entry": sum(1 for r in app if not r["ledger"]),
        "compatibility_ledger": {"file": "ledger/ledger.json", "entries": len(items)},
        "rea_evidence": REA,
        "rules": "ledger/functions/tools/build_function_ledger.py (module documentation)",
    }
    # One function on each line, so that a change of one record is a change of one line.
    large["FUNCTION_LEDGER.json"] = ('{"meta": ' + json.dumps(meta, indent=1) + ',\n"functions": [\n'
                                     + ",\n".join(json.dumps(r, separators=(",", ":")) for r in records) + "\n]}\n")

    buf = io.StringIO()
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(["id", "upstream_file", "line", "owner", "name", "kind", "scope", "applicable", "inline", "status", "rust", "tests",
                "rule", "ledger_ids"])
    for r in records:
        u, d = r["upstream"], r["disposition"]
        w.writerow([r["id"], u["file"], u["line"], u["owner"], u["name"] or "", u["kind"], r["scope"], int(r["applicable"]), int(r["inline"]),
                    r["status"], symbols(d, "rust", "file", "symbol"), symbols(d, "tests", "file", "name"), (d or {}).get("rule", ""),
                    " ".join(r["ledger"])])
    large["FUNCTION_LEDGER.csv"] = buf.getvalue()

    by_file = collections.defaultdict(list)
    for r in records:
        by_file[r["upstream"]["file"]].append(r)
    md, md_rows = ["# Function ledger", "", HEAD], 0
    for file in sorted(by_file):
        rows = by_file[file]
        scope = rows[0]["scope"]
        if scope == "third-party":
            md += [f"## `{file}`", "", f"Third-party code, {len(rows)} functions; not listed (see FUNCTION_LEDGER.json).", ""]
            continue
        entries = ", ".join(f"`{e}`" for e in rows[0]["ledger"]) or "none"
        md += [f"## `{file}`", "", f"Scope: {scope}. Ledger entries: {entries}.", ""]
        body = []
        for r in rows:
            u, d = r["upstream"], r["disposition"]
            if r["inline"]:
                continue
            body.append([u["line"], f"`{u['owner']}.{u['name']}`", u["kind"], r["status"],
                         "; ".join(f"`{x['file']}` `{x['symbol']}`" for x in (d or {}).get("rust", [])),
                         "; ".join(f"`{x['name']}`" for x in (d or {}).get("tests", []))])
        md_rows += len(body)
        inline = sum(1 for r in rows if r["inline"])
        md += [table(body, ["line", "function", "kind", "status", "Rust", "tests"]) if body else "No named function.", ""]
        if inline:
            md += [f"Inline functions (callbacks, part of the functions above): {inline}.", ""]
    large["FUNCTION_LEDGER.md"] = "\n".join(md)

    un = [r for r in app if r["status"] == "UNRESOLVED_MAPPING"]
    u_md = ["# Unresolved function mappings", "", HEAD,
            f"Applicable functions that nobody has inspected: **{len(un)}** of {len(app)}. Each needs one of: the Rust symbol that holds it, a finding that Rust does not hold it, or the reason why Rust does not need it.", ""]
    grouped = collections.defaultdict(list)
    for r in un:
        grouped[r["upstream"]["file"]].append(r)
    for file in sorted(grouped):
        rows = grouped[file]
        entries = ", ".join(f"`{e}`" for e in rows[0]["ledger"]) or "no ledger entry"
        u_md += [f"## `{file}` ({len(rows)}; {entries})", ""] + [f"- `{r['upstream']['name']}` ({r['upstream']['line']})" for r in rows] + [""]
    large["UNMAPPED_FUNCTIONS.md"] = "\n".join(u_md)

    def pct(n, d):
        return f"{100 * n / d:.1f}%" if d else "n/a"
    s = ["# Function coverage summary", "", HEAD,
         "## Counts", "",
         table([
             ["JavaScript files read", len(files)],
             ["files that did not parse", meta["files_with_parse_error"]],
             ["functions, all", total],
             ["of these: inline (callbacks and arrow functions without a name; part of the function around them)", meta["inline_functions"]],
             ["of these: named functions", total - meta["inline_functions"]],
             ["applicable functions (named, X_ITE's own engine code)", len(app)],
         ], ["item", "count"]), "",
         "## All functions by scope", "",
         table([[k, v, sum(1 for r in records if r["scope"] == k and not r["inline"])] for k, v in sorted(c_scope.items())],
               ["scope", "functions", "named"]), "",
         "Only `engine` and `component-module` functions are applicable. Third-party libraries (`lib/ammojs`, `lib/nurbs`, `lib/libtess.js`, `assets/lib`), shader source strings and the site files are listed in the JSON and CSV with the status `NOT_APPLICABLE`.", "",
         "## Applicable functions by status", "",
         table([[k, c_status.get(k, 0), pct(c_status.get(k, 0), len(app))] for k in STATUSES], ["status", "functions", "share"]), "",
         "This table counts inspected dispositions. It is not a measure of how much of X_ITE the Rust engine implements: the engine's behavior is measured by the 346-entry ledger and its tests.", "",
         "## Applicable functions and the 346-entry ledger", "",
         table([
             ["linked to an entry by the entry's upstream file", len(app) - meta["applicable_without_ledger_entry"]],
             ["no entry lists the file", meta["applicable_without_ledger_entry"]],
         ], ["item", "functions"]), "",
         f"The 346-entry ledger (`ledger/ledger.json`, {len(items)} entries) is read only here. Its statuses are not copied into this inventory.", "",
         "## By area of the X_ITE source (applicable functions)", ""]
    areas = collections.defaultdict(lambda: collections.Counter())
    for r in app:
        a = areas[area(r["upstream"]["file"][4:])]
        a["n"] += 1; a[r["status"]] += 1
    s += [table([[f"`{k}`", v["n"], v["UNRESOLVED_MAPPING"], v["IMPLEMENTED_AND_VERIFIED"], v["IMPLEMENTED_NOT_VERIFIED"],
                  v["IMPLEMENTED_DIFFERENT"], v["PARTIALLY_IMPLEMENTED"], v["MISSING_IMPLEMENTATION"],
                  v["REPLACED_BY_RUST_DESIGN"] + v["PLATFORM_SPECIFIC"] + v["NOT_APPLICABLE"]] for k, v in sorted(areas.items())],
                ["area", "functions", "unresolved", "verified", "not verified", "different", "partial", "missing", "no Rust function needed"]), "",
          "## REA evidence", "",
          "Static analysis of the pinned source (`analyze_javascript_application`, detail `summary`): parent `" + REA[0] + "`, summary `" + REA[1] + "`. REA reported 765 files parsed (1793281 AST nodes, 0 parse failures); its graph is truncated and partial, it executed nothing, and it did not analyze Rust.", "",
          f"REA is independent evidence that the source parses without a failure. It is not a check of the file count: the extractor reads the {len(files)} `.js` files, and REA reported one file more. The pinned source also has one TypeScript declaration file (`x_ite.d.ts`, no function body), which is the likely difference; REA's summary does not list its files, so this is not verified. The function list itself comes from the extractor in `tools/`, which is deterministic.", ""]
    small["FUNCTION_COVERAGE_SUMMARY.md"] = "\n".join(s)

    b = ["# Port completion backlog", "", HEAD,
         "One row for each X_ITE source file with applicable functions whose status is open (`UNRESOLVED_MAPPING`, `MISSING_IMPLEMENTATION` or `PARTIALLY_IMPLEMENTED`). The order is the owner's order of work (directive of 2026-11-11), then the number of open functions.", "",
         "A row closes when each of its functions has an inspected disposition that is not open. The backlog does not change the 346-entry ledger. An implementation batch records dispositions only for the functions that it inspected.", ""]
    rows_by_file = collections.defaultdict(list)
    for r in app:
        rows_by_file[r["upstream"]["file"]].append(r)
    groups = collections.defaultdict(list)
    for file, rows in rows_by_file.items():
        c = collections.Counter(r["status"] for r in rows)
        n = sum(c[k] for k in OPEN)
        if n:
            groups[priority(file[4:])].append((-n, file, [f"`{file}`", len(rows), c["UNRESOLVED_MAPPING"], c["MISSING_IMPLEMENTATION"],
                                                         c["PARTIALLY_IMPLEMENTED"], ", ".join(rows[0]["ledger"]) or "none"]))
    grand = 0
    for g in sorted(groups):
        rows = sorted(groups[g])
        n = -sum(r[0] for r in rows)
        grand += n
        b += [f"## Priority {g}", "", f"{len(rows)} files, {n} open functions.", "",
              table([r[2] for r in rows], ["X_ITE file", "functions", "unresolved", "missing", "partial", "ledger entries"]), ""]
    b += [f"Total: {grand} open functions in {sum(len(v) for v in groups.values())} files.", "",
          "Priority 5 of the directive (WebGL2 and WebGPU) is a property of the Rust renderer backends and has no X_ITE source area of its own.", ""]
    small["PORT_COMPLETION_BACKLOG.md"] = "\n".join(b)

    counts = {"FUNCTION_LEDGER.csv": total, "FUNCTION_LEDGER.json": total, "FUNCTION_LEDGER.md": md_rows, "UNMAPPED_FUNCTIONS.md": len(un)}
    manifest = {
        "upstream": PIN, "source": "oracle/x_ite-pin/src", "source_files_sha256": source_sha,
        "source_files_sha256_of": "the text of each .js file's path and sha256, in sorted order (not the bytes of the tree)",
        "note": "Identity of the four generated files that are not tracked in Git. It proves that the data is the generator's output. It does not prove that a mapping is correct.",
        "records": "csv, json: all functions. md: the named functions listed. UNMAPPED_FUNCTIONS.md: the applicable functions with UNRESOLVED_MAPPING.",
        "files": {name: {"bytes": len(large[name].encode("utf-8")), "records": counts[name],
                         "sha256": hashlib.sha256(large[name].encode("utf-8")).hexdigest()} for name in LARGE},
    }
    small[MANIFEST] = json.dumps(manifest, indent=1) + "\n"
    return small, large, meta

def records_in(name, data):
    """The number of records of a large file, counted from its bytes."""
    text = data.decode("utf-8")
    if name.endswith(".json"):
        return len(json.loads(text)["functions"])
    if name.endswith(".csv"):
        return sum(1 for _ in csv.reader(io.StringIO(text))) - 1
    if name == "FUNCTION_LEDGER.md":
        return len(re.findall(r"(?m)^\| \d+ \| `", text))
    return len(re.findall(r"(?m)^- `", text))

def verify_dir(directory):
    """Fail unless the four large files of `directory` are the files of the tracked manifest."""
    manifest = json.load(open(os.path.join(OUT, MANIFEST), encoding="utf-8"))
    if sorted(manifest["files"]) != sorted(LARGE):
        sys.exit(f"{MANIFEST}: the file names are not {', '.join(LARGE)}")
    bad = []
    for name in LARGE:
        want, path = manifest["files"][name], os.path.join(directory, name)
        if not os.path.isfile(path):
            bad.append(f"{name}: missing"); continue
        data = open(path, "rb").read()
        try:
            have = {"bytes": len(data), "records": records_in(name, data), "sha256": hashlib.sha256(data).hexdigest()}
        except (ValueError, KeyError, UnicodeDecodeError) as e:
            bad.append(f"{name}: cannot read ({e})"); continue
        bad += [f"{name}: {k} is {have[k]}, the manifest has {want[k]}" for k in ("bytes", "records", "sha256") if have[k] != want[k]]
        if not bad or not bad[-1].startswith(name):
            print(f"ok  {name}  {have['bytes']} bytes, {have['records']} records, sha256 {have['sha256']}")
    if bad:
        sys.exit("\n".join(bad))
    print(f"The four files of {directory} are the files of {MANIFEST} ({manifest['upstream']}).")

def write_hints(directory, rows, rust_count, items):
    buf = io.StringIO()
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(["id", "mapping", "rust_crate", "rust_file", "rust_line", "rust_symbol", "more_rust", "tests", "test_files",
                "ledger_ids", "ledger_statuses"])
    for h in rows:
        first = h["rust"][0] if h["rust"] else {}
        w.writerow([h["id"], h["mapping"], first.get("crate", ""), first.get("file", ""), first.get("line") or "", first.get("symbol", ""),
                    max(0, len(h["rust"]) - 1), h["tests"], " ".join(h["test_files"]), " ".join(e["id"] for e in h["ledger"]),
                    " ".join(e["status"] for e in h["ledger"])])
    c_map, c_test = collections.Counter(h["mapping"] for h in rows), collections.Counter(h["tests"] for h in rows)
    statuses = collections.Counter(i["status"] for i in items)
    md = ["# Mapping hints", "",
          f"Upstream: {PIN}. A scan of the Rust sources for candidates, made by `build_function_ledger.py --hints`. Not tracked, not in the manifest; it changes when the Rust sources change.", "",
          "A hint is where to look. It is not a status: a source comment, a symbol name or a test name does not show that Rust holds the behavior, and `none` does not show that the behavior is absent.", "",
          f"Applicable functions: {len(rows)}. Rust functions indexed (crates/*/src, test modules left out): {rust_count}.", "",
          table([[k, c_map.get(k, 0)] for k in ("cited", "name", "class", "none")], ["mapping hint", "functions"]), "",
          table([[k, c_test.get(k, 0)] for k in ("direct", "entry", "none")], ["test hint", "functions"]), "",
          f"The 346-entry ledger at the time of the scan: {statuses.get('COMPLETE', 0)} COMPLETE / {statuses.get('PARTIAL', 0)} PARTIAL / {statuses.get('DIFFERENT', 0)} DIFFERENT / {statuses.get('MISSING', 0)} MISSING — total {len(items)}.", ""]
    os.makedirs(directory, exist_ok=True)
    for name, text in (("MAPPING_HINTS.csv", buf.getvalue()), ("MAPPING_HINTS_SUMMARY.md", "\n".join(md))):
        with open(os.path.join(directory, name), "w", encoding="utf-8", newline="") as f:
            f.write(text)

def main():
    argv, opt = sys.argv[1:], {}
    for name in ("--large-dir", "--hints", "--verify-dir"):
        if name in argv:
            i = argv.index(name)
            if i + 1 >= len(argv):
                sys.exit(__doc__)
            opt[name] = argv[i + 1]
            del argv[i:i + 2]
    check = "--check" in argv
    args = [a for a in argv if a != "--check"]
    if "--verify-dir" in opt:
        if args or check or len(opt) != 1:
            sys.exit(__doc__)
        return verify_dir(opt["--verify-dir"])
    if len(args) != 1 or args[0].startswith("-") or "--large-dir" not in opt:
        sys.exit(__doc__)
    if os.path.realpath(opt["--large-dir"]) == os.path.realpath(OUT):
        sys.exit("--large-dir must not be ledger/functions: the four large files are not tracked")
    files, records, items, hinted = build(args[0], "--hints" in opt)
    small, large, meta = render(files, records, items)
    os.makedirs(opt["--large-dir"], exist_ok=True)
    for name, text in large.items():
        with open(os.path.join(opt["--large-dir"], name), "w", encoding="utf-8", newline="") as f:
            f.write(text)
    bad = [name for name in LARGE if os.path.exists(os.path.join(OUT, name))]
    if bad:
        sys.exit("not tracked files are in ledger/functions: " + ", ".join(bad))
    for name, text in small.items():
        path = os.path.join(OUT, name)
        if check:
            if not os.path.exists(path) or open(path, encoding="utf-8", newline="").read() != text:
                bad.append(name)
        else:
            with open(path, "w", encoding="utf-8", newline="") as f:
                f.write(text)
    if hinted:
        write_hints(opt["--hints"], *hinted, items)
    print(json.dumps({k: v for k, v in meta.items() if k not in ("rules",)}, indent=1))
    if bad:
        sys.exit("would change: " + ", ".join(bad))

main()
