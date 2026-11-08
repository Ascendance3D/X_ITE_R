#!/usr/bin/env python3
"""Generate THIRD_PARTY_CRATES.md: the Rust crates in the wasm32 web build, from cargo metadata.

    python3 -I -B tools/third_party_notices.py            write THIRD_PARTY_CRATES.md
    python3 -I -B tools/third_party_notices.py --check    fail if the committed file is stale

The crate set is x3d-web and its normal (not dev, not build) dependencies for wasm32-unknown-unknown, as
`cargo tree -p x3d-web --target wasm32-unknown-unknown -e normal` shows them. That command resolves the features of the
x3d-web build. `cargo metadata` alone does not: its graph joins the features of every workspace member, so it also holds
crates that only a native build uses (vp9dec, winit, rustls). Each listed package must also be in that graph. Each license text is read from the
license files in the package's own source directory (the cargo registry or vendor/). Nothing is invented: a package
without a license file is marked so. The output holds no time stamp and no absolute path, and is sorted.
Needs the registry sources (cargo fetch); the check runs offline.
"""
import hashlib
import json
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "THIRD_PARTY_CRATES.md")
TARGET = "wasm32-unknown-unknown"
ROOT_PACKAGE = "x3d-web"
FILE_RE = re.compile(r"^(licen[cs]e|copying|notice|unlicen[cs]e|patents)([^a-z].*)?$", re.I)


ASSETS = [
    ("`site/fonts/Geist-Variable.woff2`", "SIL OFL 1.1", "`site/fonts/OFL.txt`; `THIRD_PARTY_NOTICES.md`"),
    ("`site/fonts/JetBrainsMono-Variable.woff2`", "SIL OFL 1.1", "`site/fonts/OFL-JetBrainsMono.txt`; `THIRD_PARTY_NOTICES.md`"),
    ("`site/fonts/PlusJakartaSans-Variable.woff2`", "SIL OFL 1.1", "`site/fonts/OFL-PlusJakartaSans.txt`; `THIRD_PARTY_NOTICES.md`"),
    ("`pkg/fields.mjs`, `pkg/script.mjs` (X_ITE field classes, generated)", "MIT (X_ITE, Copyright (c) 2009 Holger Seelig and other contributors)",
     "header of `tools/build-web.sh` output; `THIRD_PARTY_NOTICES.md`"),
    ("`pkg/x3d_web.js` (wasm-bindgen glue) and `pkg/x3d_web_bg.wasm`", "Glue: wasm-bindgen, MIT OR Apache-2.0. Module: the crates listed below.",
     "`wasm-bindgen` entry below"),
    ("`site/exciter-logo.png`, `site/exciter-wide.png`", "UNKNOWN - needs owner review", "no license statement in the repository"),
    ("`preview/textures/*.png`, `preview/worlds/**`, `example/*`", "Newly authored for X_ITE_R (MIT, `LICENSE`)",
     "`preview/DEPLOY.md` (clean-room test worlds; no historical content)"),
    ("`x_ite_r.mjs`, `audio_mirror.mjs`, `video_mirror.mjs`, `site/*.mjs`, `site/*.css`, `preview/*`, docs", "MIT, `LICENSE`",
     "`preview/DEPLOY.md`"),
]


def metadata():
    cmd = ["cargo", "metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", TARGET]
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        sys.exit(f"third_party_notices: cargo metadata failed (exit {r.returncode}):\n{r.stderr.strip()}")
    return json.loads(r.stdout)


def tree_set():
    """(name, version) of every package in the feature-resolved wasm32 build of x3d-web."""
    cmd = ["cargo", "tree", "--locked", "--offline", "-q", "-p", ROOT_PACKAGE, "--target", TARGET, "-e", "normal",
           "--prefix", "none", "-f", "{p}"]
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        sys.exit(f"third_party_notices: cargo tree failed (exit {r.returncode}):\n{r.stderr.strip()}")
    found = set()
    for line in r.stdout.splitlines():
        parts = line.split()
        if len(parts) >= 2 and parts[1].startswith("v"):
            found.add((parts[0], parts[1][1:]))
    if not found:
        sys.exit("third_party_notices: cargo tree listed no packages")
    return found


def shipped(meta):
    """The packages of the metadata graph (normal edges from x3d-web) that the feature-resolved build has."""
    pkgs = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    start = next(i for i, p in pkgs.items() if p["name"] == ROOT_PACKAGE)
    seen, todo = set(), [start]
    while todo:
        i = todo.pop()
        if i in seen:
            continue
        seen.add(i)
        for d in nodes[i]["deps"]:
            if any(k["kind"] is None for k in d["dep_kinds"]):  # normal edge
                todo.append(d["pkg"])
    walked = [pkgs[i] for i in seen]
    tree = tree_set()
    lost = tree - {(p["name"], p["version"]) for p in walked}
    if lost:
        sys.exit(f"third_party_notices: cargo tree has packages the metadata graph does not: {sorted(lost)}")
    return [p for p in walked if (p["name"], p["version"]) in tree]


def read_text(path):
    with open(path, "rb") as f:
        t = f.read().decode("utf-8", "replace")
    return t.replace("\r\n", "\n").replace("\r", "\n").rstrip() + "\n"


def license_files(pkg):
    d = os.path.dirname(pkg["manifest_path"])
    names = {n for n in os.listdir(d) if os.path.isfile(os.path.join(d, n)) and FILE_RE.match(n)}
    lf = pkg.get("license_file")
    if lf and os.path.isfile(os.path.join(d, lf)):
        names.add(lf)
    return [(n, read_text(os.path.join(d, n))) for n in sorted(names)]


SPDX_DIR = os.path.join(ROOT, "tools", "spdx-texts")
SPDX_IDS = ["Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "MIT", "MPL-2.0", "Zlib"]


def standard_texts(expr):
    """(id, text) of the standard SPDX texts for the identifiers in a license expression."""
    ids = set(re.split(r"[^A-Za-z0-9.+-]+", expr or ""))
    return [(i, read_text(os.path.join(SPDX_DIR, i + ".txt"))) for i in SPDX_IDS if i in ids]


def fence(text):
    n = 3
    while "`" * n in text:
        n += 1
    return "`" * n


def render():
    meta = metadata()
    ws = set(meta["workspace_members"])
    allp = shipped(meta)
    first = sorted((p for p in allp if p["id"] in ws), key=lambda p: p["name"])
    third = sorted((p for p in allp if p["id"] not in ws), key=lambda p: (p["name"], p["version"]))

    texts = {}  # sha -> text
    users = defaultdict(list)  # sha -> [(pkg label, file)]
    entries = []
    for p in third:
        files = license_files(p)
        refs = []
        for name, text in files:
            sha = hashlib.sha256(text.encode()).hexdigest()
            texts[sha] = text
            users[sha].append((f"{p['name']} {p['version']}", name))
            refs.append((name, sha))
        std = []
        if not refs:
            for i, text in standard_texts(p.get("license")):
                sha = hashlib.sha256(text.encode()).hexdigest()
                texts[sha] = text
                users[sha].append((f"{p['name']} {p['version']}", f"standard SPDX {i}"))
                std.append((f"standard SPDX {i}", sha))
        entries.append((p, refs, std))
    order = sorted(texts, key=lambda s: (users[s][0][0], s))
    tid = {s: f"T{i + 1}" for i, s in enumerate(order)}

    missing = [p for p, refs, std in entries if not refs]
    nolic = [p for p in third if not p.get("license")]
    counts = Counter(p.get("license") or "(none declared)" for p in third)

    o = []
    w = o.append
    w("# Third-party Rust crates in the browser build\n")
    w("Generated by `tools/third_party_notices.py` from `cargo metadata`. Do not edit by hand; `--check` fails when this "
      "file is stale. The hand-written notices for code adapted into this repository (pl_mpeg, zvidlib-vp8, rav1d, "
      "glTF decoders, fonts) are in `THIRD_PARTY_NOTICES.md`.\n")
    w("## Scope\n")
    w(f"- Crates compiled into the `{TARGET}` build of `{ROOT_PACKAGE}`: normal dependencies only. Dev dependencies "
      "and build-only tooling are not listed. Proc-macro crates are listed because `cargo tree -e normal` lists them.")
    w(f"- {len(third)} third-party packages, and {len(first)} packages of this workspace "
      "(first party, MIT, see `LICENSE`): " + ", ".join(f"`{p['name']}`" for p in first) + ".")
    w("- License names are the SPDX expressions the packages declare. The license text is copied from the package's "
      "own license files. A package that is dual-licensed (`A OR B`) lets the user pick one.")
    w("- This file is a notice list. It is not legal advice. **Codec patent and legal review is NOT COMPLETE** "
      "(`docs/codec-licensing-inventory.md`, `docs/aac-dependency.md`, `docs/decoder-admission.md`). Listing a "
      "crate here is not a claim that any codec, patent or other right is cleared.\n")
    w("## License summary\n")
    w("| Declared license expression | Packages |")
    w("| --- | ---: |")
    for k in sorted(counts):
        w(f"| {k} | {counts[k]} |")
    w("")
    w("Packages with no license file found in their published crate source (the script looked at the top level of "
      "`<name>-<version>`): " +
      (", ".join(f"`{p['name']} {p['version']}`" for p in missing) if missing else "none") + ".")
    w("Packages with no declared license expression: " +
      (", ".join(f"`{p['name']} {p['version']}`" for p in nolic) if nolic else "none") + ".\n")
    w("## Non-crate assets in the browser package\n")
    w("Licenses below come from in-repo statements only (`crates/x3d-web/web/preview/DEPLOY.md`, "
      "`THIRD_PARTY_NOTICES.md`, `site/fonts/OFL*.txt`).\n")
    w("| Asset (package path) | License | Evidence |")
    w("| --- | --- | --- |")
    for row in ASSETS:
        w("| " + " | ".join(row) + " |")
    w("")
    w("## Packages\n")
    for p, refs, std in entries:
        w(f"### {p['name']} {p['version']}\n")
        w(f"- License: {p.get('license') or 'not declared in the manifest'}")
        w(f"- Repository: {p.get('repository') or 'not declared'}")
        if refs:
            w("- License and notice files: " + ", ".join(f"`{n}` ({tid[s]})" for n, s in refs))
        else:
            w(f"- No license file found in the published crate source at {p['name']}-{p['version']}.")
            if std:
                w("- Standard SPDX text (not from the crate; the crate has no license file and no copyright line; no "
                  "copyright holder is named here): " + ", ".join(f"{n.split()[-1]} ({tid[s]})" for n, s in std))
            else:
                w("- No standard text is stored for this license expression. UNKNOWN - needs owner review.")
        w("")
    w("## License and notice texts\n")
    w("Identical texts are listed once. NOTICE files are copied verbatim. Entries marked \"standard SPDX\" are the "
      "standard license text, stored in `tools/spdx-texts/`; they carry no copyright line.\n")
    for s in order:
        us = users[s]
        w(f"### {tid[s]}\n")
        w("Used by: " + ", ".join(f"{n} (`{f}`)" for n, f in us) + "\n")
        fc = fence(texts[s])
        w(fc + "text")
        o.append(texts[s].rstrip("\n"))
        w(fc + "\n")
    return "\n".join(o).rstrip("\n") + "\n", missing, nolic, len(third)


def main():
    if any(a != "--check" for a in sys.argv[1:]):
        print(__doc__)
        return 2
    text, missing, nolic, n = render()
    if "--check" in sys.argv[1:]:
        have = open(OUT, encoding="utf-8").read() if os.path.exists(OUT) else None
        if have != text:
            print("THIRD_PARTY_CRATES.md is stale; run: python3 -I -B tools/third_party_notices.py", file=sys.stderr)
            return 1
        print(f"THIRD_PARTY_CRATES.md is current ({n} packages)")
        return 0
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print(f"wrote THIRD_PARTY_CRATES.md: {n} packages, {len(missing)} without license file, {len(nolic)} without license")
    for p in missing:
        print(f"  no license file: {p['name']} {p['version']} ({p.get('license')})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
