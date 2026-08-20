---
id: TASK-9.3
title: >-
  Chart of accounts merge: categories and accounts become one table carrying the
  TASK-9.1 classes
status: To Do
assignee: []
created_date: '2026-08-19 16:08'
labels:
  - architecture
milestone: m-0
dependencies:
  - TASK-9.1
parent_task_id: TASK-9
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Categories and accounts are two tables with two vocabularies. TASK-9.1 gives them a shared accounting class and explicitly defers the merger to the journal-lines work — this task is that merger. A journal line references an account, and `Consulting Income` and a checking account have to be the same kind of thing for a trial balance to be a read over lines rather than a join over two vocabularies.

## Proposal

One accounts table carrying the TASK-9.1 classes (asset, liability, equity, revenue, expense). Every category becomes an account with its class; bank accounts keep theirs. Everything a category carries today — `form_line`, `tax_line`, soft-delete, the advisory name uniqueness that lets a retired `Travel` coexist with a new one — carries over intact.

**This is structure, not vocabulary** (decision-5, invariant 2). The word "category" stays on every default surface: the register, review, rules, reports all keep saying what they say. That a category is an account underneath is invisible unless you go looking.

The rules engine, importers, and review flow keep working against the merged table — ids survive the migration or map through it, so stored rules and existing categorizations are untouched.

## Risk

Everything that joins `categories` or branches on `category_type` is in the blast radius: reports, the categorizer, the CLI/TUI/web category managers, the API routes. The TASK-9.1 audit list is the map. The parity requirement below is the safety net.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Categories and bank accounts live in one accounts table; every row carries a TASK-9.1 class stored as a closed set
- [ ] #2 Every existing category and account migrates with its form_line, tax_line and soft-delete state intact, and stored rules and categorizations still resolve
- [ ] #3 No user-facing surface changes vocabulary: register, review, rules and reports still say category, and no surface introduces debit/credit vocabulary
- [ ] #4 Every report produces identical figures before and after the merge on a committed fixture
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
