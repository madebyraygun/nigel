---
id: TASK-95
title: 'Web UI: reorganize the database password panel in settings'
status: To Do
assignee: []
created_date: '2026-08-12 17:51'
labels:
  - web
  - ui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Database password panel runs the change-password form and the remove-password form together with no separation: the 'Change password' button is followed immediately by the remove form's 'Current password' label (screenshot), so the two forms read as one broken form with two Current-password fields. Separate the operations visually (sub-sections, dividers, or tabs), make each form's scope unambiguous, and give Remove password the destructive treatment consistent with the rest of the app.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Change password and Remove password are visually distinct operations with unambiguous field ownership
- [ ] #2 Remove password reads as a destructive action and confirms accordingly
<!-- AC:END -->
