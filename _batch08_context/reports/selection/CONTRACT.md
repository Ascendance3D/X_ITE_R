# Batch 07 — completion contract applied

No written contract document exists. This is the contract this batch applies. It comes from the
Batch 07 brief (§3, §4, §5, §7) and the Batch 06 QA findings that caused the 06R downgrades.

An entry is COMPLETE only when all of these have evidence:

1. Real runtime: the node runs inside a loaded X3D scene through `x3d-runtime` / `x3d-render`
   (`x3d-drive`, `x3d-particles-drive`, `render_file`). Parse-only or unit-only code does not count.
2. X_ITE comparison: the same world runs in pinned X_ITE (`oracle/x_ite-pin`, `34cdf9f97`) and the
   result is compared:
   - pictures: at most 0.5 % of pixels more than 8 levels off, MSAA 0 and MSAA 4;
   - particles: per-particle state within the 06R limits, or the 06R statistical tests;
   - scene text and console log equal.
   Reference results are never edited to make a test pass.
3. Field changes: at least one inputOutput field of the node changed at run time (ROUTE or Script)
   with an X_ITE comparison after the change.
4. Lifecycle: scene load, scene reload (Inline or replace world), unload. Resources (particle
   buffers, GPU textures, buffers) are bounded over reloads and 0 after unload.
5. Removal from a live Group and, where the 06R release rules apply, a Script-created instance.
   For attached nodes that are not children (render styles, emitters, physics models, material
   extensions): assigned to and cleared from a live parent, with the parent removed from a live
   Group. (Clarified 2026-10-18 after implementation, before the ledger update. The owner may
   reject this reading; see the report, owner decisions.)
6. Particles only: at least two frame rates (the 06R rate list) for the time-dependent models.
7. GPU entries: MSAA 0 and 4.

PARTIAL: some of 1–7 are missing, and the missing items are listed in the ledger.
NOT_TESTED: code exists but 1–2 are not shown.
DIFFERENT: only with owner approval.
