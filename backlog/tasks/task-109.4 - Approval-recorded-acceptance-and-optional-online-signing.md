---
id: TASK-109.4
title: 'Approval: recorded acceptance and optional online signing'
status: To Do
assignee: []
created_date: '2026-08-16 04:22'
labels:
  - documents
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The epic's point. Two paths to one acceptance record:

- **Manual — the baseline, zero infrastructure:** `nigel document accept <id> --name "…" [--date …] [--method email]` and `nigel document decline <id>` record who, when and how. Guards in the data layer: only a sent document; terminal states refuse. On acceptance of a published document the page is republished stamped "Accepted by NAME on DATE" with the accept form removed — best-effort on the republish precedent: the acceptance is recorded either way, and a failed republish is a warning, never a lost acceptance.
- **Online — optional:** the published page's accept form takes a typed name and POSTs to a small generic Worker (shipped in-repo under e.g. `workers/document-accept/`, deployed by the operator beside their existing R2 custom domain) that writes an acceptance object beside the page (`d/{token}/acceptance.json`: name, timestamp). `nigel document sync` lists and pulls acceptance objects and records them idempotently — the `invoice sync` shape, including a SyncReport-style result with per-document failures as data. The form renders only when the accept endpoint is configured; otherwise it is absent (PayButton live/inert/absent precedent). The record states its method (`online` vs whatever the operator typed for a manual one).
- **What signing means here** is stated in the docs and on the surfaces: a recorded assent with a name, a timestamp and a method — an audit trail, not an e-signature product, and no claim about legal enforceability.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Manual accept and decline record name, date and method, refuse non-sent and terminal documents in the data layer, and status derives from the timestamps
- [ ] #2 Accepting a published document republishes the page stamped with the acceptance and without the accept form; a failed republish is a warning, never a lost acceptance
- [ ] #3 The Worker script is generic (no operator-specific content), committed with deploy docs, and writes only an acceptance object
- [ ] #4 nigel document sync is idempotent — re-running records nothing twice — and reports per-document results as data
- [ ] #5 With no accept endpoint configured the published page carries no accept form, and every other path still works
<!-- AC:END -->
