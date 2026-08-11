---
id: TASK-81
title: 'Invoicing: schedule recurring invoices'
status: To Do
assignee: []
created_date: '2026-08-10 21:49'
labels:
  - enhancement
  - invoicing
  - schema
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Retainers and hosting are billed on a cycle and every one of them is currently drafted by hand. The stock chart of accounts already seeds a "Recurring client hosting/maintenance fees" category, so the books expect recurring revenue while nothing can produce the invoices for it.

Nigel has no daemon and should not grow one: main is sync, serve is a foreground process, and the launch-time Stripe sync is the only background-ish work in the app. So the shape is almost certainly a schedule stored in the database plus a command that generates whatever is due, invoked by launchd or cron — the arrangement docs/README already describe for automated backups, and the reason TASK-39 made non-interactive unlock work. A scheduled run on an encrypted database needs NIGEL_DB_PASSWORD, which is exactly what that task delivered.

Two decisions carry the design.

First, whether a run drafts or sends. Sending is irreversible and unattended sending means a wrong figure reaches a client with nobody watching; drafting is safe but means the cycle still needs a person. A default of draft with sending opt-in per schedule is the conservative reading, and it interacts with TASK-79 — a preview before sending cannot happen if no human is present.

Second, idempotency, which is the correctness property that matters most here. A run that fires twice must not bill twice, and the failure is expensive and client-visible. There is precedent on both sides in this codebase: invoice sync is idempotent by Stripe checkout session id, while import_invoiceshelf has no dedup at all and silently doubles everything on a second run. This must be the former, and the generated invoice needs to record which schedule and which period produced it so a repeat is recognisable rather than inferred from dates.

Other things to settle: catch-up behaviour when a machine was asleep past one or more cycles (one invoice or several); month-end and short months for a monthly cycle; whether line items and amounts are fixed at schedule time or re-read per run; how numbering stays sequential through next_invoice_number when several are generated at once; and what happens to a due invoice for a client that cannot be sent to, since require_email refuses a client with no address and 3 of the 23 imported clients have no billing address.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A recurring schedule can be created against a client with line items and a cycle
- [ ] #2 A single command generates everything currently due, suitable for launchd or cron
- [ ] #3 Running it twice for the same period generates nothing the second time — the generated invoice records its schedule and period
- [ ] #4 Whether a run drafts or sends is explicit per schedule, and drafting is the default
- [ ] #5 Catch-up behaviour after missed cycles is defined and documented, not incidental
- [ ] #6 A monthly cycle behaves correctly at month end and in short months
- [ ] #7 Generating several at once keeps invoice numbering sequential
- [ ] #8 A schedule can be paused, edited and ended without deleting its history
- [ ] #9 A due invoice for an unsendable client is reported rather than silently skipped or half-sent
- [ ] #10 The command runs unattended on an encrypted database via NIGEL_DB_PASSWORD, and never prompts
<!-- AC:END -->
