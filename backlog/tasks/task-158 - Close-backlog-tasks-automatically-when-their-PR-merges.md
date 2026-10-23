---
id: TASK-158
title: Close backlog tasks automatically when their PR merges
status: To Do
assignee: []
created_date: '2026-10-23 16:27'
labels:
  - ci
  - backlog
  - tooling
milestone: m-3
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every merged PR is followed by a hand-made commit on main that ticks the task's acceptance criteria, sets it Done with a final summary naming the PR, and moves it to backlog/completed/ ("Close TASK-154.8: shipped in PR #65"). Automate that with a GitHub Action, keeping the rules in CLAUDE.md: task files change only through the backlog CLI, and task files live on main.

Trigger: a pull request closed as merged into main. Source of task ids: TASK-n and TASK-n.m references in the PR title (the convention every PR here already follows), optionally also a 'Closes TASK-n' line in the body.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When a PR merges into main, every task named in its title (and any 'Closes TASK-n' line in its body) is set Done with all acceptance criteria checked and a final summary 'Shipped in PR #N.', then moved to backlog/completed/ — all through the backlog CLI
- [ ] #2 The action commits the change to main with a message naming the PR and tasks, without any bot or co-author byline, and the commit passes check-no-real-data.sh
- [ ] #3 A task already Done, an unknown id, or a PR naming no task is a no-op with a log line, never a failure; a PR from a fork never gets write access
- [ ] #4 The action uses a pinned backlog CLI version matching the repo's, and pushing to main uses a token scoped to contents:write only
- [ ] #5 docs/backlog-cli.md (or CLAUDE.md's Backlog section, if a rule changes) describes the automation and how to opt a PR out
<!-- AC:END -->
