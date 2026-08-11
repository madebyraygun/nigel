---
id: TASK-83
title: Dashboard enter_browse category-load warning is clobbered before it renders
status: To Do
assignee: []
created_date: '2026-08-11 14:13'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In src/cli/dashboard.rs enter_browse, the Err arm for get_categories sets self.status_message to a 'Warning: could not load categories' notice, but the unconditional self.status_message = None a line later clears it before any draw, so the warning can never be seen and inline categorization silently degrades to an empty picker. Reorder so the reset happens before the load (or only in the Ok arm), and consider the same pattern at the other get_categories().unwrap_or_default() sites (cli/browse.rs, cli/report/view.rs).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A failed category load in enter_browse leaves the warning visible on the status line
- [ ] #2 A test pins the status message surviving the transition
<!-- AC:END -->
