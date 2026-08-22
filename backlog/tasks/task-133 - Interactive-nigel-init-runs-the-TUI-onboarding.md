---
id: TASK-133
title: Interactive nigel init runs the TUI onboarding
status: To Do
assignee: []
created_date: '2026-08-22 14:18'
labels:
  - cli
  - tui
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decision-8 keeps the TUI onboarding as the terminal first-run experience. Unlike the manager screens it already owns its own event loop (onboarding::run calls ratatui::init itself), so no new driver is needed — what is missing is a caller that is not the dashboard.

Wire interactive nigel init on a TTY to onboarding::run(), and move the PostSetupAction handling (Demo -> demo::setup_demo, Import -> import guidance, StartFresh — today in dashboard.rs around lines 1118-1240) to live with onboarding or init so the dashboard is not load-bearing for first run. Flagged invocations (--profile, --data-dir) and non-TTY stdin keep the current plain-prompt behaviour so scripts and agents are unaffected. effects.rs stays: onboarding is its remaining consumer once splash, goodbye and snake are removed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel init with no flags on a TTY runs the existing TUI onboarding wizard, including the demo, import and start-fresh post-setup actions
- [ ] #2 nigel init with flags or a non-TTY stdin keeps the current plain-prompt behaviour unchanged
- [ ] #3 PostSetupAction handling lives with onboarding or init, with no reference into dashboard.rs
- [ ] #4 Existing onboarding and effects tests keep passing
<!-- AC:END -->
