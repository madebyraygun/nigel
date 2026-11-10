---
id: TASK-169
title: Identical rows in one file import as one (silent data loss)
status: Done
assignee:
  - '@claude'
created_date: '2026-11-09 22:22'
updated_date: '2026-11-09 22:42'
labels:
  - bug
  - importer
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`is_duplicate_row` checks each parsed row against the database, and rows are inserted one at a time, so a second row in the same file with the same (account, date, description, amount) matches the one just inserted and is skipped as a duplicate. Two $8.00 coffees on the same day become one, and the summary reports a skipped duplicate. Statements contain legitimately identical rows (transit fares, repeat purchases), and formats without a reference number cannot tell them apart.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A file with N identical rows imports N transactions on first import
- [x] #2 Re-importing an overlapping file still skips rows already present, counting by multiplicity (file count vs database count per key)
- [x] #3 Dry run reports the same counts the real import produces
- [x] #4 Tests cover identical rows within one file, across two overlapping files, and mixed
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Worktree fix/import-row-multiplicity off main.
2. Replace per-row is_duplicate_row with a dedup pass that runs BEFORE any insert: group parsed rows by key (date, amount, description); for each key, look up the database count once (SELECT COUNT(*) on account_id/date/amount/description), then walk the rows in file order — the first <db count> rows of each key are skipped, the rest are imported. Output is a per-row skip mask.
3. Both the real import and the dry run consume that same mask, so their counts agree by construction (AC #3). The real path inserts only the rows the mask keeps; post_import still sees all parsed rows, as it does today.
4. Key the amount on f64 bits with -0.0 folded to 0.0, matching the SQL equality the count uses.
5. Shape the pass as a small Dedup type in importer.rs so TASK-168/117 can add an external-ID branch ahead of the heuristic without touching its callers.
6. Tests (fictional data): N identical rows in one file import as N; an overlapping file with k copies against m in the DB imports max(0,k-m); a mixed file (some keys new, some partly present); a dry run reports the same imported/skipped as the following real import. Update test_import_file_detects_row_duplicates if it relies on the old behaviour.
7. Update the duplicate-detection line in docs/design-constraints.md to say matching is by multiplicity.
8. cargo fmt --check, cargo test -p nigel-core and the full suite serially; open a PR.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Row dedup counts by multiplicity: rows_already_held reads each key's database count once before any insert, skips that many rows of the key in file order, and imports the rest. Real import and dry run share the same skip flags.
<!-- SECTION:FINAL_SUMMARY:END -->
