#!/usr/bin/env python3
"""The release identity of the browser package (tools/package-web-preview.sh --release ID).

    python3 -I -B tools/release_identity.py parse ID          prints "release<TAB>version<TAB>title"; exit 2 for a bad ID
    python3 -I -B tools/release_identity.py check ID COMMIT   exit 1 with one line per mismatch

ID is canary-X.Y.Z or browser-preview-X.Y.Z. The check reads the tracked files at COMMIT (git), so an untracked or
changed file has no effect, as in the package. It requires:

- the identity strings: browser.version in x_ite_r.mjs, the x3d-viewer crate version (the menu title), the H1 of
  DEPLOY.md and COMPATIBILITY.md, browser.version in COMPATIBILITY.md, and the <title> and version field of the start page;
- that every page label which names a release names this channel and this version. Page labels are <title>,
  <meta name="description">, <span class="tag">, <p class="label"> and the H1 of a Markdown page, in the pages, the
  example, the site, API.md and the md_page.py template. Running text can name another release (for example "Results
  of Browser Preview 0.1.0"); a label cannot.

Tests: python3 -I -B tools/test_release_identity.py"""
import html, re, subprocess, sys

WEB = "crates/x3d-web/web"
ID = re.compile(r"^(canary|browser-preview)-((?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*))$")
# The words a label may use for each channel. "Alpha" is the tag word of the Browser Preview pages.
CHANNEL_TITLE = {"canary": "Canary", "browser-preview": "Browser Preview"}
CHANNEL_WORDS = {"canary": {"Canary"}, "browser-preview": {"Browser Preview", "Alpha"}}
RELEASE = re.compile(r"\b(Alpha|Canary|Browser Preview) ([0-9]+\.[0-9]+\.[0-9]+)\b")
LABEL_SCOPES = (f"{WEB}/preview/", f"{WEB}/example/", f"{WEB}/site/", f"{WEB}/API.md", "tools/md_page.py")
HTML_LABELS = (
    re.compile(r"<title>(.*?)</title>", re.S),
    re.compile(r'<meta\s+name="description"\s+content="([^"]*)"', re.S),
    re.compile(r'<span class="tag">(.*?)</span>', re.S),
    re.compile(r'<p class="label">(.*?)</p>', re.S),
)


def parse(release_id):
    """(channel, version, package release name, display title) of ID; ValueError for a bad ID."""
    m = ID.match(release_id)
    if not m:
        raise ValueError(f"release ID '{release_id}' is not canary-X.Y.Z or browser-preview-X.Y.Z")
    channel, version = m.group(1), m.group(2)
    return channel, version, f"x_ite_r-{release_id}", f"{CHANNEL_TITLE[channel]} {version}"


def labels(path, text):
    """The page labels of one file."""
    found = []
    if path.endswith((".html", ".py")):
        for rx in HTML_LABELS:
            found += [html.unescape(re.sub(r"\s+", " ", m)).strip() for m in rx.findall(text)]
    if path.endswith(".md"):
        found += [line[2:].strip() for line in text.splitlines()[:1] if line.startswith("# ")]
    return found


def label_errors(files, channel, version):
    """A line for each page label that names another channel or another version."""
    errors = []
    for path in sorted(files):
        if not path.startswith(LABEL_SCOPES):
            continue
        for label in labels(path, files[path]):
            for word, ver in RELEASE.findall(label):
                if word not in CHANNEL_WORDS[channel] or ver != version:
                    errors.append(f"{path}: the label '{label}' names {word} {ver}, not {CHANNEL_TITLE[channel]} {version}")
    return errors


def required_errors(files, version, title):
    """A line for each identity string that a tracked file does not hold."""
    need = [
        (f"{WEB}/x_ite_r.mjs", f'get version () {{ return "{version}"; }}'),
        ("crates/x3d-viewer/Cargo.toml", f'version = "{version}"'),
        (f"{WEB}/preview/DEPLOY.md", f"# X_ITE_R {title}"),
        (f"{WEB}/preview/COMPATIBILITY.md", f"# X_ITE_R {title}"),
        (f"{WEB}/preview/COMPATIBILITY.md", f'`browser.version` is `"{version}"`'),
        (f"{WEB}/preview/root-index.html", f"<title>X_ITE_R {title}</title>"),
        (f"{WEB}/preview/root-index.html", f'id="f-version">{version}<'),
    ]
    return [f"{path} does not hold '{text}'" for path, text in need if text not in files.get(path, "")]


def check(release_id, files):
    """Every mismatch of the tracked files `files` ({path: text}) with ID."""
    channel, version, _, title = parse(release_id)
    return required_errors(files, version, title) + label_errors(files, channel, version)


def tracked_files(commit, root="."):
    """{path: text} of the files the check reads, at COMMIT."""
    run = lambda *a: subprocess.run(["git", "-C", root, *a], check=True, capture_output=True).stdout
    names = run("ls-tree", "-r", "--name-only", commit, "--", f"{WEB}/preview", f"{WEB}/example", f"{WEB}/site",
                f"{WEB}/API.md", f"{WEB}/x_ite_r.mjs", "crates/x3d-viewer/Cargo.toml", "tools/md_page.py").decode().split("\n")
    wanted = [n for n in names if n.endswith((".html", ".md", ".mjs", ".py", ".toml"))]
    return {n: run("show", f"{commit}:{n}").decode("utf-8") for n in wanted}


def main(argv):
    if len(argv) == 2 and argv[0] == "parse":
        try:
            _, version, release, title = parse(argv[1])
        except ValueError as e:
            print(f"package-web-preview: {e}", file=sys.stderr)
            return 2
        print(f"{release}\t{version}\t{title}")
        return 0
    if len(argv) == 3 and argv[0] == "check":
        try:
            errors = check(argv[1], tracked_files(argv[2]))
        except ValueError as e:
            print(f"package-web-preview: {e}", file=sys.stderr)
            return 2
        for e in errors:
            print(f"package-web-preview: {e}", file=sys.stderr)
        return 1 if errors else 0
    print(__doc__.split("\n\n")[1], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
