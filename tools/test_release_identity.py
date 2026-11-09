#!/usr/bin/env python3
"""Tests for tools/release_identity.py: python3 -I -B tools/test_release_identity.py

Synthetic files test each rule. The last test runs the check on the tracked tree at HEAD, so a page that names the wrong
release fails here before a package build. PR #521 review: a label of the other channel at the same version passed, and
the md_page.py help example named a fixed release."""
import subprocess, sys, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import release_identity as r

WEB = r.WEB
ROOT = Path(__file__).resolve().parent.parent


def tree(version="0.2.0", title="Canary 0.2.0", tag=None, label=None, extra=None):
    """The smallest file set that passes for `title`; `tag`/`label` replace the start page labels."""
    tag = tag or title
    label = label or title
    files = {
        f"{WEB}/x_ite_r.mjs": f'   get version () {{ return "{version}"; }}\n',
        "crates/x3d-viewer/Cargo.toml": f'[package]\nname = "x3d-viewer"\nversion = "{version}"\n',
        f"{WEB}/preview/DEPLOY.md": f"# X_ITE_R {title}\n",
        f"{WEB}/preview/COMPATIBILITY.md": f'# X_ITE_R {title} — compatibility\n\n- `browser.version` is `"{version}"`.\n',
        f"{WEB}/preview/root-index.html": (
            f'<meta name="description" content="X_ITE_R {title}: the Rust port">\n<title>X_ITE_R {title}</title>\n'
            f'<span class="tag">{tag}</span>\n<p class="label">{label}</p>\n<dd class="mono" id="f-version">{version}</dd>\n'),
    }
    files.update(extra or {})
    return files


class Parse(unittest.TestCase):
    def test_ids(self):
        self.assertEqual(r.parse("canary-0.2.0"), ("canary", "0.2.0", "x_ite_r-canary-0.2.0", "Canary 0.2.0"))
        self.assertEqual(r.parse("browser-preview-0.1.0"),
                         ("browser-preview", "0.1.0", "x_ite_r-browser-preview-0.1.0", "Browser Preview 0.1.0"))
        self.assertEqual(r.parse("canary-10.0.12")[1], "10.0.12")

    def test_bad_ids(self):
        for bad in ("", "canary-02", "canary-0.2", "beta-0.2.0", "canary-01.2.0", "canary-0.2.0-rc1", "Canary-0.2.0",
                    "browser-preview0.1.0", " canary-0.2.0"):
            with self.assertRaises(ValueError, msg=bad):
                r.parse(bad)

    def test_cli(self):
        run = lambda *a: subprocess.run([sys.executable, "-I", "-B", str(ROOT / "tools/release_identity.py"), *a],
                                        capture_output=True, text=True)
        ok = run("parse", "canary-0.2.0")
        self.assertEqual((ok.returncode, ok.stdout), (0, "x_ite_r-canary-0.2.0\t0.2.0\tCanary 0.2.0\n"))
        self.assertEqual(run("parse", "canary-02").returncode, 2)
        self.assertEqual(run("check", "beta-0.2.0", "HEAD").returncode, 2)
        self.assertEqual(run().returncode, 2)


class Check(unittest.TestCase):
    def test_consistent_tree_passes(self):
        self.assertEqual(r.check("canary-0.2.0", tree()), [])
        self.assertEqual(r.check("browser-preview-0.3.0",
                                 tree("0.3.0", "Browser Preview 0.3.0", tag="Alpha 0.3.0")), [])

    def test_other_version_fails(self):
        errors = r.check("canary-0.3.0", tree())
        self.assertEqual(len(errors), 13)  # 7 identity strings; 6 labels: 4 on the start page, 2 Markdown H1
        self.assertTrue(any("x_ite_r.mjs" in e for e in errors))
        self.assertTrue(any("x3d-viewer/Cargo.toml" in e for e in errors))

    def test_label_of_other_channel_fails(self):
        # The review case: a Browser Preview release whose copied page still says "Canary" at the same version.
        files = tree("0.3.0", "Browser Preview 0.3.0", tag="Canary 0.3.0")
        self.assertEqual(r.check("browser-preview-0.3.0", files),
                         [f"{WEB}/preview/root-index.html: the label 'Canary 0.3.0' names Canary 0.3.0, not Browser Preview 0.3.0"])
        files = tree(label="Browser Preview 0.2.0")
        self.assertEqual(len(r.check("canary-0.2.0", files)), 1)
        files = tree(tag="Alpha 0.2.0")  # "Alpha" is a Browser Preview word only
        self.assertEqual(len(r.check("canary-0.2.0", files)), 1)

    def test_labels_in_every_scope(self):
        for path, text in [
            (f"{WEB}/example/index.html", '<span class="tag">Canary 0.1.0</span>'),
            (f"{WEB}/site/page.html", "<title>X_ITE_R Browser Preview 0.2.0</title>"),
            (f"{WEB}/API.md", "# X_ITE_R Canary 0.1.0 API\n"),
            ("tools/md_page.py", '      <span class="tag">Canary 0.1.0</span>\n'),
            (f"{WEB}/preview/index.html", '<p class="label">\n  Canary 0.1.0\n</p>'),
        ]:
            self.assertEqual(len(r.check("canary-0.2.0", tree(extra={path: text}))), 1, path)

    def test_running_text_may_name_other_releases(self):
        # The start page labels the 0.1.0 results in running text (PR #521 review, finding 3).
        text = '<p>Results of Browser Preview 0.1.0 (<span class="mono">11e148db</span>). Canary 0.2.0 has not run them.</p>'
        files = tree()
        files[f"{WEB}/preview/root-index.html"] += text
        files[f"{WEB}/preview/COMPATIBILITY.md"] += "These results are for Browser Preview 0.1.0.\n"
        self.assertEqual(r.check("canary-0.2.0", files), [])

    def test_md_page_template_and_help_pass(self):
        # md_page.py: the tag is an argument; the help example uses a placeholder, not a release (finding 4).
        src = (ROOT / "tools/md_page.py").read_text()
        self.assertIn('<span class="tag">{html.escape(args.tag)}</span>', src)
        self.assertNotRegex(src, r"(Alpha|Canary|Browser Preview) [0-9]+\.[0-9]+\.[0-9]+")
        self.assertEqual(r.check("canary-0.2.0", tree(extra={"tools/md_page.py": src})), [])
        self.assertEqual(r.check("canary-0.3.0", tree("0.3.0", "Canary 0.3.0", extra={"tools/md_page.py": src})), [])

    def test_missing_file_is_an_error(self):
        files = tree()
        del files[f"{WEB}/preview/DEPLOY.md"]
        self.assertEqual(r.check("canary-0.2.0", files), [f"{WEB}/preview/DEPLOY.md does not hold '# X_ITE_R Canary 0.2.0'"])


class TrackedTree(unittest.TestCase):
    """The real pages at HEAD carry the identity of the canary and of no other release."""

    @classmethod
    def setUpClass(cls):
        cls.files = r.tracked_files("HEAD", str(ROOT))

    def test_head_is_canary_0_2_0(self):
        self.assertEqual(r.check("canary-0.2.0", self.files), [])

    def test_head_is_no_other_release(self):
        self.assertTrue(r.check("canary-0.3.0", self.files))
        self.assertTrue(any("names Canary 0.2.0, not Browser Preview 0.2.0" in e
                            for e in r.check("browser-preview-0.2.0", self.files)))

    def test_smoke_check_has_no_fixed_version(self):
        # PR #521 review, finding 1: the release smoke test accepted only "X_ITE_R 0.1.0".
        smoke = (ROOT / "crates/x3d-web/tests/preview/smoke.mjs").read_text()
        self.assertNotRegex(smoke, r"X_ITE_R \d|X_ITE_R \\?[0-9]+\\\.")
        self.assertIn("versionFacts (facts, info)", smoke)


if __name__ == "__main__":
    unittest.main()
