#!/usr/bin/env python3
"""Which CI suites a pull request needs: python3 -I -B tools/ci_changes.py [--all | --self-test]

Reads the changed paths of the checked-out pull request merge commit (`git diff HEAD^1 HEAD`; the checkout needs
fetch-depth 2) and prints `rust=true|false` and `web=true|false`, appended to $GITHUB_OUTPUT when it is set, with a
table of the reasons in $GITHUB_STEP_SUMMARY. --all prints both true (workflow_dispatch). --self-test checks the
rules on synthetic paths against this checkout's crate manifests.

A suite is left out only when every changed path is known not to reach it. Any other path, an empty diff, a HEAD
that is not a merge commit, or a git failure runs both suites.

- Documentation: `docs/**`, and `*.md` outside `oracle/`, `upstream/`, `tools/` and `.github/` (pinned references
  and the PR template hold Markdown). No test reads these files. Neither suite runs.
- Browser only (the Rust suite is left out): the browser tests and npm files of crates/x3d-web, the page adapter
  `crates/x3d-web/web/*.mjs` (not `web/example/`, which a native test includes), the browser baselines
  `oracle/web-browser-*` and their X_ITE recorders `oracle/x_ite-web-*`, and web-tests.yml.
- Native only (the browser suite is left out): a workspace crate that crates/x3d-web does not build with (its
  normal, build and target dependencies, followed through path dependencies), the other `oracle/` and `tools/`
  files except tools/build-web.sh, the REA ledger trees `_batch*_context/`, and the other workflows. The browser
  tests read only crates/x3d-web and `oracle/web-browser-*`.

This script and its workflows are not in either list, so a change to them runs both suites."""
import os, subprocess, sys, tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WEB_CRATE = "x3d-web"
PROTECTED_MD = ("oracle/", "upstream/", "tools/", ".github/")

WEB_ONLY = (
    "crates/x3d-web/tests/browser/",
    "oracle/web-browser-",
    "oracle/x_ite-web-",
)
WEB_ONLY_FILES = {
    "crates/x3d-web/package.json",
    "crates/x3d-web/package-lock.json",
    ".github/workflows/web-tests.yml",
}
NATIVE_ONLY_FILES = {
    ".github/workflows/rust-tests.yml",
    ".github/workflows/code-review.yml",
}
BOTH_FILES = {"tools/build-web.sh", "tools/ci_changes.py"}


def crate_dirs(root):
    """Package name -> crate directory (relative, e.g. crates/x3d-web), for each workspace crate."""
    out = {}
    for manifest in sorted((root / "crates").glob("*/Cargo.toml")):
        with open(manifest, "rb") as f:
            out[tomllib.load(f)["package"]["name"]] = manifest.parent.relative_to(root).as_posix()
    return out


def path_deps(root, crate_dir):
    """The crate directories this crate builds with: path dependencies of [dependencies], [build-dependencies]
    and their [target.*] forms. Dev-dependencies are not built into the crate."""
    with open(root / crate_dir / "Cargo.toml", "rb") as f:
        manifest = tomllib.load(f)
    with open(root / "Cargo.toml", "rb") as f:
        workspace = tomllib.load(f).get("workspace", {}).get("dependencies", {})
    tables = [manifest.get("dependencies", {}), manifest.get("build-dependencies", {})]
    for target in manifest.get("target", {}).values():
        tables += [target.get("dependencies", {}), target.get("build-dependencies", {})]
    out = set()
    for table in tables:
        for name, spec in table.items():
            if isinstance(spec, dict) and spec.get("workspace"):
                spec, base = workspace.get(name, {}), root
            else:
                base = root / crate_dir
            if isinstance(spec, dict) and "path" in spec:
                out.add(os.path.relpath(os.path.normpath(base / spec["path"]), root).replace(os.sep, "/"))
    return out


def web_closure(root):
    """The crate directories crates/x3d-web builds with, itself included."""
    dirs = crate_dirs(root)
    todo, seen = [dirs[WEB_CRATE]], set()
    while todo:
        d = todo.pop()
        if d not in seen:
            seen.add(d)
            todo += path_deps(root, d)
    return seen


def is_docs(p):
    return p.startswith("docs/") or (p.endswith(".md") and not p.startswith(PROTECTED_MD))


def is_web_only(p):
    return p in WEB_ONLY_FILES or p.startswith(WEB_ONLY) or (
        p.startswith("crates/x3d-web/web/") and p.count("/") == 3 and p.endswith(".mjs"))


def is_native_only(p, crates, closure):
    if p in BOTH_FILES:
        return False
    parts = p.split("/")
    if p in NATIVE_ONLY_FILES or p.startswith(("oracle/", "tools/")) or (
            len(parts) > 1 and parts[0].startswith("_batch") and parts[0].endswith("_context")):
        return True
    if len(parts) > 2 and parts[0] == "crates":
        d = "/".join(parts[:2])
        return d in crates and d not in closure
    return False


def classify(paths, root=ROOT):
    """(rust, web, rows): rows are (path, suites) for the summary."""
    if not paths:
        return True, True, [("(no changed paths)", "both")]
    crates = set(crate_dirs(root).values())
    closure = web_closure(root)
    rust = web = False
    rows = []
    for p in paths:
        if is_docs(p):
            rows.append((p, "none (documentation)"))
        elif is_web_only(p):
            web = True
            rows.append((p, "browser"))
        elif is_native_only(p, crates, closure):
            rust = True
            rows.append((p, "Rust"))
        else:
            rust = web = True
            rows.append((p, "both"))
    return rust, web, rows


def changed_paths():
    """The paths the merge commit HEAD changes against its first parent, or None when that is not known."""
    def git(*args):
        return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, check=True).stdout
    try:
        git("rev-parse", "--verify", "--quiet", "HEAD^2")
        out = git("diff", "--no-renames", "--name-only", "-z", "HEAD^1", "HEAD")
    except (subprocess.CalledProcessError, OSError) as e:
        print(f"ci_changes: no merge commit diff ({e}); running both suites", file=sys.stderr)
        return None
    return [p for p in out.decode("utf-8", "surrogateescape").split("\0") if p]


def report(rust, web, rows):
    lines = [f"rust={'true' if rust else 'false'}", f"web={'true' if web else 'false'}"]
    print("\n".join(lines))
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as f:
            f.write("\n".join(lines) + "\n")
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as f:
            f.write(f"### Changed paths: {len(rows)}\n\nRust suite: **{'run' if rust else 'not needed'}**; "
                    f"browser suite: **{'run' if web else 'not needed'}**\n\n| path | needs |\n| --- | --- |\n")
            for p, s in rows[:200]:
                f.write(f"| `{p}` | {s} |\n")
            if len(rows) > 200:
                f.write(f"| ... {len(rows) - 200} more | |\n")


def self_test():
    closure = web_closure(ROOT)
    cases = [
        ([], True, True),
        (["README.md"], False, False),
        (["docs/agents/lane-reports.md", "crates/x3d-web/web/API.md"], False, False),
        (["docs/findings-xr-c04/hsl_count.py"], False, False),
        (["oracle/cm5-build/README.md"], True, False),
        (["oracle/x_ite-pin-16.6.0/README.md"], True, False),
        (["tools/candidate.lock"], True, False),
        (["oracle/README.md"], True, False),
        ([".github/pull_request_template.md"], True, True),
        (["crates/cm-core/src/lib.rs"], True, False),
        (["crates/vrml97/src/lib.rs"], True, False),
        (["crates/x3d-oracle/src/main.rs"], True, False),
        (["crates/x3d-render/src/lib.rs"], True, True),
        (["crates/x3d-fields/src/lib.rs"], True, True),
        (["crates/x3d-web/src/wasm.rs"], True, True),
        (["crates/x3d-web/web/example/box.wrl"], True, True),
        (["crates/x3d-web/tests/session.rs"], True, True),
        (["crates/x3d-web/tests/browser/runtime02_phone.test.mjs"], False, True),
        (["crates/x3d-web/web/x_ite_r.mjs"], False, True),
        (["crates/x3d-web/package-lock.json"], False, True),
        (["oracle/web-browser-02-baseline/runtime02-baseline.json"], False, True),
        (["oracle/x_ite-web-runtime-02.mjs"], False, True),
        ([".github/workflows/web-tests.yml"], False, True),
        ([".github/workflows/rust-tests.yml"], True, False),
        (["Cargo.lock"], True, True),
        (["Cargo.toml"], True, True),
        (["tools/build-web.sh"], True, True),
        (["tools/ci_changes.py"], True, True),
        ([".github/workflows/new.yml"], True, True),
        (["crates/new-crate/src/lib.rs"], True, True),
        (["upstream"], True, True),
        (["_batch09_context/ledger/ledger.json", "_batch09_context/tools/verify_ledger.py"], True, False),
        (["_batch09_context"], True, True),
        (["README.md", "crates/cm-core/src/lib.rs", "crates/x3d-web/tests/browser/api.test.mjs"], True, True),
    ]
    bad = 0
    for paths, rust, web in cases:
        got = classify(paths)[:2]
        if got != (rust, web):
            bad += 1
            print(f"FAIL {paths}: rust, web = {got}, expected {(rust, web)}", file=sys.stderr)
    for d in ("crates/x3d-web", "crates/x3d-render", "crates/x3d-viewer", "crates/x3d-fields"):
        if d not in closure:
            bad += 1
            print(f"FAIL {d} is not in the x3d-web closure {sorted(closure)}", file=sys.stderr)
    print(f"ci_changes self-test: {len(cases)} cases, {bad} failed; x3d-web builds with {', '.join(sorted(closure))}")
    return 1 if bad else 0


def main(argv):
    if argv == ["--self-test"]:
        return self_test()
    if argv == ["--all"]:
        report(True, True, [("(workflow_dispatch: every suite)", "both")])
        return 0
    if argv:
        print(__doc__.split("\n\n")[0], file=sys.stderr)
        return 1
    paths = changed_paths()
    if paths is None:
        report(True, True, [("(unknown change set)", "both")])
        return 0
    report(*classify(paths))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
