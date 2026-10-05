---
id: TASK-109.4
title: 'Approval: acceptance, countersigning and optional online signing'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-16 04:22'
updated_date: '2026-10-05 15:16'
labels:
  - documents
milestone: m-1
dependencies: []
references:
  - docs/superpowers/specs/2026-10-05-documents-signing-design.md
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The epic's point. Two signatures, one record each, both bound to the sent version:

- **Client acceptance — manual baseline, zero infrastructure:** `nigel document accept <id> --name "…" [--date …] [--method email]` and `nigel document decline <id>` record who, when and how, against the version that was sent. Guards in the data layer: only a sent document; terminal states refuse. On acceptance of a published document the page is republished stamped "Accepted by NAME on DATE" — best-effort on the republish precedent: the acceptance is recorded either way, and a failed republish is a warning, never a lost acceptance.
- **Client acceptance — online, optional:** the published page's accept form takes a typed name and POSTs to a small generic Worker (shipped in-repo under e.g. `workers/document-accept/`, deployed by the operator beside their existing R2 custom domain) that writes an acceptance object beside the page (`d/{token}/acceptance.json`: name, timestamp). `nigel document sync` lists and pulls acceptance objects and records them idempotently — the `invoice sync` shape, including a SyncReport-style result with per-document failures as data. The form renders only when the accept endpoint is configured; otherwise it is absent (PayButton live/inert/absent precedent). The record states its method (`online` vs whatever the operator typed for a manual one).
- **Operator countersign:** `nigel document countersign <id> --name "…" [--date …] [--method …]` — admitted only from accepted; records the second signature against the same version and carries the status to *executed*, the terminal happy state. The page is republished stamped with both signatures, accept form removed.
- **What signing means here** is stated in the docs and on the surfaces: recorded assent — two names, two timestamps, a method each, bound to a checksummed version — an audit trail, not an e-signature product, and no claim about legal enforceability.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Countersign records the operator's name, date and method against the same version, is admitted only from accepted, and carries the document to executed; executed is terminal
- [ ] #2 Accepting or countersigning a published document republishes the page stamped with every signature recorded so far and without the accept form; a failed republish is a warning, never a lost signature
- [ ] #3 With no accept endpoint configured the published page carries no accept form, and every other path still works
- [ ] #4 Manual accept, request-changes and decline record name, date, method (and note) against the latest sent version per the spec's transition table
- [ ] #5 The generic Worker (workers/document-response) validates role, version, checksum, typed name (signer accept) or note (request changes) against the private manifest, writes the response once per recipient per version, and rate-limits per recipient token; vitest covers each refusal
- [ ] #6 nigel document sync reads responses through a ResponseSource trait, refuses version/checksum mismatches, records each response once (re-running records nothing twice), closes the manifest, republishes pages, and reports per document as data
<!-- AC:END -->
