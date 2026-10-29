#!/usr/bin/env python3
"""Tests of tools/answer_keys.py: the explicit store checks and the guard (issue #65). Synthetic only.

    python3 -I -B tools/test_answer_keys.py

Every fixture is a new Git repository under a temporary directory; no real key and no real checkout is read."""
import hashlib, importlib.util, os, shutil, subprocess, sys, tempfile, unittest

spec = importlib.util.spec_from_file_location("answer_keys", os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                                                          "answer_keys.py"))
ak = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ak)


def sha(b):
    return hashlib.sha256(b).hexdigest()


def git(d, *args):
    subprocess.run(["git", "-C", d, "-c", "user.name=t", "-c", "user.email=t@t", *args], check=True,
                   capture_output=True)


def put_keys(d):
    """File key k.json = "abc" and dir key d = {a: "1", b/c: "2"}."""
    os.makedirs(os.path.join(d, "d", "b"), exist_ok=True)
    for rel, text in (("k.json", "abc"), ("d/a", "1"), ("d/b/c", "2")):
        with open(os.path.join(d, rel), "w") as f:
            f.write(text)


class Case(unittest.TestCase):
    def setUp(self):
        self.tmp = os.path.realpath(tempfile.mkdtemp(prefix="answer-keys-test-"))
        self.addCleanup(shutil.rmtree, self.tmp)
        old = os.environ.pop("X3D_ORACLE_KEYS", None)
        self.addCleanup(lambda: os.environ.__setitem__("X3D_ORACLE_KEYS", old) if old is not None
                        else os.environ.pop("X3D_ORACLE_KEYS", None))

    def checkout(self, name="main", init=True):
        root = os.path.join(self.tmp, name)
        os.makedirs(os.path.join(root, "oracle"))
        listing = f"{sha(b'1')}  a\n{sha(b'2')}  b/c\n".encode()
        with open(os.path.join(root, "oracle", "answer-keys.tsv"), "w") as f:
            f.write(f"# name\tkind\tfiles\tbytes\tsha256\nk.json\tfile\t1\t3\t{sha(b'abc')}\n"
                    f"d\tdir\t2\t2\t{sha(listing)}\n")
        with open(os.path.join(root, ".gitignore"), "w") as f:
            f.write("/oracle/k.json\n/oracle/d/\n/upstream/\n")  # like the real repository's rules
        if init:
            git(root, "init", "-q")
            git(root, "add", "oracle/answer-keys.tsv", ".gitignore")
            git(root, "commit", "-qm", "m")
        return root

    def keys(self, name="keys"):
        d = os.path.join(self.tmp, name)
        put_keys(d)
        return d

    def unsafe(self, fn, text):
        with self.assertRaises(ak.KeyError_) as c:
            fn()
        self.assertEqual(c.exception.code, ak.UNSAFE, str(c.exception))
        self.assertIn(text, str(c.exception))

    def resolve(self, root, name, store):
        if store is None:
            os.environ.pop("X3D_ORACLE_KEYS", None)
        else:
            os.environ["X3D_ORACLE_KEYS"] = store
        return ak.resolve(name, root)


class Store(Case):
    def test_valid_external_root(self):
        root, keys = self.checkout(), self.keys()
        self.assertEqual(self.resolve(root, "d", keys), os.path.join(keys, "d"))
        self.assertEqual(self.resolve(root, "k.json", keys), os.path.join(keys, "k.json"))

    def test_missing_root(self):
        root = self.checkout()
        self.unsafe(lambda: self.resolve(root, "d", os.path.join(self.tmp, "nowhere")), "does not exist")

    def test_regular_file_as_root(self):
        root = self.checkout()
        f = os.path.join(self.tmp, "file")
        open(f, "w").close()
        self.unsafe(lambda: self.resolve(root, "d", f), "is not a directory")

    def test_symlink_into_a_worktree(self):
        root = self.checkout()
        put_keys(os.path.join(root, "oracle"))
        link = os.path.join(self.tmp, "link")
        os.symlink(os.path.join(root, "oracle"), link)
        self.unsafe(lambda: self.resolve(root, "d", link), "overlaps the worktree")
        entries = os.path.join(self.tmp, "entries")
        os.mkdir(entries)
        for k in ("k.json", "d"):
            os.symlink(os.path.join(root, "oracle", k), os.path.join(entries, k))
        self.unsafe(lambda: self.resolve(root, "d", entries), "inside the worktree")

    def test_inside_or_holding_a_linked_worktree(self):
        root = self.checkout()
        wt = os.path.join(self.tmp, "lanes", "wt")
        git(root, "worktree", "add", "-q", "--detach", wt)
        put_keys(os.path.join(wt, "keys"))
        self.unsafe(lambda: self.resolve(root, "d", os.path.join(wt, "keys")), "overlaps the worktree")
        put_keys(os.path.dirname(wt))
        self.unsafe(lambda: self.resolve(root, "d", os.path.dirname(wt)), "overlaps the worktree")

    def test_sibling_with_a_similar_name(self):
        root, keys = self.checkout("main"), self.keys("main-keys")
        self.assertEqual(self.resolve(root, "d", keys), os.path.join(keys, "d"))

    def test_relative_and_absolute_paths(self):
        root, keys = self.checkout(), self.keys()
        cwd = os.getcwd()
        self.addCleanup(os.chdir, cwd)
        os.chdir(self.tmp)
        self.unsafe(lambda: self.resolve(root, "d", "keys"), "not an absolute path")
        self.assertEqual(self.resolve(root, "d", os.path.join(root, "..", "keys")), os.path.join(keys, "d"))
        self.unsafe(lambda: self.resolve(root, "d", os.path.join(keys, "..", "main", "oracle")),
                    "overlaps the worktree")

    def test_missing_required_key(self):
        root, keys = self.checkout(), self.keys()
        shutil.rmtree(os.path.join(keys, "d"))
        self.unsafe(lambda: self.resolve(root, "k.json", keys), "d is absent")

    def test_manifest_name_with_a_path(self):
        for name in ("../k.json", "a/k.json", "..", "/k.json"):
            with self.assertRaises(ak.KeyError_) as c:
                ak.parse([f"{name}\tfile\t1\t3\t{sha(b'abc')}\n"])
            self.assertIn("bad row", str(c.exception))

    def test_short_or_malformed_manifest_row(self):
        for row in ("k.json", "k.json\tfile\t1", "k.json\tfile\tone\t3\tx", "k.json\tfile\t\u00b2\t3\tx"):
            with self.assertRaises(ak.KeyError_) as c:
                ak.parse([row + "\n"])
            self.assertEqual(c.exception.code, 1)
        root = self.checkout()
        with open(os.path.join(root, "oracle", "answer-keys.tsv"), "w") as f:
            f.write("k.json\n")
        with self.assertRaises(ak.KeyError_):
            ak.guard(root, ["HEAD"])

    def test_entry_linking_outside_the_store_is_measured(self):
        root, backup = self.checkout(), self.keys("backup")
        keys = os.path.join(self.tmp, "keys")
        os.mkdir(keys)
        for k in ("k.json", "d"):
            os.symlink(os.path.join(backup, k), os.path.join(keys, k))
        self.assertEqual(self.resolve(root, "d", keys), os.path.join(keys, "d"))
        with open(os.path.join(backup, "d", "a"), "w") as f:
            f.write("changed")
        with self.assertRaises(ak.KeyError_) as c:
            self.resolve(root, "d", keys)
        self.assertEqual(c.exception.code, 3)

    def test_wrong_kind(self):
        root, keys = self.checkout(), self.keys()
        os.remove(os.path.join(keys, "k.json"))
        os.mkdir(os.path.join(keys, "k.json"))
        self.unsafe(lambda: self.resolve(root, "d", keys), "k.json is not a file")

    def test_invalid_explicit_root_has_no_fallback(self):
        root = self.checkout()
        put_keys(os.path.join(root, "oracle"))
        self.assertEqual(self.resolve(root, "d", None), os.path.join(root, "oracle", "d"))
        empty = os.path.join(self.tmp, "empty")
        os.mkdir(empty)
        self.unsafe(lambda: self.resolve(root, "d", empty), "no other location is searched")

    def test_outside_a_git_checkout(self):
        root, keys = self.checkout(init=False), self.keys()
        self.unsafe(lambda: self.resolve(root, "d", keys), "cannot list the worktrees")

    def test_cli_exit_code(self):
        root = self.checkout()
        env = dict(os.environ, X3D_ORACLE_KEYS="relative")
        tool = os.path.join(root, "tools", "answer_keys.py")
        os.makedirs(os.path.dirname(tool))
        shutil.copy(ak.__file__, tool)
        r = subprocess.run([sys.executable, "-I", "-B", tool, "verify"], env=env, capture_output=True, text=True)
        self.assertEqual(r.returncode, ak.UNSAFE, r.stderr)
        self.assertIn("not an absolute path", r.stderr)


class Guard(Case):
    def linked(self):
        """HEAD tracks oracle/d and upstream as symlinks, like X_ITE_R before #179 untracked the render keys."""
        root = self.checkout()
        os.symlink("../../store/d", os.path.join(root, "oracle", "d"))
        os.symlink("../store/upstream", os.path.join(root, "upstream"))
        git(root, "add", "oracle/d", "upstream")
        git(root, "commit", "-qm", "links")
        return root

    def test_passes_for_symlinks_and_untracked_keys(self):
        root = self.linked()
        with open(os.path.join(root, "oracle", "k.json"), "w") as f:
            f.write("abc")
        ak.guard(root, ["HEAD"])

    def test_each_commit_manifest_is_read(self):
        root = self.checkout()
        tsv = os.path.join(root, "oracle", "answer-keys.tsv")
        good = open(tsv).read()
        with open(tsv, "a") as f:
            f.write("e\tdir\t0\t0\tx\n")
        git(root, "commit", "-qam", "a key e")
        git(root, "tag", "with-e")
        with open(tsv, "w") as f:
            f.write(good)
        git(root, "commit", "-qam", "no e")
        os.makedirs(os.path.join(root, "oracle", "e"))
        open(os.path.join(root, "oracle", "e", "f"), "w").close()
        ak.guard(root, ["HEAD"])
        self.unsafe(lambda: ak.guard(root, ["with-e"]), "oracle/e: real data that the ignore rules do not cover")
        with open(tsv, "w") as f:
            f.write("k.json\n")
        git(root, "commit", "-qam", "malformed")
        git(root, "tag", "bad")
        with open(tsv, "w") as f:
            f.write(good)
        with self.assertRaises(ak.KeyError_) as c:
            ak.guard(root, ["bad"])
        self.assertIn("bad row", str(c.exception))

    def test_ignored_name_that_git_would_quote(self):
        root = self.checkout()
        name = 'k "\u00e9".json'
        tsv = os.path.join(root, "oracle", "answer-keys.tsv")
        with open(tsv, "a") as f:
            f.write(f"{name}\tfile\t1\t3\t{sha(b'abc')}\n")
        with open(os.path.join(root, ".gitignore"), "a") as f:
            f.write(f"/oracle/{name}\n")
        with open(os.path.join(root, "oracle", name), "w") as f:
            f.write("abc")
        ak.guard(root, ["HEAD"])

    def test_real_data_that_is_not_ignored(self):
        root = self.checkout()
        put_keys(os.path.join(root, "oracle"))
        ak.guard(root, ["HEAD"])
        with open(os.path.join(root, ".gitignore"), "w") as f:
            f.write("/oracle/k.json\n")
        self.unsafe(lambda: ak.guard(root, ["HEAD"]), "oracle/d: real data that the ignore rules do not cover")

    def test_tracked_symlink_path_occupied_by_real_files(self):
        root = self.linked()
        os.remove(os.path.join(root, "oracle", "d"))
        put_keys(os.path.join(root, "oracle"))
        self.unsafe(lambda: ak.guard(root, ["HEAD"]), "oracle/d: real data, and HEAD tracks it as a symlink")
        os.remove(os.path.join(root, "upstream"))
        os.mkdir(os.path.join(root, "upstream"))
        self.unsafe(lambda: ak.guard(root, ["HEAD"]), "upstream: real data, and HEAD tracks it as a symlink")

    def test_tracked_parent_or_child(self):
        root = self.checkout()
        other = os.path.join(self.tmp, "wt")
        git(root, "worktree", "add", "-q", "-b", "old", other)
        shutil.rmtree(os.path.join(other, "oracle"))
        os.symlink("elsewhere", os.path.join(other, "oracle"))
        git(other, "add", "-A")
        git(other, "commit", "-qm", "oracle is a link")
        git(other, "checkout", "-q", "-b", "child", "HEAD~1")
        os.makedirs(os.path.join(other, "oracle", "d"))
        with open(os.path.join(other, "oracle", "d", "a"), "w") as f:
            f.write("1")
        git(other, "add", "-f", "oracle/d/a")
        git(other, "commit", "-qm", "a file below d")
        put_keys(os.path.join(root, "oracle"))
        ak.guard(root, ["HEAD"])
        self.unsafe(lambda: ak.guard(root, ["HEAD", "old"]),
                    "oracle/d: real data, and old tracks its parent oracle as a symlink")
        self.unsafe(lambda: ak.guard(root, ["child"]), "oracle/d: real data, and child tracks files below it")

    def test_data_below_a_symlink_is_not_real_data(self):
        real = os.path.join(self.tmp, "real")
        put_keys(os.path.join(real, "oracle"))
        root = os.path.join(self.tmp, "root")
        os.makedirs(os.path.join(root, "sub"))
        os.symlink(os.path.join(real, "oracle"), os.path.join(root, "sub", "oracle"))
        self.assertFalse(ak._real_data(root, "sub/oracle/d"))
        self.assertTrue(ak._real_data(real, "oracle/d"))

    def test_invalid_store_and_unknown_rev(self):
        root, keys = self.linked(), self.keys()
        os.environ["X3D_ORACLE_KEYS"] = keys
        ak.guard(root, ["HEAD"])
        put_keys(os.path.join(root, "store"))
        os.environ["X3D_ORACLE_KEYS"] = os.path.join(root, "store")
        self.unsafe(lambda: ak.guard(root, ["HEAD"]), "overlaps the worktree")
        os.environ.pop("X3D_ORACLE_KEYS")
        self.unsafe(lambda: ak.guard(root, ["no-such-rev"]), "not a commit")



class CheckOutput(Case):
    """A regeneration output must not write the store or a key path, directly or through a link (issue #65)."""

    def setUp(self):
        super().setUp()
        self.root, self.store = self.checkout(), self.keys()
        self.before = ak.measure(self.store)

    def tearDown(self):
        self.assertEqual(ak.measure(self.store), self.before)  # nothing here writes the store

    def passes(self, path):
        ak.check_output(path, self.root)

    def test_staging_directory_passes(self):
        self.passes(os.path.join(self.tmp, "staging", "d"))
        os.environ["X3D_ORACLE_KEYS"] = self.store
        self.passes(os.path.join(self.tmp, "staging", "d"))
        self.passes(os.path.join(self.root, "oracle", "other-baseline"))  # a tracked baseline, not a key

    def test_key_path_and_below(self):
        self.unsafe(lambda: self.passes(os.path.join(self.root, "oracle", "d")), "is the key path")
        self.unsafe(lambda: self.passes(os.path.join(self.root, "oracle", "d", "a.png")), "is below the key path")
        self.unsafe(lambda: self.passes(os.path.join(self.root, "oracle", "k.json")), "is the key path")

    def test_path_that_holds_a_key_path(self):
        self.unsafe(lambda: self.passes(os.path.join(self.root, "oracle")), "holds the key path")

    def test_link_at_a_key_path_into_the_store(self):
        os.environ["X3D_ORACLE_KEYS"] = self.store
        ak.link(self.root, ["d"], self.root)
        target = os.path.join(self.store, "d")
        self.unsafe(lambda: self.passes(os.path.join(self.root, "oracle", "d")), f"{target} is below the store")
        self.unsafe(lambda: self.passes(os.path.join(self.root, "oracle", "d", "a")), "is below the store")
        os.environ.pop("X3D_ORACLE_KEYS")  # the link target is still protected
        self.unsafe(lambda: self.passes(os.path.join(self.root, "oracle", "d", "a")), "the target of")

    def test_store_named_or_not(self):
        out = os.path.join(self.store, "d")
        os.environ["X3D_ORACLE_KEYS"] = self.store
        self.unsafe(lambda: self.passes(out), "is below the store $X3D_ORACLE_KEYS")
        os.environ.pop("X3D_ORACLE_KEYS")
        self.unsafe(lambda: self.passes(out), "holds every key")
        self.unsafe(lambda: self.passes(os.path.join(self.store, "new", "x.png")), "holds every key")

    def test_staging_symlink_into_the_store(self):
        stage = os.path.join(self.tmp, "stage")
        os.symlink(os.path.join(self.store, "d"), stage)
        self.unsafe(lambda: self.passes(os.path.join(stage, "a")), "holds every key")
        dangling = os.path.join(self.tmp, "dangling")
        os.symlink(os.path.join(self.store, "d", "new"), dangling)
        self.unsafe(lambda: self.passes(dangling), "holds every key")

    def test_unresolvable_path_is_refused(self):
        loop = os.path.join(self.tmp, "loop1")
        os.symlink(os.path.join(self.tmp, "loop2"), loop)
        os.symlink(loop, os.path.join(self.tmp, "loop2"))
        self.unsafe(lambda: self.passes(os.path.join(loop, "x.png")), "cannot resolve")
        locked = os.path.join(self.tmp, "locked")
        os.mkdir(locked)
        os.symlink(os.path.join(self.store, "d"), os.path.join(locked, "via"))
        os.chmod(locked, 0)
        self.addCleanup(os.chmod, locked, 0o700)
        if os.access(locked, os.X_OK):
            self.skipTest("running as root: a mode-0 directory is still readable")
        self.unsafe(lambda: self.passes(os.path.join(locked, "via", "x.png")), "cannot resolve")

    def test_cli_and_recorder_refuse(self):
        tool = os.path.join(self.root, "tools", "answer_keys.py")
        os.makedirs(os.path.dirname(tool))
        shutil.copy(ak.__file__, tool)
        env = {k: v for k, v in os.environ.items() if k != "X3D_ORACLE_KEYS"}
        r = subprocess.run([sys.executable, "-I", "-B", tool, "check-output", os.path.join(self.tmp, "ok"),
                            os.path.join(self.store, "d")], env=env, capture_output=True, text=True)
        self.assertEqual(r.returncode, ak.UNSAFE, r.stderr)
        self.assertIn("check-output: passed for", r.stdout)
        self.assertIn("holds every key", r.stderr)
        node = shutil.which("node")
        if node is None:
            self.skipTest("node is not installed; the recorder helper is not run")
        helper = os.path.join(self.root, "oracle", "check-output.mjs")
        shutil.copy(os.path.join(os.path.dirname(os.path.dirname(ak.__file__)), "oracle", "check-output.mjs"), helper)
        run = lambda out: subprocess.run([node, "--input-type=module", "-e",
                                          f"import {{ checkOutput }} from {helper!r}; checkOutput ({out!r}); "
                                          "console .log ('WRITE')"], env=env, capture_output=True, text=True)
        r = run(os.path.join(self.root, "oracle", "d"))
        self.assertEqual(r.returncode, ak.UNSAFE, r.stderr)
        self.assertNotIn("WRITE", r.stdout)
        self.assertIn("nothing written", r.stderr)
        r = run(os.path.join(self.tmp, "staging", "d"))
        self.assertEqual((r.returncode, r.stdout.splitlines()[-1:]), (0, ["WRITE"]), r.stderr)
        env["PATH"] = self.tmp  # no python3: the helper fails closed
        r = subprocess.run([node, "--input-type=module", "-e", f"import {{ checkOutput }} from {helper!r}; "
                            f"checkOutput ({os.path.join(self.tmp, 'staging')!r}); console .log ('WRITE')"],
                           env=env, capture_output=True, text=True)
        self.assertNotEqual(r.returncode, 0)
        self.assertNotIn("WRITE", r.stdout)

if __name__ == "__main__":
    unittest.main(verbosity=2)
