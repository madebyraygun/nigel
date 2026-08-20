---
id: TASK-120
title: 'Manual register entries: nigel add, a TUI form, and a web affordance'
status: To Do
assignee: []
created_date: '2026-08-19 17:04'
updated_date: '2026-08-20 14:19'
labels:
  - enhancement
milestone: v1
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every write path into the register runs through an import: `Import` creates rows, `Recategorize` and `Review` edit rows an import created, `Undo` rolls back an import. There is no command that adds a transaction. A business that takes or spends actual cash cannot record it; neither can an owner who paid for something personally, or anyone reconstructing a transaction their bank never saw. For a product whose primary audience is small cash-basis businesses this is a gap in the core loop, not a missing convenience.

`transactions.import_id` is already nullable, so manual rows need no schema change — but the code was written when no such row could exist. The audit of that assumption is part of this task; the known findings are below.

## Shape

A way to add a transaction directly — date, description, amount, account, category, optional vendor — on all three surfaces: a `nigel add` command, a TUI form, and a web UI affordance. Each follows its surface's existing conventions; the invoice draft form and the review flow are the closest patterns. **No new vocabulary**: this is the entry surface where the derived-second-leg invariant earns its keep — the user types one amount and picks one category, exactly as in the review flow. Nobody is asked for two accounts or a direction.

**Rules still apply.** A typed description runs through the categorization rules like any other transaction, so it gets the same suggested category an imported one would — one categorization path, not two. An entry saved without a category is flagged exactly as an unmatched import row is (`is_flagged = 1, 'No matching rule'`), so `review` finds it with no second queue.

## The decisions, argued

**Undo stays what it is.** `Undo` means "roll back the last import": `get_last_import` reads the newest `imports` row and `delete_import` deletes by `import_id` (`imports.rs`), so a manual row can never be swept into a rollback — the mechanics are already safe. What changes with manual rows is the *meaning* of "last": once one exists, the most recent activity can be a manual entry while `nigel undo` offers an older import. The confirmation prompt already names the file, date and count, which is most of the cure; add one informational line when manual entries are newer than the offered import, so nobody believes they are undoing their typo. Manual entries get **row-level edit and delete** instead of participating in undo — the honest semantic, and far cheaper than teaching the snapshot machinery a second mode.

**Edit and delete are new capability, deliberately asymmetric.** Today date, description and amount are immutable everywhere — `TransactionPatch` edits only category, vendor and flag, and the only row-deletion path in the product is `delete_import`. That stays true **for imported rows**: an imported row is a bank fact. A manual row (`import_id IS NULL`) is the user's own statement, so it is editable in full and deletable singly, with confirmation on every surface (`confirm_or_refuse` on the CLI, `confirmDialog` on the web, per the house rules).

**Duplicate detection warns, never blocks.** There is no file, so no checksum, and nothing like an OFX FITID. A same-day, same-amount warning on the same account is the most that is defensible, and it must be a warning — a cash business genuinely rings up two identical sales in a morning. One audit note to carry into the docs: `is_duplicate_row` (importer.rs) matches date + amount + description + account across **all** rows, so a manual row can suppress an identical row in a later bank import. With exact description match required this will rarely fire across origins (a typed description seldom equals the bank's), and when it does fire it is the desirable outcome for the reconstruction case. Documented, not built around.

**No snapshot per entry.** Imports snapshot because one command writes a whole statement of parsed rows. A manual entry is one row from an explicit form, and it has row-level delete — the undo *is* the delete. A snapshot per keystroke would bury the snapshots directory in noise and buy nothing the existing backup cadence does not.

## A use case the design must not preclude

Once the TASK-9.3 chart merge makes the equity categories pickable as accounts (TASK-9.1 supplies the class; the merge is what puts Owner Contribution in an account picker), a manual entry funded from an owner-contribution account records a business expense paid personally — the accountable-plan reimbursement case, currently unexpressible in Nigel. The account picker must simply offer every account rather than assuming bank products. Do **not** build the accountable-plan feature here.

## Out of scope

Recurring or scheduled entries (TASK-81 owns recurring generation), receipt attachment, petty-cash floats and till workflows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel add records a transaction with date, description, amount, account, category and optional vendor; the TUI form and web affordance do the same, each following its surface's existing form conventions
- [ ] #2 A typed description runs through the categorization rules exactly as an imported one does; an entry saved without a category is flagged for review the same way an unmatched import row is
- [ ] #3 A same-day, same-amount entry on the same account warns and never blocks, on all three surfaces
- [ ] #4 Manual rows (import_id IS NULL) can be edited in full and deleted singly, with confirmation; imported rows keep today's edit surface (category, vendor, flag) and remain undeletable row-by-row
- [ ] #5 Undo semantics are unchanged — it rolls back the last import by import_id and can never take a manual row with it (a test pins this); when manual entries are newer than the offered import, the prompt says so
- [ ] #6 No snapshot is taken per manual entry, and the reasoning is documented
- [ ] #7 No user-facing surface introduces debit/credit vocabulary; the user types one amount and picks one category
- [ ] #8 Update test coverage
- [ ] #9 Create or update documentation, making sure to remove any out of date information
- [ ] #10 All linting checks pass
- [ ] #11 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
- [ ] #12 The account picker offers every account rather than assuming bank products, so an owner-contribution-funded entry becomes expressible once the TASK-9.3 chart merge lands; no accountable-plan feature is built here
<!-- AC:END -->
