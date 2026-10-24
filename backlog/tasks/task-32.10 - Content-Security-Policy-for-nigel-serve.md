---
id: TASK-32.10
title: Content-Security-Policy for nigel serve
status: To Do
assignee: []
created_date: '2026-10-24 21:38'
labels:
  - multiuser
  - security
  - web
milestone: m-3
dependencies: []
parent_task_id: TASK-32
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Before a Nigel server is exposed on a public URL (multiuser phase 1, decision-11), every response carries a Content-Security-Policy that limits scripts, styles, frames, images and connections to what the app needs, so an injected script cannot load or exfiltrate. Must be checked against Web Awesome (shadow DOM styles, adopted stylesheets), the invoice/document preview frame and blob/PDF previews, fonts, and the Tauri/Swift shells' scheme origins.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every HTML and API response from nigel serve carries a CSP; default-src 'self' with only the additional sources the app demonstrably needs, no 'unsafe-eval', and 'unsafe-inline' for scripts absent
- [ ] #2 Every screen, dialog, preview frame, PDF/blob preview, font and the desktop shells work with the policy, with no CSP violations in the console across a scripted walk of all screens
- [ ] #3 A test asserts the header on HTML and API responses and fails when a source is widened
- [ ] #4 frame-ancestors forbids embedding by other origins; docs/api.md (security model) documents the policy
- [ ] #5 Lands before any server is exposed publicly (gates the bee deployment in TASK-32.9)
<!-- AC:END -->
