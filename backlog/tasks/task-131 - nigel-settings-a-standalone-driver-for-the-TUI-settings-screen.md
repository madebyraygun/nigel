---
id: TASK-131
title: 'nigel settings: a standalone driver for the TUI settings screen'
status: To Do
assignee: []
created_date: '2026-08-22 14:06'
labels:
  - cli
  - tui
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
settings_manager.rs is the one dashboard screen whose function has no CLI equivalent — business name, letterhead (address, phone, logo, payment instructions), password management, and the auto-update toggle. Decision-8 keeps a terminal path for it by reusing the screen, not rewriting it.

The screen is self-contained: its imports are tui.rs styles, password_manager, and nigel-core — no dashboard or effects dependency. What it lacks is a driver: like every *_manager.rs it exposes new(conn, greeting)/draw(frame)/handle_key(code, conn) -> SettingsAction and is driven today by the dashboard loop. Write a small standalone loop on the review.rs / browser.rs pattern (ratatui::init, draw + read until SettingsAction::Close, ratatui::restore) and wire a Settings variant into the Commands enum. The greeting parameter is the one dashboard-ism — pass a plain title line or draw from the shared voice once TASK-116 homes it in nigel-core.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel settings opens the existing settings screen in the terminal, covering every row the dashboard version had, including the password sub-screen
- [ ] #2 The driver follows the standalone-loop pattern of review.rs and browser.rs and restores the terminal on exit and panic
- [ ] #3 settings_manager.rs and password_manager.rs are untouched beyond what the driver needs; their existing tests keep passing
- [ ] #4 docs/commands.md documents the subcommand
<!-- AC:END -->
