---
id: TASK-102
title: 'Epic: year-end tax package — the 1120-S filing set from Nigel'
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
updated_date: '2026-08-19 16:39'
labels:
  - epic
  - tax
  - reports
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Umbrella for the work identified while preparing the 2025 federal 1120-S and California 100S by hand. The P&L side of Nigel held up well; everything *around* the P&L had to be reconstructed from bank statements, the prior year's return, and Gusto exports, and assembled into a worksheet outside the app.

Concretely, these are the things that cost time or nearly cost money in the 2025 filing:

- **Balance sheet (Schedule L).** No opening balances, so beginning-of-year cash, credit cards and the line of credit came from BofA statement PDFs. A discrepancy against the prior return's ending balance was found by eye, not by the software.
- **Distributions and AAA.** Owner distributions were the single largest missing number on Schedule K; the tax software's own AI panel flagged it before we did. Schedule M-2 (beginning AAA → income → nondeductibles → §179 → distributions) was computed on paper.
- **Officer vs. employee compensation.** The whole year's payroll was mapped to line 8 when it all belonged on line 7 (see TASK-45).
- **Nondeductibles.** The 50% meals disallowance and the penalty portion of a payroll-tax payoff both had to be carved out of expense lines by hand and re-entered on Schedule K line 16c.
- **Section 179.** The placed-in-service date for the year's equipment purchase came from an email receipt, not from Nigel. Form 4562 needs description, cost, and date.
- **Tax payments.** Federal estimates, the CA $800 minimum franchise tax, a corporate estimated payment mislabeled by the state as an LLC fee, and penalties all sat in one undifferentiated `Taxes & Licenses` bucket. A double-counted franchise payment was caught only by manual reconciliation.
- **1099 / W-8BEN.** Schedule B question 14a ("did you file required Forms 1099?") was answered from memory. Contractor totals and foreign-contractor documentation status live nowhere in the app.

The target state: `nigel report taxpackage --year <Y>` produces the complete set — P&L mapped to 1120-S lines, Schedule L, M-2/AAA, the officer/employee comp split, the asset list, the tax-payments ledger and the 1099 roster — and the filing becomes transcription rather than reconstruction.

Ordering is roughly the numbering, with one thing ahead of it: **TASK-9.1 (account classification) lands first**, because both the balance sheet and the equity treatment classify from it. TASK-102.3 is the most constrained: it consumes TASK-102.1, TASK-102.2, TASK-102.4 and TASK-102.5, and cannot be finished before them. The rest are independent and can be picked up in any order. TASK-102.9 is the capstone and should land last.

## Related existing tasks

- **TASK-9.1** (account classification) is a hard prerequisite for TASK-102.1 and TASK-102.2, and should be the first thing scheduled. TASK-9 was split for this reason — see below.
- **TASK-46** (opening balances + `--as-of`) is a hard prerequisite for TASK-102.1. Its priority should be raised to high on the strength of this epic.
- **TASK-27** (trial balance) overlaps TASK-102.1 — both need as-of-date balances and both classify from TASK-9.1. Whichever lands first should build the shared primitives.
- **TASK-45** (officer vs. employee wage split) is prerequisite to TASK-102.8 and is currently marked low priority; it belongs in this epic at medium.

## On TASK-9

TASK-9 (journal entry layer) was reviewed against this epic and **split** rather than either scheduled ahead of it or ignored:

- **TASK-9.1 — account classification.** Asset, liability, equity, revenue, expense as a shared vocabulary across accounts and categories. Additive and migratable. This epic depends on it, and building equity treatment without it would create migration debt the moment it landed.
- **TASK-9.2 — journal lines.** The general ledger proper. Deferred, and nothing here waits on it. It still carries an unresolved design question (how `invoice_payments` maps to bank transactions) that its own text calls a prerequisite, and it is a data-layer rewrite on live books. Neither belongs in front of a filing deadline.

The reasoning in short: a journal layer produces no tax output. Grant a perfect general ledger tomorrow and every task in this epic still has to be written. It makes two of them tidier and eliminates none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 TASK-102.1 through TASK-102.10 are Done, and TASK-9.1, TASK-45 and TASK-46 are Done as prerequisites
- [ ] #2 A full 1120-S filing can be completed from Nigel's exports alone, with no figure sourced from a bank statement PDF, a payroll export, or the prior year's return read by hand
- [ ] #3 A committed fixture set of books reproduces a known-good filing set through `nigel report taxpackage`, so the claim is verifiable by any contributor. Checking real books against a real return stays a private manual step and records no figures in the repository
- [ ] #4 The Claude skills in `.claude/skills/` reference the new commands and tax-aware categories, and `docs/skills.md` matches
- [ ] #5 Where the epic's scope stops is stated explicitly: which schedules are covered, whether the California 100S is in or out, and how each report behaves on the `personal` profile and for a Schedule C filer
- [ ] #6 **IMPORTANT**: Any PRs created from this epic must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TASK-27 (trial balance) is closed as superseded by TASK-9.11 on the v1 milestone; the related-tasks bullet about TASK-27 overlap now reads onto TASK-9.11.
<!-- SECTION:NOTES:END -->
