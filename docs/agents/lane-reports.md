# Lane Reports

When closing a lane or submitting a final PR for a batch of work, agents are expected to produce a lane report.

## Report Location and Structure

The report must be stored in a durable location outside this repository. **Never use `/tmp`.**

The durable location is the owner's workspace directory, structured as follows:
`<reports root>/<topic>/<YYYY-MM-DD>-<lane>/REPORT.md` (the maintainer sets the reports root, outside the repository)

**Topic Names:** The topic name (e.g., `xite-rust-project-recovery`) is **PROPOSED and PENDING OWNER APPROVAL**, not final. This must be noted in the PR body.

**Report Structure Rules:**
* There must be exactly one dated directory per lane, containing the `REPORT.md` file.
* A `SHA256SUMS` file covering every file in that dated directory must be included alongside the report.
* All evidence files associated with the lane must stay inside that dated directory.
* Never delete or overwrite an older dated directory.
* Only update `<topic>/LATEST.md` after the new report is completely written and its `SHA256SUMS` file has been verified.

## Required Report Fields

At a minimum, every lane report must contain the following fields:

1. **Date:** The date the report was generated.
2. **Starting Master SHA:** The commit hash of the master branch when the work began.
3. **Branch / PR:** The name of the branch and the PR number (or URL).
4. **Scope:** A brief description of the work undertaken.
5. **Tests Run:** A summary of tests executed, including counts for passes, failures, ignores, and skips.
6. **Ledger Effect:** A description of changes to the REA ledger. Either the entries and the proposed status change, or "None" with a reason (e.g., "None: process/docs change, no REA entry").
7. **Protected-Evidence Check Result:** The outcome of evidence checks, ensuring proper adherence to `crates/x3d-evidence` policies and listing any exceptions from `X3D_EVIDENCE_LOG`.
8. **Remaining Risks:** Any known issues, technical debt, or potential problems left unaddressed.
