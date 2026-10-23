---
id: TASK-154.18
title: 'First-run guide for new, empty books'
status: To Do
assignee: []
created_date: '2026-10-23 16:34'
labels:
  - web
  - onboarding
  - design
milestone: m-3
dependencies: []
parent_task_id: TASK-154
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After Setup creates brand-new books (Start from scratch, not demo or loaded books), the operator lands on an empty Dashboard with nothing to do next. Offer a short guided path through the four things that make the books useful:
1. Set up an account (the bank or card account statements will come from).
2. Review the chart of accounts (the categories the profile seeded; rename, add or hide).
3. Import a statement into that account.
4. Review any uncategorized transactions.

Each step hands off to the real screen (Accounts, Categories, Import, Review) rather than duplicating it, and the guide shows progress and what's next. It complements Setup (TASK-154.9) and the Swift shell's first run (TASK-150.2); it never appears for demo books or books loaded from a folder.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New empty books show a four-step guide (account, chart of accounts, import, review) with each step's state derived from the books themselves (an account exists, a statement imported, nothing left uncategorized)
- [ ] #2 Each step opens the real screen for that job and the guide returns with the next step highlighted; steps can be done out of order
- [ ] #3 The guide can be dismissed and reopened from the Dashboard or the palette, and never shows for demo or loaded books
- [ ] #4 Components in @nigel/ui with previews (each step state, complete, light and dark) and describePreviewA11y; works at phone width
<!-- AC:END -->
