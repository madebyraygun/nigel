---
id: TASK-97
title: 'Flaky test: snake_moves_right fails when random food spawns beside the head'
status: To Do
assignee: []
created_date: '2026-08-12 22:41'
labels:
  - testing
  - bug
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
cli::snake::tests::snake_moves_right asserts body.len() == 3 after a move, but SnakeGame::new() places food randomly — when it lands directly right of the head the snake eats it on the first move, grows, and the assertion fails. Seen once in CI-adjacent runs, not reproducible on demand. Fix by seeding the game deterministically in tests or placing food explicitly away from the movement path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 snake_moves_right passes regardless of where food spawns
<!-- AC:END -->
