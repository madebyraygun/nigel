---
id: TASK-9.7
title: 'Invoicing posting rules: A/R in the ledger, cash in the reports'
status: To Do
assignee: []
created_date: '2026-08-19 16:09'
labels:
  - architecture
  - invoicing
milestone: m-0
dependencies:
  - TASK-9.4
references:
  - >-
    backlog/decisions/decision-6 -
    Invoice-payments-post-to-accounts-receivable-cash-basis-reports-read-the-bank.md
parent_task_id: TASK-9
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implements decision-6 — the posting rule that ties invoicing to the ledger, and the guardrail that keeps the cash-basis promise. This is the single place the promise could break, so the task exists to make the rule enforced rather than understood.

## The rule (decision-6)

- An issued invoice posts to an A/R account (receivable against revenue).
- A recorded payment clears A/R.
- **Cash-basis reports exclude A/R entirely** — recognition happens when money hits the bank, which is what the register already records.
- **Unpaid invoices never appear in cash-basis income.** If one does, the change meant to strengthen the product has broken its primary use case.

## The link

`invoice_payments` gains a nullable `transaction_id` naming the bank deposit that settled the payment. Linking is suggested (same amount inside a date window) and **user-confirmed, never silent** — all financial modifications require confirmation. A linked deposit books its cash against A/R instead of an income category, so the money is recognised exactly once, at the bank. An amount difference (a processor deposit net of fees is the known case) must be categorised explicitly as part of confirming the link — a fee leg on the same entry — never absorbed. Unlinked disagreements surface on a reconciliation surface and nothing auto-resolves; the bank wins for cash-basis figures either way.

## Fixture

An issued-unpaid invoice (Acme) and a part-paid one (Cedar Systems), with invented amounts: the cash-basis P&L shows only the banked money, A/R carries the rest, and the parity/regression test pins it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An issued invoice posts to an A/R account and a recorded payment clears it, per decision-6
- [ ] #2 Cash-basis reports exclude A/R entirely: on a fixture with an unpaid invoice (Acme) and a part-paid one (Cedar Systems), the cash-basis P&L shows only banked money, and a test pins it
- [ ] #3 invoice_payments gains a nullable transaction_id; links are suggested by amount and date window but always user-confirmed, and an amount difference must be categorised explicitly (fee leg) as part of confirming
- [ ] #4 Unlinked disagreements surface on a reconciliation surface and nothing auto-resolves; cash-basis figures follow the bank either way
- [ ] #5 A/R appears on the balance sheet and trial balance, and never in cash-basis income
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
