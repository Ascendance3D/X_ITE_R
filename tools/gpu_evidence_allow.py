#!/usr/bin/env python3
"""Print X3D_EVIDENCE_ALLOW for the GPU runner: every answer key of oracle/answer-keys.tsv, comma separated.

The runner is a public-repository runner and never holds the private keys, so every manifest key may be absent. A GPU
adapter is not a key: no-gpu-adapter and backend-absent are not listed, so a missing GPU still fails. A key added to the
manifest is allowed here without an edit of the workflow (the hand-kept list missed x1-baseline.json, run 41275398837).
"""
import sys
from pathlib import Path

manifest = Path(__file__).resolve().parent.parent / "oracle" / "answer-keys.tsv"
names = []
for line in manifest.read_text().splitlines():
    if not line.strip() or line.startswith("#"):
        continue
    fields = line.split("\t")
    if len(fields) < 4:
        sys.exit(f"{manifest}: bad row {line!r}")
    names.append(fields[0])
if not names:
    sys.exit(f"{manifest}: no keys")
print(",".join(names))
