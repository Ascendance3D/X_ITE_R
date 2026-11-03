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
- Fast checks only (FAST_ONLY_FILES, FAST_ONLY_DIRS): repository metadata (`.gitignore`, `LICENSE`, the PR and issue
  templates, the actionlint config), `oracle/README.md`, the tools that only "Fast checks" runs or that nothing in a pull
  request runs (the answer-key resolver and its tests, the reference-pin and candidate gates, `nightly.sh`,
  `check_gpu_test_lists.py`, `test_ci_changes.py`, `gpu_evidence_allow.py`, `strict-suite.sh`, `fetch-dxc.sh`, `package-desktop.sh`,
  `x3d-view-launch.sh`, `audio-smoke.sh`, `wayland-smoke.sh`), the GPU workflow with its preflight action (they run on
  master and by hand only), and the trees `ledger/` (the REA coverage ledger, its snapshots, history and tools) and
  `evidence/` (committed lane logs). No Rust or browser test reads either tree; only comments name `evidence/`.
  `tools/test_ci_changes.py` pins each entry. Each is named; no Rust or
  browser test reads it. Neither suite runs. `.gitattributes` is not in the list: it changes checked-out bytes.
- Browser only (the Rust suite is left out): the browser tests and npm files of crates/x3d-web (not the worlds in
  `tests/browser/worlds/`, which a native test includes), the page adapter
  `crates/x3d-web/web/*.mjs` (not `web/example/`, which a native test includes), the browser baselines
  `oracle/web-browser-*` and their X_ITE recorders `oracle/x_ite-web-*`, web-tests.yml and its browser-deps action.
- Native only (the browser suite is left out): a workspace crate that crates/x3d-web does not build with (its
  normal, build and target dependencies, followed through path dependencies), the other `oracle/` and `tools/`
  files except tools/build-web.sh and the synthetic Script worlds `oracle/synthetic-x6*`, `oracle/synthetic-inline-scripts*`,
  `oracle/synthetic-externproto*` and `oracle/synthetic-followers*` (both suites read them), and the other workflows. The browser tests read crates/x3d-web,
  `oracle/web-browser-*` and the synthetic Script worlds named above.

This script and its workflows are not in either list, so a change to them runs both suites.

A draft pull request (DRAFT=true in the environment) runs neither suite: "Fast checks" and the result checks still
run. Mark the PR ready for review to run the suites (the ready_for_review event starts a new run), or start them on the
branch with `gh workflow run "Rust tests" --ref <branch>` and `gh workflow run "Web tests" --ref <branch>`."""
import os, subprocess, sys, tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WEB_CRATE = "x3d-web"
PROTECTED_MD = ("oracle/", "upstream/", "tools/", ".github/")

WEB_ONLY = (
    "crates/x3d-web/tests/browser/",
    "oracle/web-browser-",
    "oracle/x_ite-web-",
    ".github/actions/browser-deps/",
)
# Browser test files that a native test also reads (crates/x3d-web/tests/async_loading.rs includes a texture).
WEB_NATIVE_FIXTURES = ("crates/x3d-web/tests/browser/worlds/",)
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
# Oracle files that a native test and a browser test both read: the synthetic Script worlds and their X_ITE recording
# (crates/x3d-script's tests and crates/x3d-web/tests/browser/script01.test.mjs).
BOTH_PREFIXES = (
    "oracle/synthetic-x6/",
    "oracle/synthetic-x6-baseline.json",
    "oracle/synthetic-inline-scripts/",
    "oracle/synthetic-inline-scripts-baseline.json",
    "oracle/synthetic-externproto/",
    "oracle/synthetic-externproto-baseline.json",
    "oracle/synthetic-followers/",
    "oracle/synthetic-followers-baseline.json",
)
# Read by no Rust or browser test. "Fast checks" runs the checks that read the tools (tools/test_answer_keys.py,
# tools/verify-reference-pin.py, tools/verify-candidate.sh, tools/selftest-identity.sh). Exact names only.
FAST_ONLY_FILES = {
    ".gitignore",
    ".git-blame-ignore-revs",
    "LICENSE",
    ".coderabbit.yaml",
    ".github/CODEOWNERS",
    ".github/pull_request_template.md",
    "oracle/README.md",
    "tools/answer_keys.py",
    "tools/test_answer_keys.py",
    "tools/verify-reference-pin.py",
    "tools/reference-digest.py",
    "tools/verify-candidate.sh",
    "tools/selftest-identity.sh",
    "tools/candidate.lock",
    "tools/nightly.sh",
    # Issue #180. check_gpu_test_lists.py is a "Fast checks" step. The files below are run by no pull request job:
    # gpu-tests.yml (push to master and workflow_dispatch only; "Fast checks" lints it with actionlint and
    # check_gpu_test_lists.py reads it), its preflight action, the actionlint config, and the helper scripts that only
    # a person or gpu-tests.yml starts. No Rust or browser test reads any of them.
    "tools/check_gpu_test_lists.py",
    "tools/test_ci_changes.py",
    "tools/gpu_evidence_allow.py",
    "tools/strict-suite.sh",
    "tools/fetch-dxc.sh",
    "tools/package-desktop.sh",
    "tools/x3d-view-launch.sh",
    "tools/audio-smoke.sh",
    "tools/wayland-smoke.sh",
    ".github/actionlint.yaml",
    ".github/workflows/gpu-tests.yml",
    ".github/workflows/macos-tests.yml",
}
# ledger/ and evidence/: no Rust or browser test, workflow or tool opens a file under them (searched crates/, oracle/tools,
# tools/, .github/ and the oracle recorders); only comments name evidence/. tools/test_ci_changes.py pins both searches.
FAST_ONLY_DIRS = (".github/ISSUE_TEMPLATE/", ".github/actions/runner-preflight/", "ledger/", "evidence/")


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


def is_fast_only(p):
    return p in FAST_ONLY_FILES or p.startswith(FAST_ONLY_DIRS)


def is_web_only(p):
    if p.startswith(WEB_NATIVE_FIXTURES):
        return False
    return p in WEB_ONLY_FILES or p.startswith(WEB_ONLY) or (
        p.startswith("crates/x3d-web/web/") and p.count("/") == 3 and p.endswith(".mjs"))


def is_native_only(p, crates, closure):
    if p in BOTH_FILES or p.startswith(BOTH_PREFIXES):
        return False
    parts = p.split("/")
    # ledger/ is Fast-checks-only (FAST_ONLY_DIRS, tested first); this clause is the rule before issue #180 moved it, kept so
    # that tools/test_ci_changes.py can remove the new entries and compare with the old classes.
    if p in NATIVE_ONLY_FILES or p.startswith(("oracle/", "tools/")) or (
            len(parts) > 1 and parts[0] == "ledger"):
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
        elif is_fast_only(p):
            rows.append((p, "none (Fast checks only)"))
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
        (["oracle/x_ite-pin-16.6.0/README.md"], True, False),
        (["tools/candidate.lock"], False, False),
        (["oracle/README.md"], False, False),
        ([".github/pull_request_template.md"], False, False),
        ([".gitignore"], False, False),
        ([".gitignore", "tools/answer_keys.py", "tools/test_answer_keys.py", "docs/agents/safe-checkout.md"], False, False),
        (["LICENSE", ".git-blame-ignore-revs", ".coderabbit.yaml", ".github/CODEOWNERS"], False, False),
        ([".github/ISSUE_TEMPLATE/bug.yml"], False, False),
        (["tools/verify-reference-pin.py", "tools/reference-digest.py", "tools/verify-candidate.sh"], False, False),
        (["tools/selftest-identity.sh", "tools/nightly.sh"], False, False),
        ([".gitattributes"], True, True),
        (["tools/evidence_summary.py"], True, False),
        (["oracle/tools/rea_b05.py"], True, False),
        (["oracle/x_ite-shot.mjs"], True, False),
        (["oracle/answer-keys.tsv"], True, False),
        (["oracle/x_ite-pin/README.md"], True, False),
        (["tools/answer_keys.py", "crates/vrml97/src/lib.rs"], True, False),
        ([".gitignore", "crates/x3d-web/web/x_ite_r.mjs"], False, True),
        (["tools/build-web.sh", ".gitignore"], True, True),
        (["crates/vrml97/src/lib.rs"], True, False),
        (["crates/x3d-oracle/src/main.rs"], True, False),
        (["crates/x3d-render/src/lib.rs"], True, True),
        (["crates/x3d-fields/src/lib.rs"], True, True),
        (["crates/x3d-web/src/wasm.rs"], True, True),
        (["oracle/synthetic-x6/s01-router.wrl"], True, True),
        (["oracle/synthetic-x6-baseline.json"], True, True),
        (["oracle/synthetic-inline-scripts/basic.wrl"], True, True),
        (["oracle/synthetic-inline-scripts-baseline.json"], True, True),
        (["oracle/synthetic-externproto/main-basic.wrl"], True, True),
        (["oracle/synthetic-externproto-baseline.json"], True, True),
        (["oracle/synthetic-followers/order.wrl"], True, True),
        (["oracle/synthetic-followers-baseline.json"], True, True),
        (["oracle/synthetic-inline-reload-baseline.json"], True, False),
        (["crates/x3d-web/web/example/box.wrl"], True, True),
        (["crates/x3d-web/tests/session.rs"], True, True),
        (["crates/x3d-web/tests/browser/runtime02_phone.test.mjs"], False, True),
        (["crates/x3d-web/tests/browser/worlds/tex/red.png"], True, True),
        (["crates/x3d-web/tests/browser/worlds/sensors.wrl"], True, True),
        (["crates/x3d-web/web/x_ite_r.mjs"], False, True),
        (["crates/x3d-web/package-lock.json"], False, True),
        (["oracle/web-browser-02-baseline/runtime02-baseline.json"], False, True),
        (["oracle/x_ite-web-runtime-02.mjs"], False, True),
        ([".github/workflows/web-tests.yml"], False, True),
        ([".github/actions/browser-deps/action.yml"], False, True),
        ([".github/actions/other/action.yml"], True, True),
        ([".github/actionlint.yaml", ".github/workflows/gpu-tests.yml", ".github/workflows/macos-tests.yml"], False, False),
        ([".github/actions/runner-preflight/action.yml"], False, False),
        (["tools/check_gpu_test_lists.py", "tools/test_ci_changes.py", "tools/gpu_evidence_allow.py", "tools/strict-suite.sh"], False, False),
        (["tools/fetch-dxc.sh", "tools/package-desktop.sh", "tools/x3d-view-launch.sh"], False, False),
        (["tools/evidence_summary.py"], True, False),
        ([".github/workflows/rust-tests.yml"], True, False),
        (["Cargo.lock"], True, True),
        (["Cargo.toml"], True, True),
        (["tools/build-web.sh"], True, True),
        (["tools/ci_changes.py"], True, True),
        ([".github/workflows/new.yml"], True, True),
        (["crates/new-crate/src/lib.rs"], True, True),
        (["upstream"], True, True),
        (["ledger/ledger.json", "ledger/tools/verify_ledger.py", "ledger/snapshots/before-i339/ledger.json"], False, False),
        (["ledger/history/b08-recovery/live_head.txt", "evidence/b06b08-final/gpu-identity.txt"], False, False),
        (["ledger/ledger.json", "crates/vrml97/src/lib.rs"], True, False),
        (["tools/audio-smoke.sh", "tools/wayland-smoke.sh"], False, False),
        (["ledger"], True, True),
        (["evidence"], True, True),
        (["ledgers/x.json"], True, True),
        (["evidence-x/log.txt"], True, True),
        (["README.md", "crates/vrml97/src/lib.rs", "crates/x3d-web/tests/browser/api.test.mjs"], True, True),
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
    if argv == [] and os.environ.get("DRAFT") == "true":
        report(False, False, [("(draft pull request: mark it ready for review, or use workflow_dispatch)", "none")])
        return 0
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
