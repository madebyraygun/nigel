---
id: TASK-111
title: >-
  Web UI: the register toolbar scrolls away on a phone, and the table renders at
  full height
status: To Do
assignee: []
created_date: '2026-08-16 16:58'
labels:
  - web
  - ui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On a 390x844 phone the register screen host measures 14,092px tall and the table 13,831px of it: all 270 demo rows render at full height and the shell's content area scrolls the lot. Three things follow, and only the first is what it looks like.

**The filters scroll away.** The app header is already fixed (48px at top 0, outside the scrolling content area) — that part is fine. What leaves is wc-register-toolbar: the account select, the period pager, the All/Year/Month granularity, the search field and the row count. On a 270-row register that means scrolling back thousands of pixels to change an account or type a search.

**Landing position makes it worse.** The screen scrolls to today on load (indexOfToday, the TUI's scroll_to_today parity). With the page rather than the table doing the scrolling, opening the register drops you near the bottom of a 14,000px page with no filters on screen at all.

**The table itself overflows sideways.** The scroller reports 680px of content in a 336px box, so Category is cut mid-word and Amount is off-screen entirely — visible in the screenshot as 'Software Subscrip'.

The table's fill mode is supposed to prevent exactly this: the host grows into the content area and the scroller takes what is left, with --nc-register-min-height (12rem) as the floor below which the page scrolls instead. At this width that floor is not what is happening — 779px of content area minus a 218px bar leaves room for the 12rem floor, yet the table renders its full content height. Establish why before changing layout: the fix may be that fill is not applying at this width rather than anything to do with the toolbar.

Layout is the second half. wc-register-toolbar is a flex-wrap row, so at 390px every control takes its own line and the bar costs 172px before a single transaction is visible. Denser is possible — the granularity segmented control beside the pager, the row count inline with the search — but decide it against the sticky behaviour rather than separately, since a sticky bar's height is the whole cost.

Related: task-91 (virtualize or lazy-load the register table) addresses the row count directly, and PR #17 is open against it. A virtualized table changes these measurements, so check whether that lands first.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The account, period, granularity and search controls stay reachable while scrolling the register on a phone
- [ ] #2 Why the table renders at full height rather than filling and scrolling internally at this width is established and stated, and fixed if that is the cause
- [ ] #3 Opening the register on a phone does not land the user below the filters
- [ ] #4 The toolbar costs materially less than 172px of a 390px viewport, without hiding a control behind a menu that takes a tap to discover
- [ ] #5 Category and Amount are readable on a phone, whether by horizontal scroll within the table, a narrower column set, or a different row shape
- [ ] #6 Measured at 390x844 before and after, with the numbers in the task
<!-- AC:END -->
