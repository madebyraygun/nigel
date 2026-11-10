---
id: TASK-180
title: Several SimpleFIN feeds into one Nigel account
status: To Do
assignee: []
created_date: '2026-11-10 19:20'
labels:
  - feature
  - importer
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Some banks split one debt across several feeds. A business card program can show a parent account that carries payments and the card's total balance, plus one account per cardholder that carries that person's charges; a line of credit can do the same with its draw accounts. In the books that is one liability account, which is how statement imports already book it. Sync allows one SimpleFIN feed per Nigel account (`simplefin_accounts.account_id` is UNIQUE and `map_account` refuses a second feed), so the cardholder feeds cannot be synced at all. Mapping them to separate Nigel accounts leaves every one of those balances drifting forever, because charges land on one account and the payments on another.

Allow several feeds per Nigel account. Exactly one feed per account supplies the bank-reported balance, because a parent's balance already includes its children and adding them would double the debt. Feeds that only carry transactions are mapped with an explicit option, so nobody adds a second balance by accident.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A Nigel account can be fed by more than one SimpleFIN account; schema and migration drop the one-feed-per-account constraint
- [ ] #2 Mapping records whether the feed supplies the account's balance; mapping a second balance feed to an account is refused with a message naming the existing balance feed and the transactions-only option
- [ ] #3 `nigel sync accounts --map <key> <account> --no-balance` maps a transactions-only feed; the web Settings mapping offers the same choice
- [ ] #4 A sync records a reported balance only from the balance feed; transactions from every feed import into the account and dedup by external id per feed
- [ ] #5 A feed's first sync starts after the latest row from that feed or without an id, so another feed's newer rows do not make it skip earlier transactions
- [ ] #6 `sync accounts` (table and JSON) shows which feed supplies each account's balance
- [ ] #7 docs/importers.md and docs/commands.md describe several feeds per account and the balance rule
- [ ] #8 Tests cover mapping, the refused second balance feed, a sync from two feeds into one account, the first-sync window, and the migration of an existing database
<!-- AC:END -->
