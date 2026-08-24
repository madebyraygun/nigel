# Changelog

## [1.1.0] (WIP)

### Changed
- **Report exports moved into the report viewer** — `e` (PDF) and `t` (text) export exactly the period on screen, so any year or month you can navigate to can be exported, not just the current year
- **Single "View Reports" picker** — the standalone export report/format screens are gone; the picker's last entry, "Export All Reports", exports every report at once (Enter for PDF, `t` for text)
- **Register exports from the dashboard's browser** — picking "Transaction Register" opens the interactive browser, where `x` (PDF) and `t` (text) export the full register; the register exports over an open date range instead of being clipped to the current year. `nigel browse register` is unchanged and binds neither key

### Added
- **Personal books** — `nigel init --profile personal` seeds a household chart of accounts (groceries, rent, utilities, …) instead of the business one; the K-1 worksheet steps aside in the report pickers, `report all`, and the web report directory, while everything else works the same. Transfers between your own accounts (the `Transfer` category) no longer count as income or spending in the P&L, expense breakdown, or cash flow on either profile — the register, account balances, and tax summary still show them, because there the cash movement itself is the point. `GET /api/status` reports the profile so the browser and the terminal hide the same worksheet
- **Delete a draft invoice entered by mistake** — `nigel invoice delete 1252`, `d` on the dashboard's invoice detail, a Delete action in the browser, and `DELETE /api/invoices/{number}`. Void is for the invoice a client has seen; delete is for the one that should never have existed, and only a draft that was never published and carries no payments qualifies. Everything else — sent, partial, overdue, paid, and void, which is a record that something happened — refuses with one sentence on every surface. The invoice and its line items go in one transaction. **The invoice number is not reused**: deleting the newest draft leaves a gap, because reissuing a number that may already have been quoted or exported is worse than a gap in a sequence
- **Duplicate an invoice** — `nigel invoice duplicate 1248`, `c` on the dashboard's invoice detail, a Duplicate action in the browser, and `POST /api/invoices/{number}/duplicate`. The copy is a fresh draft under the next number with a new token, carrying the client, currency, notes, terms and every line item and nothing about what happened to the source — no published date, no void date, no payment link. **The term travels, the dates do not**: a Net-14 invoice duplicates as a draft due fourteen days after *its* own issue date, and a source with no due date yields one that never goes overdue. Any state duplicates — draft, sent, paid or void — because duplication reads a shape rather than a status, and last quarter's paid invoice is the most useful thing to copy. The one refusal is an archived client, through the same guard `nigel invoice new` goes through
- **Recurring invoice schedules** — `nigel invoice schedule add/list/show/edit/pause/resume/end/run` puts a retainer or a hosting fee on a monthly, quarterly or yearly cycle, seeded from line items or from an existing invoice with `--from`. `nigel invoice schedule run` generates everything currently due and **drafts by default**; sending is an opt-in per schedule (`--autosend`), because a wrong figure reaching a client with nobody watching is worse than a draft nobody sent. **Each missed cycle bills as its own invoice dated that period's issue date**, not the day the run happened, so a machine that was asleep through February and March produces a February invoice dated February and a March one dated March. **A rerun is idempotent by recorded provenance**: every generated invoice stores which schedule and which period produced it, and a period already billed is skipped, so a cron misfire costs nothing. Editing a schedule changes future invoices and never past ones; pausing and ending keep every row and everything they generated. Built for launchd and cron — the run **never prompts**, taking `NIGEL_DB_PASSWORD` on an encrypted database the way `nigel backup` does and failing immediately with a sentence naming the variable when there is none, and exiting non-zero when anything a schedule asked for did not happen
- **Void takes down what an invoice published** — cancelling an invoice now deactivates its Stripe payment link and replaces its published page with a "this invoice has been voided" notice, so a client cannot pay an invoice `invoice sync` has stopped polling. The PDF stays where it is and the address keeps resolving. Both steps run after the cancellation has committed and neither can undo it: if Stripe or R2 refuses, the invoice is still void and the command prints the payment link's URL so you can deactivate it by hand. Nothing is required — an installation with no invoicing keys voids exactly as it did before, and an ordinary draft void says nothing extra. The same behaviour and the same sentences appear in the dashboard's invoice screen and on `POST /api/invoices/:number/void`, which gains an optional `paymentLinkUrl` and `teardownWarnings`
- **Invoicing in the web UI** — `nigel serve` now covers the whole invoicing surface: a clients manager, an invoice list with an A/R aging strip, a full-view invoice editor with repeatable line items, a sandboxed preview of the client-facing page, payments, void, Stripe sync, and a send that names every consequence before it happens and reports which step failed if one does. A/R aging joins the reports screen as a ninth report, and every figure the browser prints is tested against the figures `nigel invoice list/show/aging` and `nigel client list` print
- **`NIGEL_DB_PASSWORD` for unattended use** — an encrypted database can be unlocked without a terminal, so `nigel backup` runs from launchd, cron, or CI. The variable is consulted whenever the database is encrypted and takes precedence over the prompt; an unusable value (wrong, empty, or not valid UTF-8) is a hard error rather than a fall back to a prompt no scheduled job could answer. Plaintext databases ignore it entirely. See "Automated backups" in the README for the recommended keychain-sourced invocation and its tradeoffs
- **`nigel recategorize`** — non-interactive bulk category reassignment by transaction IDs or filters (`--from-category`, `--uncategorized`, `--year`/`--month`/`--from`/`--to`, `--pattern` with `--match-type`, `--account`, `--min-amount`/`--max-amount`) with `--dry-run` preview and `--yes` confirmation for filter-based moves; clears review flags like an in-app review

## [1.0.1] - 2026-08-05

### Fixed
- **Missing K-1 worksheet income** — income categories in the stock chart of accounts carried no `form_line` mapping, which prevented them from showing up on the K1 report. Income categories without a `form_line` now count toward gross receipts automatically, flagged with an `(auto)` note
- **K-1 meals limit applied inconsistently** — the headline Total Deductions now uses the 50%-limited meals figure, matching the Other Deductions sub-table
- **Clippy `collapsible_match` failures blocking CI** — four match-arm `if`s collapsed into match guards

### Added
- **Needs-mapping section on the K-1 worksheet** — expense categories with activity but no recognized `form_line` are listed with their totals instead of being silently excluded; a reserved `excluded` value marks categories deliberately outside the return (e.g. transfers)
- **`form_line` vocabulary** — `1120S-1a` (gross receipts), `1120S-2` (cost of goods sold), `1120S-5` (other income), alongside the existing `1120S-N`, `K-N`, and new `excluded` values; a schema migration backfills the stock chart-of-accounts categories

### Changed
- **Schema migrations run before any data-bearing command** — existing databases pick up migrations during normal use instead of only on `init`/`demo`/`restore`. The first command after upgrading prints a one-line migration notice; encrypted databases prompt for the password as usual

## [1.0.0] - 2026-03-02

### Added
- **Interactive TUI dashboard** — running `nigel` with no arguments launches a full-screen dashboard with YTD P&L, account balances, monthly income/expense bar chart, and single-key command menu
- **First-run onboarding** — guided setup screen with animated logo, collects user name, business name, optional password, and offers demo/fresh/load options
- **Splash screen** — rainbow gradient ASCII logo with floating particle effects on launch
- **Goodbye screen** — reverse logo animation with "Goodbye!" text and particle effects on dashboard quit
- **Database encryption** — SQLCipher encryption with `nigel password set/change/remove` commands; password prompted at launch, never persisted to disk
- **Schema migration system** — sequential versioned migrations run automatically on startup with savepoint transactions
- **Import enhancements** — `--dry-run` preview mode, malformed row tracking, generic CSV format auto-detection
- **`nigel undo`** — rolls back the last import by removing its transactions and import record
- **`nigel restore`** — recovers a database from a backup file
- **Interactive register browser** — scrollable transaction list with inline category/vendor editing, flag toggling, and text search (`/` to search)
- **Unified report command** — `nigel report <type>` with `--mode view|export`, `--format pdf|text`, and `--output` flags; interactive ratatui views with date navigation (Left/Right arrows, `m` toggles month/year)
- **Settings screen** — manage application settings from the dashboard TUI
- **Editable chart of accounts** — `nigel categories add/rename/update/delete` with TUI management screen
- **Account management** — `nigel accounts add/rename/delete` with TUI screen; delete blocked if account has transactions
- **`nigel rules test`** — dry-run pattern matching against existing transactions
- **Shell completions** — `nigel completions bash|zsh|fish|powershell`
- **Back navigation in review** — Esc undoes previous categorization, Tab skips forward
- **`nigel review --id`** — re-review a specific transaction by ID
- **`nigel rules delete`** — soft-delete categorization rules
- **Business name header** in text file exports
- **Page title headers** on all TUI screens
- **Keyboard shortcuts** on dashboard menu items
- **Snake game** easter egg accessible from dashboard
- **Version display** — version number shown at the bottom center of splash and onboarding screens
- **GitHub Actions CI** workflow
- **Integration tests** for CLI dispatch paths

### Changed
- Export picker shows format selection step (PDF / Text) instead of defaulting to PDF
- Demo transactions generated dynamically (18 months from current date) instead of hardcoded dates
- Browse register shows all transactions by default (no implicit year filter)
- Review screen migrated to ratatui from raw crossterm
- BofA importers refactored to share parsing helpers

### Fixed
- Splash screen no longer dissolves out before transitioning to dashboard — logo stays solid after reveal
- BofA CSV parsing when cardholder names contain commas
- Scroll-to-today bounds in short terminals
- Report parameter panics on empty date filters
- DB reliability: `last_insert_rowid()` correctness and SQLite busy timeout
- Import: `import_id` now populated on transactions
- Importer safety: `parse_amount` returns `Option`, streaming credit card detection
- Report bugs: `fiscal_year_start`, cashflow balance, K-1 sign corrections
- `account_names()` no longer silently discards errors
- Demo data balanced for realistic income/expense ratios
- Hardened error handling, file permissions, and first-run messages
- Migration edge cases and password trim warnings
- Compiler warnings without default features
- Date filter error handling and result collection

## [0.1.1] - 2026-02-27

### Added
- **Transaction register report** — `nigel report register` and `nigel export register` show all transactions for a date period with category, vendor, and account details. Supports `--year`, `--month`, `--from`/`--to`, and `--account` filters. Included in `nigel export all`.

### Fixed
- PDF table layout: header separator lines no longer overlap first data row text
- PDF spacing: tighter header-to-line gap, better separation between data rows and totals

## [0.1.0] - 2026-02-25

Initial release.
