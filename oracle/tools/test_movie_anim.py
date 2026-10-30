#!/usr/bin/env python3
"""Tests of oracle/tools/movie_anim.py write_baseline (PR #190 review).

    python3 -I -B oracle/tools/test_movie_anim.py

The case builds the run 1 output of 2 targets in a temporary directory, lets the second target exist already, and
expects the tool to stop before it writes anything."""
import importlib.util, os, tempfile, unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("movie_anim", os.path.join(HERE, "movie_anim.py"))
ma = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ma)

CASES = [("movie-anim", "oracle/s/movie-anim.wrl", "oracle/s/movie-anim.json")]
MSAA = ["0", "4"]


class Case(unittest.TestCase):
    def test_an_existing_target_writes_nothing(self):
        with tempfile.TemporaryDirectory() as d:
            tmp, out_dir = os.path.join(d, "tmp"), os.path.join(d, "baseline")
            for m in MSAA:
                os.makedirs(os.path.join(tmp, "run1", f"movie-anim-msaa{m}", "js"))
                open(os.path.join(tmp, "run1", f"movie-anim-msaa{m}", "js.json"), "w").write("{}")
            existing = os.path.join(out_dir, "movie-anim-msaa4")
            os.makedirs(existing)
            open(os.path.join(existing, "js.json"), "w").write("old")

            with self.assertRaises(SystemExit) as e:
                ma.write_baseline(tmp, out_dir, CASES, MSAA, {})

            self.assertIn("movie-anim-msaa4 exists", str(e.exception.code))
            self.assertEqual(sorted(os.listdir(out_dir)), ["movie-anim-msaa4"])
            self.assertEqual(os.listdir(existing), ["js.json"])
            self.assertEqual(open(os.path.join(existing, "js.json")).read(), "old")

            # The same case with no existing target writes the whole baseline.
            out_new = os.path.join(d, "new")
            ma.write_baseline(tmp, out_new, CASES, MSAA, {})
            self.assertEqual(sorted(os.listdir(out_new)),
                             ["cases.txt", "movie-anim-msaa0", "movie-anim-msaa4", "recordings.json"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
