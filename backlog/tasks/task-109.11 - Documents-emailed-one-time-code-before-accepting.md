---
id: TASK-109.11
title: 'Documents: emailed one-time code before accepting'
status: To Do
assignee: []
created_date: '2026-10-05 15:16'
updated_date: '2026-10-07 18:44'
labels:
  - documents
milestone: m-4
dependencies: []
parent_task_id: TASK-109
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The link is the only credential for an online response. Before the Worker records an accept, email a short-lived one-time code to the signer's address and require it, proving control of the inbox. Needs a Mailgun key in the Worker and a code store with expiry. The response format reserves a code field (docs/superpowers/specs/2026-10-05-documents-signing-design.md).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Accept requires a valid unexpired code emailed to the signer
- [ ] #2 Codes are single-use, rate-limited and expire
<!-- AC:END -->
