---
id: TASK-109.2
title: 'File a document: nigel document add, list, show and kinds'
status: To Do
assignee: []
created_date: '2026-08-16 04:22'
updated_date: '2026-08-17 04:56'
labels:
  - documents
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Ingest for the *filed* source — a document produced outside Nigel arrives as a finished PDF and is filed against a client (the drafted source is task 109.9):

- `nigel document add <file.pdf> --client <id> --kind proposal --title "…"` copies the file under `<data_dir>/documents/`, records path + checksum, and creates the row as a draft. A duplicate (same client + same checksum) is refused as a structured Conflict naming the existing document (the imports checksum precedent) — filing the same rendered PDF twice is almost always a mistake.
- Basic surface: `nigel document list` (status/kind/client filters), `nigel document show <id>` (detail including source, file or body, versions, status, dates and the signature records), `nigel document kinds` to list/add/deactivate kinds.
- Printing goes through pure `format_*` functions (`cli/invoice.rs` precedent) so future parity fixtures can capture a text side without a terminal.
- Filed ingest is PDF-only in v1 by design: it is what external tools produce and what send attaches. The file is validated as a PDF by magic bytes, not extension.

This command is one of the skills contract's two core verbs — a business-specific skill renders a PDF with whatever tools it owns and files it with one command; nothing here knows or cares what produced the file. (The other verb — handing Nigel Markdown to render — is task 109.9's `--body-file`.)
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Filing a valid PDF creates a draft linked to the client and stores the file under the data directory with its checksum recorded
- [ ] #2 A duplicate (same client, same checksum) is refused as a structured Conflict naming the existing document
- [ ] #3 A non-PDF is refused by content (magic bytes), not by extension
- [ ] #4 list, show and kinds print through pure format functions, with fictional-cast fixtures; show carries source, versions and the signature records
- [ ] #5 An archived client refuses a new document with the data layer's own sentence
<!-- AC:END -->
