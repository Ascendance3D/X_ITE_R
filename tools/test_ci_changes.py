#!/usr/bin/env python3
"""Tests for tools/ci_changes.py (issue #180): python3 -I -B tools/test_ci_changes.py

Run by "Fast checks". It pins the classifications issue #180 added, and proves that no other tracked path changed class:
the baseline is the classifier with the #180 entries removed, so every path that reached a suite before still reaches
it, and the only paths that leave a suite are the pinned ones."""
import functools, subprocess, sys, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ci_changes as c

# classify() reads the crate manifests on every call; the answers do not change within a test run.
c.crate_dirs = functools.lru_cache(maxsize=None)(c.crate_dirs)
c.web_closure = functools.lru_cache(maxsize=None)(c.web_closure)

# What issue #180 moved to "Fast checks only", and why each is safe: see the comment in ci_changes.py.
ADDED_FILES = {
    "tools/check_gpu_test_lists.py",
    "tools/test_ci_changes.py",
    "tools/gpu_evidence_allow.py",
    "tools/strict-suite.sh",
    "tools/fetch-dxc.sh",
    "tools/package-desktop.sh",
    "tools/x3d-view-launch.sh",
    ".github/actionlint.yaml",
    ".github/workflows/gpu-tests.yml",
}
ADDED_DIRS = (".github/actions/runner-preflight/",)
# Already neither-suite before #180 (issue #180 item 1 named them); pinned again so they cannot regress.
METADATA = [".gitignore", ".coderabbit.yaml", "LICENSE", ".git-blame-ignore-revs", ".github/CODEOWNERS",
            ".github/pull_request_template.md", ".github/ISSUE_TEMPLATE/bug.yml", "tools/answer_keys.py",
            "tools/test_answer_keys.py", "tools/verify-reference-pin.py"]
# Python or shell under tools/ that a suite still runs or reads: they must keep reaching it.
STILL_REACH = {
    "tools/evidence_summary.py": (True, False),  # the Rust job's "Evidence skips" step
    "tools/build-web.sh": (True, True),
    "tools/ci_changes.py": (True, True),
    ".github/workflows/code-review.yml": (True, False),  # AI review: left as it was
    ".github/workflows/rust-tests.yml": (True, False),
    ".github/workflows/web-tests.yml": (False, True),
}


def tracked():
    out = subprocess.run(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=c.ROOT, capture_output=True, check=True).stdout
    return [p for p in out.decode().split("\0") if p]


def suites(p):
    return c.classify([p])[:2]


class Classification(unittest.TestCase):
    def test_added_paths_run_no_suite(self):
        for p in sorted(ADDED_FILES) + [d + "action.yml" for d in ADDED_DIRS]:
            self.assertEqual(suites(p), (False, False), p)

    def test_metadata_runs_no_suite(self):
        for p in METADATA:
            self.assertEqual(suites(p), (False, False), p)

    def test_paths_that_reach_a_suite_still_do(self):
        for p, want in STILL_REACH.items():
            self.assertEqual(suites(p), want, p)

    def test_only_the_added_paths_changed_class(self):
        """Every tracked path outside the added set classifies as it did before #180."""
        files, dirs = set(c.FAST_ONLY_FILES), c.FAST_ONLY_DIRS
        try:
            c.FAST_ONLY_FILES = files - ADDED_FILES
            c.FAST_ONLY_DIRS = tuple(d for d in dirs if d not in ADDED_DIRS)
            before = {p: suites(p) for p in tracked()}
        finally:
            c.FAST_ONLY_FILES, c.FAST_ONLY_DIRS = files, dirs
        after = {p: suites(p) for p in tracked()}
        moved = {p for p in before if before[p] != after[p]}
        expected = {p for p in before if p in ADDED_FILES or p.startswith(ADDED_DIRS)}
        self.assertEqual(moved, expected)
        for p in moved:
            self.assertEqual(after[p], (False, False), p)

    def test_added_files_exist_and_are_not_read_by_a_suite(self):
        """An added file that is gone would hide a stale entry; a reference from crates/ would mean a test reads it."""
        tr = set(tracked())
        for p in ADDED_FILES:
            self.assertIn(p, tr, p)
        refs = subprocess.run(["git", "grep", "-l", "-E", "|".join(
            f.rsplit("/", 1)[-1] for f in ADDED_FILES if f.startswith("tools/")), "--", "crates", "oracle/tools"],
            cwd=c.ROOT, capture_output=True, text=True).stdout.split()
        # Comments that name a script are fine; a test that opens one is not. Only .rs/.mjs/.py under crates count.
        for r in (r for r in refs if r.endswith((".rs", ".mjs", ".js", ".py", ".sh", ".toml"))):
            text = (c.ROOT / r).read_text(errors="replace")
            for f in ADDED_FILES:
                if f.startswith("tools/"):
                    for line in text.splitlines():
                        if f in line:
                            self.assertTrue(line.lstrip().startswith(("//", "#", "///", "//!", "*")), f"{r}: {line}")


if __name__ == "__main__":
    unittest.main()
