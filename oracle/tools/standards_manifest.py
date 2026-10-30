#!/usr/bin/env python3
"""Check the standards fixture manifests: python3 -I -B oracle/tools/standards_manifest.py check [MANIFEST.json ...]

With no manifest named, checks every oracle/standards/manifests/*.json; with names, only those manifests. Prints one
line per problem and exits 1 when there is one, else prints a summary and exits 0. Standard library only; it reads
files and writes nothing.

oracle/standards/MANIFEST.schema.json is the source of the required fields and the enums. The checks:

- Every entry and manifest field the schema requires is present, with the type and the value set the schema gives,
  and no other field is present.
- `sha256` and `bytes` equal the file. The path is a regular file below `oracle/`, with no symlink on the way, no `..`,
  and is not a row of `oracle/answer-keys.tsv` or below one.
- The first line of the file equals `header`, and `header` is the header of the declared `standard`.
- Dependencies: every URL of a `url` or `*Url` field and of an EXTERNPROTO statement, in the file and, in turn, in each
  local VRML or X3D classic file it names. A relative URL resolves against the file that holds it (ISO/IEC 14772-1
  4.5.3 item 3; this check does not model item 1, the base of a URL inside a PROTO definition), without its
  `#fragment`. It must stay below `oracle/`. `javascript:`, `ecmascript:`, `vrmlscript:` and `data:` strings hold no
  dependency. Any other URL with a scheme must be http or https on a host in the reserved `.invalid` domain. The
  declared `dependencies` must be exactly that set: present files with their sha256 and bytes, and `absent: true` for
  a local path that does not exist or an `.invalid` URL.
- No entry has `ISO_VERIFIED` (no promotion; README.md), every entry has `private: false`, and the class, tag and
  manifest rules of README.md hold.
- No path is in two of the checked manifests. With no manifest named, also: every scene file below
  `oracle/standards/` is an entry of a manifest.
"""
import hashlib, json, os, re, sys
from urllib.parse import unquote, urlsplit

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
SCHEMA = "oracle/standards/MANIFEST.schema.json"
MANIFESTS = "oracle/standards/manifests"
LIBRARY = "oracle/standards/"
HEADERS = {
    "VRML97": "#VRML V2.0 utf8",
    "blaxxun-compat": "#VRML V2.0 utf8",
    "X3D-3.3": "#X3D V3.3 utf8",
    "X3D-4.0": "#X3D V4.0 utf8",
    "X3D-4.1-regression": "#X3D V4.1 utf8",
}
SCENE_EXTENSIONS = (".wrl", ".x3dv")
INLINE_SCHEMES = ("javascript", "ecmascript", "vrmlscript", "data")


def load_json(path):
    with open(path, "rb") as f:
        return json.loads(f.read().decode("utf-8"))


def answer_keys(root):
    """The repository paths of the rows of oracle/answer-keys.tsv."""
    out = []
    try:
        with open(os.path.join(root, "oracle", "answer-keys.tsv"), "rb") as f:
            for line in f.read().decode("utf-8").splitlines():
                if line.strip() and not line.startswith("#"):
                    out.append("oracle/" + line.split("\t")[0])
    except FileNotFoundError:
        pass
    return out


def tokens(text):
    """The tokens of a VRML or X3D classic file: strings (unescaped, as ("s", value)), brackets and words. Comments
    and whitespace (commas included) are dropped."""
    out, i, n = [], 0, len(text)
    while i < n:
        c = text[i]
        if c == "#":
            j = text.find("\n", i)
            i = n if j < 0 else j + 1
        elif c.isspace() or c == ",":
            i += 1
        elif c == '"':
            j, buf = i + 1, []
            while j < n and text[j] != '"':
                if text[j] == "\\" and j + 1 < n:
                    j += 1
                buf.append(text[j])
                j += 1
            out.append(("s", "".join(buf)))
            i = j + 1
        elif c in "[]{}":
            out.append(("p", c))
            i += 1
        else:
            j = i
            while j < n and not text[j].isspace() and text[j] not in '[]{}#",':
                j += 1
            out.append(("w", text[i:j]))
            i = j
    return out


def urls(text):
    """The URL strings of the `url` and `*Url` fields and of the EXTERNPROTO statements, in file order."""
    toks, out, i = tokens(text), [], 0

    def value(k):
        if k < len(toks) and toks[k][0] == "s":
            out.append(toks[k][1])
            return k + 1
        if k < len(toks) and toks[k] == ("p", "["):
            k += 1
            while k < len(toks) and toks[k] != ("p", "]"):
                if toks[k][0] == "s":
                    out.append(toks[k][1])
                k += 1
            return k + 1
        return k

    while i < len(toks):
        kind, v = toks[i]
        if kind == "w" and v == "EXTERNPROTO":
            k, depth = i + 2, 0
            while k < len(toks):
                if toks[k] == ("p", "["):
                    depth += 1
                elif toks[k] == ("p", "]"):
                    depth -= 1
                    if depth == 0:
                        break
                k += 1
            i = value(k + 1)
        elif kind == "w" and (v == "url" or v.endswith("Url")):
            i = value(i + 1)
        else:
            i += 1
    return out


def safe_path(root, rel, keys):
    """An error text for a repository path that is not a regular file below oracle/ reached without a symlink, or that
    is an answer key or below one; None when it is fine. A missing file is reported by the caller."""
    if rel != os.path.normpath(rel).replace(os.sep, "/") or rel.startswith("/") or ".." in rel.split("/"):
        return "not a normalized repository path"
    if not rel.startswith("oracle/"):
        return "not below oracle/"
    for k in keys:
        if rel == k or rel.startswith(k + "/"):
            return f"is the answer key {k} or below it (oracle/answer-keys.tsv)"
    parts = rel.split("/")
    for n in range(1, len(parts) + 1):
        if os.path.islink(os.path.join(root, *parts[:n])):
            return "a symlink is on the path"
    return None


def scan_dependencies(root, rel):
    """(present, absent, remote, errors): the local files the scene loads, transitively, the local paths it names
    that do not exist, the .invalid URLs it names, and the URLs that break the dependency rule."""
    present, absent, remote, errors, todo, seen = set(), set(), set(), [], [rel], set()
    while todo:
        cur = todo.pop()
        if cur in seen:
            continue
        seen.add(cur)
        with open(os.path.join(root, cur), "rb") as f:
            data = f.read()
        if data[:2] == b"\x1f\x8b":
            errors.append(f"{cur}: gzip-compressed scene; the dependency scan reads text only")
            continue
        for u in urls(data.decode("utf-8", "replace")):
            s = u.strip()
            scheme = re.match(r"^([A-Za-z][A-Za-z0-9+.-]*):", s)
            if scheme and scheme.group(1).lower() in INLINE_SCHEMES:
                continue
            if scheme:
                parts = urlsplit(s)
                host = (parts.hostname or "").lower()
                if parts.scheme.lower() in ("http", "https") and (host == "invalid" or host.endswith(".invalid")):
                    remote.add(s)
                else:
                    errors.append(f"{cur}: URL {s!r} is neither local nor on an .invalid host")
                continue
            target = unquote(s.split("#", 1)[0].split("?", 1)[0])
            if not target:
                continue
            dep = os.path.normpath(os.path.join(os.path.dirname(cur), target)).replace(os.sep, "/")
            if not dep.startswith("oracle/") or dep.startswith("../"):
                errors.append(f"{cur}: URL {s!r} resolves outside oracle/")
            elif os.path.isfile(os.path.join(root, dep)):
                present.add(dep)
                if dep.endswith(SCENE_EXTENSIONS):
                    todo.append(dep)
            elif os.path.lexists(os.path.join(root, dep)):
                errors.append(f"{cur}: URL {s!r} names {dep}, which is not a regular file")
            else:
                absent.add(dep)
    return present, absent, remote, errors


def file_facts(path):
    with open(path, "rb") as f:
        data = f.read()
    return hashlib.sha256(data).hexdigest(), len(data), data.split(b"\n", 1)[0].rstrip(b"\r").decode("utf-8", "replace")


class Schema:
    def __init__(self, root):
        s = load_json(os.path.join(root, SCHEMA))
        self.top_required = s["required"]
        self.top_props = s["properties"]
        self.entry = s["$defs"]["entry"]
        self.dependency = s["$defs"]["dependency"]

    def enum(self, field):
        return self.entry["properties"][field]["enum"]


def check_types(where, obj, spec, problems):
    """The required, additionalProperties, type, enum, const and pattern rules of one schema object."""
    if not isinstance(obj, dict):
        problems.append(f"{where}: not an object")
        return False
    for field in spec.get("required", []):
        if field not in obj:
            problems.append(f"{where}: missing required field {field!r}")
    props = spec.get("properties", {})
    for field, v in obj.items():
        if field not in props:
            problems.append(f"{where}: unknown field {field!r}")
            continue
        p, ok = props[field], True
        if "const" in p:
            ok = v == p["const"] and type(v) is type(p["const"])
        elif "enum" in p:
            ok = isinstance(v, str) and v in p["enum"]
        elif "type" in p:
            types = p["type"] if isinstance(p["type"], list) else [p["type"]]
            ok = any({"string": isinstance(v, str), "integer": type(v) is int, "object": isinstance(v, dict),
                      "array": isinstance(v, list), "null": v is None}[t] for t in types)
            if ok and isinstance(v, str):
                ok = len(v) >= p.get("minLength", 0) and re.search(p.get("pattern", ""), v) is not None
            if ok and type(v) is int:
                ok = v >= p.get("minimum", v)
            if ok and isinstance(v, list) and "items" in p and "$ref" not in p["items"]:
                item = p["items"]
                ok = all((isinstance(x, str) and len(x) >= item.get("minLength", 0)
                          and re.search(item.get("pattern", ""), x) is not None) for x in v)
                ok = ok and (not p.get("uniqueItems") or len(set(v)) == len(v))
        if not ok:
            problems.append(f"{where}: field {field!r} has a value the schema does not allow: {v!r}")
    return True


def check_entry(root, schema, manifest, e, keys, problems):
    where = f"{manifest}: {e.get('id', '?') if isinstance(e, dict) else '?'}"
    if not check_types(where, e, schema.entry, problems):
        return
    if "provenance" in e and isinstance(e["provenance"], dict):
        check_types(where + " provenance", e["provenance"], schema.entry["properties"]["provenance"], problems)
    if "expected" in e and isinstance(e["expected"], dict):
        check_types(where + " expected", e["expected"], schema.entry["properties"]["expected"], problems)
        console = e["expected"].get("console", [])
        if not (isinstance(console, list) and all(isinstance(x, str) for x in console)):
            problems.append(f"{where}: expected.console is not a list of strings")
    path = e.get("path")
    if not isinstance(path, str):
        return
    bad = safe_path(root, path, keys)
    if bad:
        problems.append(f"{where}: path {path}: {bad}")
        return
    full = os.path.join(root, path)
    if not os.path.isfile(full):
        problems.append(f"{where}: path {path} is not a file")
        return
    sha, size, first = file_facts(full)
    if e.get("sha256") != sha:
        problems.append(f"{where}: sha256 {e.get('sha256')} != file {sha}")
    if e.get("bytes") != size:
        problems.append(f"{where}: bytes {e.get('bytes')} != file {size}")
    if e.get("header") != first:
        problems.append(f"{where}: header {e.get('header')!r} != first line {first!r}")
    want = HEADERS.get(e.get("standard"))
    if want and not (isinstance(e.get("header"), str) and (e["header"] == want or e["header"].startswith(want + " "))):
        problems.append(f"{where}: header {e.get('header')!r} is not the {e.get('standard')} header {want!r}")

    cls, tags = e.get("class"), e.get("tags", [])
    tags = tags if isinstance(tags, list) else []
    standard = e.get("expected", {}).get("standard") if isinstance(e.get("expected"), dict) else None
    if cls == "ISO_VERIFIED":
        problems.append(f"{where}: ISO_VERIFIED is not allowed: no fixture is promoted (README.md, promotion rule)")
    if e.get("private") is not False:
        problems.append(f"{where}: private must be false")
    if e.get("origin") == "private":
        problems.append(f"{where}: origin private is not allowed in a manifest")
    if e.get("license_record") == "none" and e.get("origin") != "project-authored":
        problems.append(f"{where}: origin {e.get('origin')} needs a license_record")
    if manifest == "iso-vrml97":
        if e.get("standard") != "VRML97":
            problems.append(f"{where}: iso-vrml97 holds VRML97 entries only")
        if cls != "ISO_REVIEW_NEEDED":
            problems.append(f"{where}: iso-vrml97 entries are ISO_REVIEW_NEEDED, not {cls}")
        if not e.get("spec_citations"):
            problems.append(f"{where}: an ISO candidate needs spec_citations")
        if standard not in ("valid", "invalid"):
            problems.append(f"{where}: an ISO candidate states expected.standard valid or invalid")
    if manifest == "regression" and cls != "REGRESSION_ONLY":
        problems.append(f"{where}: regression entries are REGRESSION_ONLY, not {cls}")
    if "behavior" in tags and (cls != "REGRESSION_ONLY" or standard != "unstated"):
        problems.append(f"{where}: tag behavior needs class REGRESSION_ONLY and expected.standard unstated")
    if ("negative" in tags) != ("/negative/" in path):
        problems.append(f"{where}: tag negative and a /negative/ folder go together")
    if "negative" in tags and standard != "invalid":
        problems.append(f"{where}: a negative fixture states expected.standard invalid")
    if path.startswith(LIBRARY) and e.get("origin") != "project-authored":
        problems.append(f"{where}: files of the library are project-authored")
    ref = e.get("reference")
    if isinstance(ref, str) and ref != "none":
        bad = safe_path(root, ref, keys)
        if bad or not os.path.exists(os.path.join(root, ref)):
            problems.append(f"{where}: reference {ref}: {bad or 'does not exist'}")

    deps = e.get("dependencies")
    if not isinstance(deps, list):
        return
    present, absent, remote = {}, set(), set()
    for d in deps:
        dw = f"{where} dependency {d!r}"
        if not check_types(dw, d, schema.dependency, problems):
            continue
        if ("path" in d) == ("url" in d):
            problems.append(f"{dw}: needs exactly one of path and url")
        elif d.get("absent"):
            if "sha256" in d or "bytes" in d:
                problems.append(f"{dw}: an absent dependency has no sha256 or bytes")
            if "path" in d:
                absent.add(d["path"])
            else:
                remote.add(d["url"])
        elif "url" in d:
            problems.append(f"{dw}: a url dependency must be absent")
        elif "sha256" not in d or "bytes" not in d:
            problems.append(f"{dw}: a present dependency needs sha256 and bytes")
        else:
            bad = safe_path(root, d["path"], keys)
            if bad:
                problems.append(f"{dw}: {bad}")
                continue
            if not os.path.isfile(os.path.join(root, d["path"])):
                problems.append(f"{dw}: does not exist; mark it absent")
                continue
            sha, size, _ = file_facts(os.path.join(root, d["path"]))
            if (d["sha256"], d["bytes"]) != (sha, size):
                problems.append(f"{dw}: sha256/bytes != file {sha} {size}")
            present[d["path"]] = d
    want_present, want_absent, want_remote, errors = scan_dependencies(root, path)
    problems += [f"{where}: {x}" for x in errors]
    for p in sorted(want_present - set(present)):
        problems.append(f"{where}: loads {p}, which is not in dependencies")
    for p in sorted(set(present) - want_present):
        problems.append(f"{where}: dependency {p} is not loaded by the scene")
    for p in sorted(want_absent ^ absent):
        problems.append(f"{where}: absent dependency {p}: " +
                        ("named by the scene but not declared" if p in want_absent else "not named by the scene"))
    for p in sorted(absent):
        if os.path.lexists(os.path.join(root, p)):
            problems.append(f"{where}: absent dependency {p} exists")
    for u in sorted(want_remote ^ remote):
        problems.append(f"{where}: .invalid URL {u}: " +
                        ("named by the scene but not declared" if u in want_remote else "not named by the scene"))


def check(root=ROOT, manifests=None):
    """The problems found, as text lines, and the number of entries checked."""
    problems, keys, schema = [], answer_keys(root), Schema(root)
    # The scene coverage check needs every manifest: with named manifests it would report the scenes of the others.
    full_set = manifests is None
    if full_set:
        d = os.path.join(root, MANIFESTS)
        manifests = sorted(os.path.join(d, f) for f in os.listdir(d) if f.endswith(".json"))
    owner, count = {}, 0
    for m in manifests:
        name = os.path.basename(m)[:-5]
        try:
            doc = load_json(m)
        except (OSError, ValueError) as e:
            problems.append(f"{name}: cannot read: {e}")
            continue
        check_types(name, doc, {"required": schema.top_required, "properties": schema.top_props}, problems)
        if isinstance(doc, dict) and doc.get("manifest") != name:
            problems.append(f"{name}: manifest field {doc.get('manifest')!r} != file name")
        entries = doc.get("entries", []) if isinstance(doc, dict) else []
        entries = entries if isinstance(entries, list) else []
        ids = [e.get("id") for e in entries if isinstance(e, dict)]
        for i in sorted({i for i in ids if ids.count(i) > 1}, key=str):
            problems.append(f"{name}: id {i} appears more than once")
        for e in entries:
            count += 1
            check_entry(root, schema, name, e, keys, problems)
            if not isinstance(e, dict):
                continue
            neg = e.get("negative_of")
            if isinstance(neg, str) and neg not in ids:
                problems.append(f"{name}: {e.get('id')}: negative_of {neg} is not an id of this manifest")
            p = e.get("path")
            if isinstance(p, str):
                if p in owner:
                    problems.append(f"{name}: path {p} is also in {owner[p]}")
                owner[p] = name
    if full_set:
        for d, _, files in os.walk(os.path.join(root, LIBRARY)):
            for f in files:
                rel = os.path.relpath(os.path.join(d, f), root).replace(os.sep, "/")
                if f.endswith(SCENE_EXTENSIONS) and rel not in owner:
                    problems.append(f"{rel}: a library scene in no manifest")
    return problems, count


def main(argv):
    if not argv or argv[0] != "check":
        print(__doc__.split("\n\n", 1)[0], file=sys.stderr)
        return 2
    problems, count = check(ROOT, [os.path.abspath(a) for a in argv[1:]] or None)
    for p in problems:
        print(p)
    if problems:
        print(f"standards_manifest: {len(problems)} problem(s) in {count} entries", file=sys.stderr)
        return 1
    print(f"standards_manifest: {count} entries OK")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
