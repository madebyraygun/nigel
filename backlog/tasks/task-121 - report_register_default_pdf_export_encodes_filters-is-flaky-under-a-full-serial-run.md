---
id: TASK-121
title: >-
  report_register_default_pdf_export_encodes_filters is flaky under a full
  serial run
status: To Do
assignee: []
created_date: '2026-08-19 18:26'
labels:
  - bug
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Observed during TASK-50/51/52 work, on a machine running several test suites concurrently: the cli_dispatch test report_register_default_pdf_export_encodes_filters failed once in a full serial run, then passed alone, passed on a 124/124 suite rerun, and passed on the final full run. Nothing in the import path touches PDF register export, so the failure is unrelated to that branch. Needs a root cause — the usual suspects for tests that only fail in company are shared HOME/config state, a port, or a temp path collision.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The flake reproduces or the shared-state cause is identified by inspection
- [ ] #2 The test is made hermetic against that cause and survives 20 consecutive full serial runs
<!-- AC:END -->
