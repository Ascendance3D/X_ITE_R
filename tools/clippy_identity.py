#!/usr/bin/env python3
"""Compare Clippy diagnostics with a recorded baseline by diagnostic identity, not by count.

Identity = (crate, lint, level, message, primary file, primary span source text). The line and column are not part of
the identity: a diagnostic whose code moved is "moved", not "new". Cargo reports one diagnostic once per target (lib,
test, bin); the tool keeps one.

A head diagnostic whose identity is not in the baseline is not reported as "new" at once. When a removed baseline
diagnostic has the same crate, lint and level and differs in only one of message, file or span text, the pair is
"changed": a rename or an edit of the same source, to check by hand. Only the rest are "new".

    clippy_identity.py export CARGO.json > BASELINE.jsonl
    clippy_identity.py compare BASELINE.jsonl CARGO.json [--github] [--strict]
    clippy_identity.py --self-test

`compare` exits 0 (the CI step is a report, not a gate) unless `--strict` is given and a new diagnostic is found.
`--github` adds a warning annotation for each new and changed diagnostic. Old diagnostics never fail the compare.
The baseline is not edited to hide a new diagnostic: a new one is fixed, or the owner accepts it.
"""
import collections
import json
import sys

FIELDS = ("crate", "lint", "level", "message", "file", "span")


def from_cargo(lines):
    """The diagnostics of `cargo clippy --message-format=json` output, one per identity and location."""
    out, seen = [], set()
    for line in lines:
        line = line.strip()
        if not line.startswith("{"):
            continue
        rec = json.loads(line)
        if rec.get("reason") != "compiler-message":
            continue
        msg = rec["message"]
        if msg.get("level") not in ("warning", "error"):
            continue
        spans = [s for s in msg.get("spans", []) if s.get("is_primary")]
        if not spans:
            continue
        s = spans[0]
        # The package id ends in "<path or url>#<name>@<version>" or "<path>/<name>#<version>"; keep the name only.
        pkg = rec["package_id"].split("#")
        crate = pkg[1].split("@")[0] if "@" in pkg[-1] else pkg[0].rsplit("/", 1)[-1]
        d = {
            "crate": crate,
            "lint": (msg.get("code") or {}).get("code") or "<none>",
            "level": msg["level"],
            "message": msg["message"],
            "file": s["file_name"],
            "span": " ".join(t["text"].strip() for t in s.get("text", []))[:240],
            "line": s["line_start"],
            "column": s["column_start"],
        }
        key = ident(d) + (d["line"], d["column"])
        if key not in seen:
            seen.add(key)
            out.append(d)
    return out


def ident(d):
    return tuple(d[f] for f in FIELDS)


def compare(base, head):
    """New, changed, removed and moved diagnostics of `head` against `base`."""
    bc = collections.Counter(ident(d) for d in base)
    hc = collections.Counter(ident(d) for d in head)
    base_locs = collections.defaultdict(list)
    for d in base:
        base_locs[ident(d)].append((d["line"], d["column"]))
    # A multiset difference: of an identity present n times more in head, report n (the last ones).
    extra = hc - bc
    added = [d for d in reversed(head) if extra[ident(d)] > 0 and not extra.subtract({ident(d): 1})][::-1]
    # Moved: in the baseline with the same identity, at another location (an extra copy is new, not moved).
    extra_ids = {id(d) for d in added}
    moved = [d for d in head if ident(d) in bc and id(d) not in extra_ids and (d["line"], d["column"]) not in base_locs[ident(d)]]
    gone = bc - hc
    removed = [d for d in base if gone[ident(d)] > 0 and not gone.subtract({ident(d): 1})]
    changed, new = [], []
    for d in added:
        pair = next((r for r in removed if r["crate"] == d["crate"] and r["lint"] == d["lint"] and r["level"] == d["level"]
                     and sum(r[f] != d[f] for f in ("message", "file", "span")) == 1), None)
        if pair:
            removed.remove(pair)
            changed.append((pair, d))
        else:
            new.append(d)
    return new, changed, removed, moved


def show(d):
    return f"{d['crate']} {d['lint']} {d['file']}:{d['line']}: {d['message']}"


def annotate(d, title):
    text = f"{d['lint']}: {d['message']}".replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    return f"::warning file={d['file']},line={d['line']},title={title}::{text}"


def run_compare(base, head, github=False, strict=False, out=sys.stdout):
    new, changed, removed, moved = compare(base, head)
    print(f"Clippy identity compare: baseline {len(base)}, head {len(head)}: new {len(new)}, changed {len(changed)}, "
          f"removed {len(removed)}, moved {len(moved)}", file=out)
    for d in new:
        print(f"NEW {show(d)}", file=out)
    for old, d in changed:
        print(f"CHANGED {show(d)}\n    was {show(old)}  (check the source identity: a rename or an edit)", file=out)
    for d in removed:
        print(f"REMOVED {show(d)}", file=out)
    for d in moved:
        print(f"MOVED {show(d)}", file=out)
    if github:
        for d in new:
            print(annotate(d, "New Clippy diagnostic"), file=out)
        for _, d in changed:
            print(annotate(d, "Changed Clippy diagnostic (check the source identity)"), file=out)
    return 1 if strict and new else 0


def self_test():
    def diag(n, line, **kw):
        d = {"crate": "c", "lint": "clippy::l", "level": "warning", "message": f"m{n}", "file": "f.rs", "span": f"s{n}",
             "line": line, "column": 1}
        d.update(kw)
        return d

    base = [diag(1, 10), diag(2, 20), diag(3, 30)]
    cases = [
        ("same", base, (0, 0, 0, 0)),
        ("one removed", base[:2], (0, 0, 1, 0)),
        ("one moved", [diag(1, 17), diag(2, 20), diag(3, 30)], (0, 0, 0, 1)),
        ("one new", base + [diag(4, 40, lint="clippy::other")], (1, 0, 0, 0)),
        ("renamed symbol: changed, not new", [diag(1, 10), diag(2, 20), diag(3, 30, message="m3 renamed")], (0, 1, 0, 0)),
        ("a second copy of an identity is new", base + [diag(1, 50)], (1, 0, 0, 0)),
    ]
    for name, head, want in cases:
        got = tuple(len(x) for x in compare(base, head))
        assert got == want, f"{name}: (new, changed, removed, moved) {got}, want {want}"
    sink = open("/dev/null", "w")
    assert run_compare(base, base + [diag(4, 40, lint="clippy::other")], strict=True, out=sink) == 1
    assert run_compare(base, base + [diag(4, 40, lint="clippy::other")], out=sink) == 0
    print(f"clippy_identity self-test: {len(cases) + 2} cases pass")


def load_baseline(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def main(args):
    if args == ["--self-test"]:
        self_test()
        return 0
    if len(args) == 2 and args[0] == "export":
        with open(args[1], encoding="utf-8") as f:
            for d in sorted(from_cargo(f), key=lambda d: (d["crate"], d["file"], d["line"], d["column"], d["lint"])):
                print(json.dumps(d, ensure_ascii=False, sort_keys=True))
        return 0
    if len(args) >= 3 and args[0] == "compare" and set(args[3:]) <= {"--github", "--strict"}:
        with open(args[2], encoding="utf-8") as f:
            head = from_cargo(f)
        return run_compare(load_baseline(args[1]), head, "--github" in args, "--strict" in args)
    sys.exit(__doc__)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
