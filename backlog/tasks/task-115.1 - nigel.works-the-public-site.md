---
id: TASK-115.1
title: 'nigel.works: the public site'
status: To Do
assignee: []
created_date: '2026-08-17 15:27'
labels:
  - product
  - docs
milestone: m-0
dependencies: []
parent_task_id: TASK-115
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The forward-facing page at nigel.works, built on the existing `site/` machinery.

- The page tells the ladder in the foundation document's order: what Nigel is (local-first books, the file is yours), build-it-yourself with a link to the repository, Nigel Desktop with download + purchase, Nigel Cloud with what each phase hosts — copy traceable to `docs/product/foundation.md`, never contradicting it.
- Download and purchase route through the merchant of record's hosted checkout; the site itself stays static (no backend, no analytics beyond what the host provides, no cookies requiring a banner).
- Screenshots and examples use the demo database and the fictional cast only.
- The existing docs/FAQ site content (task 22 territory) folds in or links out — one site, one navigation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel.works serves the positioning page with download and purchase flows, and every claim on it traces to docs/product/foundation.md
- [ ] #2 The site is static, carries no tracking requiring consent, and all screenshots use demo data with the fictional cast
- [ ] #3 A visitor can reach the repository, the purchase flow and the Cloud waitlist/signup in one click each from the landing page
<!-- AC:END -->
