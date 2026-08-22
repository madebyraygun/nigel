---
id: TASK-116
title: Nigel's conversational voice is missing from the web app
status: To Do
assignee: []
created_date: '2026-08-19 14:30'
updated_date: '2026-08-21 00:21'
labels:
  - frontend
milestone: m-0
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI dashboard greets the user by first name with a rotating conversational line (GREETINGS in cli/dashboard.rs — 'Kettle's on.', 'Another day, another CSV.', 'Shall we see where the money's gone?'). The web app has none of this: the SPA dashboard is all data, no Nigel. The voice is a core part of the brand and should meet the user in the web and desktop clients the way it does in the terminal.

The greetings currently live in the nigel CLI crate, unreachable from the SPA; part of the work is deciding where the voice lives so the TUI and the SPA draw from one source rather than drifting apart (nigel-core with the API serving it, most likely — the user's first name is already server-side metadata from onboarding).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The web dashboard greets the user by first name with a rotating line in Nigel's voice, matching the TUI's tone
- [ ] #2 The TUI and the SPA draw greetings from a single source — adding a line in one place reaches both
- [ ] #3 The greeting respects the no-name case the way the TUI does (falls back gracefully when onboarding collected no name)
<!-- AC:END -->
