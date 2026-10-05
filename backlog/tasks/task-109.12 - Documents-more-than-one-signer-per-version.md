---
id: TASK-109.12
title: 'Documents: more than one signer per version'
status: To Do
assignee: []
created_date: '2026-10-05 15:16'
labels:
  - documents
dependencies: []
parent_task_id: TASK-109
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Sending currently requires exactly one signer; the schema already allows several (docs/superpowers/specs/2026-10-05-documents-signing-design.md). Add the rule that a version is accepted when every signer has accepted, progress in the status ('1 of 2 signed'), and the UI to add signers.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A version with several signers is accepted only when all have accepted
- [ ] #2 Status, CLI show and the web timeline show signing progress
<!-- AC:END -->
