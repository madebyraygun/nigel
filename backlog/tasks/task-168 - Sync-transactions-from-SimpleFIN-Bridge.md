---
id: TASK-168
title: Sync transactions from SimpleFIN Bridge
status: Done
assignee:
  - '@claude'
created_date: '2026-11-09 22:22'
updated_date: '2026-11-09 23:55'
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
- [x] #1 `nigel sync setup <token>` exchanges a setup token for an access URL, stored with the other credentials and never logged or printed
- [x] #2 `nigel sync accounts` lists SimpleFIN accounts and maps each to a Nigel account; unmapped accounts are skipped
- [x] #3 `nigel sync [--since DATE] [--dry-run]` imports new transactions through the existing import path, so rules, snapshots, import history and undo all apply
- [x] #4 Transactions carry an `external_id`; one already imported by ID is never imported again, even if its description changes
- [x] #5 A first sync can start after the last CSV import date so overlapping rows are not duplicated
- [x] #6 Each account's reported balance is stored with its as-of date, and `report balance` can show it beside Nigel's computed figure
- [x] #7 Settings in the web app can paste a setup token, show linked accounts and run a sync
- [x] #8 The docs give an example timer unit that runs the sync, in the same shape as the existing schedule timers
- [x] #9 Tests use a stubbed SimpleFIN server and fictional data; docs/importers.md gains the source
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Builds on TASK-169's dedup pass. Proposed as three PRs on one branch line: A (dedup core + import refactor), B (SimpleFIN + CLI + balances + docs), C (web Settings).

PR A — external-ID dedup (reused by TASK-117)
1. Migration 18: transactions.external_id TEXT NULL, plus a partial unique index on (account_id, external_id) WHERE external_id IS NOT NULL. Mirror it in the db.rs base schema.
2. ParsedRow gains external_id: Option<String>, namespaced by source (simplefin:<sfin account id>:<txn id>, and later ofx:<FITID>). SimpleFIN ids are unique per account only, so the namespace prevents collisions when two feeds land in one Nigel account. CSV parsers set None.
3. The 169 Dedup pass gets an ID branch: a row with an external_id is skipped if (account, external_id) already exists or appeared earlier in the same file. Description is ignored, which satisfies AC #4. Rows without an ID keep the multiplicity heuristic, and the DB count includes ID-carrying rows so a CSV imported after a sync still dedupes.
4. Split import_file into resolve+parse and import_parsed(conn, account_id, source label, checksum, ParseOutcome, dry_run). The file path keeps its behaviour, and sync calls import_parsed directly, so rules, the imports row (import history and undo) and rejects all apply (AC #3).

PR B — SimpleFIN
5. nigel-core simplefin module: decode the base64 setup token, POST to the claim URL, store the returned access URL in settings.json (0600, alongside the Stripe/Mailgun secrets). It is redacted from Debug and from every error message, and never printed (AC #1). HTTPS only. 403 means revoked/invalid and 402 means payment required; both map to clear errors. Bounded timeouts, as with the invoicing clients. GET {access}/accounts?start-date=..&end-date=.. with pending excluded.
6. Migration 19: simplefin_accounts (sfin_id, org name, name, currency, account_id NULL UNIQUE) for the mapping, and account_balances (account_id, balance, as_of, source). nigel sync accounts lists and refreshes them, with --map <sfin-id> <account> and --unmap. Unmapped accounts are skipped (AC #2).
7. nigel sync [--since DATE] [--dry-run]: one snapshot, then per mapped account a ParseOutcome from the feed (date via clock::local_day on posted, amount parsed from the string, pending rows skipped). Unparseable transactions go to rejects. Each account runs through import_parsed + categorize in one transaction. The imports row filename is 'SimpleFIN: <org> <account>', and the checksum is a hash of that account's payload. An account with nothing new writes no imports row and does not error.
8. Default start date (AC #5): with no --since, a first sync for an account starts the day after that account's latest imported transaction date. Later syncs start from the last sync minus a short overlap window, and ID dedup absorbs the overlap.
9. Store each account's balance with balance-date (AC #6). nigel report balance shows a 'Bank reported (as of …)' figure beside the computed one when present. This does not depend on TASK-46.
10. Docs: docs/importers.md SimpleFIN section; docs/commands.md; a systemd service+timer example in docs/runbooks (AC #8 — the repo has no existing schedule timer to match, so this sets the shape); README Features line.
11. Tests: a stubbed SimpleFIN server (local axum/tiny_http in tests) with fictional cast data covering the claim, 403/402, mapping, first sync after a CSV import, re-sync with a changed description, dry run, and balances (AC #9).

PR C — web (AC #7)
12. API: POST /api/sync/setup, GET /api/sync/accounts, PUT mapping, POST /api/sync/run (dry-run flag), documented in docs/api.md. The access URL is never returned.
13. A @nigel/ui component (wc-sync-settings: token entry, linked accounts with a mapping select, run/preview button, results) with a preview and describePreviewA11y, consumed by the Settings screen.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
SimpleFIN Bridge sync across four stacked PRs: #83 (multiplicity dedup, TASK-169), #84 (transactions.external_id with (account, id) dedup and import_parsed, reusable by TASK-117's FITID), #85 (nigel sync setup/accounts/sync, bank-reported balances, docs, systemd timer runbook), and the web Settings 'Bank sync' card with /api/sync endpoints.
<!-- SECTION:FINAL_SUMMARY:END -->
