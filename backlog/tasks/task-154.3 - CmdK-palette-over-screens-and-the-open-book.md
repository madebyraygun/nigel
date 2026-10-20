---
id: TASK-154.3
title: Cmd+K palette over screens and the open book
status: To Do
assignee: []
created_date: '2026-10-20 22:57'
labels:
  - web
  - design
milestone: m-3
dependencies:
  - TASK-154.2
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Spec section 2, without book switching. Cmd+K / Ctrl+K toggles the palette anywhere; the search pill opens it; Esc or the backdrop closes it. Sections: Books (the open book only, marked OPEN, with its kind, path and lock state) and Go to (every screen, including Undo last import). Case-insensitive substring match on labels, kind and path; arrow keys move through one flat list; Enter activates; focus lands in the input; an empty state names the query. The Books scope chip, 'New books…' and 'Open books from a folder…' arrive with book switching. On the Mac the native palette (TASK-144.10) replaces this one.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Cmd+K and Ctrl+K toggle the palette from every screen, and Esc and the backdrop close it
- [ ] #2 Typing filters books and screens; arrow keys and Enter navigate; an unmatched query shows 'Nothing matches "{q}".'
- [ ] #3 Keyboard and screen-reader use works end to end: focus is trapped while open and returns to the opener on close
- [ ] #4 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->
