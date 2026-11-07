#!/usr/bin/env python3
"""Offline secret scan of this repository with Gitleaks and TruffleHog; the output never holds a candidate value.

    python3 -I tools/secret_scan.py [--tree] [--history] [--repo DIR] [--reviewed FILE] [--json OUT]

With neither --tree nor --history, it does both. --tree scans the tracked files at HEAD (an export of `git archive
HEAD`, so untracked and ignored files are not read); --history scans every commit reachable from any ref.

Offline: each scanner runs in a new network namespace with no interface but loopback (`unshare -rn`), so no
verification request, update check or other outbound connection can leave the machine. TruffleHog also gets
--no-verification and --no-update. When the namespace cannot be made, the scan stops (exit 2): there is no option to
run with network access. Docs: docs/agents/secret-scan.md.

Exceptions, both exact:
- Gitleaks: the [[allowlists]] of .gitleaks.toml (rule + path + match shape).
- TruffleHog: tools/secret-scan-reviewed.tsv. A finding is reviewed only when its detector, its file path and the
  SHA-256 of its raw value are all on one line. Any other value, file or detector is a new finding.

Output: per finding the tool, rule or detector, file, commit (12 digits), line, value length and the first 12 hex digits
of the SHA-256 of the value. Values are read in memory only, to hash them; Gitleaks runs with --redact=100.

Exit: 0 when every finding is reviewed and every file was read; 1 when there is a new finding; 2 on a tool error
(missing tool, wrong version, no network namespace, a scanner that failed or did not report its end); 3 when files
could not be read (listed as NOT SCANNED; a coverage gap, not a pass).
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSIONS = {"gitleaks": "8.30.1", "trufflehog": "3.99.8"}
TIMEOUT_S = 900
# A file TruffleHog could not read (it takes raw f32 samples for Brotli, and truncated gzip fixtures fail to open).
# Reviewed only for the exact Git blob IDs listed; a changed or new blob at that path is NOT SCANNED again.
UNREADABLE = "unreadable"


def die(msg):
    print(f"secret_scan: {msg}", file=sys.stderr)
    sys.exit(2)


def offline(cmd):
    return ["unshare", "-rn", "--", *cmd]


def check_tools():
    if shutil.which("unshare") is None:
        die("unshare not found: cannot run the scanners without network access")
    if subprocess.run(offline(["true"]), capture_output=True).returncode != 0:
        die("cannot create a network namespace (unshare -rn): the scan does not run with network access")
    for tool, want in VERSIONS.items():
        if shutil.which(tool) is None:
            die(f"{tool} not found (version {want} expected)")
        out = subprocess.run([tool, "--version"], capture_output=True, text=True)
        have = (out.stdout + out.stderr).split()
        if want not in have and f"v{want}" not in have:
            die(f"{tool} {' '.join(have)}: version {want} expected")


def sha(value):
    return hashlib.sha256(value.encode("utf-8", "surrogatepass")).hexdigest()


def load_reviewed(path):
    reviewed = set()
    for n, line in enumerate(Path(path).read_text().splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        cols = line.split("\t")
        width = 40 if cols[0] == UNREADABLE else 64
        if len(cols) < 4 or not re.fullmatch(f"[0-9a-f]{{{width}}}", cols[2]) or not cols[3].strip():
            die(f"{path}:{n}: want detector<TAB>path<TAB>sha256 (or Git blob ID)<TAB>classification")
        reviewed.add((cols[0], cols[1], cols[2]))
    return reviewed


def gitleaks(mode, target, cwd, config):
    with tempfile.TemporaryDirectory(prefix="secret-scan-gl-") as tmp:
        report = os.path.join(tmp, "r.json")
        cmd = ["gitleaks", mode, "--config", str(config), "--redact=100", "--no-banner", "--no-color",
               "--report-format", "json", "--report-path", report, "--exit-code", "0", "--log-level", "error", target]
        p = subprocess.run(offline(cmd), cwd=cwd, capture_output=True, text=True, timeout=TIMEOUT_S)
        if p.returncode != 0 or not os.path.exists(report):
            die(f"gitleaks {mode} failed (exit {p.returncode}): {p.stderr.strip()[-400:]}")
        rows = json.load(open(report))
    return [{"tool": "gitleaks", "scan": mode, "rule": r["RuleID"], "file": r["File"], "commit": r.get("Commit", "")[:12],
             "line": r["StartLine"], "len": None, "sha256_12": None, "reviewed": False} for r in rows]


def trufflehog(mode, target, cwd, reviewed, template):
    cmd = ["trufflehog", mode, target, "--no-verification", "--no-update", "--json", "--log-level=0"]
    p = subprocess.run(offline(cmd), cwd=cwd, capture_output=True, text=True, timeout=TIMEOUT_S)
    if p.returncode != 0:
        die(f"trufflehog {mode} failed (exit {p.returncode})")
    finished, unread = False, set()
    for line in p.stderr.splitlines():
        try:
            log = json.loads(line)
        except ValueError:
            continue
        if log.get("msg") == "finished scanning":
            finished = True
        elif log.get("level") == "error" and log.get("path"):
            unread.add(log["path"])
    if not finished:
        die(f"trufflehog {mode} did not report the end of the scan")
    out = []
    for line in p.stdout.splitlines():
        r = json.loads(line)
        md = r["SourceMetadata"]["Data"]
        src = md.get("Git") or md.get("Filesystem") or {}
        digest = sha(r.get("Raw", ""))
        out.append({"tool": "trufflehog", "scan": mode, "rule": r["DetectorName"], "file": src.get("file", ""),
                    "commit": src.get("commit", "")[:12], "line": src.get("line"), "len": len(r.get("Raw", "")),
                    "sha256_12": digest[:12],
                    "reviewed": (r["DetectorName"], src.get("file", ""), digest) in reviewed})
        if template is not None and not out[-1]["reviewed"]:
            template.add(f"{r['DetectorName']}\t{src.get('file', '')}\t{digest}\tTODO classify")
        del r
    return out, sorted(unread)


def blobs(repo, mode, path):
    """The Git blob IDs of `path`: at HEAD (tree), or every one it had in any commit (history)."""
    if mode == "tree":
        cmd = ["git", "-C", str(repo), "rev-parse", f"HEAD:{path}"]
    else:
        cmd = ["git", "-C", str(repo), "log", "--all", "--no-abbrev", "--raw", "--format=", "--", path]
    out = subprocess.run(cmd, capture_output=True, text=True).stdout.split("\n")
    ids = {line.split()[3] if mode == "history" else line for line in out if line.strip()}
    return sorted(i for i in ids if re.fullmatch("[0-9a-f]{40}", i) and i != "0" * 40)


def export_head(repo, dest):
    p = subprocess.run(["git", "-C", str(repo), "archive", "--format=tar", "HEAD"], capture_output=True, check=True)
    with tempfile.TemporaryFile() as f:
        f.write(p.stdout)
        f.seek(0)
        with tarfile.open(fileobj=f) as t:
            t.extractall(dest, filter="data")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--tree", action="store_true")
    ap.add_argument("--history", action="store_true")
    ap.add_argument("--repo", default=str(ROOT))
    ap.add_argument("--reviewed", default=None)
    ap.add_argument("--json", default=None, help="write the redacted findings to this file")
    ap.add_argument("--template", default=None,
                    help="write review lines (full value SHA-256, no value) of the new TruffleHog findings and unread files"
                         " to this file, to classify by hand before they go into the reviewed list")
    a = ap.parse_args()
    repo = Path(a.repo).resolve()
    config = repo / ".gitleaks.toml"
    reviewed = load_reviewed(a.reviewed or repo / "tools/secret-scan-reviewed.tsv")
    if not config.exists():
        die(f"{config} not found")
    check_tools()
    modes = [m for m, on in (("tree", a.tree), ("history", a.history)) if on] or ["tree", "history"]

    findings, unread, template = [], [], set() if a.template else None
    for mode in modes:
        if mode == "history":
            findings += gitleaks("git", ".", repo, config)
            th, gap = trufflehog("git", f"file://{repo}", repo, reviewed, template)
        else:
            with tempfile.TemporaryDirectory(prefix="secret-scan-tree-") as tmp:
                export_head(repo, tmp)
                findings += gitleaks("dir", ".", tmp, config)
                th, gap = trufflehog("filesystem", ".", tmp, reviewed, template)
        findings += th
        for p in gap:
            ids = blobs(repo, mode, p)
            if ids and all((UNREADABLE, p, i) in reviewed for i in ids):
                continue
            unread.append(f"{mode}: {p}")
            if template is not None:
                template.update(f"{UNREADABLE}\t{p}\t{i}\tTODO reason" for i in ids if (UNREADABLE, p, i) not in reviewed)

    new = [f for f in findings if not f["reviewed"]]
    for f in findings:
        state = "reviewed" if f["reviewed"] else "NEW"
        print(f"{state}\t{f['tool']}\t{f['scan']}\t{f['rule']}\t{f['file']}\t{f['commit']}\t{f['line']}\t"
              f"len={f['len']}\tsha256_12={f['sha256_12']}")
    for p in unread:
        print(f"NOT SCANNED\t{p}")
    for mode in modes:
        n = [f for f in findings if f["scan"] in ({"git"} if mode == "history" else {"dir", "filesystem"})]
        print(f"summary {mode}: gitleaks {sum(f['tool'] == 'gitleaks' for f in n)} new; trufflehog "
              f"{sum(f['tool'] == 'trufflehog' for f in n)} findings, "
              f"{sum(f['tool'] == 'trufflehog' and not f['reviewed'] for f in n)} new")
    print(f"tools: gitleaks {VERSIONS['gitleaks']}, trufflehog {VERSIONS['trufflehog']}; network: none (unshare -rn)")
    if template is not None:
        Path(a.template).write_text("".join(f"{t}\n" for t in sorted(template)))
    if a.json:
        Path(a.json).write_text(json.dumps({"findings": findings, "not_scanned": unread, "versions": VERSIONS}, indent=1))
    sys.exit(1 if new else 3 if unread else 0)


if __name__ == "__main__":
    main()
