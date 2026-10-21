#!/usr/bin/env python3
"""Batch 09 ledger check: python3 -I -B verify_b09.py [ledger.json]
Every entry outside tools/ledger_b09.py's P equals ledger/before/ledger.json (the 25 DIFFERENT included), the changed ones keep
earlier notes as a prefix, the set of IDs is unchanged, and the total is 346.

This checks the Batch 09 step only. Its default input is ledger/before-cloud/ledger.json, the ledger that step produced:
ledger_b09.AUTH names the ledger of another tree (xite-rust-full-port-rea-completion), which this repository does not have.
Later batches change the authoritative ledger on purpose, so run verify_ledger.py for it (verify_ledger.py replays this step
and every later one up to ledger.json); with an explicit path this tool still checks that file against ledger/before."""
import collections, json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
from ledger_b09 import P, BEFORE
AUTH = os.path.join(HERE, "..", "ledger", "before-cloud", "ledger.json")
after = {i["id"]: i for i in json.load(open(sys.argv[1] if len(sys.argv) > 1 else AUTH))["items"]}
before = {i["id"]: i for i in json.load(open(BEFORE))["items"]}
bad = []
if set(before) != set(after): bad.append(f"ID set differs: {sorted(set(before) ^ set(after))}")
for k, b in before.items():
    a = after.get(k)
    if a is None: continue
    if k not in P:
        if a != b: bad.append(f"unrelated entry changed: {k}")
        continue
    if b["status"] == "DIFFERENT": bad.append(f"DIFFERENT entry in P: {k}")
    if a["status"] != P[k][0]: bad.append(f"{k}: {a['status']} != {P[k][0]}")
    if b.get("notes") and not a["notes"].startswith(b["notes"]): bad.append(f"{k}: earlier notes lost")
c = collections.Counter(i["status"] for i in after.values())
print(dict(c), "total", sum(c.values()), f"coverage {c['COMPLETE']}/346 = {c['COMPLETE']/346:.2%}")
if sum(c.values()) != 346: bad.append("total is not 346")
print("\n".join(bad) or "OK"); sys.exit(1 if bad else 0)
