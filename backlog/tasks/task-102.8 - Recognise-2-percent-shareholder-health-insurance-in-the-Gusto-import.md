---
id: TASK-102.8
title: Recognise 2% shareholder health insurance in the Gusto import
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
labels:
  - tax
  - importers
dependencies:
  - TASK-45
parent_task_id: TASK-102
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Health insurance premiums for a 2% S-corporation shareholder-employee are not a benefits expense — they are additional wages. Gusto handles the mechanics through a dedicated pay item: the amount lands in W-2 Box 1 and Box 16, is exempt from Social Security, Medicare, FUTA and SDI, and prints a Box 14 notation. On the 1120-S it belongs with officer compensation on line 7, and it unlocks the self-employed health insurance deduction on the shareholder's personal return.

The 2025 books paid premiums personally rather than through payroll, so the deduction was forfeited. The pay item goes into Gusto before December 2026, which means the importer will start seeing it in the 2026 payroll exports whether or not it knows what it is. As things stand it will land in `Payroll — Benefits` (`1120S-18`) — the wrong line, and one that quietly understates officer compensation.

## Proposal

Building on TASK-45's officer/employee split (which is a prerequisite — this is the same per-employee parsing problem with an extra pay item):

- Recognise the 2% shareholder health pay item in the Gusto XLSX parser and route it to officer compensation (`1120S-7`), not to `Payroll — Benefits`.
- Report it separately within the officer compensation figure, since it is needed as its own number twice over: on the shareholder's W-2 Box 14, and on the personal return's self-employed health insurance deduction worksheet.
- Surface the annual total in the K-1 prep worksheet so the personal-return figure comes out of Nigel rather than out of a W-2 read by hand.

Gusto's exact column label for this item should be confirmed against a real export before coding to it; the label has been known to vary between report types. Fail loudly on an unrecognised benefit column rather than defaulting it into `Payroll — Benefits` — a silent misclassification here costs a deduction, which is precisely how 2025 went.

Relevant code: `src/importer.rs` (Gusto importer), `src/db.rs` (categories), `src/reports.rs` (K-1 worksheet).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Gusto importer recognises the 2% shareholder health insurance pay item and books it to officer compensation, not to benefits
- [ ] #2 The annual total is reported as its own figure within officer compensation on the K-1 prep worksheet
- [ ] #3 An unrecognised payroll benefit column is surfaced as a warning rather than silently defaulted
- [ ] #4 Tests cover a Gusto export fixture containing the pay item
- [ ] #5 Documentation explains the treatment and why it differs from ordinary employee health benefits
- [ ] #6 Update test coverage
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
