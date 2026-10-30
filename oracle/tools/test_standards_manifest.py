#!/usr/bin/env python3
"""Tests of oracle/tools/standards_manifest.py, then one check of the real manifests.

    python3 -I -B oracle/tools/test_standards_manifest.py

Each synthetic case builds a small repository tree (the real schema, an answer-key table, scenes and one manifest)
in a temporary directory, changes one thing, and expects the check to pass or to name the problem."""
import copy, hashlib, importlib.util, json, os, shutil, tempfile, unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("standards_manifest", os.path.join(HERE, "standards_manifest.py"))
sm = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sm)

LIB = "oracle/standards/iso/vrml97/inline/positive/"
MAIN = LIB + "main.wrl"
CHILD = LIB + "child.wrl"
SCENES = {
    MAIN: '#VRML V2.0 utf8\n# test\nInline { url [ "gone.wrl" "http://case.invalid/a.wrl" "child.wrl" ] }\n',
    CHILD: "#VRML V2.0 utf8\nShape { geometry Box { } }\n",
}


def facts(text):
    b = text.encode()
    return hashlib.sha256(b).hexdigest(), len(b)


def entry(path, text, **over):
    sha, size = facts(text)
    e = {
        "id": path[len("oracle/"):], "path": path, "sha256": sha, "bytes": size, "standard": "VRML97",
        "encoding": "classic", "header": text.split("\n", 1)[0], "class": "ISO_REVIEW_NEEDED",
        "x_ite_parity": "UNKNOWN", "origin": "project-authored", "license": "MIT",
        "provenance": {"author": "t", "date": "2026-10-29", "method": "test"}, "license_record": "none",
        "dependencies": [], "expected": {"standard": "valid"}, "reference": "none",
        "spec_citations": ["ISO/IEC 14772-1:1997 6.25"], "negative_of": None, "private": False,
    }
    e.update(over)
    return e


def main_entry(**over):
    sha, size = facts(SCENES[CHILD])
    deps = [{"path": CHILD, "sha256": sha, "bytes": size}, {"path": LIB + "gone.wrl", "absent": True},
            {"url": "http://case.invalid/a.wrl", "absent": True}]
    return entry(MAIN, SCENES[MAIN], dependencies=deps, **over)


class Case(unittest.TestCase):
    def setUp(self):
        self.root = os.path.realpath(tempfile.mkdtemp(prefix="standards-manifest-test-"))
        self.addCleanup(shutil.rmtree, self.root)
        os.makedirs(os.path.join(self.root, "oracle/standards/manifests"))
        shutil.copy(os.path.join(sm.ROOT, sm.SCHEMA), os.path.join(self.root, sm.SCHEMA))
        self.write("oracle/answer-keys.tsv", "# name\tkind\nkey.json\tfile\t1\t1\tx\nkeydir\tdir\t1\t1\tx\n")
        for p, t in SCENES.items():
            self.write(p, t)
        self.entries = [main_entry(), entry(CHILD, SCENES[CHILD], tags=["library"])]

    def write(self, rel, text):
        p = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w") as f:
            f.write(text)

    def manifest(self, entries, name="iso-vrml97", **over):
        doc = {"schema": "oracle/standards/MANIFEST.schema.json", "manifest": name, "description": "t",
               "entries": entries}
        doc.update(over)
        self.write(f"oracle/standards/manifests/{name}.json", json.dumps(doc))

    def problems(self, entries=None):
        if entries is not None or not os.listdir(os.path.join(self.root, "oracle/standards/manifests")):
            self.manifest(self.entries if entries is None else entries)
        return sm.check(self.root)[0]

    def assertProblem(self, text, entries=None):
        found = self.problems(entries)
        self.assertTrue(any(text in p for p in found), f"no problem with {text!r} in {found}")

    def changed(self, i=0, **over):
        out = copy.deepcopy(self.entries)
        out[i].update(over)
        return out


class Valid(Case):
    def test_valid_manifest_passes(self):
        self.assertEqual(self.problems(), [])

    def test_regression_behavior_entry_passes(self):
        self.manifest([entry(CHILD, SCENES[CHILD], **{"class": "REGRESSION_ONLY", "tags": ["behavior"],
                                                      "expected": {"standard": "unstated"}})], "regression")
        self.manifest([main_entry()])
        self.assertEqual(sm.check(self.root)[0], [])


class Hashes(Case):
    def test_sha256(self):
        self.assertProblem("sha256", self.changed(sha256="0" * 64))

    def test_bytes(self):
        self.assertProblem("bytes", self.changed(bytes=1))

    def test_dependency_hash(self):
        e = self.changed()
        e[0]["dependencies"][0]["sha256"] = "0" * 64
        self.assertProblem("sha256/bytes != file", e)


class Headers(Case):
    def test_first_line(self):
        self.assertProblem("!= first line", self.changed(header="#VRML V2.0 utf8 extra"))

    def test_standard_header(self):
        self.write(CHILD, "#X3D V4.0 utf8\nShape { }\n")
        sha, size = facts("#X3D V4.0 utf8\nShape { }\n")
        e = self.changed(1, header="#X3D V4.0 utf8", sha256=sha, bytes=size)
        e[0]["dependencies"][0].update(sha256=sha, bytes=size)
        self.assertProblem("is not the VRML97 header", e)


class PrivatePaths(Case):
    def test_answer_key_file(self):
        self.write("oracle/key.json", "#VRML V2.0 utf8\n")
        self.assertProblem("answer key oracle/key.json",
                           [entry("oracle/key.json", "#VRML V2.0 utf8\n", spec_citations=["x"])])

    def test_below_answer_key_dir(self):
        self.write("oracle/keydir/a.wrl", "#VRML V2.0 utf8\n")
        self.assertProblem("answer key oracle/keydir", [entry("oracle/keydir/a.wrl", "#VRML V2.0 utf8\n")])

    def test_symlink(self):
        os.symlink(os.path.join(self.root, CHILD), os.path.join(self.root, LIB + "link.wrl"))
        self.assertProblem("symlink", [entry(LIB + "link.wrl", SCENES[CHILD])])

    def test_dotdot(self):
        self.assertProblem("normalized", [entry(LIB + "../positive/child.wrl", SCENES[CHILD])])

    def test_outside_oracle(self):
        self.write("crates/a.wrl", SCENES[CHILD])
        self.assertProblem("not below oracle/", [entry("crates/a.wrl", SCENES[CHILD])])

    def test_private_true(self):
        self.assertProblem("private must be false", self.changed(private=True))

    def test_origin_private(self):
        self.assertProblem("origin private", self.changed(origin="private"))


class Dependencies(Case):
    def scene(self, url_text):
        text = f"#VRML V2.0 utf8\nInline {{ url {url_text} }}\n"
        self.write(MAIN, text)
        return text

    def test_remote_host(self):
        text = self.scene('"http://example.com/a.wrl"')
        self.assertProblem("neither local nor on an .invalid host", [entry(MAIN, text), self.entries[1]])

    def test_urn(self):
        text = self.scene('"urn:web3d:x"')
        self.assertProblem("neither local nor on an .invalid host", [entry(MAIN, text), self.entries[1]])

    def test_outside_oracle(self):
        text = self.scene('"../../../../../../crates/a.wrl"')
        self.assertProblem("resolves outside oracle/", [entry(MAIN, text), self.entries[1]])

    def test_undeclared(self):
        e = self.changed()
        del e[0]["dependencies"][0]
        self.assertProblem("loads " + CHILD, e)

    def test_not_loaded(self):
        e = self.changed()
        e[0]["dependencies"].append(e[0]["dependencies"][0] | {"path": LIB + "other.wrl"})
        self.write(LIB + "other.wrl", SCENES[CHILD])
        self.assertProblem("not loaded by the scene", e + [entry(LIB + "other.wrl", SCENES[CHILD])])

    def test_absent_exists(self):
        self.write(LIB + "gone.wrl", SCENES[CHILD])
        e = self.changed()
        self.assertProblem("loads " + LIB + "gone.wrl", e + [entry(LIB + "gone.wrl", SCENES[CHILD])])

    def test_absent_undeclared_url(self):
        e = self.changed()
        del e[0]["dependencies"][2]
        self.assertProblem("named by the scene but not declared", e)

    def test_url_dependency_must_be_absent(self):
        e = self.changed()
        e[0]["dependencies"][2] = {"url": "http://case.invalid/a.wrl", "sha256": "0" * 64, "bytes": 1}
        self.assertProblem("must be absent", e)

    def test_transitive(self):
        child = '#VRML V2.0 utf8\nInline { url "deep/leaf.wrl" }\n'
        self.write(CHILD, child)
        self.write(LIB + "deep/leaf.wrl", SCENES[CHILD])
        sha, size = facts(child)
        e = self.changed(1, sha256=sha, bytes=size, dependencies=[])
        e[0]["dependencies"][0].update(sha256=sha, bytes=size)
        found = self.problems(e)
        self.assertTrue(any("loads " + LIB + "deep/leaf.wrl" in p and e[0]["id"] in p for p in found), found)

    def test_urls_tokenizer(self):
        text = ('#VRML V2.0 utf8\n# url "comment.wrl"\nEXTERNPROTO P [ field SFFloat a ] [ "x.wrl#P", "y.wrl" ]\n'
                'Background { backUrl "b.png" }\nScript { url "javascript: var s = \\"q.wrl\\";" }\n'
                'PROTO Q [ exposedField MFString url [ ] ] { Inline { url IS url } }\nAnchor { url "a.wrl" '
                'description "d.wrl" }\n')
        self.assertEqual(sm.urls(text), ["x.wrl#P", "y.wrl", "b.png", 'javascript: var s = "q.wrl";', "a.wrl"])


class Classes(Case):
    def test_iso_verified_refused(self):
        self.assertProblem("ISO_VERIFIED is not allowed", self.changed(**{"class": "ISO_VERIFIED"}))

    def test_iso_manifest_class(self):
        self.assertProblem("iso-vrml97 entries are ISO_REVIEW_NEEDED", self.changed(**{"class": "REGRESSION_ONLY"}))

    def test_iso_manifest_standard(self):
        self.assertProblem("VRML97 entries only", self.changed(standard="X3D-4.0", header="#X3D V4.0 utf8"))

    def test_iso_citations(self):
        self.assertProblem("needs spec_citations", self.changed(spec_citations=[]))

    def test_regression_class(self):
        self.manifest([main_entry()], "regression")
        self.manifest([self.entries[1]])
        self.assertTrue(any("regression entries are REGRESSION_ONLY" in p for p in sm.check(self.root)[0]))

    def test_behavior_tag(self):
        self.assertProblem("tag behavior needs", self.changed(tags=["behavior"]))

    def test_negative_folder(self):
        self.assertProblem("tag negative and a /negative/ folder", self.changed(tags=["negative"]))

    def test_license_record(self):
        self.assertProblem("needs a license_record", self.changed(origin="third-party-permitted"))

    def test_library_origin(self):
        self.assertProblem("project-authored", self.changed(origin="unknown", license_record="r.txt"))


class Schema(Case):
    def test_missing_field(self):
        e = self.changed()
        del e[0]["license"]
        self.assertProblem("missing required field 'license'", e)

    def test_unknown_field(self):
        self.assertProblem("unknown field 'extra'", self.changed(extra=1))

    def test_enum(self):
        self.assertProblem("'x_ite_parity'", self.changed(x_ite_parity="SAME"))

    def test_bool_is_not_integer(self):
        self.assertProblem("'bytes'", self.changed(bytes=True))

    def test_tag_pattern(self):
        self.assertProblem("'tags'", self.changed(tags=["Behavior"]))

    def test_provenance_date(self):
        self.assertProblem("'date'", self.changed(provenance={"author": "t", "date": "29 Oct", "method": "m"}))

    def test_manifest_name(self):
        self.manifest(self.entries, manifest="regression")
        self.assertTrue(any("!= file name" in p for p in sm.check(self.root)[0]))

    def test_duplicate_id(self):
        self.assertProblem("appears more than once", self.changed(1, id=self.entries[0]["id"]))

    def test_negative_of(self):
        self.assertProblem("negative_of", self.changed(negative_of="nope"))


class Coverage(Case):
    def test_scene_in_no_manifest(self):
        self.write(LIB + "stray.wrl", SCENES[CHILD])
        self.assertProblem("stray.wrl: a library scene in no manifest")

    def test_path_in_two_manifests(self):
        self.manifest([entry(CHILD, SCENES[CHILD], **{"class": "REGRESSION_ONLY"})], "regression")
        self.manifest(self.entries)
        self.assertTrue(any("is also in" in p for p in sm.check(self.root)[0]))


class Real(unittest.TestCase):
    def test_real_manifests(self):
        problems, count = sm.check()
        self.assertEqual(problems, [])
        self.assertGreater(count, 0)

    def test_cli(self):
        self.assertEqual(sm.main(["check"]), 0)
        self.assertEqual(sm.main([]), 2)


if __name__ == "__main__":
    unittest.main(verbosity=2)
