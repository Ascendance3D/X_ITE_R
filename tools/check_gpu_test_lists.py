#!/usr/bin/env python3
"""The GPU test lists of gpu-tests.yml and macos-tests.yml must equal GPU_TESTS of rust-tests.yml (names only, same order not required)."""
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent


def names(path):
    text = (root / ".github/workflows" / path).read_text()
    block = re.search(r"GPU_TESTS: >-\n((?:\s+\|?\s*test\([^)]*\)\n)+)", text)
    if not block:
        sys.exit(f"{path}: no GPU_TESTS block")
    return set(re.findall(r"test\(([^)]*)\)", block.group(1)))


hosted = names("rust-tests.yml")
for other in ("gpu-tests.yml", "macos-tests.yml"):
    gpu = names(other)
    if hosted != gpu:
        sys.exit(f"GPU test lists differ: only in rust-tests.yml {sorted(hosted - gpu)}; only in {other} {sorted(gpu - hosted)}")
print(f"GPU test lists agree: {len(gpu)} names")
