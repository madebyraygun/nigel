---
id: TASK-172
title: Regex rules match case-sensitively while contains and starts_with do not
status: To Do
assignee: []
created_date: '2026-11-09 22:22'
labels:
  - bug
  - rules
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`categorizer::matches` uppercases both sides for `contains` and `starts_with`, but passes regex patterns through unchanged. A regex rule `(LONDON|BRIGHTON)` misses "Old Shades London" while a contains rule `london` matches it, and nothing in the docs or CLI says the match types differ.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Regex rules match case-insensitively by default (RegexBuilder::case_insensitive), consistent with the other match types; `(?-i)` opts out
- [ ] #2 `rules test` and categorization agree
- [ ] #3 Docs state the case rule for all three match types
- [ ] #4 A changelog entry covers existing regex rules whose matches widen
<!-- AC:END -->
