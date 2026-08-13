---
id: TASK-102.7
title: Contractor payment report with W-9 and W-8BEN status for 1099-NEC
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
updated_date: '2026-08-13 17:30'
labels:
  - tax
  - reports
dependencies: []
parent_task_id: TASK-102
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
1120-S Schedule B question 14a asks whether the corporation filed all required Forms 1099. For the 2025 return that was answered from memory, and initially answered wrong. Nigel holds every contractor payment already — `Contract Labor` carries the whole year of them — but has no way to total them per payee or to say which payees needed a form.

The related problem is documentation status. One contractor is foreign, performed all work outside the United States, and is therefore outside 1099-NEC reporting and US withholding entirely under the service-source rule — but only because a signed Form W-8BEN is on file. That form expires at the end of the third full calendar year after signing, and the contractor has to be re-papered if the engagement outlives it. Nothing in the books records either fact.

## Proposal

Two pieces:

1. **Payee totals.** The `vendor` field already set by the rules engine is the natural grouping key. `nigel report 1099 --year <Y>` lists each vendor in reportable categories (Contract Labor, Legal & Professional, Rent) with the year's total, flagging those at or above the $600 threshold. Payments by credit card or third-party network are reportable by the processor, not the payer — worth surfacing the account type per payee so those can be excluded knowingly rather than accidentally.
2. **A payee register.** Vendor name, entity type, tax documentation on file (`W-9` / `W-8BEN` / none), the date it was signed, and a computed expiry for W-8BEN (31 December of the third full year after signing). The report flags a vendor over the threshold with no documentation, and any documentation expiring before the next filing season.

The vendor field is free text populated by rules today, so some normalisation or a proper `payees` table is likely needed for totals to be trustworthy — two rules writing `Adobe` and `Adobe Inc` produce two rows. Worth deciding early whether this rides on the existing `vendor` string or promotes payees to a first-class table; the latter would also serve invoicing's client list one day.

Relevant code: `src/db.rs` (payee register + migration), `src/reports.rs`, `src/cli/report/`, `src/categorizer.rs` (vendor normalisation).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `nigel report 1099 --year <Y>` totals payments per payee across reportable categories and flags those at or above the $600 threshold
- [ ] #2 Payee tax documentation (W-9, W-8BEN, none) and its signing date can be recorded, with W-8BEN expiry computed
- [ ] #3 The report flags payees over the threshold with missing documentation, and documentation expiring before the next filing season
- [ ] #4 Payments made by credit card or third-party payment network are identified so they can be excluded from 1099-NEC reporting deliberately
- [ ] #5 Vendor names group reliably, whether by normalisation or a first-class payee record
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
