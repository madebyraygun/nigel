---
id: TASK-88
title: 'Web UI: status glyphs missing from IBM Plex Mono — replace with wc-icon-* SVGs'
status: To Do
assignee: []
created_date: '2026-08-11 21:23'
labels:
  - web
  - ui
  - theme
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
IBM Plex Mono has no glyph for the status/UI characters ✗ ⟳ ◑ ● ◆ ▲ ⊘ ◻ (verified against the complete upstream font, not a subsetting artifact). With the mono typeface as primary (TASK-76, PR #201), wc-invoice-status, wc-send-dialog and wc-reconciliation-history render these via per-glyph font fallback, which breaks visual consistency. Replace the character glyphs with wc-icon-* SVG icons through the existing WcIconBase. Surfaced during TASK-76 implementation.
<!-- SECTION:DESCRIPTION:END -->
