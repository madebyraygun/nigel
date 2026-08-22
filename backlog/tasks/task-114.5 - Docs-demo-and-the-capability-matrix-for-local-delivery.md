---
id: TASK-114.5
title: 'Docs, demo and the capability matrix for local delivery'
status: To Do
assignee: []
created_date: '2026-08-17 13:27'
updated_date: '2026-08-21 00:21'
labels:
  - docs
milestone: m-0
dependencies: []
parent_task_id: TASK-114
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The capstone: the mode is documented as a choice with stated costs, and the demo proves it.

- The invoicing and documents docs gain a delivery section: the setting, the outbox layout, the compose paths per platform, the mailto fallback and its attachment limitation, and the attested-sent statement.
- A hosted-versus-local capability matrix in one place: what each mode delivers, what local gives up (hosted page, pay button, online acceptance, republish, delivery observability, recurring auto-send) and what survives (everything else, including invoice sync when Stripe is configured).
- `nigel demo` plus `delivery = "local"` works end to end with zero cloud configuration — the desktop first-run story.
- README updated per the documentation policy: local delivery is the no-accounts quick start for invoicing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The delivery section and the capability matrix are documented where invoicing and documents are documented
- [ ] #2 On a fresh demo database with delivery=local and no cloud keys, the full invoice and document lifecycles complete end to end
- [ ] #3 README describes local delivery as the zero-configuration path, and ./scripts/check-no-real-data.sh passes
<!-- AC:END -->
