---
id: TASK-135
title: Plain-text invoice email
status: In Progress
assignee:
  - '@dalton'
created_date: '2026-08-24 23:06'
labels: []
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The invoice email currently reuses the published page HTML as its body, which email clients render badly. Replace it with a plain-text body built from the shared document model: subject unchanged, body carrying the invoice header, dates, line items, the money lines from MoneySummary::lines(), a 'Pay now:' line with the public page URL, and notes/terms/payment instructions when set. The PDF stays attached. The published page, PDF, templates and preview are untouched.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Invoice emails are sent with a text body and no html field
- [ ] #2 Body includes header, dates, line items, money lines, Pay now URL, and notes/terms/payment instructions when set; absent values omit their block
- [ ] #3 Text body money lines come from the same MoneySummary::lines() the page and PDF use
- [ ] #4 PDF remains attached; To/CC, subject, From/Reply-To unchanged
- [ ] #5 docs/invoicing.md and docs/design-constraints.md describe the new email body rule
<!-- AC:END -->
