---
id: TASK-33.16
title: Two app-screen buttons still show the hand cursor
status: To Do
assignee: []
created_date: '2026-08-18 02:16'
updated_date: '2026-08-21 00:21'
labels:
  - tauri
  - ui
milestone: m-0
dependencies: []
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-33.11 removed every cursor: pointer from @nigel/ui, which is what its AC #3 asked for. Two declarations outside that scope survived, and they are the shape the rule exists to catch: web/apps/app/src/screens/import.ts:108 and web/apps/app/src/screens/review.ts:103 both style a real button — border, surface background, padding — and give it a hand cursor. A native control shows the arrow.

A third, web/apps/app/src/screens/rules.ts:88, is a button styled as a link (brand colour, underlined) and keeps the pointer correctly. The two in web/packages/ui/preview/app/preview-app.ts are the preview harness, not shipped chrome, and are out of scope.

The durable fix is not three edits: screens keep restyling bare buttons because there is no primitive for a secondary action. Consider whether these two belong in @nigel/ui as a component before patching the declarations in place.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The buttons in import.ts and review.ts show the arrow cursor
- [ ] #2 cursor: pointer in web/apps/app is reserved for elements that behave as links
<!-- AC:END -->
