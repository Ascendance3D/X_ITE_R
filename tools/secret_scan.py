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

Exit: 0 when every finding is reviewed and no file was left NOT SCANNED (files a tool skips by design are coverage
lines, not a failure; see docs/agents/secret-scan.md); 1 when there is a new finding; 2 on a tool error
(missing tool, wrong version, no network namespace, a scanner that failed or did not report its end); 3 when files
could not be read (listed as NOT SCANNED; a coverage gap, not a pass). Precedence: 2 stops the scan at once; else 1
when there is a new finding, whether or not files were also NOT SCANNED; else 3.
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
# Files TruffleHog leaves out by its built-in extension list (images, video, audio, fonts, glTF binaries), per scan mode
# and extension. Reported as a coverage limit; they are not content-scanned by TruffleHog.
SKIPPED = {"tree": {}, "history": {}}  # by outer scan, not by TruffleHog mode (text copies run as filesystem)
# TruffleHog stops reading a Git diff at a line longer than this (it does not log it as an error).
LONG_LINE = 10 * 1024 * 1024
# Text formats that BOTH scanners leave out by extension (Gitleaks' default allowlist and TruffleHog's extension list,
# checked 2026-11-07 with a planted token). Their blobs are copied as .txt files and scanned by both tools.
BOTH_SKIP = (".svg", ".gltf")
COPY_HEADER = b"secret_scan.py text copy\n"


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


def grouped(digest):
    """A digest as 8-digit groups joined by ":", the form of the reviewed list."""
    return ":".join(digest[i:i + 8] for i in range(0, len(digest), 8))


def sha(value):
    return hashlib.sha256(value.encode("utf-8", "surrogatepass")).hexdigest()


def load_reviewed(path):
    reviewed = set()
    try:
        text = Path(path).read_text()
    except OSError as e:
        die(f"cannot read the reviewed list {path}: {e}")
    for n, line in enumerate(text.splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        cols = line.split("\t")
        if len(cols) >= 3:
            groups = cols[2].split(":")
            if any(len(g) != 8 for g in groups):
                die(f"{path}:{n}: write the digest in 8-digit groups joined by ':'")
            cols[2] = "".join(groups)
        width = 40 if cols[0] == UNREADABLE else 64
        if len(cols) != 4 or not re.fullmatch(f"[0-9a-f]{{{width}}}", cols[2]) or not cols[3].strip():
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
            # Scanner diagnostics are not printed: they could hold source text. Rerun Gitleaks by hand to read them.
            die(f"gitleaks {mode} failed (exit {p.returncode}; {len(p.stderr)} bytes of diagnostics not printed)")
        rows = json.load(open(report))
    return [{"tool": "gitleaks", "scan": mode, "rule": r["RuleID"], "file": r["File"], "commit": r.get("Commit", "")[:12],
             "line": r["StartLine"], "len": None, "sha256_12": None, "reviewed": False} for r in rows]


def trufflehog(mode, target, cwd, reviewed, template, scan=None):
    # Log level 5: the "skipping file" lines (files TruffleHog leaves out by extension) are debug output.
    cmd = ["trufflehog", mode, target, "--no-verification", "--no-update", "--json", "--log-level=5"]
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
        elif str(log.get("msg", "")).startswith("skipping file") and log.get("path"):
            ext = Path(log["path"]).suffix.lower() or log.get("ext") or "(none)"
            bucket = SKIPPED[scan or ("history" if mode == "git" else "tree")]
            bucket[ext] = bucket.get(ext, 0) + 1
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
                    "sha256_12": digest[:12], "sha256": digest,
                    "reviewed": (r["DetectorName"], src.get("file", ""), digest) in reviewed})
        if template is not None and not out[-1]["reviewed"]:
            template.add(f"{r['DetectorName']}\t{src.get('file', '')}\t{grouped(digest)}\tTODO classify")
        del r
    return out, sorted(unread)


def written(repo, *pathspec):
    """Every (path, blob ID) pair that some reachable commit wrote: the raw diff of each commit against each parent
    (-m) and of each root commit (--root). Every path/blob pair of any reachable tree is written by at least one of
    these diffs. --no-renames: a rename is a delete and an add, so the new path is listed with its blob."""
    cmd = ["git", "-C", str(repo), "log", "--all", "-m", "--root", "--no-renames", "--no-abbrev", "--raw", "-z",
           "--format=", "--", *pathspec]
    tokens = subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.split("\0")
    pairs, i = set(), 0
    # -z raw: ":<mode> <mode> <old blob> <new blob> <status>" NUL "<path>" NUL, record after record. Read in order, so a
    # path is never taken for a record (even a path that starts with ":").
    while i < len(tokens):
        head = tokens[i].lstrip("\n")
        fields = head.split()
        if head.startswith(":") and len(fields) == 5 and i + 1 < len(tokens):
            if fields[3] != "0" * 40:
                pairs.add((tokens[i + 1], fields[3]))
            i += 2
        else:
            i += 1
    return pairs


def blobs(repo, mode, path):
    """The Git blob IDs of `path`: at HEAD (tree), or every one it had in any commit (history)."""
    if mode == "tree":
        out = subprocess.run(["git", "-C", str(repo), "rev-parse", f"HEAD:{path}"], capture_output=True, text=True)
        ids = {out.stdout.strip()}
    else:
        ids = {oid for p, oid in written(repo, f":(literal){path}") if p == path}
    return sorted(i for i in ids if re.fullmatch("[0-9a-f]{40}", i) and i != "0" * 40)


def text_copies(repo, mode, config, reviewed, template):
    """Scan the BOTH_SKIP blobs (at HEAD, or every one in history) as .txt copies; findings name the real path."""
    git = ["git", "-C", str(repo)]
    if mode == "tree":
        # -z: paths unquoted (core.quotePath would quote non-ASCII names and end them with '"').
        rows = subprocess.run(git + ["ls-tree", "-r", "-z", "HEAD"], capture_output=True, text=True, check=True).stdout
        pairs = [(l.split("\t", 1)[1], l.split()[2]) for l in rows.split("\0") if l and l.split()[1] == "blob"]
    else:
        # Not rev-list --objects: it names only one path for a blob that is at several paths.
        pairs = written(repo)
    pairs = [(p, o) for p, o in pairs if p.lower().endswith(BOTH_SKIP)]
    names, findings, gap = {}, [], []
    if not pairs:
        return findings, gap, 0
    with tempfile.TemporaryDirectory(prefix="secret-scan-text-") as tmp:
        paths_of = {}
        for path, oid in pairs:
            paths_of.setdefault(oid, set()).add(path)
        # One copy per blob; a finding in it is reported at every path that blob has.
        for i, oid in enumerate(sorted(paths_of)):
            data = subprocess.run(git + ["cat-file", "blob", oid], capture_output=True, check=True).stdout
            name = f"t{i:06d}.txt"
            # A neutral first line: TruffleHog also skips by sniffed content type (an SVG with its xmlns header is an
            # image to it even as .txt). Line numbers of these findings are corrected by one below.
            Path(tmp, name).write_bytes(COPY_HEADER + data)
            names[name] = sorted(paths_of[oid])
        label = "git" if mode == "history" else None
        def fix_line(n):
            return n - 1 if isinstance(n, int) and n > 1 else n

        def real_paths(reported):
            # The scanners may write the copy as "t000001.txt", "./t000001.txt" or an absolute path; an unknown
            # name is a tool error, never a silently wrong path.
            base = os.path.basename(reported)
            if base not in names:
                die(f"a scanner reported an unknown text copy {reported!r}")
            return names[base]

        for f in gitleaks("dir", ".", tmp, config):
            for path in real_paths(f["file"]):
                findings.append({**f, "file": path, "scan": label or "dir", "via": "text copy", "line": fix_line(f["line"])})
        th, unread = trufflehog("filesystem", ".", tmp, set(), None, scan=mode)
        for f in th:
            for path in real_paths(f["file"]):
                g = {**f, "file": path, "scan": label or "filesystem", "via": "text copy", "line": fix_line(f["line"])}
                g["reviewed"] = (g["rule"], path, g["sha256"]) in reviewed
                if template is not None and not g["reviewed"]:
                    template.add(f"{g['rule']}\t{path}\t{grouped(g['sha256'])}\tTODO classify")
                findings.append(g)
        gap = [p for u in unread for p in real_paths(u)]
    return findings, gap, len(pairs)


def long_line_blobs(repo):
    """Paths of history blobs with a line longer than LONG_LINE bytes (TruffleHog skips the rest of such a diff).
    Every path of such a blob is listed (written(), not rev-list --objects, which names one path per blob)."""
    paths_of = {}
    for path, oid in written(repo):
        paths_of.setdefault(oid, set()).add(path)
    sizes = subprocess.run(["git", "-C", str(repo), "cat-file", "--batch-check=%(objecttype) %(objectname) %(objectsize)"],
                           input="".join(f"{o}\n" for o in sorted(paths_of)).encode(), capture_output=True,
                           check=True).stdout.decode("utf-8", "replace")
    out = set()
    for line in sizes.splitlines():
        kind, oid, size = (line.split(" ") + ["", ""])[:3]
        if kind == "blob" and int(size) > LONG_LINE:
            data = subprocess.run(["git", "-C", str(repo), "cat-file", "blob", oid], capture_output=True, check=True).stdout
            if max(map(len, data.split(b"\n"))) > LONG_LINE:
                out.update(paths_of[oid])
    return sorted(out)


def export_head(repo, dest):
    p = subprocess.run(["git", "-C", str(repo), "archive", "--format=tar", "HEAD"], capture_output=True, check=True)
    with tempfile.TemporaryFile() as f:
        f.write(p.stdout)
        f.seek(0)
        with tarfile.open(fileobj=f) as t:
            for m in t.getmembers():
                if m.name.startswith("/") or ".." in Path(m.name).parts or not (m.isfile() or m.isdir() or m.issym()):
                    die(f"git archive member {m.name!r} is not a plain repository path")
                # A symlink must stay inside the export (the fallback extraction below has no filter to check it).
                target = os.path.normpath(os.path.join(os.path.dirname(m.name), m.linkname)) if m.issym() else ""
                if m.issym() and (os.path.isabs(m.linkname) or target == ".." or target.startswith("../")):
                    die(f"git archive symlink {m.name!r} points outside the repository")
            try:
                t.extractall(dest, filter="data")
            except TypeError:  # Python without tarfile extraction filters (before 3.12 and its backports)
                t.extractall(dest)


def main():
    try:
        run()
    # ValueError: also UnicodeDecodeError (a non-UTF-8 path) and JSONDecodeError (malformed scanner output).
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, tarfile.TarError, OSError, ValueError) as e:
        die(f"{type(e).__name__}: {e}")


def run():
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
    shallow = subprocess.run(["git", "-C", str(repo), "rev-parse", "--is-shallow-repository"], capture_output=True,
                             text=True).stdout.strip()
    if "history" in modes and shallow != "false":
        die(f"the history scan needs a full clone (is-shallow-repository: {shallow or 'unknown'})")

    findings, unread, template, used = [], [], set() if a.template else None, set()
    for mode in modes:
        if mode == "history":
            findings += gitleaks("git", ".", repo, config)
            th, gap = trufflehog("git", f"file://{repo}", repo, reviewed, template)
            unread += [f"history: {p} (a line over 10 MiB; TruffleHog skips the rest of the diff)"
                       for p in long_line_blobs(repo)]
        else:
            with tempfile.TemporaryDirectory(prefix="secret-scan-tree-") as tmp:
                export_head(repo, tmp)
                findings += gitleaks("dir", ".", tmp, config)
                th, gap = trufflehog("filesystem", ".", tmp, reviewed, template)
        findings += th
        extra, extra_gap, copied = text_copies(repo, mode, config, reviewed, template)
        findings += extra
        print(f"coverage {mode}: {copied} {'/'.join(BOTH_SKIP)} blobs scanned as text copies by both tools")
        unread += [f"{mode}: {p} (text copy)" for p in extra_gap]
        for p in gap:
            ids = blobs(repo, mode, p)
            used.update((UNREADABLE, p, i) for i in ids)
            if ids and all((UNREADABLE, p, i) in reviewed for i in ids):
                continue
            unread.append(f"{mode}: {p}")
            if template is not None:
                template.update(f"{UNREADABLE}\t{p}\t{grouped(i)}\tTODO reason" for i in ids if (UNREADABLE, p, i) not in reviewed)
                if not ids:
                    template.add(f"# {UNREADABLE}\t{p}\t(no Git blob found for this path; cannot be reviewed)")

    new = [f for f in findings if not f["reviewed"]]
    used.update((f["rule"], f["file"], f["sha256"]) for f in findings if f["tool"] == "trufflehog")
    for f in findings:
        f.pop("sha256", None)
    for f in findings:
        state = "reviewed" if f["reviewed"] else "NEW"
        print(f"{state}\t{f['tool']}\t{f['scan']}\t{f['rule']}\t{f['file']}\t{f['commit']}\t{f['line']}\t"
              f"len={f['len']}\tsha256_12={f['sha256_12']}")
    for p in unread:
        print(f"NOT SCANNED\t{p}")
    for mode, exts in SKIPPED.items():
        if exts:
            listed = ", ".join(f"{e} {n}" for e, n in sorted(exts.items(), key=lambda x: -x[1]))
            print(f"coverage {mode}: TruffleHog skipped {sum(exts.values())} files by"
                  f" its extension list (not content-scanned by TruffleHog): {listed}")
    if modes == ["tree", "history"]:
        # Informational: a reviewed entry that no finding or unread file of either scan used (the file or value is gone).
        for d, p, v in sorted(reviewed - used):
            print(f"stale reviewed entry\t{d}\t{p}\t{grouped(v)[:17]}")
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
