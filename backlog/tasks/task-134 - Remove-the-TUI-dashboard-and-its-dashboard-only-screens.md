---
id: TASK-134
title: Remove the TUI dashboard and its dashboard-only screens
status: To Do
assignee: []
created_date: '2026-08-22 15:05'
labels:
  - tui
dependencies:
  - TASK-116
  - TASK-123
  - TASK-128
  - TASK-129
  - TASK-130
  - TASK-131
  - TASK-132
  - TASK-133
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The execution task for decision-8. With the web gaps closed (TASK-128/129/130, TASK-123, TASK-116) and the CLI keeps in place (TASK-131 settings, TASK-132 bare nigel, TASK-133 init onboarding), delete the dashboard surface: dashboard.rs and the manager screens reachable only from it (account, category, client, invoice, import, reconcile, rules, load, undo, settings-driver wiring aside, password via settings), plus splash.rs, goodbye.rs and snake.rs. onboarding.rs and effects.rs stay (decision-8), as do the CLI-invoked TUI screens: report viewers, register browser, reviewer, and the settings screen behind nigel settings.

The sweep that rides along: docs/architecture.md drops its dashboard bullets and tree entries; docs/walkthrough.md is rewritten around the web UI; README loses the dashboard screenshot and bullet; docs/invoicing.md drops its From-the-dashboard section; docs/design-constraints.md parity invariants become two-way (CLI/API); docs/api.md drops TUI-parity notes; site/index.html drops the dashboard-preview section and dashboard.png. Dashboard-only bug tasks close by deletion (TASK-82, 83, 85, 37, 97, 127); planned tasks lose their TUI leg (TASK-9.9, 17, 18, 19, 20, 21, 109.6, 119, 120, 122).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The dashboard, its manager screens, splash, goodbye and snake are removed; onboarding, effects, the report viewers, register browser, reviewer and settings screen remain and their tests pass
- [ ] #2 cargo test, fmt and clippy -D warnings pass across the CI feature combos with no dead code left behind
- [ ] #3 Docs and site describe the current state: architecture, README, walkthrough (rewritten around the web UI), invoicing, design-constraints (two-way invariants), api, and the site's dashboard section and screenshots
- [ ] #4 TASK-82, 83, 85, 37, 97 and 127 are closed as resolved by removal; the listed planned tasks have their TUI legs edited out
- [ ] #5 CHANGELOG records the removal and points terminal users at the CLI equivalents and nigel settings
<!-- AC:END -->
