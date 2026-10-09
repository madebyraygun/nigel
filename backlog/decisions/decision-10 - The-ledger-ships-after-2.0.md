---
id: decision-10
title: The ledger ships after 2.0
date: '2026-10-07 18:43'
status: accepted
---
## Context

Decision-5 scopes cash-basis double entry for the release it calls v1, and TASK-9 carries
the work: classify every account, write a journal line pair for every transaction,
transfer and split, rewire every report to read the ledger, and prove the cash-basis
figures unchanged byte for byte. It is the riskiest change in the backlog — a backfill
over every row of every operator's books, and a rewrite under every report.

The 2.0 release is native apps with the Mac app in the Mac App Store, a server/client
model, invoices and contracts. None of the four needs the ledger; each builds on the
single-entry register, and the server/client work adds its own risk to the
data layer (users, attribution, an audit log, optimistic concurrency).

Integer money (decision-7, TASK-59) is the part of the ledger's groundwork that stands on
its own. It fixes a live correctness bug class (the reconciler disagreeing with its own
serialized discrepancy) and converts the storage format of every amount column.

## Decision

**The ledger (TASK-9 and its subtasks) ships in 2.1. Integer money stays in 2.0.**

- **Integer money stays in 2.0.** A storage migration is cheapest before a public launch:
  once the store app puts Nigel on machines the project never sees, every format change
  is a migration run on books nobody can inspect. TASK-59 lands after the documents
  branch merges, under decision-7's parity gate.
- **The journal migration waits for 2.1.** It is a second backfill and a report rewrite;
  running it in the same release as the money conversion, the multiuser data layer and a
  first sandboxed store submission stacks the riskiest changes into one release with
  nothing between them to fall back to. After 2.0 it lands on books already in integer
  minor units, which TASK-9.4's balance enforcement requires.
- **Decision-5's invariants and scope are unchanged.** Its "v1" scope boundary now names
  the 2.1 release.

## Consequences

- TASK-9 and its subtasks move to the 2.1 milestone; TASK-59 stays on 2.0.
- 2.0 balance sheets and trial balances remain derived from the register, as today.
- The multiuser work (epic 32) puts its attribution and audit hooks at the data-layer
  write functions, so the ledger routes through them in 2.1 rather than around them.
- Decision-6 (invoicing off the ledger) holds for 2.0 trivially, since there is no ledger.
