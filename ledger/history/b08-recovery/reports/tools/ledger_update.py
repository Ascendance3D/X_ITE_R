#!/usr/bin/env python3
"""Batch 08 ledger update: one set_status.py call per selected entry, full text each time.
set_status.py REPLACES notes/tests/rust and takes SPACE-separated names. Merge prior text by hand.
Filled in only at the end of Batch 08, after the gate run. Empty until then."""
import subprocess, sys

S = "/home/ryan/Projects/cybertown/.reports/xite-rust-full-port-rea-completion/tools/set_status.py"
E = []  # (kind node|sub, status, names, fields)

if not E:
    sys.exit("Batch 08 ledger_update.py: no entries yet (skeleton)")
for kind, status, names, fields in E:
    cmd = [sys.executable, "-I", S, kind, status, names]
    for k, v in fields.items():
        cmd += [f"--{k}", v]
    subprocess.run(cmd, check=True)
