---
id: TASK-72
title: >-
  Web UI: dialogs and form fields render unpainted — theme ::part() overrides
  never match
status: To Do
assignee: []
created_date: '2026-08-09 00:45'
labels:
  - bug
  - web
  - ui
  - theme
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Edit client dialog renders with no panel background: the clients table behind it shows straight through the header, body and footer, and the Name, Email and Billing address fields have no visible field chrome. Only the focused Notes textarea looks painted, and that is Web Awesome own focus styling rather than ours.

Root cause is not the dialog. @nigel/theme ships the intended rules — wa-dialog::part(header|body|footer) and wa-input/wa-select/wa-textarea::part(base) both set background: var(--wa-color-surface) in global.ts, and they do reach the built stylesheet. But main.ts loads that stylesheet at document level, while the wa-* primitives live inside wc-* shadow roots (wa-dialog inside wc-manager-dialog, wa-input inside wc-client-form). ::part() crosses exactly one shadow boundary and only for parts exposed in the same tree, and nothing calls exportparts, so those rules ship and never match. Design tokens still work because custom properties inherit across shadow boundaries, which is why colour and type look correct while the surfaces do not.

nigelTheme, the CSSResult a component would adopt into its own shadow root to make these rules apply, currently has zero consumers anywhere in apps/ or packages/ui.

Affects every wc-manager-dialog consumer — accounts, categories, rules and clients — not just clients, and every wa-* form field in a wc-* wrapper.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Add/Edit client dialog renders an opaque panel; nothing behind it is legible through it
- [ ] #2 Name, Email and Billing address fields carry the same visible field chrome as the focused Notes field
- [ ] #3 The mechanism reaches wa-* primitives nested inside wc-* shadow roots, rather than relying on document-level ::part() rules that cannot match
- [ ] #4 The same fix holds for the accounts, categories and rules dialogs, not only clients
- [ ] #5 Either nigelTheme gains its consumers or the dead export is removed — the theme does not keep shipping rules that never apply
- [ ] #6 A preview state covers a dialog open over a populated list, and describePreviewA11y passes with zero violations
<!-- AC:END -->
