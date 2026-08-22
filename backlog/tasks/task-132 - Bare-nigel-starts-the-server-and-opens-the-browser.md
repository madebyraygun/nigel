---
id: TASK-132
title: Bare nigel starts the server and opens the browser
status: To Do
assignee: []
created_date: '2026-08-22 14:07'
labels:
  - cli
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decision-8: with the TUI dashboard deprecated, running nigel with no subcommand starts the web server and opens the browser with the one-time session link — the behaviour nigel serve already has. main.rs routes None to cli::serve::run() instead of cli::dashboard::run(); the splash/onboarding/dashboard chain behind it is removed by the deprecation work.

First run lands on the web setup screen (screens/setup.ts), which already covers profile, identity, password, and fresh/demo/load-existing; nigel init remains the terminal path. The encrypted-DB unlock is the web unlock screen — the TUI splash unlock goes with the dashboard. serve flags (--no-open, port) should behave identically on the bare invocation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel with no subcommand behaves exactly like nigel serve: server on 127.0.0.1, browser opened with the session link
- [ ] #2 First run with no settings file lands on the web setup screen instead of failing on a missing database
- [ ] #3 An encrypted database lands on the web unlock screen
- [ ] #4 README and docs/commands.md describe the new bare invocation
<!-- AC:END -->
