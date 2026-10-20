#!/usr/bin/env python3
"""Batch 09 cloud ledger update: targeted edits of the authoritative ledger (_batch09_context/ledger/ledger.json).

  python3 -I ledger_b09_cloud.py check     # verify only (exit 1 on a problem)
  python3 -I ledger_b09_cloud.py apply     # apply C to the authoritative ledger, then verify

ledger_b09.py is not used: its `apply` rebuilds from ledger/before/ledger.json and writes to another tree's path. This tool starts
from the authoritative ledger itself, and ledger/before-cloud/ledger.json is the snapshot it was taken from at the start of the
cloud session (231 COMPLETE, 55 PARTIAL, 25 DIFFERENT, 33 NOT_TESTED, 2 MISSING). `apply` is idempotent: an entry already at its
target is left as it is.

Each entry of C: (new status, tests appended, missing text, note appended). Earlier notes are kept as a prefix.
`verify` checks: the 346 IDs of the snapshot, every entry outside C equal to the snapshot, no DIFFERENT entry in C or changed, and
for each entry of C its status, its earlier notes kept and its earlier tests kept.
"""
import collections, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-cloud", "ledger.json")

# id: (status, tests, missing, note)
C = {
}


def apply():
    d = json.load(open(AUTH))
    for i in d["items"]:
        p = C.get(i["id"])
        if not p:
            continue
        st, tests, missing, note = p
        if i["status"] == st and note in (i.get("notes") or ""):
            continue
        i["status"] = st
        i["tests"] = list(i.get("tests") or []) + [t for t in tests if t not in (i.get("tests") or [])]
        i["missing"] = missing
        i["notes"] = (i.get("notes", "") + " | " if i.get("notes") else "") + note
    json.dump(d, open(AUTH, "w"), indent=1)


def verify():
    after = {i["id"]: i for i in json.load(open(AUTH))["items"]}
    before = {i["id"]: i for i in json.load(open(SNAP))["items"]}
    bad = []
    if set(before) != set(after):
        bad.append(f"ID set differs: {sorted(set(before) ^ set(after))}")
    for k, b in before.items():
        a = after.get(k)
        if a is None:
            continue
        if k not in C:
            if a != b:
                bad.append(f"unrelated entry changed: {k}")
            continue
        if b["status"] == "DIFFERENT":
            bad.append(f"DIFFERENT entry in C: {k}")
        if a["status"] != C[k][0]:
            bad.append(f"{k}: {a['status']} != {C[k][0]}")
        if b.get("notes") and not a["notes"].startswith(b["notes"]):
            bad.append(f"{k}: earlier notes lost")
        if any(t not in a.get("tests", []) for t in b.get("tests", [])):
            bad.append(f"{k}: earlier tests lost")
    c = collections.Counter(i["status"] for i in after.values())
    print(dict(sorted(c.items())), "total", sum(c.values()))
    if sum(c.values()) != 346:
        bad.append("total is not 346")
    if c["DIFFERENT"] != 25:
        bad.append("DIFFERENT count is not 25")
    print("\n".join(bad) or "OK")
    return not bad


if __name__ == "__main__":
    if sys.argv[1:] == ["apply"]:
        apply()
    sys.exit(0 if verify() else 1)
