---
id: decision-6
title: Invoice payments post to accounts receivable; cash-basis reports read the bank
date: '2026-08-19 16:06'
status: accepted
---
## Context

TASK-9.2 flagged one open question as a prerequisite rather than an implementation
detail: `invoice_payments` has no `transaction_id` (see the v4 migration in
`crates/nigel-core/src/migrations.rs`), so an invoice payment and the bank deposit
representing the same money are two unlinked records. Invoicing shipped as its own
source of truth — `invoices`, `invoice_payments`, derived status, aging buckets — with
no tie to the register. On single entry that is tolerable, because the bank transaction
is the P&L's source of truth and the invoicing tables are bookkeeping beside it.

Under a ledger it stops being tolerable. If journal entries are generated from bank
transactions while invoices post revenue in parallel, the result is either
double-counted revenue or a permanent unreconciled gap. This is the single place where
the cash-basis promise (decision-5, invariant 1) could be broken: a posting rule that
recognises revenue when an invoice is issued puts unpaid invoices into a cash-basis
P&L, and the change meant to strengthen the product has broken its primary use case.

## Decision

The posting rule:

- **An issued invoice posts to an A/R account** (receivable against revenue). The ledger
  records the claim the moment it exists, which is what makes A/R a real balance-sheet
  line and the aging report a read rather than a parallel derivation.
- **A recorded payment clears A/R.**
- **Cash-basis reports exclude A/R entirely.** Recognition happens when money hits the
  bank, which is what the register already records. TASK-9.2's own text says the bank
  transaction is the source of truth for the P&L; this keeps that literally true.
- **Unpaid invoices never appear in cash-basis income.** A fixture with an issued-unpaid
  invoice (Acme) and a part-paid one (Cedar Systems) must show only the banked money in
  the cash P&L, and a test pins it.

The alternatives fail on the repo's own evidence. *Posting nothing until payment* keeps
cash pure but leaves A/R and aging as a parallel derivation forever — the two-sources-
of-truth structure this work exists to retire, and no A/R line for the balance sheet to
read. *Recognising revenue at issue in all reports* is accrual by default, which breaks
the primary use case outright. Recording everything and recognising at the bank is the
only rule that satisfies both invariants, and it is the standard way double-entry
engines keep cash-basis books.

**The reconciliation mechanism** is a nullable `transaction_id` on `invoice_payments`,
linking a payment to the bank deposit that settled it. Linking is suggested — same
amount inside a date window — and **user-confirmed, never silent**, per the standing
rule that all financial modifications require confirmation. A linked deposit's entry
books the cash against A/R instead of against an income category, so the money is
recognised exactly once, at the bank, and the receivable closes.

**When they disagree**, the bank wins and nothing auto-resolves:

- A payment with no matching deposit leaves A/R open in the ledger even though the
  invoice reads paid; it surfaces on a reconciliation surface, and cash-basis income is
  unaffected because nothing was recognised.
- Amounts that differ — a processor deposit net of fees is the known case — are not
  absorbed: confirming the link requires categorising the difference explicitly (a fee
  expense leg on the same entry), so the gross clears A/R and the fee is visible.
- A deposit already categorised as income and later linked is recategorised onto A/R as
  part of the confirmed link; that is the correction path for double-counting, and it is
  a user action, not a heuristic.

## Consequences

- `invoice_payments.transaction_id` arrives with the journal schema work, and the
  posting rules land as their own TASK-9 subtask implementing this decision.
- The cash-basis parity test must include unpaid and part-paid invoices, because that is
  the case this decision exists to protect.
- Accrual later is a report toggle: revenue recognised at the A/R posting instead of at
  the bank leg. No schema change, per decision-5.
- The aging report keeps working exactly as it does; what changes is that its figures
  become readable off the ledger instead of provable only against a parallel table.
