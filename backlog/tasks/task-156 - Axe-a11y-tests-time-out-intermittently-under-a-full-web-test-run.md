---
id: TASK-156
title: Axe a11y tests time out intermittently under a full web test run
status: To Do
assignee: []
created_date: '2026-10-21 19:27'
labels:
  - web
  - tests
  - flaky
milestone: m-3
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Under a full `npm test` in web/ (especially while the machine is also running cargo tests), describePreviewA11y cases time out at random: wc-wordmark and wc-register-table have both failed this way in separate runs, and passed when rerun alone. Seen repeatedly while building the 2.0 refresh (PRs #58, #64). A flaky gate trains people to rerun until green, which hides real failures.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The cause is identified (per-test timeout too low for axe on large previews, shared jsdom/browser contention, or a component that never settles) and fixed
- [ ] #2 Ten consecutive full web test runs pass with no axe timeout, including one run alongside a serial cargo test
- [ ] #3 Any timeout raised is scoped to the a11y suites and documented in the test helper
<!-- AC:END -->
