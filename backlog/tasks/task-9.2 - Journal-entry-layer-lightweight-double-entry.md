---
id: TASK-9.2
title: Journal entry layer (lightweight double-entry)
status: To Do
assignee: []
created_date: '2026-08-13 15:45'
labels:
  - enhancement
  - architecture
dependencies:
  - TASK-9.1
parent_task_id: TASK-9
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/81'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Introduce a `journal_entries` table that wraps transactions into balanced debit/credit pairs, giving Nigel a real general ledger underneath its single-entry surface. Carries forward the second half of the original TASK-9; the classification half is now TASK-9.1.

## Motivation, re-scoped

This task was originally justified by the invoicing module needing AR tracking. That is no longer the driver: PR #172 shipped invoicing with its own `invoices` / `invoice_payments` tables, derived status, and aging buckets, all without a journal layer.

With TASK-9.1 delivering the account classification, most of what remained on the old motivation list — trial balance, a balance sheet with real equity, distributions out of the deduction totals — is achievable without journal lines. What journal lines still buy is the **structural guarantee**: the books cannot silently fail to tie, because nothing can be recorded that does not balance. Today's derivations are correct but have to be separately proven correct, and every new report that derives a balance is another place to get it wrong.

That is worth having. It is not worth blocking a filing deadline on.

## Open design question — a prerequisite, not a detail

`invoice_payments` has no `transaction_id`. An invoice payment and the bank deposit representing the same money are two unlinked records. On cash basis this is tolerable — the bank transaction is the source of truth for the P&L — but AR and the P&L can disagree with nothing tying them together.

If this layer generates journal entries from transactions while invoices sit in a parallel table, the result is either double-counted revenue or a permanent unreconciled gap. **Settle how invoice payments map to bank transactions before designing the schema.**

## Proposed approach

- Merge the chart of accounts: categories become accounts, bank accounts become accounts, all carrying the classes established in TASK-9.1
- Each transaction generates two journal lines (e.g. debit expense, credit bank)
- Existing import/review/categorize workflow unchanged — journal entries generated automatically
- Does NOT require: accrual-basis support, manual journal entries, full GL complexity

## Key constraint

The user-facing experience should remain simple. A freelancer importing a bank CSV should never see the word "debit" unless they go looking for it.

## Sequencing

Depends on TASK-9.1 for the class vocabulary. Nothing in TASK-102 depends on this task, and it should not be scheduled as tax-season work.

TASK-27 (trial balance) does **not** wait on this — it is derivable from single-entry data today and TASK-27 says so explicitly. The old sequencing note here claimed the opposite; that contradiction is resolved in TASK-27's favour. If this layer lands later, TASK-27's report should be reimplemented as a straight query over journal lines and its derivation logic dropped.

---
*Migrated from [GitHub issue #81](https://github.com/madebyraygun/nigel-keeps-your-books/issues/81)*
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The invoice-payment to bank-transaction mapping is decided and documented before schema work begins
- [ ] #2 Every transaction produces balanced journal lines over a merged chart of accounts, generated automatically from the existing workflow
- [ ] #3 Existing reports produce identical figures before and after the cutover on a real books fixture
- [ ] #4 No user-facing surface introduces debit/credit vocabulary
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
