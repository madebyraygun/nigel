---
id: decision-6
title: Invoicing stays off the ledger in v1; recognition is the bank deposit
date: '2026-08-19 16:06'
status: accepted
---
## Context

TASK-9.2 flagged one open question as a prerequisite rather than an implementation
detail: `invoice_payments` has no `transaction_id` (see the v4 migration in
`crates/nigel-core/src/migrations.rs`), so an invoice payment and the bank deposit
representing the same money are two unlinked records. Invoicing shipped as its own
source of truth — `invoices`, `invoice_payments`, derived status, aging buckets — with
no tie to the register.

Under a ledger, the question is what invoices post, and both naive answers are wrong.
Recognising revenue when an invoice is issued puts unpaid invoices into a cash-basis
P&L, which breaks the product's primary use case. Generating ledger entries from bank
transactions while invoices *also* post revenue double-counts it. This is the single
place the cash-basis promise (decision-5) could be broken.

## Decision

**In v1, invoices post nothing to the ledger. The ledger records what the bank
records.** A deposit is categorized to income exactly as today; that categorization is
the recognition, it happens once, and it happens at the bank. There is no A/R account
in v1 — deliberately: a cash-basis balance sheet carries no receivables (TASK-102.1
already treats them as not-tracked), and Nigel is cash-basis first. The invoicing
subsystem — invoices, payments, derived status, aging — continues as shipped, beside
the ledger rather than in it. Unpaid invoices never appear in cash-basis income, not
because a report excludes them but because they structurally cannot: they never post.

**The payment–deposit link is reconciliation, not posting.** `invoice_payments` gains a
nullable `transaction_id` naming the bank deposit that settled the payment. Links are
suggested — same amount inside a date window — and **user-confirmed, never silent**,
per the standing rule that all financial modifications require confirmation. What the
link buys is exactly what the missing column costs today: the invoicing tables and the
register become checkable against each other, and revenue cannot be double-counted
because only one of the two records ever posts.

**When they disagree, the bank wins**, trivially — it is the only thing posting:

- A payment with no matching deposit inside the window surfaces on a reconciliation
  surface; cash-basis income is unaffected because nothing was recognised.
- Amounts that differ — a processor deposit net of fees is the known case — are
  recorded on the link as the explanation of the difference. Nothing books a fee leg in
  v1; the deposit is the income figure, as it is today.
- Nothing auto-resolves. Ambiguity is a queue, not a heuristic.

**Deferred, not rejected.** A/R posting at issue is the first accrual posting rule, and
it stays exactly where decision-5 puts accrual: later feature work against a schema
that needs no change. When accrual ships, invoices gain posting rules (A/R at issue,
cleared at payment), reports gain the basis toggle, an opening A/R entry is derived
from then-open invoices, and the A/R account is one row in the merged chart — the same
way A/P will be. Nothing is added to the schema now on its behalf.

**Alternative considered: post A/R at issue in v1, with cash-basis reports excluding
A/R legs.** It satisfies the invariants on paper and it is how accrual-capable engines
usually work. Rejected for v1: it puts accrual-shaped recording into a release whose
every cash figure it leaves unchanged, manufactures a balance-sheet line a cash-basis
filer does not report, and buys nothing the reconciliation link does not already buy —
the aging report already works off the invoicing tables. Recording claims that every
report must then carefully un-recognise is complexity spent against the cash-basis-first
posture.

## Consequences

- The posting-rules subtask narrows to the reconciliation link: `transaction_id`, the
  suggest-and-confirm flow, and the disagreement surface.
- The cash-basis parity fixture still includes an issued-unpaid invoice (Acme) and a
  part-paid one (Cedar Systems), pinning that neither moves the cash P&L.
- The v1 trial balance and balance sheet carry no A/R, matching what a cash-basis filer
  reports.
- Accrual later is invoice posting rules plus the report toggle plus the opening-A/R
  derivation — feature work, no migration, per decision-5.
