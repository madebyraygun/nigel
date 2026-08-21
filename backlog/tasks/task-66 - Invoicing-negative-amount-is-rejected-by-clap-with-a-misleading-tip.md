---
id: TASK-66
title: 'Invoicing: negative --amount is rejected by clap with a misleading tip'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-07 23:09'
updated_date: '2026-08-21 00:21'
labels:
  - invoicing
  - bug
milestone: m-0
dependencies: []
references:
  - 'archived PR #172'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nigel invoice pay 1248 --amount -5 never reaches the app's validator: clap reports "unexpected argument '-5'" and suggests "-- -5", which also fails. Only --amount=-5 reaches the real validation ("--amount must be a finite number greater than zero"). Set allow_hyphen_values on the amount arg so the space-separated form lands in the same friendly validator.

Found during pre-merge testing of PR #172.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 invoice pay --amount -5 (space-separated) reports the app's greater-than-zero validation error, not a clap parse error
<!-- AC:END -->
