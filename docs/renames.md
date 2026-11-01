# Renamed folders (2026-11-01)

Folders with a leading underscore or "batch" in the name were renamed. Older reports, ledger snapshots, ledger notes and the
DIFFERENT ledger entries still use the old names; read them through this table. `ledger/tools/verify_ledger.py` resolves the old
evidence paths with the same map.

| Old | New |
|---|---|
| `_batch09_context/ledger/ledger.json` | `ledger/ledger.json` |
| `_batch09_context/ledger/before*/` | `ledger/snapshots/before*/` |
| `_batch09_context/tools/` | `ledger/tools/` |
| `_batch09_context/{HANDOFF.md,cloud,nav-pointer,render-inputs,scene-sai,step2-errors,step3-*}` | `ledger/history/b09/…` |
| `_batch08_context/` | `ledger/history/b08-recovery/` |
| `oracle/rea-batchNN-<rest>` | `oracle/rea-bNN-<rest>` |

The ledger step `ledger/tools/ledger_rename.py` rewrote the old paths in the `tests` and `oracle` fields of 38 entries that are not
DIFFERENT. No status and no note changed. The snapshot files are unchanged, so every step tool keeps its sha256 pin.
The step tools `ledger_*.py` keep their old text and only run against the ledger of their own step; `verify_ledger.py` is the
check for the current ledger.
