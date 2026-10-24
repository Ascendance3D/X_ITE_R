#!/usr/bin/env python3
"""Repository-local ledger check: python3 -I -B verify_ledger.py [ledger.json]

Reads the authoritative ledger (default _batch09_context/ledger/ledger.json) and checks, without changing anything:
  - 346 unique IDs, every status one of the five allowed values
  - the promotion history: HISTORY is the chain of ledger snapshots and the tool that made each next step. For every step,
    the entries outside the tool's change set are byte-identical from one snapshot to the next, each changed entry has the
    status the tool sets (or keeps its status when the tool sets none) and keeps its earlier notes as a prefix. The last step
    ends at the authoritative ledger.
  - totals equal to the totals that follow from the first snapshot (ledger/before) and the statuses of the chain; they are not
    fixed numbers, so a new batch adds one step to HISTORY (its snapshot in ledger/before-bNN and its tool's change set)
  - the 25 DIFFERENT entries equal the Batch 09 snapshot (ledger/before-cloud) byte for byte
  - every file path named in an entry's tests / rust fields exists (missing ones are reported, never fixed)
Exit 1 on a hard failure; missing evidence paths are listed as warnings and do not change the exit code.

BASELINE_B09 keeps the totals this tool enforced before Batch 11 (the Batch 09 ledger after the ten promotions); it is printed
for reference only."""
import collections, importlib, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
LEDGER = os.path.join(HERE, "..", "ledger")
AUTH = sys.argv[1] if len(sys.argv) > 1 else os.path.join(LEDGER, "ledger.json")
ALLOWED = {"COMPLETE", "PARTIAL", "DIFFERENT", "NOT_TESTED", "MISSING"}
BASELINE_B09 = {"COMPLETE": 245, "PARTIAL": 74, "DIFFERENT": 25, "NOT_TESTED": 0, "MISSING": 2}
PATH = re.compile(r"\b((?:crates|oracle|tools|evidence|upstream)/[\w./@+-]+\.(?:rs|wrl|x3d|x3dv|html|mjs|js|json|py|sh|md|png))")

# (snapshot the step starts from, tool, its change set as {id: status or None}); a step ends at the next one's snapshot, the last
# at the authoritative ledger.
def _first(name, attr): return {k: v[0] for k, v in getattr(importlib.import_module(name), attr).items()}
HISTORY = [
    ("before", "ledger_b09.py P", lambda: _first("ledger_b09", "P")),
    ("before-cloud", "ledger_b09_cloud.py C", lambda: _first("ledger_b09_cloud", "C")),
    ("before-promotion", "ledger_b09_promote.py P", lambda: {k: "COMPLETE" for k in importlib.import_module("ledger_b09_promote").P}),
    ("before-b10", "ledger_b10.py C", lambda: _first("ledger_b10", "C")),
    ("before-b11", "ledger_b11.py C", lambda: _first("ledger_b11", "C")),
    ("before-b12", "ledger_b12.py C", lambda: _first("ledger_b12", "C")),
    ("before-b13", "ledger_b13.py C", lambda: _first("ledger_b13", "C")),
    ("before-b19", "ledger_b19.py C", lambda: _first("ledger_b19", "C")),
]

def load(p): return json.load(open(p))["items"]
items = load(AUTH)
bad, warn = [], []
ids = [i["id"] for i in items]
if len(ids) != 346 or len(set(ids)) != 346: bad.append(f"IDs: {len(ids)} total, {len(set(ids))} unique (want 346/346)")
cur = {i["id"]: i for i in items}
for i in items:
    if i["status"] not in ALLOWED: bad.append(f"{i['id']}: status {i['status']!r} not allowed")

snaps = [os.path.join(LEDGER, s, "ledger.json") for s, _, _ in HISTORY]
status = {i["id"]: i["status"] for i in load(snaps[0])}  # replayed through the chain
for n, (snap, tool, changes) in enumerate(HISTORY):
    last = n == len(HISTORY) - 1
    a = {i["id"]: i for i in load(snaps[n])}
    b = cur if last else {i["id"]: i for i in load(snaps[n + 1])}
    step = f"{snap} -> {'ledger.json' if last else HISTORY[n + 1][0]} ({tool})"
    if set(a) != set(b): bad.append(f"{step}: ID set differs: {sorted(set(a) ^ set(b))}"); continue
    C = changes()
    for k in a:
        if k not in C:
            if a[k] != b[k]: bad.append(f"{step}: entry outside the change set changed: {k}")
            continue
        if "DIFFERENT" in (a[k]["status"], b[k]["status"]): bad.append(f"{step}: DIFFERENT entry in the change set: {k}")
        want = C[k] or a[k]["status"]
        if b[k]["status"] != want: bad.append(f"{step}: {k} is {b[k]['status']}, the tool sets {want}")
        if a[k].get("notes") and not b[k]["notes"].startswith(a[k]["notes"]): bad.append(f"{step}: {k}: earlier notes lost")
        status[k] = want
    print(f"step {step}: {len(C)} entries, totals after {dict(sorted(collections.Counter(i['status'] for i in b.values()).items()))}")

c = collections.Counter(i["status"] for i in items)
expected = collections.Counter(status.values())
for s in ALLOWED:
    if c.get(s, 0) != expected.get(s, 0): bad.append(f"total {s}: {c.get(s, 0)} != {expected.get(s, 0)} from the promotion history")

snap = {i["id"]: i for i in load(os.path.join(LEDGER, "before-cloud", "ledger.json"))}
diff = [k for k, v in snap.items() if v["status"] == "DIFFERENT"]
if len(diff) != 25: bad.append(f"snapshot has {len(diff)} DIFFERENT, want 25")
for k in diff:
    if cur.get(k) != snap[k]: bad.append(f"DIFFERENT entry changed: {k}")

for i in items:
    refs = set(PATH.findall(" ".join(i.get("tests", []) + i.get("rust", []))))
    for r in sorted(refs):
        if not os.path.exists(os.path.join(ROOT, r)): warn.append(f"{i['id']}: missing evidence path {r}")
print(dict(c), "total", sum(c.values()))
print("history baseline (Batch 09, not enforced):", BASELINE_B09)
for w in warn: print("WARN", w)
print(f"{len(warn)} missing evidence references")
print("\n".join(bad) or "OK"); sys.exit(1 if bad else 0)
