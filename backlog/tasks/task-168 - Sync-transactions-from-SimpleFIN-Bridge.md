---
id: TASK-168
title: Sync transactions from SimpleFIN Bridge
status: To Do
assignee: []
created_date: '2026-11-09 22:22'
labels:
  - importer
  - enhancement
dependencies: []
references:
  - 'https://www.simplefin.org/protocol-v1.html'
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Bank CSV downloads are manual and differ from bank to bank. SimpleFIN Bridge is a low-cost, read-only aggregator: a one-time setup token is exchanged for an access URL, and that URL returns accounts, current balances and transactions as JSON. Supporting it lets any Nigel user sync on a schedule instead of downloading statements.

Every SimpleFIN transaction carries a stable `id`, so dedup can be exact rather than the (date, amount, description) heuristic. TASK-117 needs the same mechanism for OFX FITIDs. Build it once: an `external_id` on transactions, with dedup on (account, external_id) when present.

Relates to TASK-117 (shared external-ID dedup) and TASK-46 (balances).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `nigel sync setup <token>` exchanges a setup token for an access URL, stored with the other credentials and never logged or printed
- [ ] #2 `nigel sync accounts` lists SimpleFIN accounts and maps each to a Nigel account; unmapped accounts are skipped
- [ ] #3 `nigel sync [--since DATE] [--dry-run]` imports new transactions through the existing import path, so rules, snapshots, import history and undo all apply
- [ ] #4 Transactions carry an `external_id`; one already imported by ID is never imported again, even if its description changes
- [ ] #5 A first sync can start after the last CSV import date so overlapping rows are not duplicated
- [ ] #6 Each account's reported balance is stored with its as-of date, and `report balance` can show it beside Nigel's computed figure
- [ ] #7 Settings in the web app can paste a setup token, show linked accounts and run a sync
- [ ] #8 The docs give an example timer unit that runs the sync, in the same shape as the existing schedule timers
- [ ] #9 Tests use a stubbed SimpleFIN server and fictional data; docs/importers.md gains the source
<!-- AC:END -->
