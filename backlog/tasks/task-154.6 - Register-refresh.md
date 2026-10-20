---
id: TASK-154.6
title: Register refresh
status: To Do
assignee: []
created_date: '2026-10-20 22:57'
labels:
  - web
  - design
milestone: m-3
dependencies:
  - TASK-154.1
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Spec section 5. Rebuild screens/register.ts on the new toolbar (account select, period stepper, All/Year/Month segmented control, search, row count, Keys) and table card (fixed columns, sticky header, flag button, left-edge stripe for selected and flagged rows, footer total), keeping inline edit, keyboard shortcuts and the flag toggle. The table scrolls itself and opens at the most recent rows. Absorbs TASK-111 (toolbar scrolls away on a phone, table renders at full height); coordinate with TASK-129 (category filters and export).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every existing register feature and shortcut still works, covered by the existing tests
- [ ] #2 The table scrolls within the card, opens at the newest rows, and the footer total matches the filtered rows
- [ ] #3 On a phone the toolbar stays reachable and the table scrolls (TASK-111's criteria)
- [ ] #4 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->
