---
id: TASK-33.11
title: Kill the web tells in the desktop shell
status: To Do
assignee: []
created_date: '2026-08-17 13:35'
labels:
  - tauri
  - ui
dependencies: []
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The desktop shell puts nigel's SPA in a webview, where web behavioural tells that pass unnoticed in a browser read as "this is a website in a box". Boxcraft has written these up in docs/dev/native-feel-conventions.md and is starting its own Tauri epic; the conventions are engine-level rather than app-specific, so they transfer.

A survey of web/ found the gaps concrete rather than hypothetical: overscroll-behavior appears nowhere, so the document rubber-bands; user-select appears nowhere, so dragging across toolbars and labels paints a blue selection; 21 cursor: pointer declarations sit in @nigel/ui, most on buttons, where a native control shows an arrow; and no field sets spellcheck=false, so amounts, account names and invoice numbers get red squiggles.

Two conventions are already met and should stay that way: :focus-visible is used 32 times against a single bare :focus, and prefers-reduced-motion is honoured in 17 places.

Placement follows the same rule boxcraft uses: platform conditionals belong in the shell and the app composition layer, never in @nigel/ui primitives. A wc-* component must not branch on OS or ask whether it is running under Tauri.

Safari and WebKit are the baseline engine to test against: WebKitGTK renders the Linux shell and WKWebView the macOS one, and Windows uses Chromium-based WebView2, so what works in Safari and Chrome works in all three.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 overscroll-behavior: none at the app root, so the document does not rubber-band and scrolling stays inside panels and lists
- [ ] #2 user-select: none on chrome — toolbars, buttons, labels, panel headers — with selectable text kept in content and inputs
- [ ] #3 Buttons and controls show the arrow cursor; cursor: pointer is reserved for true links, and the 21 existing declarations in @nigel/ui are triaged against that rule
- [ ] #4 spellcheck=false on amount, account, category and invoice-number fields
- [ ] #5 draggable=false on icons and decorative images, so they produce no drag ghost
- [ ] #6 No platform conditional lives in @nigel/ui: no OS branch, no user-agent sniffing, no check for whether Tauri is present
- [ ] #7 The conventions are written down in docs/, credited to boxcraft's version, so a later component follows them without rediscovering them
<!-- AC:END -->
