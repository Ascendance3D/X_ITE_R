#!/usr/bin/env python3
"""Repository-local ledger check: python3 -I verify_ledger.py [ledger.json]

Reads the authoritative ledger (default _batch09_context/ledger/ledger.json) and checks, without changing anything:
  - 346 unique IDs, every status one of the five allowed values
  - totals equal to EXPECTED (edit EXPECTED when a ledger commit changes them)
  - the 25 DIFFERENT entries equal the Batch 09 snapshot (ledger/before-cloud) byte for byte
  - the recorded promotion history: every entry of ledger_b09_promote.P is COMPLETE and its notes keep the pre-promotion notes
  - every file path named in an entry's tests / rust fields exists (missing ones are reported, never fixed)
Exit 1 on a hard failure; missing evidence paths are listed as warnings and do not change the exit code."""
import collections, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from ledger_b09_promote import P as PROMOTED  # the 10 Batch 09 node promotions
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
AUTH = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-cloud", "ledger.json")
PRE = os.path.join(HERE, "..", "ledger", "before-promotion", "ledger.json")
ALLOWED = {"COMPLETE", "PARTIAL", "DIFFERENT", "NOT_TESTED", "MISSING"}
EXPECTED = {"COMPLETE": 245, "PARTIAL": 74, "DIFFERENT": 25, "NOT_TESTED": 0, "MISSING": 2}
PATH = re.compile(r"\b((?:crates|oracle|tools|evidence|upstream)/[\w./@+-]+\.(?:rs|wrl|x3d|x3dv|html|mjs|js|json|py|sh|md|png))")

def load(p): return json.load(open(p))["items"]
items = load(AUTH)
bad, warn = [], []
ids = [i["id"] for i in items]
if len(ids) != 346 or len(set(ids)) != 346: bad.append(f"IDs: {len(ids)} total, {len(set(ids))} unique (want 346/346)")
cur = {i["id"]: i for i in items}
for i in items:
    if i["status"] not in ALLOWED: bad.append(f"{i['id']}: status {i['status']!r} not allowed")
c = collections.Counter(i["status"] for i in items)
for s in ALLOWED:
    if c.get(s, 0) != EXPECTED[s]: bad.append(f"total {s}: {c.get(s, 0)} != {EXPECTED[s]}")
snap = {i["id"]: i for i in load(SNAP)}
if set(snap) != set(cur): bad.append(f"ID set differs from snapshot: {sorted(set(snap) ^ set(cur))}")
diff = [k for k, v in snap.items() if v["status"] == "DIFFERENT"]
if len(diff) != 25: bad.append(f"snapshot has {len(diff)} DIFFERENT, want 25")
for k in diff:
    if cur.get(k) != snap[k]: bad.append(f"DIFFERENT entry changed: {k}")
pre = {i["id"]: i for i in load(PRE)}
if len(PROMOTED) != 10: bad.append(f"promotion history lists {len(PROMOTED)} entries, want 10")
for k in PROMOTED:
    a, b = cur.get(k), pre.get(k)
    if a is None or b is None: bad.append(f"{k}: missing from ledger or pre-promotion snapshot"); continue
    if a["status"] != "COMPLETE": bad.append(f"{k}: promotion undone, status {a['status']}")
    if b.get("notes") and not a["notes"].startswith(b["notes"]): bad.append(f"{k}: pre-promotion notes lost")
    if "Promotion review" not in a["notes"]: bad.append(f"{k}: no promotion note")
for i in items:
    refs = set(PATH.findall(" ".join(i.get("tests", []) + i.get("rust", []))))
    for r in sorted(refs):
        if not os.path.exists(os.path.join(ROOT, r)): warn.append(f"{i['id']}: missing evidence path {r}")
print(dict(c), "total", sum(c.values()))
for w in warn: print("WARN", w)
print(f"{len(warn)} missing evidence references")
print("\n".join(bad) or "OK"); sys.exit(1 if bad else 0)
