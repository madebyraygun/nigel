---
id: TASK-102.10
title: 'Claude skill: year-end tax review'
status: To Do
assignee: []
created_date: '2026-08-13 15:20'
labels:
  - tax
  - skills
dependencies:
  - TASK-102.9
parent_task_id: TASK-102
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A third Claude skill alongside `importer-engineer` and `csv-rule-reviewer`: walk the books before filing, run the tax package, and hand back a punch list of what needs a human decision.

Deliberately scheduled after TASK-102.9 rather than written now. A skill that instructs Claude to run commands that do not exist yet is worse than no skill — it will improvise, and improvised tax figures are the failure mode this whole epic exists to prevent. The interim hints added to the two existing skills cover what is possible today.

## Workflow the skill should encode

1. Run the pre-flight from `nigel report taxpackage` and triage every warning.
2. Walk uncategorized and needs-mapping transactions, proposing categories with the tax consequence stated — not just "looks like software" but "Software & Subscriptions, 1120S-19, fully deductible."
3. Flag transactions that are probably misfiled for tax purposes even though they are categorized: equipment purchases over the de minimis threshold with no asset record, penalties sitting inside `Taxes & Licenses`, owner payments booked as expenses, contractor payments to payees with no W-9 on file.
4. Reconcile December so year-end balances are trustworthy before Schedule L is read off them.
5. Produce the punch list: what Claude changed, what needs a decision from the filer, and what has to come from outside the books (beginning AAA, prior-year figures, placed-in-service dates).

## Guardrails

The skill must state plainly that it prepares figures and does not give tax advice, and it must never silently pick a treatment where the answer depends on facts not in the books — §179 versus bonus depreciation, whether a payment is a penalty or interest, whether work was performed inside the United States. Those get asked, not assumed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `.claude/skills/year-end-tax-review/SKILL.md` exists and triggers on year-end and filing-preparation requests
- [ ] #2 The skill only invokes commands that exist at the time it ships
- [ ] #3 It produces a punch list separating what it changed, what needs a decision, and what must come from outside the books
- [ ] #4 It asks rather than assumes wherever the treatment depends on facts not recorded in the books
- [ ] #5 `docs/skills.md` documents the skill alongside the existing two
- [ ] #6 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
