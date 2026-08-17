---
id: TASK-109.7
title: 'Web UI: documents screen'
status: To Do
assignee: []
created_date: '2026-08-16 04:22'
updated_date: '2026-08-17 04:57'
labels:
  - documents
  - spa
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`#/documents` on the invoices screen's arrangement — one screen with views keyed off `ctx.params`, filters as links — plus:

- Upload via `wc-dropzone` (a PDF variant of the import screen's lazy-upload flow), filing against a client and kind — the filed source. Drafting is a form (client, kind, title) that creates from the kind's template and opens the editor.
- A Markdown editor for draft bodies — a `wc-markdown-editor` component in `@nigel/ui` (textarea plus rendered preview), saving through `PATCH /api/documents/{id}`.
- Detail with the published page / PDF previewed through `wc-document-frame` (which already owns every iframe and the sandbox constant), collapsed by default like the invoice preview; the version list and both signature records rendered on the detail, each signature with the version it binds to.
- Send in a dialog that survives its own request and shows the step trace (`wc-send-dialog` precedent, `wa-hide` prevented mid-flight); revise as a confirmed action returning to the editor.
- Accept, decline and countersign as confirmed actions collecting the recorded fields (name, date, method).
- `can*` flags from the server, never re-derived from status; a failed load and a refused action are separate states (the invoices screen's rules); a reason→sentence table in a `documents-errors.ts` beside `invoicing-errors.ts`, with the same two deliberate fallbacks.
- Every new `wc-*` component ships with a co-located preview and `describePreviewA11y` per the component-first workflow; `apps/app` composes and holds no primitives; all server access through `src/api` so the guard test stays green.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A document can be drafted, edited, filed, sent, revised and carried to executed entirely in the browser, with versions and both signature records shown on the detail
- [ ] #2 Every new component ships with a co-located preview and passing describePreviewA11y, reads theme tokens, and adopts controlsCss where it renders a wa-* primitive
- [ ] #3 Guardrails render from details.reason with the two deliberate fallbacks (a 400 and an unrecognized 409 render the server's sentence)
- [ ] #4 All server access goes through src/api and the guard test stays green
<!-- AC:END -->
