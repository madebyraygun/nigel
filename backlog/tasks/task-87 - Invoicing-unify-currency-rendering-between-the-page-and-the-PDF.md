---
id: TASK-87
title: 'Invoicing: unify currency rendering between the page and the PDF'
status: To Do
assignee: []
created_date: '2026-08-11 20:06'
labels:
  - invoicing
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The invoice HTML page prints 'USD 250.00' while the PDF prints fmt::money's '$250.00' for the same figure. fmt::money is dollar-only, so unifying on it would break non-USD invoices; unifying on the page's form changes every existing PDF. Needs a decision on currency-aware formatting (currency code + symbol mapping, or code-prefixed everywhere). Surfaced during TASK-78 spec review; out of scope there.
<!-- SECTION:DESCRIPTION:END -->
