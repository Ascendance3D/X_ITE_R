#!/usr/bin/env python3
"""Self-test of tools/secret_scan.py and the repository rules: python3 -I tools/test_secret_scan.py

Makes a disposable Git repository in a temporary directory with this repository's .gitleaks.toml, plants synthetic
values made at run time (random; no credential is stored in this file), and runs the scan in tree and history modes.
It checks that the reviewed exceptions hold only for their exact case, and that a planted credential outside them is
found by both scanners. Needs what tools/secret_scan.py needs (Gitleaks, TruffleHog, unshare -rn).
"""
import hashlib
import os
import secrets
import shutil
import string
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
from secret_scan import grouped  # noqa: E402

B62 = string.digits + string.ascii_uppercase + string.ascii_lowercase


def b62(n, width):
    s = ""
    while n:
        n, r = divmod(n, 62)
        s = B62[r] + s
    return s.rjust(width, "0")


def github_token():
    """A token in the GitHub classic-PAT format (30 random base62 characters and their CRC32 check). Not a real token."""
    body = "".join(secrets.choice(B62) for _ in range(30))
    return "ghp" + "_" + body + b62(zlib.crc32(body.encode()), 6)


def run(repo, reviewed, mode):
    p = subprocess.run([sys.executable, "-I", str(ROOT / "tools/secret_scan.py"), f"--{mode}", "--repo", str(repo),
                        "--reviewed", str(reviewed)], capture_output=True, text=True, timeout=600)
    rows = [line.split("\t") for line in p.stdout.splitlines() if line.startswith(("NEW\t", "reviewed\t"))]
    return p.returncode, rows, p.stdout + p.stderr


def main():
    failures = []

    def check(ok, what):
        print(("PASS " if ok else "FAIL ") + what)
        if not ok:
            failures.append(what)

    # The namespace the scanners run in has no outbound network (an IP literal: no DNS needed to try).
    # The probe itself reports whether connect() failed, so a namespace that cannot start (no "result" line) is a
    # failure of this check, and any refusal (ENETUNREACH, EPERM from a seccomp policy, ...) counts as blocked.
    probe = ("import socket\ntry:\n socket.create_connection(('192.0.2.1', 443), 3)\n print('result: connected')\n"
             "except OSError:\n print('result: blocked')")
    net = subprocess.run(["unshare", "-rn", "--", sys.executable, "-I", "-c", probe], capture_output=True, text=True)
    check(net.returncode == 0 and net.stdout.strip() == "result: blocked", "scanner namespace: outbound connection refused")

    with tempfile.TemporaryDirectory(prefix="secret-scan-test-") as tmp:
        repo = Path(tmp) / "repo"
        repo.mkdir()
        shutil.copy(ROOT / ".gitleaks.toml", repo / ".gitleaks.toml")
        digest = hashlib.sha256(secrets.token_bytes(32)).hexdigest()
        key = "".join(secrets.choice(B62) for _ in range(40))
        token = github_token()
        files = {
            # N1: the reviewed shape, in a reviewed file: no finding.
            "oracle/cm5-xml-baseline.json": f'{{"tokenHash":"{digest}"}}\n',
            # P1: another key in the same reviewed file: a finding.
            "oracle/cm5-css-baseline.json": f'{{"apiKey":"{key}"}}\n',
            # P2: the reviewed shape in another file: a finding.
            "oracle/other-baseline.json": f'{{"tokenHash":"{digest}"}}\n',
            # P3: a credential-format token in source: both scanners.
            "src/settings.rs": f'const TOKEN: &str = "{token}";\n',
            # G: a text format that TruffleHog skips by extension (.svg): Gitleaks must still find the token.
            "assets/icon.svg": f'<svg xmlns="http://www.w3.org/2000/svg"><desc>{token}</desc></svg>\n',
            # P4: the same token in a second file; the review list names only src/settings.rs.
            "src/other.rs": f'const TOKEN: &str = "{token}";\n',
        }
        for name, body in files.items():
            (repo / name).parent.mkdir(parents=True, exist_ok=True)
            (repo / name).write_text(body)
        env = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.invalid",
               "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.invalid"}
        # U: a truncated gzip file TruffleHog cannot open; a second commit writes a second blob of it.
        (repo / "fixtures").mkdir()
        (repo / "fixtures/cut.wrl.gz").write_bytes(b"\x1f\x8b\x00\x00")

        def git(*cmd):
            return subprocess.run(["git", "-C", str(repo), *cmd], check=True, env=env, capture_output=True,
                                  text=True).stdout.strip()

        git("init", "-q")
        git("add", "-A")
        git("commit", "-q", "-m", "synthetic")
        blob1 = git("rev-parse", "HEAD:fixtures/cut.wrl.gz")
        (repo / "fixtures/cut.wrl.gz").write_bytes(b"\x1f\x8b\x00\x00\x00")
        git("commit", "-q", "-am", "second blob")
        blob2 = git("rev-parse", "HEAD:fixtures/cut.wrl.gz")

        reviewed = Path(tmp) / "reviewed.tsv"
        token_sha = hashlib.sha256(token.encode()).hexdigest()
        reviewed.write_text(f"Github\tsrc/settings.rs\t{grouped(token_sha)}\tsynthetic test entry\n")

        for mode in ("tree", "history"):
            rc, rows, out = run(repo, reviewed, mode)
            gl = {(r[3], r[4]) for r in rows if r[1] == "gitleaks"}
            if os.environ.get("SECRET_SCAN_TEST_DEBUG"):
                print("\n".join("\t".join(r[:5]) for r in rows))
            th = {(r[0], r[3], r[4]) for r in rows if r[1] == "trufflehog"}
            check(rc == 1, f"{mode}: exit 1 with new findings (got {rc})")
            check(not any(f == "oracle/cm5-xml-baseline.json" for _, f in gl), f"{mode}: N1 reviewed tokenHash not reported")
            check(("generic-api-key", "oracle/cm5-css-baseline.json") in gl, f"{mode}: P1 other key in reviewed file found")
            check(("generic-api-key", "oracle/other-baseline.json") in gl, f"{mode}: P2 tokenHash in other file found")
            check(("github-pat", "src/settings.rs") in gl, f"{mode}: P3 gitleaks finds the token")
            check(("github-pat", "src/other.rs") in gl, f"{mode}: P4 gitleaks finds the second copy")
            check(("reviewed", "Github", "src/settings.rs") in th, f"{mode}: P3 trufflehog finds it; exact review applies")
            check(("NEW", "Github", "src/other.rs") in th, f"{mode}: P4 same value, other path: NEW")
            check(("github-pat", "assets/icon.svg") in gl, f"{mode}: G gitleaks finds the token in a skipped .svg")
            check(("NEW", "Github", "assets/icon.svg") in th, f"{mode}: G trufflehog finds it in the .svg text copy")
            check(token not in out and key not in out and digest not in out, f"{mode}: no planted value in the output")

        # U: the reviewed list names only the first blob: history and tree both report the file NOT SCANNED.
        cut = "fixtures/cut.wrl.gz"
        reviewed.write_text(f"Github\tsrc/settings.rs\t{grouped(token_sha)}\tsynthetic test entry\n"
                            f"unreadable\t{cut}\t{grouped(blob1)}\tsynthetic truncated gzip\n")
        for mode in ("history", "tree"):
            _, _, out = run(repo, reviewed, mode)
            check(f"NOT SCANNED\t{mode}: {cut}" in out, f"{mode}: U unreviewed second blob is NOT SCANNED")
        # U: both blobs reviewed: the file is no longer NOT SCANNED.
        with reviewed.open("a") as f:
            f.write(f"unreadable\t{cut}\t{grouped(blob2)}\tsynthetic truncated gzip\n")
        for mode in ("history", "tree"):
            _, _, out = run(repo, reviewed, mode)
            check(f"{mode}: {cut}" not in out, f"{mode}: U all blobs reviewed: not reported")

        # L: a blob with a line over 10 MiB (TruffleHog skips the rest of that diff): history reports it NOT SCANNED.
        (repo / "fixtures/long.txt").write_text("x" * (10 * 1024 * 1024 + 16) + "\n")
        git("add", "fixtures/long.txt")
        git("commit", "-q", "-m", "long line")
        _, _, out = run(repo, reviewed, "history")
        check("NOT SCANNED\thistory: fixtures/long.txt" in out, "history: L line over 10 MiB is NOT SCANNED")

        # Without the review entry the token is NEW in both files.
        reviewed.write_text("")
        rc, rows, _ = run(repo, reviewed, "tree")
        check({r[4] for r in rows if r[0] == "NEW" and r[1] == "trufflehog" and r[3] == "Github"}
              == {"src/settings.rs", "src/other.rs", "assets/icon.svg"},
              "tree: empty review list: trufflehog reports every copy NEW")

    print(f"{'FAIL' if failures else 'PASS'}: {len(failures)} failed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
