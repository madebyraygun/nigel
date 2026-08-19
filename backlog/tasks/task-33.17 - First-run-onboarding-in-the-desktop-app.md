---
id: TASK-33.17
title: First-run onboarding in the desktop app
status: To Do
assignee: []
created_date: '2026-08-19 14:30'
labels:
  - frontend
dependencies: []
parent_task_id: TASK-33
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A fresh machine — no settings.json, no database — currently gets nothing from the desktop shell: main.rs opens get_data_dir()/nigel.db with no init_db pre-flight, so first launch creates a zero-byte SQLite file and every data route fails on missing tables. The SPA has no boot state for 'there are no books yet'. Onboarding today is CLI-only (cli/onboarding.rs).

Build a first-run experience for the desktop app, designed desktop-first, that is not merely competent but delightful — this is the first thing a new user ever sees of Nigel. It carries the brand: the gradient ASCII logo (effects::LOGO in the TUI; the website's #ascii-logo with its per-character rainbow gradient in site/styles.css) and Nigel's conversational voice (the GREETINGS rotation and 'Hello, {first name}' header in cli/dashboard.rs). It collects what the TUI onboarding collects — user name, business name, profile (business vs personal), optional database password — and offers the same three exits: view the demo, start from scratch, load an existing data directory. The flow should be served by the API as a boot state (e.g. needs-setup, alongside starting/locked/ready) with an SPA setup screen, so the web client on an un-onboarded machine can share it rather than silently initializing default books the way nigel serve does today.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 On a machine with no settings.json and no database, the desktop app boots into the onboarding flow — never a broken dashboard or a zero-byte database
- [ ] #2 The flow collects name, business name, profile and optional password, and offers demo / start fresh / load existing directory, matching the TUI onboarding's choices
- [ ] #3 The gradient ASCII logo and the conversational voice are present in the flow, consistent with the TUI and the website
- [ ] #4 Completing onboarding writes settings.json, creates and migrates the database under the chosen profile, and lands in a ready dashboard without restarting the app
- [ ] #5 A browser hitting nigel serve on an un-onboarded machine reaches the same setup flow instead of silently getting default books
- [ ] #6 Onboarding components ship through @nigel/ui with previews and passing a11y states per the component-first workflow
<!-- AC:END -->
