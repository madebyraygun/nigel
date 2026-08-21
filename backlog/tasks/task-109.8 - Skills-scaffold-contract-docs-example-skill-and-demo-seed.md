---
id: TASK-109.8
title: 'Skills scaffold: contract docs, example skill and demo seed'
status: To Do
assignee: []
created_date: '2026-08-16 04:22'
updated_date: '2026-08-21 00:21'
labels:
  - documents
  - skills
  - docs
milestone: m-1
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The capstone that makes Documents an extension surface rather than one operator's feature:

- `docs/documents.md`: setup (the R2 objects, the optional accept Worker and its deploy steps, the config keys), the command reference, the lifecycle (draft → sent → accepted → executed, with declined and withdrawn terminal), templates and versioning, and the signing-scope statement (recorded two-party assent, no legal claim).
- The **skill contract** section covers both halves: hand Nigel content — `nigel document draft … --body-file` or a Markdown body through `POST /api/documents` — and let Nigel render it, or render a PDF with whatever tools the skill owns and file it with `nigel document add`. Either way the contract is the documented CLI/API surface; a private skill needs no code in this repository, which is the whole point.
- A generic example skill in `.claude/skills/` (e.g. `document-drafter`) exercising the drafting half end to end with the fictional cast: takes gathered content, drafts it against a demo client under the right kind, and reports the `nigel document send` command to run next; the filed half is exercised in the same skill's docs with a one-command example. `docs/skills.md` updated to describe it.
- `nigel demo` seeds documents across the statuses including executed, spanning both sources (fictional cast, dated relative to today like the demo invoices, guarded like `insert_demo_invoicing`), so none of the three surfaces is empty on the database meant for exploring.
- CLAUDE.md and README updated per the documentation policy.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 docs/documents.md covers setup, the command reference, the lifecycle through executed, templates, versioning, both halves of the skill contract and the signing-scope statement
- [ ] #2 The example skill runs end to end on demo data using only public commands and the fictional cast, exercising the drafting half of the contract, and docs/skills.md describes it
- [ ] #3 nigel demo seeds documents across the statuses including executed, spanning both sources, dated relative to today, guarded like the invoicing seed
- [ ] #4 CLAUDE.md and README are updated in the same push and ./scripts/check-no-real-data.sh passes
<!-- AC:END -->
