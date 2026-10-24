---
id: TASK-32.3
title: 'Roles and authorization: admin, bookkeeper, read-only'
status: To Do
assignee: []
created_date: '2026-08-06 16:27'
updated_date: '2026-10-24 21:54'
labels:
  - multiuser
  - backend
milestone: m-3
dependencies:
  - TASK-32.2
parent_task_id: TASK-32
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Server-side authorization enforced per route, not just hidden in the UI. Read-only can view reports, the register, and lists but cannot mutate anything including import and review. Bookkeeper can do all bookkeeping operations. Admin additionally manages users, settings, database password operations, and boot unlock. Document the role matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The role matrix is documented and enforced server-side on every route
- [ ] #2 Read-only users are rejected from all mutation endpoints including import, review, and categorize
- [ ] #3 Only admins reach user management, settings mutation, password operations, and unlock
- [ ] #4 Tests cover the matrix for each role
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Spec (docs/superpowers/specs/2026-10-23-multiuser-phase-1-design.md §8): letterhead (logo and payment instructions included) and countersigning are admin-only; shared mode refuses data-folder switching and books-password changes; ROUTE_ROLES denies any route missing from the table.

Design: docs/superpowers/specs/2026-10-23-multiuser-phase-1-design.md (approved, PR #74). Implementation sequence in §17.
<!-- SECTION:NOTES:END -->
