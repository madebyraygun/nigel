---
id: TASK-109.9
title: 'Drafting and editing: Markdown bodies and per-kind templates'
status: To Do
assignee: []
created_date: '2026-08-17 04:55'
updated_date: '2026-08-21 00:21'
labels:
  - documents
milestone: m-1
dependencies: []
parent_task_id: TASK-109
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The *drafted* source: a document authored in Nigel and rendered by Nigel.

- `nigel document draft --client <id> --kind proposal --title "…"` creates a draft from the kind's template: a Markdown body pre-filled from the template's starter wording, with `{{KEY}}` placeholders (client fields, dates, branding, title) resolved at render time rather than draft time, so a later edit to the client row shows through.
- `nigel document edit <id>` opens the body in `$EDITOR` (draft only — the data-layer guard decides; the CLI just relays its sentence). The body may also arrive whole: `nigel document draft … --body-file draft.md` — that flag is the drafting half of the skills contract, usable with no interactive step.
- Per-kind templates are data, not code: seeded generic wording for Proposal, Estimate and Agreement (fictional cast only, nothing business-specific), exported and overridden under `<data_dir>/templates/documents/` with `nigel document template export --kind …` / `nigel document template path` — the `invoice template` precedent, including the broken-override-arrives-as-a-sentence rule.
- Rendering goes through one seam shared by preview and send (the `render_invoice` precedent) so surfaces cannot drift; the paginated renderer itself is task 109.10.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Drafting from each seeded kind produces a draft with a Markdown body; placeholders resolve at render time from the live client row
- [ ] #2 Editing is refused for any non-draft status by a data-layer guard, and the CLI prints the guard's own sentence
- [ ] #3 Templates are seeded exactly once, exportable and overridable per kind under the data dir, and a broken override is reported as a sentence naming the problem, never a panic
- [ ] #4 --body-file files a complete Markdown body in one command, usable by a skill with no interactive step
- [ ] #5 No template or seeded wording carries business-specific content; fictional cast only in fixtures
<!-- AC:END -->
