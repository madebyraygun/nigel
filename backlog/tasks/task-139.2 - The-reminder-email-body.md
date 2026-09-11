---
id: TASK-139.2
title: The reminder email body
status: To Do
assignee: []
created_date: '2026-09-11 16:21'
labels:
  - invoicing
dependencies: []
parent_task_id: TASK-139
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Its own renderer beside `render_email_text`, sharing the MoneySummary and money seams so the figure it quotes cannot disagree with the published page.

Short by design: invoice number, what is still outstanding, how far past due, the pay link, and a line telling someone who has already paid to ignore it. No line items and no PDF attachment — both are one click away on the page, and repeating them makes a reminder read as a second invoice.

A before-due offset renders the same body with the tense corrected. Escalating copy per offset was considered and rejected: three to five bodies to keep in step, and tone is a template-override question rather than a reminders one.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A reminder body renders the number, the outstanding balance, the distance from the due date and the published page URL
- [ ] #2 The figure comes through the same MoneySummary seam the page and the PDF use
- [ ] #3 A before-due offset reads as a heads-up, a past-due one as a chase
- [ ] #4 No line items and no attachment ride along
- [ ] #5 Tests cover before-due, past-due and a partially paid remainder
<!-- AC:END -->
