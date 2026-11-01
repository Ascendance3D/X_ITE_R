#!/usr/bin/env python3
"""Check a ledger.json after the Batch 08 update (run on the local machine, after set_status.py).

    python3 -I tools/ledger_verify.py PATH/TO/ledger.json [--accept ID ...]

* Every entry outside the 20 selected ones is equal to ledger/before/ledger.json.
* Each selected entry has the status of tools/ledger_propose.py, or its ALT status when the ID is passed
  with --accept (an owner decision recorded elsewhere), or PARTIAL when a local gate failed for it.
* Earlier notes are kept (the before text is a prefix of the new text).
* The counts add up to 346. Prints the totals and coverage.
"""
import collections, json, os, sys

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(HERE, "tools"))
from ledger_propose import P  # noqa: E402

args = sys.argv[1:]
path = args[0]
accept = set(args[args.index("--accept") + 1:]) if "--accept" in args else set()
before = {i["id"]: i for i in json.load(open(os.path.join(HERE, "ledger", "before", "ledger.json")))["items"]}
after = {i["id"]: i for i in json.load(open(path))["items"]}
bad = []

if set(before) != set(after):
    bad.append(f"entry set differs: {sorted(set(before) ^ set(after))}")

for id_, b in before.items():
    a = after.get(id_)
    if a is None:
        continue
    if id_ not in P:
        if a != b:
            bad.append(f"unrelated entry changed: {id_}")
        continue
    status, alt = P[id_][0], P[id_][5]
    allowed = {status, "PARTIAL"} | ({"COMPLETE"} if id_ in accept and alt else set())
    if a["status"] not in allowed:
        bad.append(f"{id_}: status {a['status']}, allowed {sorted(allowed)}")
    if b.get("notes") and not a.get("notes", "").startswith(b["notes"]):
        bad.append(f"{id_}: earlier notes not kept")

c = collections.Counter(i["status"] for i in after.values())
print(dict(c), "total", sum(c.values()), f"coverage {c['COMPLETE'] / 346:.2%}")
if sum(c.values()) != 346:
    bad.append("total is not 346")
print("\n".join(bad) or "OK")
sys.exit(1 if bad else 0)
