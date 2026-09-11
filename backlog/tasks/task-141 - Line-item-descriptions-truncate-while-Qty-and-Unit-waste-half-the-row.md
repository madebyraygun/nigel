---
id: TASK-141
title: Line item descriptions truncate while Qty and Unit waste half the row
status: To Do
assignee: []
created_date: '2026-09-11 16:41'
labels:
  - invoicing
  - web
  - bug
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On the invoice edit form the three inputs share the row badly. Description is a single-line input that cuts a real description off mid-word, while Qty and Unit each get an input wide enough for a figure that is almost always two to four characters — so the column that needs the space is the one that has none.

`wc-line-items` styles every input at `width: 100%` with `min-width: 5rem`, and the table hands the numeric columns as much room as the description. Two changes: the description wraps to as many lines as it needs, and the numeric columns shrink to something sized for the figures they hold.

The read-only rendering on the invoice detail has the same problem — a long description there is truncated too, where there is no input to blame.

Pairs with TASK-137, which is the other thing wrong with this component.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A long line item description wraps to multiple lines in the editor rather than truncating
- [ ] #2 A long description wraps in the read-only rendering on the invoice detail
- [ ] #3 Qty and Unit are sized for the figures they carry, not given equal share with the description
- [ ] #4 Row height follows the tallest cell, and the Amount, reorder and delete controls stay aligned with it
- [ ] #5 The preview covers a short description and one long enough to wrap to three lines
- [ ] #6 describePreviewA11y still passes
<!-- AC:END -->
