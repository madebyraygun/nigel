---
id: TASK-154
title: 'Epic: web app refresh'
status: To Do
assignee: []
created_date: '2026-10-20 22:57'
labels:
  - epic
  - web
  - design
milestone: m-3
dependencies: []
references:
  - docs/design/app-refresh/README.md
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Bring the design handoff in docs/design/app-refresh/ into the web app: new tokens, a floating sidebar and status line, a breadcrumb, a Cmd+K palette, and reworked Dashboard, Register, Invoices, Settings and Setup screens, plus the mobile drawer. Identity is unchanged: IBM Plex Mono, the ink and pastel ramps, the rainbow gradient as an accent. The spec is high fidelity: where it differs from @nigel/theme, its values win. Mock data in the handoff is the demo books.

Scope by surface:
- The web refresh ships for the browser (nigel serve), the iPad web app (TASK-124), and the Tauri shell on Windows and Linux.
- On the Mac, the Swift shell (TASK-144) hides the web sidebar and status line in hosted mode (TASK-144.5) and draws native chrome that follows this design's structure and look: sidebar groups (144.6), the palette (144.10), the settings window (144.8). The HTML prototypes' Tauri Liquid Glass notes apply to the native shell, not the web build.
- Switching between sets of books (breadcrumb Books scope, palette Books section, Settings > Location > Switch books, the recent-books registry) is its own task in 2.1. Until then the breadcrumb, palette and status line show the one open book.

Every subtask follows the component-first workflow in CLAUDE.md: components in @nigel/ui with previews and describePreviewA11y, screens in apps/app consume them, tokens from @nigel/theme only.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every subtask is done and the screens match the handoff's screenshots in light and dark at desktop and phone widths
- [ ] #2 docs/native-feel.md and docs/architecture.md describe the shell as built
<!-- AC:END -->
