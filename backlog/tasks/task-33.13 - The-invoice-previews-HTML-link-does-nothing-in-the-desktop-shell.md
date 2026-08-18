---
id: TASK-33.13
title: The invoice preview's HTML link does nothing in the desktop shell
status: To Do
assignee: []
created_date: '2026-08-18 01:33'
labels:
  - tauri
  - ui
dependencies: []
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The invoice preview offers two links below the frame. "Download the PDF" saves through the native dialog. "Open the HTML page" is a plain anchor carrying target="_blank", which a webview has no tab to honour, so in the desktop shell it does nothing at all — no window, no error.

The two also no longer match: the PDF link became a button when wc-invoice-preview gained a pdfTarget, so one renders as an underlined anchor and the other as a button, side by side.

What the desktop should do instead is a design question rather than an obvious fix. The preview frame already renders that same HTML inline, so the link is a convenience for seeing it full size. Opening it in a second Tauri window keeps it in the app; handing it to the system browser leaves the app entirely, and would mean taking back the opener plugin that the always-save simplification removed.

Whatever it becomes, it belongs in the api seam beside exportTarget and invoicePreviewTarget rather than as a branch inside a screen, and the two links should read as the same kind of control.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Opening the HTML page does something deliberate in the desktop shell, and the choice between a second window and the system browser is recorded with its reasoning
- [ ] #2 The two links below the preview read as the same kind of control rather than an anchor beside a button
- [ ] #3 The browser build is unchanged: the anchor still opens a new tab
- [ ] #4 Whatever the desktop does is reached through the api client, not a branch inside a screen
<!-- AC:END -->
