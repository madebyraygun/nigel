---
id: TASK-109.8
title: 'Skills scaffold: contract docs, example skill and demo seed'
status: To Do
assignee: []
created_date: '2026-08-16 04:22'
labels:
  - documents
  - skills
  - docs
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The capstone that makes Documents an extension surface rather than one operator's feature:

- `docs/documents.md`: setup (the R2 objects, the optional accept Worker and its deploy steps, the config keys), the command reference, the lifecycle, and the signing-scope statement (recorded assent, no legal claim).
- The **skill contract** section: how a business-specific skill participates — gather content however it likes, render a PDF with whatever tools it owns, file it with `nigel document add` (or `POST /api/documents`), and optionally advance the lifecycle. The contract is the documented CLI/API surface; a private skill needs no code in this repository, which is the whole point.
- A generic example skill in `.claude/skills/` (e.g. `document-filer`) exercising the contract end to end with the fictional cast: takes a drafted document, files it against a demo client under the right kind, and reports the `nigel document send` command to run next. `docs/skills.md` updated to describe it.
- `nigel demo` seeds a handful of documents across the statuses (fictional cast, dated relative to today like the demo invoices, guarded like `insert_demo_invoicing`), so none of the three surfaces is empty on the database meant for exploring.
- CLAUDE.md and README updated per the documentation policy.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 docs/documents.md covers setup, the command reference, the lifecycle, the skill contract and the signing-scope statement
- [ ] #2 The example skill runs end to end on demo data using only public commands and the fictional cast, and docs/skills.md describes it
- [ ] #3 nigel demo seeds documents across the statuses, dated relative to today, guarded like the invoicing seed
- [ ] #4 CLAUDE.md and README are updated in the same push and ./scripts/check-no-real-data.sh passes
<!-- AC:END -->
