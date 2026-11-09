#!/usr/bin/env python3
"""Ledger audit of 2026-11-09: python3 -I -B audit_1109.py <nextest log> > audit.tsv

Maps every ledger entry to the tests that ran, without changing anything. A test counts for an entry when the entry's
tests, notes or missing text names its Rust test binary (crates/<crate>/tests/<name>.rs, or <name> / <name>.rs alone) or
its test function (a name without an underscore only when written as <name>.rs). The nextest log is the output of tools/strict-suite.sh (PASS / FAIL / SKIP lines). The .mjs, .py and browser
references of an entry are listed only: this audit does not run them or read their logs (see OTHER-EVIDENCE).

Columns: id, status, rust paths present, Rust tests that ran for the entry (passed / failed / skipped), failing tests,
.mjs references, .py references, class:
  RUST-PASS      at least one named Rust test ran and every named Rust test passed (none failed, none skipped)
  RUST-SKIP      no named Rust test failed, but at least one was skipped: not counted as a pass
  RUST-FAIL      a named Rust test failed (needs a manual trace before any status change)
  OTHER-EVIDENCE only browser / Node / Python evidence is named; an unaudited reference, traced by hand in the report
  TRACED-PASS    no binary is named, but the oracle directories / synthetic suites / corpus it names are read by Rust
                 tests that ran, and every one of them passed (none skipped)
  NO-TRACE       no test is named; the coverage needs a manual trace
"""
import collections, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
LINE = re.compile(r"^\s*(PASS|FAIL|SKIP|SIGSEGV|SIGABRT|TIMEOUT|LEAK|LEAK-FAIL)\s+\[[^\]]*\]\s+(?:\(\s*\d+/\d+\)\s+)?(\S+)\s+(\S+)\s*$")
WORD = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def results(path):
    out = {}
    for line in open(path, errors="replace"):
        m = LINE.match(line)
        if m:
            verdict, binary, test = m.groups()
            # A later line (a retry) replaces an earlier one.
            out[(binary, test)] = "PASS" if verdict == "PASS" else "SKIP" if verdict == "SKIP" else "FAIL"
    return out


def test_sources():
    """crates/<crate>/tests/<name>.rs -> (binary, source text)."""
    out = {}
    crates = os.path.join(ROOT, "crates")
    for crate in sorted(os.listdir(crates)):
        d = os.path.join(crates, crate, "tests")
        if not os.path.isdir(d): continue
        for f in sorted(os.listdir(d)):
            if f.endswith(".rs"):
                out[f"{crate}::{f[:-3]}"] = open(os.path.join(d, f), errors="replace").read()
    return out


# Evidence written as words, not paths: the corpus suites (private answer keys; strict mode fails when one is missing).
CORPUS_TESTS = re.compile(r"^(x0|x1|items|items2|corpus)_\w*matches_x_ite$|^\w*corpus\w*$")


def trace(text, sources, ran):
    """Second pass for an entry that names no test binary: the oracle directories and synthetic suites it names, and the
    corpus suites when it names a corpus, mapped to the test binaries whose source reads them."""
    keys = set(re.findall(r"oracle/([\w.-]+)", text)) | set(re.findall(r"\b(synthetic-[\w-]+)", text))
    keys = {k for k in keys if k not in ("tools", "x_ite-pin")}
    binaries = {b for b, src in sources.items() if any(k in src for k in keys)}
    hits = {(b, t): v for (b, t), v in ran.items() if b in binaries}
    if re.search(r"\bcorpus\b|every corpus suite", text):
        hits.update({(b, t): v for (b, t), v in ran.items() if b.endswith("::corpus") or b == "vrml97::oracle" or b == "x3d-scene::xml_json_oracle"})
    return hits, sorted(keys)


def main():
    if len(sys.argv) != 2: raise SystemExit(__doc__)
    ran = results(sys.argv[1])
    by_binary = collections.defaultdict(list)  # binary short name (after "crate::") -> [(full, test, verdict)]
    by_test = collections.defaultdict(list)
    for (binary, test), v in ran.items():
        short = binary.split("::", 1)[1] if "::" in binary else binary
        by_binary[short].append((binary, test, v))
        by_test[test.rsplit("::", 1)[-1]].append((binary, test, v))
    items = json.load(open(os.path.join(ROOT, "ledger", "ledger.json")))["items"]
    sources = test_sources()
    print("\t".join(["id", "status", "rust_paths", "rust_pass", "rust_fail", "rust_skip", "failing", "mjs", "py", "class", "traced_via"]))
    summary = collections.Counter()
    for i in items:
        text = " ".join(i.get("tests", []) + [i.get("notes", ""), i.get("missing", "")])
        words = set(WORD.findall(text))
        hits = {}
        rs_files = set(re.findall(r"([A-Za-z0-9_]+)\.rs\b", text))
        for w in words:
            # A name without an underscore (fog, runtime, script) is an ordinary word too: it counts only as <name>.rs.
            if "_" not in w and w not in rs_files: continue
            for full, test, v in by_binary.get(w, []) + by_test.get(w, []):
                hits[(full, test)] = v
        rust_ok = all(os.path.exists(os.path.join(ROOT, p.split(" ")[0].split(",")[0])) for p in i.get("rust", []))
        via = ""
        if not hits:
            hits, keys = trace(text, sources, ran)
            via = " ".join(keys) + (" corpus" if re.search(r"\bcorpus\b", text) else "")
        c = collections.Counter(hits.values())
        failing = sorted(f"{b} {t}" for (b, t), v in hits.items() if v == "FAIL")
        mjs = sorted(set(re.findall(r"[\w./-]+\.test\.mjs|[\w./-]+\.mjs", text)))
        py = sorted(set(re.findall(r"[\w./-]+\.py", text)))
        if failing: cls = "RUST-FAIL"
        elif c["SKIP"]: cls = "RUST-SKIP"
        elif c["PASS"] and via: cls = "TRACED-PASS"
        elif c["PASS"]: cls = "RUST-PASS"
        elif mjs or py: cls = "OTHER-EVIDENCE"
        else: cls = "NO-TRACE"
        summary[(i["status"], cls)] += 1
        print("\t".join([i["id"], i["status"], "yes" if rust_ok else "check", str(c["PASS"]), str(c["FAIL"]), str(c["SKIP"]),
                         "; ".join(failing), " ".join(mjs), " ".join(py), cls, via]))
    for (s, cls), n in sorted(summary.items()): print(f"# {s} {cls} {n}", file=sys.stderr)
    print(f"# tests in the log: {len(ran)}, {collections.Counter(ran.values())}", file=sys.stderr)


main()
