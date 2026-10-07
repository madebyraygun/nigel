---
id: TASK-145
title: Move the open GitHub issues into backlog tasks and close them
status: Done
assignee: []
created_date: '2026-09-30 18:11'
updated_date: '2026-10-07 18:32'
labels:
  - repo
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Backlog.md is where the work is tracked, and the open issues on the public repo sit outside it. Each open issue (#45, #46, #48 at time of filing) becomes a backlog task carrying its substance, and the issue is closed with a comment pointing at the task. Issue text is public input: check each for real book data before copying it into a task file.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every open GitHub issue has a backlog task carrying its description and any repro, with the issue URL as a --ref
- [x] #2 An issue already covered by an existing task is linked to that task rather than duplicated
- [x] #3 Each issue is closed with a comment naming the task that replaces it
- [x] #4 No open issues remain on the repo, and check-no-real-data.sh exits 0 over the new task files
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
#45 → TASK-148, #46 → TASK-140 (existing epic, ref added), #48 → TASK-149. Issues closed.
<!-- SECTION:FINAL_SUMMARY:END -->
