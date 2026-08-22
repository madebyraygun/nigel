---
id: TASK-127
title: Three dashboard and settings tests race the machine's real config directory
status: To Do
assignee: []
created_date: '2026-08-22 12:55'
labels:
  - tests
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Running `cargo test -p nigel --lib` in parallel fails a rotating subset of cli::dashboard::tests::{a_returning_launch_still_prepares_its_books, export_all_text_writes_the_profile_matrix, every_report_slug_opens_its_own_viewer} and cli::settings_manager::tests::update_check_loads_from_settings. The same run with --test-threads=1 is green, and the failure set differs run to run, so the tests are contending for one shared resource rather than asserting anything wrong.

Reproduced on a clean main (479 passed, 3 failed) as well as on feat/import-integrity, so it predates both. CI does not see it, which fits the theory: the contended resource is the developer machine's real ~/.config/nigel/settings.json and the data directory it names, which a CI runner does not have.

Worth fixing because it trains everyone to read a red local suite as noise. The fix is to give these tests their own config dir the way the fixture-capture tests already do, rather than to serialize them.
<!-- SECTION:DESCRIPTION:END -->
