---
id: TASK-9.7
title: >-
  Invoicing reconciliation: payments linked to bank deposits, off the ledger in
  v1
status: To Do
assignee: []
created_date: '2026-08-19 16:09'
updated_date: '2026-08-19 16:40'
labels:
  - architecture
  - invoicing
milestone: m-0
dependencies:
  - TASK-9.4
references:
  - >-
    backlog/decisions/decision-6 -
    Invoicing-stays-off-the-ledger-in-v1-recognition-is-the-bank-deposit.md
parent_task_id: TASK-9
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implements decision-6: invoicing stays off the ledger in v1, and what ties the two subsystems together is a reconciliation link, not a posting rule. There is no A/R account in v1 — a deposit categorized to income is the recognition, once, at the bank, exactly as today — so this task's job is to make the invoicing tables and the register checkable against each other instead of silently divergent.

## The link

`invoice_payments` gains a nullable `transaction_id` naming the bank deposit that settled the payment. Links are suggested — same amount inside a date window — and **user-confirmed, never silent**: all financial modifications require confirmation. An amount difference (a processor deposit net of fees is the known case) is recorded on the link as the explanation; nothing books a fee leg in v1, and the deposit stays the income figure.

## The disagreement surface

A payment with no matching deposit inside the window, or a difference the operator has not confirmed, surfaces on a reconciliation surface — a queue, not a heuristic. Nothing auto-resolves, and cash-basis figures follow the bank unconditionally, because the bank is the only thing posting.

## The guardrail

This is the single place the cash-basis promise could break, so the fixture pins it: an issued-unpaid invoice (Acme) and a part-paid one (Cedar Systems), with invented amounts — neither moves the cash-basis P&L by a cent, and no A/R appears on any v1 report. When accrual ships later, this link is also what the opening-A/R derivation reads; nothing more is built for that now.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Invoices and invoice payments post nothing to the ledger; no A/R account exists on any v1 surface, and a deposit categorized to income remains the sole recognition path
- [ ] #2 invoice_payments gains a nullable transaction_id; links are suggested by amount within a date window and always user-confirmed, and an amount difference is recorded on the link as its explanation without booking a fee leg
- [ ] #3 Unmatched payments and unconfirmed differences surface on a reconciliation surface; nothing auto-resolves, and cash-basis figures follow the bank unconditionally
- [ ] #4 On a fixture with an issued-unpaid invoice (Acme) and a part-paid one (Cedar Systems), the cash-basis P&L is unchanged by both, and a test pins it
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
