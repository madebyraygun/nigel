---
id: TASK-103
title: >-
  Flaky or broken test:
  paying_a_published_invoice_paints_a_frame_before_it_republishes
status: Done
assignee: []
created_date: '2026-08-13 18:25'
updated_date: '2026-08-13 19:25'
labels:
  - bug
  - tui
  - invoicing
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`cli::invoice_manager::tests::paying_a_published_invoice_paints_a_frame_before_it_republishes` fails on main. Confirmed against a clean checkout of origin/main with `cargo test --no-default-features -- --test-threads=1`: 933 pass, this one fails. It is the only failure in the suite.

Found while verifying an unrelated history rewrite — the same test failed identically on the rewritten and unrewritten trees, which is how it was ruled out as rewrite damage and identified as pre-existing.

The failure output shows a rendered invoice with Subtotal/Total $2,000.00, a $500.00 payment and a $1,500.00 balance, followed by `Recorded $500.00 against invoice #1248 (partial).` — so the payment path itself runs. The assertion about the frame painted *before* the republish is what does not hold.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The failure is reproduced and its cause identified — a genuine regression in the two-phase paint, or a test that asserts on frame ordering it cannot rely on
- [ ] #2 The test passes on main, or is replaced by one that pins the intended behaviour
- [ ] #3 If the cause is a real regression, the TUI pay path repaints before the republish as task-68.4 intended
- [ ] #4 `cargo test` and `cargo test --no-default-features` are both green
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Root cause: the test lacked the TempConfigDir guard, so republish_after_payment read the developer's real settings.json. With R2 configured the republish succeeded rather than being skipped, producing no 'old balance' warning — and uploading to a live bucket. Fixed by applying the same guard cli::invoice uses for the same scenario.
<!-- SECTION:NOTES:END -->
