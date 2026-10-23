---
id: TASK-159
title: Bring your own AI provider key for auto-categorizing
status: To Do
assignee: []
created_date: '2026-10-23 16:34'
labels:
  - ai
  - categorization
  - settings
milestone: m-4
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Let the operator add an API key for an AI provider of their choice and use it for simple, bounded jobs, starting with suggesting a category (and vendor) for uncategorized transactions. Nigel stays local-first: nothing is sent unless a key is set and the operator turns the feature on, and the operator sees exactly what is sent (description, amount, date, the category list) before it is.

Suggestions are suggestions: they land in Review for acceptance, never written straight to the books, and they sit beside the existing rules engine (an accepted suggestion can become a rule). Store the key with the other secrets (see the settings-keys task).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Settings accepts a provider and an API key (at least one provider; the design allows others), stored as a secret, with a test-connection check
- [ ] #2 With the feature on, Review offers a suggested category and vendor for uncategorized transactions; nothing is applied without the operator accepting it, and an accepted suggestion can be saved as a rule
- [ ] #3 Before the first request the operator sees which fields are sent; nothing is sent when no key is set or the feature is off, and requests never include account numbers or other identifiers beyond the transaction description
- [ ] #4 Failures (bad key, rate limit, offline) degrade to no suggestion with a sentence, never block Review
- [ ] #5 docs describe the data that leaves the machine; the App Store privacy audit (TASK-150.6) is updated
<!-- AC:END -->
