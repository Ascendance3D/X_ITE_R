# Lane Reports

When closing a lane or submitting a final PR for a batch of work, agents are expected to produce a lane report.

## Report Location

The report must be stored in a durable location outside this repository. **Never use `/tmp`.**

The durable location is the owner's workspace directory:
`~/Projects/cybertown/.reports/<topic>/<YYYY-MM-DD>-<lane>/`

**Topic Names:** The topic name (e.g., `xite-rust-project-recovery`) must be marked as needing owner approval in the PR body.

## Required Report Fields

At a minimum, every lane report must contain the following fields:

1. **Date:** The date the report was generated.
2. **Starting Master SHA:** The commit hash of the master branch when the work began.
3. **Branch / PR:** The name of the branch and the PR number (or URL).
4. **Scope:** A brief description of the work undertaken.
5. **Tests Run:** A summary of tests executed, including counts for passes, failures, ignores, and skips.
6. **Ledger Effect:** A description of changes to the REA ledger. Either the entries and the proposed status change, or "None" with a reason (e.g., "None: process/docs change, no REA entry").
7. **Protected-Evidence Check Result:** The outcome of evidence checks, ensuring X3D evidence skip rules were correctly followed.
8. **Remaining Risks:** Any known issues, technical debt, or potential problems left unaddressed.
