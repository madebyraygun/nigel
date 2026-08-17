---
id: TASK-109.10
title: Paginated page and PDF rendering for prose documents
status: To Do
assignee: []
created_date: '2026-08-17 04:55'
labels:
  - documents
dependencies: []
parent_task_id: TASK-109
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Drafted documents are prose, and prose crosses pages. The invoice PDF renderer draws at fixed offsets with no page-break logic (the `MAX_ADDRESS_LINES` clamp exists because of it), so this task builds the missing renderer rather than stretching that one:

- Markdown body + per-kind template → the published page (HTML) and a multi-page PDF: page breaks, margins, a running header/footer (title, page number), the `Branding` block — both renderings fed by one decision layer (`invoicing/document.rs` precedent: decide once, render twice).
- Used by preview and send for drafted documents. Filed PDFs bypass it — they arrive rendered — and get only the viewer wrapper page.
- Pure and offline: no network, no configuration, exercised entirely by tests.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A long fixture document (several pages of headings, paragraphs and lists) renders to a PDF with correct page breaks and page numbers
- [ ] #2 The page and the PDF are fed by one shared decision layer; no value that appears on both is computed twice
- [ ] #3 Rendering needs no network and no configuration, and tests cover it with fictional-cast fixtures
- [ ] #4 A filed PDF is never re-rendered; it gets the viewer wrapper page only
<!-- AC:END -->
