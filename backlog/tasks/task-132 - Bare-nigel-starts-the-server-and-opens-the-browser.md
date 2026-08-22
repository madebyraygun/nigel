---
id: TASK-132
title: Bare nigel starts the server and opens the browser
status: To Do
assignee: []
created_date: '2026-08-22 14:07'
updated_date: '2026-08-22 14:19'
labels:
  - cli
dependencies:
  - TASK-133
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decision-8: with the TUI dashboard deprecated, running nigel with no subcommand starts the web server and opens the browser with the one-time session link — the behaviour nigel serve already has. main.rs routes None to the serve path instead of cli::dashboard::run(); the splash/dashboard chain behind it is removed by the deprecation work.

On a first run (no settings file), bare nigel runs the TUI onboarding first (the standalone entry TASK-133 wires up), then starts the server and opens the browser — setup never depends on the browser. An existing encrypted database lands on the web unlock screen; the TUI splash unlock goes with the dashboard. serve flags (--no-open, port) should behave identically on the bare invocation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel with no subcommand behaves exactly like nigel serve: server on 127.0.0.1, browser opened with the session link
- [ ] #2 An encrypted database lands on the web unlock screen
- [ ] #3 README and docs/commands.md describe the new bare invocation
- [ ] #4 First run with no settings file runs the TUI onboarding, then starts the server and opens the browser
<!-- AC:END -->
