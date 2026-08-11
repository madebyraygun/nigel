---
id: TASK-76
title: 'Theme: adopt a mono primary typeface for a terminal visual brand'
status: To Do
assignee: []
created_date: '2026-08-09 00:46'
updated_date: '2026-08-11 20:02'
labels:
  - enhancement
  - web
  - ui
  - theme
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-76-mono-typeface-design.md
  - docs/superpowers/plans/2026-08-11-task-76-mono-typeface.md
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel is a terminal tool with a web front end, and the type does not say so. Move the primary typeface to a mono — IBM Plex Mono or Fira Mono — so the browser reads as the same product as the CLI.

This is a token change first: @nigel/theme owns --wa-font-family-sans and the rest of typography.ts, so the switch should happen there rather than in components. Worth checking against the places where type is doing real work — the register and report tables, wc-money (already tabular figures, which a mono only helps), the aging strip labels, and the invoice HTML template, which is a client-facing document and may well want to stay in a text face rather than follow the app.

Fonts must be self-hosted and bundled: the SPA is embedded in the binary by rust-embed and nigel serve is a localhost server with no network guarantee, so a webfont CDN is not an option. Weight matters to binary size — subset to the ranges actually used.

Decide explicitly whether this is mono everywhere or mono for UI chrome and data with a text face for prose, and whether the published invoice page follows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The primary typeface is a self-hosted mono, bundled rather than fetched from a CDN
- [ ] #2 No network request is made for a font at runtime
- [ ] #3 A decision is recorded on whether the client-facing invoice template follows the app or keeps a text face
- [ ] #4 Register and report tables, wc-money figures and the aging strip stay legible and aligned at the new metrics
- [ ] #5 Line lengths and control heights are re-checked — mono is wider, and the manager tables and dialogs must not overflow
- [ ] #6 The contrast and a11y suites still pass
- [ ] #7 Added font weight to the embedded bundle is measured and noted
<!-- AC:END -->
