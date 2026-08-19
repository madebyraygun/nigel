---
id: TASK-9.9
title: 'Splits: one transaction, several category legs, on CLI, TUI and web'
status: To Do
assignee: []
created_date: '2026-08-19 16:10'
labels:
  - enhancement
milestone: m-0
dependencies:
  - TASK-9.8
parent_task_id: TASK-9
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Splits are the user-visible upside of the ledger: one transaction, several category legs. A mixed-purchase receipt — one card swipe at Globex covering equipment, supplies and sales tax — becomes expressible as one transaction split across categories, with the amounts summing to the whole.

It also retires a documented workaround: TASK-102.4 tells the operator to record a mixed payment (deductible tax, deductible interest, nondeductible penalty in one entry) as **separate transactions**, because single entry cannot split one. Once this lands, that payment is one transaction with three legs, the register still matches the bank statement line for line, and TASK-102.4's documentation gets updated to say so.

## Shape

- Splitting is an edit/review action on a transaction, available on all three surfaces (CLI, TUI, web): the user picks N categories and amounts that sum to the transaction. The account leg stays derived from the import; nobody is asked for two accounts or a direction, and no debit/credit vocabulary appears (decision-5, invariant 2).
- The ledger entry carries one line per split leg and still balances by construction (TASK-9.4's invariant does the enforcing).
- Reports count each leg in its category; the register keeps showing one bank-facing transaction, because that is what the statement shows.
- Rules keep matching whole transactions; splitting stays a manual act in v1. A rule that splits is future work, not this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A transaction can be split across multiple categories on all three surfaces, with leg amounts that must sum to the transaction amount
- [ ] #2 The ledger entry carries one line per split leg and still balances by construction; the register keeps showing one bank-facing transaction
- [ ] #3 Reports count each leg in its category, and the parity fixture gains a split transaction
- [ ] #4 The mixed-payment case TASK-102.4 documents as separate transactions is expressible as one split transaction, and TASK-102.4's documentation is updated to point at splits
- [ ] #5 The account leg stays derived; no surface asks for two accounts or a direction, and no debit/credit vocabulary appears
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
