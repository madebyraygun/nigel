---
id: TASK-102.6
title: 'Tax payments ledger: separate estimates, franchise tax and penalties'
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
updated_date: '2026-08-13 19:49'
labels:
  - tax
  - reports
dependencies: []
parent_task_id: TASK-102
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`Taxes & Licenses` currently absorbs everything: business licenses, the California $800 minimum franchise tax, corporate estimated tax payments, payroll tax penalties. Three problems surfaced in the 2025 filing from that one bucket:

- A franchise tax payment was **double-counted** and only caught by manual reconciliation against FTB records.
- An April payment to the FTB was a corporate estimated tax payment but was recorded (and mislabeled by the state) as an "LLC Estimated Fee" — a distinction that determines whether it credits against the 100S at all, and which required a phone call to the state to unpick.
- The payroll-tax penalty portion sat inside the same line and had to be carved out as nondeductible (see TASK-102.4).

Two traps sit inside that simple-sounding rule, and the 2025 filing walked into both:

- **An S corporation rarely owes federal income tax at all.** Payments to the IRS from the business account are far more likely employer payroll taxes (deductible) or the *shareholder's personal* estimated tax paid from company funds — and the latter is a **distribution**, Schedule K line 16d, not a corporate payment credit. Filing it as a credit understates distributions, which is the single largest thing this epic exists to fix.
- **State tax is both.** California franchise and income tax paid by the corporation is deductible on the federal 1120-S *and* a credit against the CA 100S. A binary expense-or-credit model cannot express a treatment that differs by return. California's pass-through entity elective tax has the same shape — an entity-level federal deduction against a shareholder-level state credit — and is worth deciding on explicitly rather than discovering later.

## Proposal

Seeded sub-categories under a `Taxes` grouping, each with the right form mapping:

| Category | Treatment |
| --- | --- |
| Licenses & Permits | deductible, `1120S-12` |
| State Franchise Tax | deductible, `1120S-12` |
| Federal Estimated Tax | a payment credit **only** where the corporation itself owes tax (built-in gains, excess net passive income, LIFO recapture) |
| State Estimated Tax | not an expense — a payment credit |
| Payroll Taxes (employer) | deductible, `1120S-12` |
| Penalties & Fines | nondeductible, `K-16c` (shared with TASK-102.4) |

Plus `nigel report taxpayments --year <Y>` listing every payment with date, taxing authority, period it applies to, and confirmation number where captured — the ledger you take to the return's payments section, and the thing that makes a double-counted payment obvious at a glance.

The design question worth settling: whether estimated payments are a category type of their own (cleanest — they are neither expense nor equity) or an expense category excluded from the P&L by its form mapping (cheapest). If TASK-102.2 introduces an `equity` type, adding a fourth variant is less alarming than it sounds.

Relevant code: `src/db.rs` (seed + migration), `src/reports.rs`, `src/cli/report/`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Tax payments are seeded as distinct categories for licenses, franchise tax, federal and state estimates, employer payroll taxes and penalties; existing databases migrate without losing history
- [ ] #2 Payments to the IRS from the business account are classified by what they actually are: employer payroll taxes (deductible), or the shareholder's *personal* estimated tax paid from company funds, which is a distribution under Schedule K line 16d — not a corporate payment credit. A federal estimated-tax credit category applies only where the corporation itself owes tax (built-in gains, excess net passive income, LIFO recapture)
- [ ] #3 State franchise and income tax paid by the corporation is treated as deductible on the federal return **and** as a credit on the state return; the model can express both rather than forcing a single treatment
- [ ] #4 `nigel report taxpayments --year <Y>` lists each payment with date, authority, applicable period and amount
- [ ] #5 The report flags likely duplicates — same authority, same period, same amount — rather than leaving them to be spotted by eye
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
