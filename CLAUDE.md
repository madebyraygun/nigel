# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Nigel — a Rust CLI bookkeeping tool to replace QuickBooks for small consultancies, usable for personal finances as well. Cash-basis, single-entry accounting with bank CSV/XLSX imports, rules-based categorization, and SQLite storage. A database keeps books under a **profile** — `business` (the default, Schedule C / 1120-S chart of accounts) or `personal` (a household chart with no tax mapping) — chosen at `nigel init --profile` or in onboarding and stored in the `metadata` table.

## ⛔ Public repository — no real book data (MANDATORY)

This repo is public and Nigel is developed against the operator's live books. Never commit anything read off them, in any file or commit message: amounts (revenue, COGS, payroll, distributions, balances, discrepancies), real people's names, client or vendor names and their reference numbers, addresses, EINs, SSNs, bank details, ownership splits.

Allowed: statutory figures every filer shares (the CA $800 minimum, the 1099 threshold, the $2,500 de minimis safe harbour, the 50% meals limit), and the fictional fixture cast — Acme, Cedar Systems, Juniper Labs, Harbor & Vale, Globex, Initech — with invented amounts.

Verifying against real books is fine. **Write the step, not the numbers**: "must show positive ordinary business income rather than a loss — compare locally against the filed return." An acceptance criterion only the operator can discharge is a defect in the criterion; use a fixture instead.

**Scope: content in the tree, not authorship.** The rule is about author-related and business-related PII appearing *in the app* — fixtures, docs, templates, test data, task notes, commit messages. Git author metadata is correct and must never be treated as a violation: every commit here is authored by its real author, and the history rewrite scrubbed content, never authorship. The org's own package and repository metadata — crate name, GitHub slug, Pages domain, maintainer address — is likewise how a published project identifies itself and is out of scope.

The check runs automatically: `build.rs` points `core.hooksPath` at the tracked `.githooks/` on the first `cargo build`, and `.githooks/pre-commit` refuses a commit that would introduce a hit. Judge it by its **exit status**, never by grepping its output — a grep for `OK` matches a failure report too, which is how a refused commit once got through. Sweep by hand as well when touching `backlog/`, `docs/`, `CLAUDE.md` or `README.md`:

```bash
./scripts/check-no-real-data.sh --staged    # or with no argument to scan the tree
```

It hard-fails on identity strings and warns on figures shaped like real book data; every warning must be statutory or fixture data. CI runs the same script on every push and pull request.

If real data does reach a commit, stop and tell the operator. A force-push does not remove it — the object stays retrievable by SHA through the web UI and API, deleting the branch does not help, and a pull request cannot be deleted at all. Only a GitHub Support purge finishes the job.

Features stay general-purpose: no compiled-in payroll column labels, cap tables, or single-state tax rules. Those belong in configuration or an editable default.

## Architecture

- **Crate layout:** lib + bin. `src/lib.rs` exposes every module (`db`, `models`, `reports`, `reviewer`, `importer`, `invoicing`, `categorizer`, `reconciler`, `migrations`, `settings`, `error`, `fmt`, `browser`, `tui`, `effects`, `pdf`, `cli`, `server`, and the eight single-file data layers `accounts`, `categories`, `rules`, `imports`, `backup`, `password`, `updater`, `clock`) as the `nigel` library; `src/main.rs` is the `nigel` binary and holds only clap parsing, the ratatui panic hook, and the dispatch pre-flight, calling into the library via `nigel::`
- **CLI:** Clap derive app in `src/cli/mod.rs` — subcommands are optional; running `nigel` with no arguments launches the interactive dashboard. Subcommands: init, demo, import, undo, categorize, recategorize, review, reconcile, accounts, categories, rules, report, browse, client, invoice, load, backup, restore, serve, status, password, update, completions
- **Database:** SQLite via rusqlite (bundled-sqlcipher) in `src/db.rs` — tables: accounts, categories (with form_line for 1120-S mapping), transactions, rules, imports, reconciliations, metadata (key-value store for per-database settings like company_name, profile, and next_invoice_number), clients, invoices, invoice_line_items, invoice_payments. Two chart-of-accounts templates (`BUSINESS_CATEGORIES`, `PERSONAL_CATEGORIES`); `init_db_with_profile()` seeds the chosen one and stamps the `profile` metadata key together, only when the categories table is empty, so re-running init never reseeds or restamps; `get_profile()` reads it back with absent-means-business (every pre-profile database carried the business chart). Optional SQLCipher encryption via `PRAGMA key`; password stored in runtime global `Mutex<Option<String>>` (`set_db_password`/`get_db_password`); `get_connection()` reads it internally so zero call-site changes needed; `open_connection()` for explicit password; `is_encrypted()` probes a DB file; `validate_password()` tests a password without side effects; `prompt_password_if_needed()` prompts via rpassword with 3 retries (used by CLI subcommands)
- **Importers:** `src/importer.rs` — `ImporterKind` enum dispatch (bofa_checking, bofa_credit_card, bofa_line_of_credit, gusto_payroll); each variant implements `detect()` and `parse()`; `GenericCsvConfig` supports user-defined column mappings stored as profiles in `csv_profiles` table (`save_csv_profile`/`load_csv_profile`/`list_csv_profiles`, the last returning `CsvProfile { name, config }` for the API); malformed CSV rows are counted and reported in import output; `built_in_formats()` lists the compiled-in importers (`ImporterFormat { key, name, account_types }`) for the API's format picker; `ImportResult` reports `format` (the resolved importer key, a profile name, or `generic` — `None` for a duplicate file, which is answered before resolution) and `import_id` (the `imports` row created, `None` for a dry run)
- **TUI:** `tui.rs` — shared ratatui helpers (style constants, `money_span`, `wrap_text`, `ReportView` trait with `date_params()`, `run_report_view()`) for interactive screens; `ReportViewAction` enum includes `Continue`, `Close`, and `Reload` (for date navigation); `browser.rs`, `cli/review.rs`, `cli/report/view.rs`, and `cli/dashboard.rs` use ratatui `Terminal::draw()` render loop
- **Dashboard:** `cli/dashboard.rs` — single-struct state machine with `DashboardScreen` enum; Home screen shows YTD P&L, account balances, monthly income/expense bar chart, and a command chooser menu with single-key shortcuts (b=Browse, i=Import, r=Review, c=Reconcile, a=Accounts, t=charT of accounts, u=rUles, z=Undo, n=iNvoices, k=Clients, v=View Reports, l=Load, p=Settings, s=Snake); all commands render as inline TUI screens; outer loop only re-initializes when Load changes the data directory. F5 refreshes dashboard data.
- **Account Manager:** `cli/account_manager.rs` — inline TUI screen for managing accounts (list, add, rename, delete); uses form sub-screens for add/rename with text input and type selector; delete blocks if account has transactions
- **Guardrail reasons:** `error.rs` carries `DeleteBlock` (`subject` and `BlockReason`, which carries its own count) and the `NotFound`/`Invalid`/`DuplicateName`/`Blocked`/`Conflict` `NigelError` variants. `accounts::delete_blocker`, `categories::delete_blocker`, `invoicing::clients::delete_blocker` and `invoicing::invoices::delete_blocker` return the structured block; `categories::blocking_reason` is a thin `to_string()` wrapper over it for the TUI status line. `BlockReason` carries the count on the variants that count something (`HasTransactions`/`HasActiveRules`/`HasInvoices`) and `NotDeletable` carries none, because a refusal about the row's own state has nothing to count and `DeleteBlock::count()` answering `None` is what keeps a `"count": 0` nobody chose off the wire. `Display` reproduces the CLI's original wording verbatim, so the same error reads identically in a terminal and carries a machine code over HTTP
- **Category Manager:** `cli/category_manager.rs` — inline TUI screen for managing the chart of accounts (categories); list/add/edit/delete with form sub-screens for name, type (income/expense selector), tax line, and form line; soft-delete blocked if category has transactions or active rules; data layer in `categories.rs`
- **Rules Manager:** `cli/rules_manager.rs` — inline TUI screen for viewing and deleting categorization rules; scrollable list with soft-delete confirmation; reads through the shared `rules::list_rules` data layer and deletes through `rules::deactivate_rule`
- **Client Manager:** `cli/client_manager.rs` — inline TUI screen for the invoicing clients (`k` on the dashboard); list, add and edit, with a four-field form (name, email, billing address, notes). `c` opens a contacts sub-screen for the selected client — `a` add, `e` edit, `d` delete, `b` make billing, `Esc` back — reusing `ClientForm`'s field machinery with three labels rather than bolting a row editor into a form where every printable key types into a field. Every write there is the whole list through `set_contacts`, so the screen invents no invariant the data layer does not enforce, and `b` on the only contact says so instead of rewriting it. Email is not shape-checked, because `nigel client add` does not check it either. Adds through `invoicing::clients::add_client` and edits through `update_client`, sending every field so a blank optional one travels as `Some(None)` and clears the column. `d` deletes behind an inline `Screen::ConfirmDelete` overlay, asking `delete_blocker` **before** the confirmation so a client with invoices gets the block's sentence on the status line and is never offered a dialog that would fail — `account_manager`'s precedent. `x` archives or unarchives the selected row and `A` shows or hides archived rows, which the list omits by default; neither is confirmed, because both are reversible in one keystroke and the confirmations in this app are for the things that are not. An archived row carries a `(archived)` marker inside the name column's own budget rather than in a fourth column, so every row stays the same width, and the footer names the verb the selected row will get
- **Invoice Manager:** `cli/invoice_manager.rs` — inline TUI screen for invoices (`n` on the dashboard); a scrollable list (number, status, client, total, balance, due) over `invoicing::invoices::list_invoices`, and a detail view carrying line items, payment history, balance and the Stripe link. `Detail.client` is an `Option` rendered with `optional_display`: a clientless invoice is unreachable (the delete guard and the foreign key both refuse it) but representable, and a detail view that refused to open would hide the one invoice worth looking at. The four actions live on the detail view only — `s` send, `p` record payment, `v` void, `d` delete — each behind a confirmation and each pre-flighted through the same guards the CLI uses (`invoices::ensure_not_void`, `invoices::payment_amount`, `invoices::ensure_voidable`, `validate_date`, and `cli::invoice::build_clients`), so the two front ends refuse the same things in the same words. Send and void are both two-phase — the key handler returns `InvoiceAction::Perform`, the controller paints the blocking frame (`Screen::Sending`/`Screen::Voiding`) and only then calls `perform_pending`, which dispatches on the screen — because void now reaches Stripe and R2 as well. A void with nothing left live reports on the status line; one that could not tear something down lands on the result screen, which wraps its body so a payment link's URL is readable rather than truncated. `d` stays one-phase for the opposite reason — a delete is one local transaction and reaches nothing, so a "Deleting…" frame would promise a wait that never happens — and it closes the detail, reloads the list and reports on the status line, since the invoice it was showing is gone. The footer advertises `d` only when `Detail.deletable` says so, which is `delete_blocker` called at load time rather than a second copy of its rule, because `draw` has no connection. `a` (or `n`) on the list opens the **draft form** — client selector, issue and due dates, currency, and repeatable line-item rows, creating through `invoicing::create_invoice`. Drafts only: send stays the deliberate action on the detail view. The row keys are `Ins`/`F2` to add a line below the focused row and `Del`/`F3` to remove it, because the dashboard hands the screen a bare `KeyCode` (no `Ctrl` chord is reachable), every printable character belongs to the description cell, and an Apple keyboard has no `Insert`. The form owns exactly two rules — a blank or unparseable quantity/unit amount, worded for the field rather than for `--item` — and every other refusal is the data layer's own sentence: it runs `validate_items`, `validate_date` and `validate_currency` in `create_invoice`'s order **only to learn which field a failure belongs to**, then calls `create_invoice`, which re-runs all of them and stays the sole writer. The message renders on its own line directly under the field it is about
- **Rules data layer:** `rules.rs` holds the whole rules surface as `&Connection` functions — `list_rules`/`get_rule` (`RuleRow`), `add_rule` (`NewRule`), `update_rule` (`RuleUpdate`, partial, `vendor: Some(None)` clears), `deactivate_rule` (soft), `test_pattern` (`RuleTestResult`, the dry run `nigel rules test` prints), `validate_match_type`, and `resolve_category_id`. The CLI subcommands are wrappers that resolve a category name to an id and print; the API passes ids straight through
- **Import Screen:** `cli/import_manager.rs` — inline TUI form for importing bank statements; file path input + account selector; runs import + auto-categorization and shows results
- **Undo Screen:** `cli/undo_manager.rs` — inline TUI screen for undoing the last import; shows import details (filename, account, date, transaction count) and confirms before deleting; data layer in `imports.rs` (`list_imports` returns the full history newest-first; `get_last_import` is its first row)
- **Reconcile Screen:** `cli/reconcile_manager.rs` — inline TUI form for account reconciliation; account selector + month/balance input; shows reconciled/discrepancy result
- **Load Screen:** `cli/load_manager.rs` — inline TUI form for switching data directories; validates path and triggers dashboard reload
- **Reports:** `cli/report/` — unified report command with `--mode view|export`, `--format pdf|text`, and `--output` flags; `mod.rs` dispatches to `view.rs` (interactive ratatui views), `reports::text` (comfy_table formatting), or `export.rs` (PDF export); non-TTY automatically falls back to plain text stdout. `reports::register_range_label()` (register period label: `2025-03`, `FY 2025`, or `All dates` when no date filter is given — unlike `reports::date_range_label`, which defaults a missing year to the current FY) and `reports::register_subtitle()`, which appends active register filters to that label for text and PDF headers, are pure functions the CLI's report dispatch calls into. `TableReportView` supports interactive date navigation: Left/Right arrows page between periods, `m` toggles month/year granularity. `src/reports/` owns the report vocabulary: `ReportKind` (one variant per report, `as_str()` giving the CLI/export slug — `Aging` is `aging`, with `DateGranularity::None`) and `DateGranularity` (MonthAndYear, YearOnly, None), with `ReportKind::granularity()` as the single mapping between them; `ReportCommands::report_name()` delegates to it, and the views read their granularity from it. `PeriodMode` and the current-period state stay in `view.rs`. Inside the dashboard's report viewer, `e` exports the viewed period as PDF and `t` as text (the `e` hint and key are absent without the `pdf` feature); the footer advertises these keys only when the view is hosted by the dashboard, which is the only place they are bound. The dashboard's `v` menu entry opens a single "View Reports" picker (`report_picker_items` — `REPORT_TYPES`, or `PERSONAL_REPORT_TYPES` without the K-1 row, with `canonical_report_idx` translating personal selections back); its last entry, "Export All Reports" (canonical index `EXPORT_ALL_IDX` = `REPORT_SLUGS.len()`, the "all reports" index of `do_export`/`do_text_export`), exports every report — Enter for PDF (text when built without the `pdf` feature), `t` always for text. "Transaction Register" (`REGISTER_IDX`) delegates to the register browser rather than a `ReportView`; the dashboard intercepts `x` (PDF) and `t` (text) for it via `browser_export_action` — `e` belongs to the browser's inline editing — gated on `RegisterBrowser::export_hints_enabled()` and skipped while `is_capturing_input()` reports that a search/jump/edit prompt is swallowing keystrokes. `effective_year` gives the register an open date range so its export matches the unfiltered browser; every other report falls back to the current year
- **K-1 worksheet mapping:** `reports::resolve_k1_mapping()` maps each category to a 1120-S worksheet slot from its `form_line`. Vocabulary: `1120S-1a` (gross receipts), `1120S-2` (cost of goods sold), `1120S-5` (other income), `1120S-N` (deduction lines 7-19), `K-N` (Schedule K items), `excluded` (intentionally outside the worksheet, e.g. transfers). Income categories with no `form_line` fall back to gross receipts and are listed in the report's `auto_mapped` note; expense categories with no `form_line` are collected in `unmapped` and surfaced in a "Needs mapping" section, excluded from all totals. The worksheet reports Gross Receipts, Cost of Goods Sold, and Gross Profit; `total_deductions` is the sum of deductible amounts (meals limited to 50%).
- **Effects:** `effects.rs` — shared pastel rainbow gradient palette, `gradient_color()` interpolation, `Particle` struct with `new()`/`seeded()`/`tick()`/`is_dead()`, `pre_seed_particles()`, and `tick_particles()` helpers; used by splash, goodbye, onboarding, and snake screens
- **Splash:** `cli/splash.rs` — 1.5-second splash screen shown on app launch (skipped during first-run onboarding); displays Nigel ASCII logo with rainbow gradient text and pre-seeded floating particle background; dismissable by any keypress. For encrypted databases, the splash holds indefinitely (no auto-fade) and displays an inline masked password input below the logo; supports up to 3 attempts with error feedback; `run()` for unencrypted, `run_with_password(db_path)` for encrypted
- **Goodbye:** `cli/goodbye.rs` — 1.2-second farewell screen shown when quitting the dashboard; displays Nigel ASCII logo with "Goodbye!" text, plays the reverse of the splash reveal animation (characters disappear), with particle background; dismissable by any keypress
- **Updater:** `updater.rs` — launch-time version check; `check_for_update()` queries the GitHub Releases API for the latest version and compares via `semver`; `check_with_cooldown()` is the data-only check — opt-out, 24-hour cooldown (stored in `last_update_check` in settings.json), then `check_for_update()` — and `check_and_notify()` is the sentence `update_notice()` formats from it. `cli/update.rs` is `nigel update`: it calls `check_for_update()`, then downloads the correct platform binary and self-replaces via the `self_replace` crate; opt-out via `update_check: false` in settings; dashboard shows yellow notification bar; CLI prints to stderr. `nigel serve` runs `check_with_cooldown` in a background task at startup and reports the version as `updateAvailable` on `/api/status`, so the web dashboard shows the same notice without the request path ever waiting on GitHub
- **Settings Manager:** `cli/settings_manager.rs` — inline TUI screen for managing app settings; five editable letterhead rows (business name, address, phone, logo, payment info — DB metadata `company_name`, `company_address`, `company_phone`, `company_logo`, `payment_instructions`), then password management and the auto-update check toggle; password sub-screen delegates to `PasswordManager`. `Screen::Editing(usize)` is keyed by the row's `MENU_*` constant and `metadata_key(row)` is the single mapping from a row to the key it writes, so one edit handler serves all five. The address and payment-instruction rows take `\n` as a two-character escape and store real newlines (the form has one single-line buffer per field and no multi-line widget), and re-display the escape rather than a raw newline. The escape is **symmetric** — the backslash is escaped too — so a value survives a form round trip unchanged; without that a stored literal `\n` would be rewritten into a real line break every time the field was opened and saved. The logo row takes a **path** and stores a `data:` URI: the MIME is declared from the bytes and `document::parse_logo` then checks the bytes against it, so a `.png` holding a JPEG is refused on the status line with the stored key untouched. An empty field clears its key, as it always has for the name
- **Invoicing:** `src/invoicing/` — accounts-receivable only, kept out of the transaction register. `invoices.rs`/`clients.rs` (data layer, `next_number` from the `next_invoice_number` metadata key starting at 1248, derived status via `refresh_status` — `void` derived from the `voided_at` column the way `sent` is derived from `published_at` — `ar_aging_detail` (the aging report: buckets, the open invoices behind them, and the outstanding total) with `ar_aging` as its buckets, `list_invoices` (every invoice with its client and paid-to-date in one query — a per-row `paid_amount` would be N+1 on a screen that redraws — taking an optional status filter, which accepts the six status words plus `open`, and an optional client id) and `payments` (one invoice's history, oldest first), `payment_amount` (the explicit amount or the whole outstanding balance, refusing a settled invoice as `Conflict { code: "no_balance" }` and a NaN or non-positive amount as `Invalid`), `validate_payment_method` (the `invoice_payments` CHECK set, called by `record_payment` so an unknown method never reaches the constraint), `parse_date`/`validate_date` (the one date rule, called by all five date writers — `create_invoice`, `update_invoice`, `record_payment`, `void_invoice`, `mark_published` — plus migration v6 and the InvoiceShelf import; `validate_date` returns the **normalized** zero-padded `YYYY-MM-DD` the way `validate_currency` returns the uppercased code, and `parse_date` the `NaiveDate` behind it for callers that want the day itself. A four-digit year is required before chrono is consulted, because `%Y` reads `26-8-9` as the year 26 AD; `is_overdue` compares ISO strings, so an unpadded date would never read as past due), `validate_items` (called by `create_invoice` and `update_invoice`: at least one line, finite figures, and a finite total above zero — each line total and the sum are checked *after* the arithmetic, since `1e308 * 1e308` is infinity and two opposite-sign overflows sum to NaN, which serde renders as `null` against a number), `is_void`/`ensure_not_void` (called by `send_invoice` and `record_payment` themselves, not only by the CLI wrapper, so a caller reaching the data layer directly cannot send or pay a cancelled invoice), `update_client`/`client_summary` behind `nigel client edit`/`client show`, `add_client`/`update_client` refusing an empty or duplicate name the way `accounts::add_account` does, `delete_blocker`/`delete_client` blocking a delete while the client has invoices (`DeleteBlock::invoices`, reason `has_invoices`, every status counted), `delete_blocker`/`delete_invoice` (delete is for the draft entered by mistake — never published, no payments — and everything else refuses as `DeleteBlock::not_deletable`, reason `not_deletable`; the guard runs inside the delete's own transaction beside the line-item cascade, the payment rows are asserted absent rather than cascaded, and `next_invoice_number` is deliberately left where it is), and `update_invoice`/`void_invoice` guarded by `ensure_editable`/`ensure_voidable`; both update structs use the `Option` leaves-alone / `Option<Option<_>>` clears convention `rules::RuleUpdate` established), `gateway.rs` (`PaymentGateway`/`AssetPublisher`/`Mailer` traits, implemented by `stripe.rs`, `r2.rs`, `mailgun.rs` and faked in tests so nothing hits the network; `PaymentGateway::deactivate_payment_link` is Stripe's `active=false`, the only way a payment link goes out of service, and `AssetPublisher::publish_page` rewrites index.html alone, leaving the PDF beside it, and `public_base`/`logo_url`/`publish_logo` are the letterhead's own object — `public_base` because a *recorded* address is only usable while this installation still serves it, `logo_url` provided rather than required and pure, so a page can be rendered against an address before the object exists), `r2.rs` (the S3-API publisher, plus the three pure functions that name and check an address: `object_key(token, PAGE_OBJECT | PDF_OBJECT)` is the layout, `public_url` hands out `{base}/{token}/index.html` — the *file*, because a static host is not required to have an opinion about directories and a plain R2 custom domain answers the directory form with a 404 — and `validate_public_base_url` / `public_base_url_warning` are the hard and soft checks on the setting behind it), `document.rs` (`MoneySummary`/`MoneyLine` — the one place that decides which money lines a document prints and what they say: Subtotal and Tax only when tax is non-zero, Total always, Paid and Balance due once anything is paid, with a balance inside half a cent of zero clamped so no document prints `-0.00`. `money(amount, currency)` is the one figure format both documents use, for line items and the money block alike — separators, two decimals, `$` for USD and a code prefix (`EUR 6,600.00`) otherwise, since a symbol cannot say which dollar it means and not every symbol survives printpdf's WinAnsi built-ins. It closes TASK-87: the page used to print a bare `3600.00` where the PDF printed `$3,600.00`, and the block mixed `$` rows with `USD` rows. `fmt::money` stays dollar-only and stays the reports' and the CLI's. `emphasis` is **positional** — exactly the last line carries it, which is Total on an unpaid invoice, Balance due once something is paid and Credit on an overpayment — and both documents render it by **weight alone at the shared body size**, so the block reads as a column of figures with the amount owed picked out rather than as two headlines with a whisper between them. Both renderers consume `lines()`, which is what makes the page and the PDF agree by construction rather than by review; `address_lines` is the same idea for a multi-line billing address. Every other shared decision of the house layout lives here too: `CompanyBlock`/`company_block` (the From block — the name trimmed, the address through the same `address_lines` a client's goes through, the phone through `email_line`'s trim-or-`None`, and `is_empty` so neither document draws a "From" over nothing), `MetaRow`/`meta_rows` (Invoice ID emphasised, Issue Date, and Due Date only when there is one), `due_value`/`terms_block_text` (single-line terms ride beside the due date as `2026-09-05 (Net 30)`; a paragraph stays its own block, and folded terms never also print as one), `payment_lines` (`address_lines` without the clamp — the operator's own prose about their own bank is not something to cut off with a `...`), the shared **document palette and metrics** — `DocumentColor` with `hex()` for CSS and `unit_rgb()` for printpdf, `BORDER_GRAY` (the one medium neutral every structural rule on both documents is drawn in), `ROW_SHADE` (the zebra tint), `row_is_shaded` (the second item row and every other one after it, so neither document counts "every other" from a different end), and `LOGO_WIDTH_FRACTION`/`LOGO_HEIGHT_FRACTION` (a share of each document's own measure rather than a length, because the page counts in `rem` against its body width and the PDF in millimetres against its printable width, and what has to match is how large the mark reads) — and `parse_logo`/`Logo`/`MAX_LOGO_BYTES` (prefix → `image/png`|`image/jpeg` allow-list → base64 → magic bytes → the 128 KiB cap → pixel dimensions read from the PNG `IHDR` or the JPEG `SOFn` frame, so the module validates a logo identically in a build with no `pdf` feature and so no `image` crate; the empty string is no logo rather than an error), `render_html.rs` + `templates/invoice.html` (single-pass `{{KEY}}` expansion over a fixed placeholder vocabulary (`PLACEHOLDERS`); text keys are escaped values an author places, *fragment* keys are pre-built blocks that are empty when there is nothing to say — `DUE`, `NOTES`, `TERMS`, `PAY`, `COMPANY_BLOCK` (the whole ruled From block), `CLIENT_ADDRESS_BLOCK`, `CLIENT_EMAIL_BLOCK`, `LOGO` (the `<img>`, absent rather than broken when the stored value cannot be parsed), `META_ROWS`, `TERMS_BLOCK`, `PAYMENT_BLOCK`, and `TOTALS`, the money block as `<tr>` rows. `REQUIRED` never grows, so a template exported before a vocabulary change keeps validating; `REQUIRED_ALTERNATIVES` is how `{{TOTALS}}` stands in for `{{TOTAL}}` without demanding either. `PayButton` renders a live link, an inert placeholder, or nothing; `load_template`/`template_path` resolve `<data_dir>/templates/invoice.html` over the embedded `DEFAULT_TEMPLATE`, validating at load; `Branding` carries the template, the whole letterhead — company name, address, phone, logo and payment instructions — and the contact address in from the CLI, resolved once by `invoicing::wiring::company_profile` rather than by each construction site), `render.rs` (`render_invoice` — the one seam turning an invoice into the HTML+PDF pair `send` publishes and `preview` writes locally, plus `pay_button_for`, which lives here rather than in the CLI layer so preview, send and a republish cannot disagree: void *and* paid-in-full omit the button, which is what makes a republished page stop offering to charge a settled invoice; it loads the line items *and* `paid_amount`, building the `MoneySummary` both renderers take, so every caller above it shows the same figures with no signature change; `pdf: None` without the feature), `send.rs` (`send_invoice_traced` — Stripe link → render → R2 publish → Mailgun email → mark published, reporting each step as a `(SendStep, StepOutcome)` pair and a failure as `SendFailure` carrying the step, the steps completed, whether the email had already gone out and the invoice's status; any failure leaves the invoice a draft, and `send_invoice` is the two-line wrapper over it that answers the public URL for the CLI and the TUI. `require_email`/`ensure_payable` are the precheck guards, raised as `Conflict { code: "client_missing_email" | "invoice_not_payable" }` so a missing address is a 409 naming the client rather than a 500), `void.rs` (`void_invoice_with_teardown` — the orchestration layer all three front ends void through, the way `send_invoice_traced` centralizes send: `void_invoice` commits first, then the Stripe link is deactivated and the published page is replaced with `render_html::voided_page_html`. Both are **best-effort** and neither can roll the void back; `VoidOutcome` reports each as a `TeardownStep` (`NotApplicable`/`Skipped`/`Done`/`Failed`) and `warnings()` is the one place the sentences live, so a terminal and a browser describe the same void identically. The gateway and publisher arrive as `Option`, because void is the one invoicing command that works on an installation with nothing configured — the whole config matrix is on the module), `logo.rs` (`pending`/`publish`/`published_logo_url`/`fingerprint`/`set_company_logo` — the letterhead logo as a content-addressed, immutable object beside the page: `pending` answers the address without uploading so the page can be rendered against it, `publish` writes the object after the render has succeeded and only when the `published_logo` record does not already name it, and a failure degrades that render to the stored `data:` URI plus a sentence), `republish.rs` (`republish_invoice` — the corrected page a payment puts back, built on `void.rs`'s shape: the write commits first, nothing out here can undo it, and every outcome is a `Republished` variant plus a sentence `RepublishOutcome::warnings()` owns, so a terminal and a browser describe the same republish identically. Infallible by construction — a broken template, an unreachable R2 and a failed read are all warnings, because the payment is money already received. Without the `pdf` feature it falls back to `publish_page`, correcting the page and leaving the attachment the client was actually sent), `sync.rs` (`sync_all_report` — pull-based Stripe reconciliation, deduplicated by checkout session ID, returning a `SyncReport` of recorded, checked and per-invoice failures rather than printing them, since a browser cannot read the server's stderr; only a run where every invoice failed is an error), `import_invoiceshelf.rs` (one-time InvoiceShelf migration, cents → dollars). `invoicing::http_client()` is the one `reqwest::blocking::Client` all four external call sites go through, bounded at `CONNECT_TIMEOUT` (10s) and `REQUEST_TIMEOUT` (30s) — reqwest applies neither by default, and an unbounded call is a wedged terminal for the CLI and a blocking thread the server never gets back. CLI surfaces in `cli/client.rs` and `cli/invoice.rs`, whose printing lives in the pure `format_invoice_list`/`format_invoice_show`/`format_client_list` (mirroring `reports::text`, so the figure-parity fixtures can produce a text side without a terminal; `format_aging` is `reports::text`'s own). Every money figure in them goes through `fmt::money` (`$1,850.00`), which is what `format_aging` and `wc-money` already print, so the CLI, the TUI and the browser cannot disagree about how an amount reads; a line item's quantity keeps plain decimals, being a count rather than an amount. Every invoicing struct that crosses the wire derives `Serialize` with camelCase names following the task-31.2 pattern — `Client`, `Invoice`, `InvoiceLineItem`, `InvoicePayment`, `InvoiceStatus`, `NewLineItem` (also `Deserialize`, being a request input), `InvoiceListRow`, `ClientSummary`, `AgingReport`, `ImportSummary` — while the gateway values and the three external clients stay unserializable by construction. Usage doc in `docs/invoicing.md`
- **Launch sync:** `main.rs` `sync_invoice_payments()` — runs before any command that reads or writes the books when `stripe_secret_key` is configured; prints a stderr notice on recorded payments or on failure and never blocks the command; skipped for init, demo, load, update, password, restore, completions, serve (its database may still be locked and startup must not block on the network), `invoice sync`, `invoice preview` (defined to make no network call), and `invoice template` (touches no database)
- **Web server:** `src/server/` — `nigel serve` runs an axum app on 127.0.0.1 serving the JSON API and the embedded SPA from the same binary; behind the default-on `serve` feature (axum, tokio, tower, rust-embed, open, subtle). `mod.rs` owns the tokio runtime (the crate's only async entry point — `main` stays sync) and assembles the router; `auth.rs` holds the session token, Host/Origin validation, and cookie parsing; `error.rs` defines `ApiError`/`ApiErrorCode` and the `{"error": {...}}` envelope; `state.rs` holds `AppState` (db path, session token, build features, unlock gate, `db_gate`) — the db path is behind an `RwLock` because the settings screen can switch data directories while the server runs, and `db_gate` is a `tokio::sync::RwLock` readers hold while a connection is open so encrypt/decrypt/switch can rewrite the database file exclusively; `secret.rs` wraps password strings so they cannot be printed (redacted `Debug`, zeroized on drop); `extract.rs` wraps axum's `Json`/`Path` extractors as `ApiJson`/`ApiPath` so a malformed body or a non-numeric id answers in the error envelope instead of axum's plain text; `routes/` gets one module per domain (`status`, which owns `/status` and `/unlock`, the eight `reports` endpoints and their `exports` downloads, the `accounts`/`categories`/`rules`/`imports` lists and their writes, plus `transactions`, `review`, `reconcile`, `settings`, `clients` and `invoices`, and a JSON 404 fallback); `routes/mod.rs` layers the locked guard over the whole `/api` router, so a route added anywhere is guarded unless it is named in the short ungated list, and holds the shared helpers — `with_conn()` (the `spawn_blocking` + `db::get_connection` wrapper every handler runs its query through) and `with_conn_api()` (the same, for work that answers in `ApiError` — an export refusing a format this build cannot render), `ensure_account_exists()`, `not_found_because()` (which re-answers the data layer's plain `NotFound` with the name of the thing a handler was looking up, for the routes that resolve more than one), the `Deleted` response body, and `double_option()` for telling an absent PATCH field from an explicit `null`; `uploads.rs` owns the spool a browser import passes through (sanitized filename, one directory per upload, 0600/0700, resolve-by-id, hourly purge); `static_files.rs` serves `web/dist` via rust-embed with an index.html fallback for SPA routes. Handlers open their own connection per request — no pool
- **Read API:** `src/server/routes/reports.rs` exposes the eight reports at `/api/reports/{pnl,expenses,tax,cashflow,balance,flagged,register,k1}`, each wrapped as `{ granularity, report }` where `granularity` comes from `ReportKind::granularity()`. Which date parameters a route accepts mirrors its `nigel report` subcommand exactly, derived from that same granularity plus per-route `ranges`/`account` flags (`ParamSpec`); the register's `--category`/`--uncategorized` filters are the one CLI-only exception — `/api/reports/register` takes only `account` plus dates, and `RawQuery` names the two filter parameters solely to answer them with a 400 rather than let serde drop them silently. `RawQuery` takes every parameter as a string so validation errors land in the standard envelope instead of axum's plain-text `Query` rejection. `routes/{accounts,categories,rules,imports}.rs` serve the five list endpoints as bare JSON arrays
- **Invoicing API:** `src/server/routes/clients.rs` serves `GET /api/clients` (optional `includeArchived`, strictly `true`/`false` or a 400) and `GET /api/clients/{id}` — the detail route is 68.1's `client_summary` with the client's own fields `#[serde(flatten)]`ed beside its invoice history and open balance, so a screen showing one client gets one object rather than the CLI's nested shape. `src/server/routes/invoices.rs` serves `GET /api/invoices` (optional `status`, which reuses `list_invoices`'s own vocabulary — the six words plus `open` — and `clientId`), `GET /api/invoices/{number}`, `GET /api/invoices/aging` (optional `asOf`) and `GET /api/invoices/next-number`, plus the two non-JSON preview routes `…/{number}/preview` and `…/{number}/preview.pdf`. `{number}` is the invoice number, not the row id: it is what the CLI takes and what the user reads off the page. The detail response flattens the invoice, so the skipped `token` stays off the wire and a computed `publicUrl` carries the address instead; the `canEdit`/`canSend`/`canVoid`/`canPay`/`canDelete` flags are `ensure_editable`/`ensure_voidable`/`ensure_not_void`/`payment_amount`/`delete_blocker` **called**, never re-derived from `status`, because an edit is blocked by recorded payments as well as by status, and so is a delete. `routes::not_found_because` narrows the data layer's plain `NotFound` to `invoice_not_found` or `client_not_found` at the route, since a handler that resolves both owes the caller the distinction. Filtering the list by a client that does not exist is a 404, not an empty array — the `ensure_account_exists` reasoning. The preview routes go through `with_conn_api` and render via `invoicing::render::render_invoice` with the pay button and `contact_email` placeholder `cli::invoice` already picks, take no gateway (so no network call is reachable), set `Content-Security-Policy: sandbox` and `X-Frame-Options: SAMEORIGIN` on the HTML (the one response that overrides the blanket `DENY`, which blocks same-origin framing too and would leave the SPA's preview iframe blank), and answer `pdf: None` as a 501 carrying `PDF_DISABLED_MESSAGE` while the HTML route keeps working
- **Invoicing write API:** `src/server/routes/clients.rs` adds `POST /api/clients` and `PATCH /api/clients/{id}` (both taking `contacts` as a whole-list replacement — a plain `Option`, not `double_option`, because an empty array is how the list is cleared — and answering 400 when `email` and `contacts` arrive together), `DELETE /api/clients/{id}` and `POST /api/clients/{id}/archive`/`unarchive` (a state transition with a server-written timestamp, so a verb rather than a `ClientPatch` field — `POST …/void`'s precedent, each answering the refreshed `Client`) — the patch body is `ClientUpdate` field for field (the three nullable columns `double_option`, `name` a plain `Option` because the column is NOT NULL), and every refusal is the data layer's: the empty and duplicate name checks in `add_client`/`update_client`, the has-invoices block in `delete_client`. `src/server/routes/invoices.rs` adds `POST /api/invoices` (201 with the created draft's `InvoiceDetail`), `PATCH /api/invoices/{number}`, `DELETE /api/invoices/{number}` (`Deleted::new(invoice.id)`, the `DELETE /api/clients/{id}` body — the whole handler is `find_invoice` then `delete_invoice`, keeping the data layer's code and sentence and adding through `enrich_block` only the two facts a refusal's *advice* turns on: `status`, and `canVoid` as `ensure_voidable` **called**, because "void it instead" is a dead end for an invoice with payments, which refuses void too), and `POST …/{number}/void` and `…/{number}/pay` — `PATCH`, void and pay answer the *refreshed* detail because `refresh_status` derives the status on almost every write — void answers `VoidResult`, which is that detail flattened plus an optional `paymentLinkUrl` (a Stripe link still live) and `teardownWarnings`, since a teardown that could not reach Stripe or R2 is a 200 carrying a voided invoice, never a failed request. `void_with` is the trait-taking seam `send_with` established, so the teardown is fake-tested and no test in the module can reach the network. The route opens no transaction of its own — `update_invoice`, `void_invoice` and `record_payment` each run their guard inside theirs, so draft-only is read from the current row rather than from anything the client sent — and it re-derives no guard, only enriching the conflicts it forwards with the figures a screen wants (`status` on `not_draft`, `total`/`paid` on `has_payments` and `no_balance`, `status`/`canVoid` on `not_deletable`), leaving the code and the sentence as the data layer wrote them. `items` is a whole-list replacement matching the CLI's repeatable `--item`, validated by the data layer's own `validate_items` (see the Invoicing entry) rather than by the route, so `nigel invoice new` refuses the same figures; the route adds only the date shape check, through `routes::reports::parse_date`, so `2026-4-1` is a 400 here where `validate_date` accepts it
- **Invoicing send API:** `src/server/routes/invoices.rs` adds `POST /api/invoices/{number}/send` and `POST /api/invoices/sync`. Send is a **blocking request** — the whole orchestration runs inside it and the response carries the refreshed `InvoiceDetail`, the public URL and the step trace — and it requires `{"confirm": true}` at the wire level, answering 400 `confirmation_required` without it, because a confirm dialog is a convention the next screen can forget. Failures answer where they stopped: `From<SendFailure> for ApiError` in `server/error.rs` is the one `match` on `SendStep`, mapping the three gateway steps to the new `ApiErrorCode::UpstreamFailed` (502, `upstream_failed`) with the service named, a missing PDF feature to 501, a `NigelError::Db` at any step to 500 (an R2 outage and a failed database write must not read alike), and the config/load/precheck refusals to whatever the data layer already answers, gaining only the step. `details` carries `step`, `completed`, `emailSent` and `invoiceStatus`; `send_not_configured` carries `missing` as key names only. The pre-flight order is the step vocabulary's own: the settings are resolved first (`config`), then the invoice is looked up and a void one refused, then the template is loaded — before any client is *called*, so a broken override costs no Stripe link, no upload and no email, which is 68.3's ordering. `send_with`/`sync_with` take the three gateway traits, so the whole orchestration is fake-tested and no test in the module can reach the network. Sync answers `SyncReport` with per-invoice failures as data; only a wholly failed run is a 502
- **Write API:** `src/server/routes/transactions.rs` (`PATCH /api/transactions/:id`, `POST /api/categorize`), `review.rs` (queue, one-by-id, apply, undo), `reconcile.rs` (`POST /api/reconcile`, `GET /api/reconciliations`), and the create/update/delete halves of `accounts.rs`, `categories.rs`, `rules.rs` (plus `POST /api/rules/test`) and `imports.rs` (`DELETE /api/imports/:id`). Handlers validate, then call the same `&Connection` data layer the TUI uses; guardrail failures travel as typed `NigelError` variants and become 409s with a structured `details.reason`. Multi-field edits run in one `unchecked_transaction` so a rejected value leaves nothing half-applied
- **Export API:** `src/server/routes/exports.rs` serves the eight reports as downloads at `/api/exports/{pnl,expenses,tax,cashflow,balance,flagged,register,k1}` with a required `format=pdf|text` plus the same date/account parameters as the matching `/api/reports` route — validated by that module's `ParamSpec::for_kind()` + `ReportParams::parse()`, so the two route families cannot disagree. Handlers fetch through the same `reports::get_*` functions and render via `pdf::render_*` or `reports::text::format_*` + `with_header()`; the `pdf` feature is answered in one place (a `cfg`'d pair of `render_pdf` functions), so `format=pdf` without the feature is a 501 carrying `reports::PDF_DISABLED_MESSAGE` — the same sentence the CLI prints — while `format=text` always works. `Content-Disposition` uses `reports::export_file_stem()`, the CLI's own `<report>-<date>` naming. `GET /api/status` advertises the capability as `pdfExport` because a browser downloads an anchor without inspecting it. `reports::date_range_label()` (moved out of `cli/export.rs`) builds the period label both the CLI and the API print under a PDF's title. Bulk `report all` and `--output` file writing stay CLI-only
- **Settings API:** `src/server/routes/settings.rs` covers what `settings_manager.rs` does plus `nigel load` — `GET/PUT /api/settings/app` (only `updateCheck` is web-editable; the response struct is hand-written because `settings::Settings` has no `rename_all` and would put snake_case on the wire), `GET`/`PUT /api/settings/company` (the whole letterhead as one camelCase object — `name`, `address`, `phone`, `logo`, `paymentInstructions` — every field trimmed, an empty one clearing its key, `document::parse_logo` run **before** any write so a bad logo is a 400 that leaves the other four alone, and the five `set_metadata` calls in one `unchecked_transaction`; one route rather than five because the fields are only ever correct together. `StatusResponse.companyName` is unchanged — the sidebar and the document title want a bare name), `POST /api/settings/data-dir`, and `POST /api/settings/password/{set,change,remove}` wrapping `password.rs`'s `&Path` functions. Every one of them is behind the locked guard — including the two that never open the database, because nothing on the unlock screen reads them and `change`/`remove` would otherwise be an unthrottled password oracle reachable without passing the gate
- **Web UI (SPA):** `web/` is an npm workspace (no turbo) with three packages built in order. `@nigel/theme` composes per-category Lit `css` token modules into one `CSSResult` plus a generated plain stylesheet; tokens shadow Web Awesome's `--wa-*` namespace (no WA stylesheet is loaded — theming rides custom properties) and nigel-specific tokens use `--nc-*`; the `wa-*` treatment is **not** in that document sheet — a `::part()` rule reaches one shadow boundary down from the tree it is written in, and every `wa-*` primitive sits inside a `wc-*` shadow root inside a screen inside `nigel-app`, so it ships as a separate `controlsCss` that the components rendering primitives adopt (`static styles = [controlsCss, css`…`]`, controls first so a component can still override), enforced by a source-scan guard test in each package. Adopting that sheet is necessary but not sufficient: Web Awesome's compiled component CSS reads its *structural* styling — padding, borders, control heights, focus rings — from `--wa-*` properties that ship in the stylesheet this app never loads, and an undefined custom property is not a default but a discarded declaration (no button padding, `border-style` falling to `none`, and an undefined token inside a `calc()` voiding the whole rule). `src/tokens/wa-contract.ts` defines that vocabulary in terms of the package's own tokens, which is the mechanism to prefer over per-part rules: it inherits to every component at once, dark mode and print follow for free because `var()` resolves at use time, and it does not clobber WA's disabled/appearance variants the way an outer-tree `::part()` rule does. `__tests__/wa-contract.test.ts` reads the required token list out of the installed Web Awesome package rather than a written-down list, so an upgrade that adds one fails the build instead of silently unstyling a control — in **two** classes, because a variant's colour is two hops: `variants.styles` points the generic `--wa-color-fill-loud` at the family token `--wa-color-<variant>-fill-loud` and the component reads the generic one, so the *intermediate* names are deliberately never defined at `:root` (that would pin every control to one variant) while the *leaf families* for all five variants are demanded, walked from the component module rather than its styles entry because `variants.styles` hangs off the class. A missing family is not a default but nothing, so the declaration is discarded and the control renders in the neutral fill — which is what made every delete dialog's destructive action a grey button. The three semantic families are mixed from the colour the variant is named after (10% and 16% washes for the quiet and normal fills, the solid colour loud, `--wa-color-on-brand` as the label — the mode's ink for a saturated fill), and `contrast.test.ts` holds every on/fill pairing to AA by resolving the family through to the hex it ends at (`__tests__/token-resolution.ts`) rather than reading the declaration. `--wa-color-neutral-border-loud` is the outlined button's edge and so owes WCAG 1.4.11's 3:1, which the panel border it used to name did not meet; `--nc-color-on-gradient` is the one colour deliberately not mode-dependent, because the pastel ramp it sits on is the same in light and dark; the package also ships **fonts**: IBM Plex Mono weights 400/500/600, subset and committed under `src/fonts/`, declared by `src/tokens/font-faces.ts` with relative URLs and copied to `dist/fonts/` by `build-css.js`, so both Vite roots emit hashed copies and rust-embed bakes them in — nothing is fetched at runtime, and guard tests fail the build on a font host, a preconnect hint or an absolute font URL. `--wa-font-family-sans` and `--wa-font-family-mono` are the same Plex Mono stack; the token keeps its `sans` name because it means "the primary UI face" and is read by Web Awesome's own internals. `color-mode.ts` is the package's one behaviour module — the three-state light/dark/system contract (`COLOR_MODES`, `readMode`/`writeMode`/`applyMode`/`resolveMode`/`initColorMode`), stored in `localStorage` under `nigel.color-mode` and applied as a class on `<html>`, with **`system` writing no class at all** so the CSS media query tracks the OS live with no listener and no reload. It is not in `settings.json`: every `/api/settings/*` route is behind the locked guard, so a server-stored preference could not be honoured on the unlock screen an encrypted database shows first, and the preference is per-browser by nature. `wc-mode-switcher` (a `wa-radio-group`) is fully controlled and lives on the settings screen; `apps/app/index.html` carries a blocking inline copy of the read-and-apply so the stored mode is on `<html>` before first paint, with `color-mode-bootstrap.test.ts` failing the build if that copy drifts from the module's constants; the brand palette is derived from `src/effects.rs` (a parity test fails if they drift) and every solid color is held to WCAG AA in both light and dark by a contrast test. Light mode is deliberately not paper-white with near-black text: the canvas carries a lavender-grey tint, the text is a soft charcoal (about 11.6:1 on a card rather than 14:1), and three planes are meant to be distinguishable — `--nc-color-sidebar-bg` lowest, `--wa-color-bg` above it, cards the only true white. Two token pairs exist because one value cannot serve two contrast thresholds: `--nc-color-income-fill`/`--nc-color-expense-fill` are the chart bars, which are large graphics owing WCAG 3:1 and so sit much lighter than the same-hue figures, which owe 4.5:1 and keep `--nc-color-income`/`--nc-color-expense`; and `NIGEL_PALETTE_INK` is the wordmark ramp for light surfaces, the same seven hues with lightness traded for saturation, because gradient-filled text makes every stop a foreground colour and the pastels are illegible on a light plane. The ink ramp is additive precisely so `NIGEL_PALETTE` stays pinned to `effects.rs`; dark mode keeps the pastels. Both new pairs are reset in the print block alongside the tokens they shadow. The hover and focus-visible treatment is two things, and neither is a halo. Every button fades in a **`--wa-border-width-m` (2px) edge in its own colour**, drawn in two halves so the box never changes size: the `--wa-border-width-s` border WA already reserved space for, plus the remainder as an `inset` box-shadow, which is clipped to the padding edge and so lands flush inside the border and reads as one solid edge. Widening the border itself would move every neighbour a pixel on hover; the remainder is `calc(--wa-border-width-m - --wa-form-control-border-width)` rather than a literal `1px`, so a change to either token stays an `-m` edge, and `controls.test.ts` fails if the rule ever sets `border-width` itself. `controlsCss` names the colour per variant in one line each (`--nc-hover-border`, the semantic three reading `--wa-color-danger` and friends so one declaration serves both modes, neutral reading `--wa-color-text` because an outlined button already sits on its own border tokens and hovering to them is no change at all) and applies it in a **single** rule, so every button is excluded on the same terms: a `plain` button is a row action drawn as bare text, a disabled one is refusing, and a `loading` one drops the click in `handleClick`. The edge takes `--nc-duration-slow` (500ms), which is why that rule **restates `wa-button`'s whole base-part transition**: WA transitions six properties there over `--wa-transition-fast`, and a `transition` declared in the outer tree replaces that list rather than adding to it, so every one of them is named again with `border-color` *and* `box-shadow` — the edge's two halves, which have to fade together — on the slower duration — `controls.test.ts` pins which properties this is now answerable for. The brand button additionally **drifts its own gradient** while hovered or focused: `--nc-grad-brand` is a `repeating-linear-gradient` and `--nc-grad-brand-size` (216.6667%) is the size that makes one `background-position: 100%` shift exactly one period, so `nc-brand-cycle` (2.4s linear, infinite) scrolls the same seven colours with no seam and no jump wherever the pointer leaves — nothing is hue-rotated or recoloured. The arithmetic is counted in the ramp's own step: seven stops across the element is six gaps, one period is the ramp plus a seventh step wrapping magenta back to pink, and the image is thirteen of those steps, which puts the ramp exactly across the element at rest so a button that never moves looks as it always did (`nigel-theme.test.ts` asserts the derivation rather than the numbers). Anything else reading `--nc-grad-brand` must set that size or get the ramp at six-thirteenths scale, tiled — which is why the dark-mode wordmark names the plain `brandRamp` export instead. The `@keyframes` are defined once in `tokens/gradient.ts` and composed into **both** `nigelTheme` and `controlsCss`, because keyframe names are tree-scoped and the animated element is inside `wa-button`'s shadow root — neither the tree the rule is written in nor the document. `prefers-reduced-motion` stops the drift in `controlsCss` and zeroes `--nc-duration-slow` in `motion.ts`; the edge still draws, instantly, so hover and focus keep an indication when the motion goes. `:focus-visible` is stated on the host alone, which suffices because `wa-button` sets `delegatesFocus`. `@nigel/ui` holds the `wc-*` Lit components, `WcIconBase` + icons, and the preview harness (glob manifest + query-string router on port 9090); each component has a co-located `.preview.ts` and a test that runs axe over every state the preview declares. `@nigel/app` is composition only: `main.ts` (3-line bootstrap), `nigel-app` root container, the screen registry, the app store, and the api client. Cherry-picked Web Awesome imports only, never the autoloader. Build output goes to `web/dist`, which `rust-embed` bakes into the binary
- **SPA routing and api seam:** `web/apps/app/src/screens/registry.ts` is a `Record<ScreenId, ScreenDef>` describing every screen once — title, nav label, icon, `inNav`, `render()` — and the sidebar, header, and content area all derive from it, so a missing screen is a compile error. `location.hash` (`#/<screen>?<params>`) is the only writer of route state; `hash-route.ts` parses and serialises it. `web/apps/app/src/api/` is the only module that talks to the server: `types.ts` mirrors the serde structs by hand (camelCase), `client.ts` defines `ApiClient` (one typed method per endpoint, no generic `request()`) and `FetchApiClient`, and owns the `appLocked` (423) and `appUnauthorized` (401) transport signals. `request()` branches on a `FormData` body and omits the JSON content type for it, because only the browser can generate the multipart boundary — that is the whole of `uploadImport`'s special handling, and it takes no progress callback, since `fetch` cannot report upload progress and promising it would bind every future implementation to producing it. A guard test fails the build on any `fetch(`/XHR/EventSource/WebSocket/`sendBeacon` outside `src/api`
- **SPA dashboard:** `web/apps/app/src/screens/dashboard.ts` (`nigel-dashboard-screen`) mirrors the TUI home screen — year-to-date P&L, account balances, twelve months of cash flow, the flagged count, and the update notice. `state/dashboard-store.ts` fetches the four reports in parallel and holds each in its own `{ data, loading, error }` slot, so a failure lands on one card with an inline retry rather than blanking the screen; `screens/dashboard-data.ts` maps `CashflowReport` to chart buckets, taking the last twelve months **that have data** with no calendar zero-fill, exactly as `cli/dashboard.rs` does, so the two front ends chart the same books
- **SPA register:** `web/apps/app/src/screens/register.ts` (`nigel-register-screen`) is the web `browser.rs` — account filter, period nav, incremental search, row selection, inline category/vendor editing and flag toggling. Search is client-side and stays that way (`/api/reports/register` has no search parameter): `screens/register-data.ts` holds `rowMatches` (the TUI's `recompute_search_matches` — case-insensitive substring over description, vendor and category name, missing fields treated as empty), `indexOfToday` (`scroll_to_today` parity, last row dated on or before today, local date not UTC), `registerParamsFrom` (route to request; `from`/`to` only as a pair and winning over `year`/`month`) and `buildPatch` (only changed fields, since an empty PATCH is a 400 and `categoryId: null` is refused). Account and period changes navigate rather than mutate state, so filters are links; `?q=` seeds the search but is not written back per keystroke. Edits are optimistic, replaced by the row the server answers with, rolled back with a toast on failure; flags are sent as a state, never a toggle. `@nigel/ui` gains `wc-register-table` (roving-tabindex `role="grid"`, arrows/Home/End/PgUp/PgDn/Enter/Esc/`f`/`/`, hand-built category combobox because this Web Awesome build has no searchable select, and no `wa-*` in a row outside edit mode), `wc-register-toolbar`, and `wc-period-nav` — the granularity-driven pager the report screens share, with `allowAll` for the register's unfiltered default The shortcuts hang off **three** things and were dead without any one of them: the `keydown` listener is on the host, not on the scroller, so a key fires from the focused row, from the flag button on it, or from the scroller itself; `tabStopId` falls back to the first row, so the roving tabindex has a home before anything is selected and a date-filtered register is reachable by Tab at all; and a key arriving with `Ctrl`, `Meta` or `Alt` held is passed through untouched, so `Ctrl+F` opens find-in-page rather than flagging a transaction and `Ctrl+Home` is the browser's. The split at a control inside a cell is the ARIA grid pattern's: an **activation** key (`Enter`, `Space`) whose composed path starts on an interactive element is that control's, so the flag button answers both identically, while the **navigation** keys stay the grid's and move between rows from wherever focus is. `Esc` outside edit mode clears the selection and consumes the key — TUI parity — and the tab-stop fallback is what makes that safe, since the table stays reachable with nothing selected. A row holding the stop only as a **fallback** does not select itself on `focusin`, so Tab returning to the table never moves the cursor — a click still does, and a row focused while the selection is elsewhere still does. `focusSelectedRow()` focuses the tab stop without selecting anything, and the screen calls it after a load unless the user is already in one of its own controls, so arriving from a sidebar click lands the keyboard on the table while a reload never pulls the caret out of the search box. `REGISTER_SHORTCUTS` is the legend, exported from `wc-register-table.ts` beside the switch that implements it and carrying the real `KeyboardEvent.key` values, so the component's tests walk the legend and refuse a line that does nothing. `wc-shortcut-help` renders it: a `<button>` (plain, because `aria-expanded`/`aria-controls` have to land on the real button) over an absolutely-positioned panel, so opening the legend moves nothing on the page — a disclosure rather than a dialog, since the content is a definition list with nothing focusable in it. It closes on focus leaving it, on an outside `pointerdown`, and on `Escape`; the key is consumed and focus returned to the trigger only when focus was inside the popover or on the trigger, so an `Escape` meant for something else closes the panel without stealing either. The panel is clamped to the viewport rather than only right-anchored, so a trigger sitting left on a wrapped toolbar does not push it off the edge. The screen is full height: `wc-app-shell`'s content area is a flex column, `nigel-register-screen` grows into it, and the table's `fill` attribute hands it everything left below the toolbar, so the Net row sits at the bottom of the window and nothing renders under it. Under `fill` the **host** is what grows and the scroller only shrinks into it, so a three-match search draws a three-row box rather than a full-height bordered one — a sticky footer is pulled up by its scroller and never pushed down. `--nc-register-min-height` (12rem) is where shrinking stops; below it the page scrolls instead, which is what a viewport shortened by a docked devtools panel needs, and it sits on the host because the scroller has a border and must stay free to hug short content. `--nc-register-height` (60vh) is the cap on the **content-sized** mode only — the reports screen's read-only view inside a page that scrolls as a whole — and is deliberately retired under `fill`, where a cap and a parent-driven height cannot both decide. Paging measures the scroller less the sticky header and Net row, which are painted over it, so PgDn never skips the rows they cover; one visible row pages by one, and the TUI's 20 is the fallback only when nothing could be measured at all. Above `virtualizeAbove` rows (120) the table **windows**: only the visible slice plus an eight-row overscan is in the DOM, with a spacer `<tr>` above and below carrying the height of what was left out, so the scrollbar still measures the whole register and `aria-rowcount`/`aria-rowindex` still say where a row is. Measured on 1,872 rows: 20,989 DOM nodes to 436, first render 2,414 ms to 95 ms, a search keystroke 14,700 ms to 36 ms, against a new ~19 ms per scroll event. What pays for it is a uniform row height — `table-layout: fixed`, a `colgroup` where description is the only auto column (the TUI's `Fill(1)`), and text cells that clip with an ellipsis instead of wrapping, keeping the full string in the DOM and on the cell's `title`. Every row being one line tall is what lets the window place row *n* at `n * rowHeight` by arithmetic, which is the only way to reach a row that is not rendered: `coverIndex` puts an index into the DOM and `scrollToRowIndex` moves the scroll box, and keyboard navigation, inline editing, the search jump and scroll-to-today all go through the pair. The window goes back to the top only when the row the window is anchored on is no longer among the rows — a search, an account or a period change — and never for a same-set mutation such as an optimistic edit dropping one row, which would cost the user their place. The rows in the DOM are **keyed**, so the row being edited keeps its inputs when the window scrolls past it and is drawn in its own place between spacers rather than sliced out mid-edit. `aria-rowcount` counts the header, every transaction and the Net row, which carries its own `aria-rowindex`, `wc-register-toolbar`, and `wc-period-nav` — the granularity-driven pager the report screens share, with `allowAll` for the register's unfiltered default. The screen is full height: `wc-app-shell`'s content area is a flex column, `nigel-register-screen` grows into it, and the table's `fill` attribute hands the scroller everything left below the toolbar, so the Net row sits at the bottom of the window and nothing renders under it. Without `fill` the table stays content-sized under `--nc-register-height` (60vh), which is what the reports screen's read-only view wants inside a page that scrolls as a whole
- **SPA Snake:** the dashboard hides the TUI's easter egg behind the same key. `s` at the window — never from a screen, because the game covers the whole app and outlives what was underneath — opens `wc-snake` as a fullscreen overlay over an `inert` shell. `src/snake-trigger.ts` is every guard on that key and is pure so the refusals are testable against a hand-built event: a bare unmodified `s`, not a repeat, not mid-composition, not already handled, and not while a form control or a dialog is in the **composed path**, the **focus chain**, or — `hasOpenModal`, a shadow-root walk because every dialog in this app is inside one — open anywhere on the page. Three sources because no one of them covers the others: the path is where the event came from, the chain is what has the caret when the event was retargeted, and a modal with nothing focusable inside it leaves *both* pointing at the body. `deepActiveElement` follows shadow roots down, since `document.activeElement` answers `nigel-app` for a caret in the register's inline editor. `snakeAllowedOnBoot` is an exhaustive switch on `BootPhase` — only `ready` renders a dashboard to cover, and a phase added later fails the typecheck rather than defaulting the game open. **Every exit goes through one `closeSnake`** — Escape's `nc-snake-exit`, a route change, and the boot phase leaving `ready` — because the open flag and the captured focus have to fall together: a render branch that merely stopped rendering the overlay (which is what locking does) would strand both, and unlocking would put a fresh game back with a focus capture pointing at an element that no longer exists. Focus returns to the captured element, or to the shell when the screen it was on has gone. `@nigel/ui` gains `wc-snake` and `snake-engine.ts`, a pure port of `cli/snake.rs` pinned to it by `snake-parity.test.ts` (board, tick curve, food range, the full-board win) with the Rust suite's own cases ported case for case; the score renders through `wc-money`, not a fourth hand-rolled `Intl` call. The game holds the keyboard, not the browser: any event carrying ctrl/meta/alt passes through untouched, so Cmd+R reloads rather than restarting the snake. The loop stops on `document.hidden` and picks up on return — a background tab throttles timers to about once a minute, which is a snake taking blind steps into a wall the player cannot steer away from. `@nigel/theme` gains `gradientColor`, the `effects::gradient_color` interpolation, and the three mode-independent `--nc-color-arcade-*` tokens — the board keeps a dark ground in light mode because the pastels the snake is drawn in are invisible on a light one. Reduced motion stops the drifting specks and the gradient cycling along the snake, but never the snake: a Snake that does not move is not the game. The specks are always rendered and the drift is stopped in CSS alone — the reflected `reduced-motion` attribute and the media query behind it — rather than also by withholding the inline timing, which would be a second mechanism saying the same thing
- **SPA review:** `web/apps/app/src/screens/review.ts` (`nigel-review-screen`) is the web `reviewer.rs` — one flagged transaction at a time from `#/review`, or a single re-review from `#/review?id=185` (`nigel review --id`; the router has no path segments). Every applied decision is pushed onto a client-side stack of `{ transactionId, ruleId }` and Back pops one and calls `POST /api/review/:id/undo` with that rule id, which re-flags the transaction and deletes the rule outright; a skip pushes `null`, the same `Option<ReviewDecision>` the TUI stacks, so stepping back over a skip issues no request. A failed undo pushes its decision back, so the stack never claims a decision the server still holds, and the summary counts are derived from the stack (`screens/review-data.ts`: `summarize`, `toReviewItem`, `singleIdFrom`) rather than tallied, so a Back corrects them for free. A 404 on apply is read from its `details.reason`, because the route answers 404 for two opposite things: `transaction_not_found` advances with a toast and records a skip rather than wedging the queue, while `category_not_found` — and any 404 naming no reason — keeps the transaction on screen with the failure beside the form, since that decision is still waiting to be made. Two deliberate departures from the TUI: **Tab does not skip** (on the web it is the focus key — Skip is a button, and only Enter/apply and Esc/back are bound), and there is **no match-type choice**, because `apply_review` writes `contains` and the apply route has no field for anything else. The rule pattern prefills with the first two words of the description (TUI parity) and drives a debounced `POST /api/rules/test`. `@nigel/ui` gains `wc-review-card`, `wc-review-progress` (a labelled bar, not dots — a post-import queue is routinely 50-200 long), `wc-review-form`, `wc-rule-test-preview`, and `wc-category-picker` — the searchable combobox lifted out so the review form and the register's inline editor share one implementation and one `categoryLabel`
- **SPA import:** `web/apps/app/src/screens/import.ts` (`nigel-import-screen`) is the web `import_manager.rs` — one screen whose panels appear as the decision is made: choose a file, preview it, confirm. The upload is **lazy**: picking a file sends nothing, and Preview uploads and dry-runs in one action, so a file chosen and thought better of never reaches the server's spool and the account is known before any bytes move. The `uploadId` is cached against the chosen file, so correcting a column mapping and previewing again re-reads the copy the server already holds; an expired upload (404 `details.reason = upload_not_found`, an hour after the fact) is re-uploaded once, silently, because the file never left the browser. A **duplicate file blocks the confirm** rather than warning about it — the server would answer 200 with zero counts, so the button would offer a no-op. **Cancel** abandons the import at any stage before the confirm: file, preview, errors and form together, the account included — which is what separates it from the reset a finished import offers, that one keeping the account because a second statement for the same one is the ordinary next thing. It is offered only when there is something to abandon (an untouched screen is already in the state it would return to, the one-account preselect included), it renders beside whichever action would carry the import forward — Preview, then the confirm, and on the duplicate-file panel, which has no forward action at all — and it is disabled mid-request, because the api client sends nothing it can call back and an abandoned upload the browser can no longer name is worse than a two-second wait. It tells the server **nothing**, and there is no endpoint for it to tell: a spooled upload is a file on disk with an mtime that the hourly sweep collects, which has to hold anyway for the closed tabs, dead networks and quit browsers that abandon an upload without clicking anything. `screens/import-data.ts` holds the pure half: `importRequestBody`/`confirmRequestBody` derive `format` and `mapping` from one form field so sending both (a 400) is unrepresentable and `saveProfile` can never travel without the mapping it names, `initialImportForm`/`sameImportForm` (the one definition of the untouched screen, so preselecting the only account cannot read as an import somebody started), `previewCounts`/`resultCounts`, `formatLabel`, and `routeImportError`, which files each failure where its cause is — a 413 under the dropzone, a 501 and other 400s under the format select, a mapping 400 under the mapping form, an unknown account under the account select plus a toast, and 423/401 nowhere, because the shell gates those before a screen exists. `@nigel/ui` gains `wc-dropzone` (drag-and-drop plus picker; the well is a `<button>` so keyboard and mouse share one path, and it checks extension and size client-side because the server can only answer 413 after 25 MB have crossed the wire), `wc-import-form` (account, format and the generic column mapping in one component — the format list is flat because Web Awesome's select has no option-group element), `wc-sample-table`, and `wc-count-grid` (labelled integers, deliberately not the money-formatting `wc-stat-card`)
- **SPA reports:** `web/apps/app/src/screens/reports.ts` (`nigel-reports-screen`) serves all nine reports and the directory that lists them — `#/reports` for the landing, `#/reports?report=pnl&year=2025` for one (a query parameter, since the router has no path segments). `screens/reports-data.ts` holds the catalog (one `ReportDef` per slug: title, icon, and the date parameters that route accepts) plus the pure table mappers, shaped deliberately like `reports::text` so both front ends print the same rows in the same order. Eight of the nine are `/api/reports` slugs that also export; A/R aging is the ninth and carries no export links, so `ReportViewSlug` is the view vocabulary and `reportDef(slug)` the lookup — not `REPORTS[slug]`, which has no `aging` key. `reportDef(slug).supports` gates what reaches the request — the server answers 400 for a parameter its route does not take, so `year` on `/api/reports/balance` is an error, not a no-op. `wc-period-nav` takes its granularity from the `granularity` field on the response envelope rather than a client-side table, runs with `allowAll` off (an unfiltered view belongs to the register browser), and defaults to the current year, which is what `TableReportView::new` seeds the TUI's own date navigation with. The K-1 worksheet is **composed** from `wc-panel`/`wc-report-table`/`wc-notice-bar` rather than given a component of its own — a `wc-k1-worksheet` would have to take `K1PrepReport` as a property, dragging API types into `@nigel/ui` — and renders in `format_k1`'s order including the auto-mapped note and the needs-mapping section. The register view reuses `wc-register-table` in `readonly` mode and links to `#/register` for editing. `@nigel/ui` gains `wc-report-table` (one declarative table for every report section: columns and rows as data, section/subtotal/total emphasis, optional row links), `wc-export-links` and `wc-link-grid`
- **SPA exports and printing:** export buttons are plain `<a download>` links whose href comes from `ApiClient.exportUrl(slug, format, params)` — a download link is as much a hardcoded address as a `fetch`, so the api seam owns it, and the guard test now fails on any quoted `/api/` literal outside `src/api` (comment lines excepted, since documenting the seam is not routing around it). PDF is offered only when `/api/status` reports `pdfExport`: a link cannot inspect its response, so on a build without the pdf feature the button would otherwise save a 501 envelope as `pnl.pdf`. `@nigel/theme`'s `print.ts` composes last into `nigelTheme` and carries the half of printing a document sheet can do: colours repainted by redefining tokens at `:root` (custom properties are what inherit through shadow boundaries) and 1.5cm margins. Hiding the chrome is the other half and lives with the elements, since a rule that hides an element has to be in that element's tree — `wc-app-shell` hides its own header, banner and sidebar slot and unclamps its `100vh`/`overflow: hidden` box so a long report is not cropped to one screenful, `wc-nav-sidebar`/`wc-toast`/`wc-export-links`/`wc-period-nav`/`wc-register-toolbar` each hide themselves, and `controlsCss` carries `wa-button`/`wa-select` hiding and the repeating-table-heading rules into every root that hosts a control
- **SPA managers:** `web/apps/app/src/screens/{accounts,categories,rules}.ts` are one screen three times over — list, Add, per-row Edit and Delete — sharing `wc-manager-layout`, `wc-manager-table` and `wc-manager-dialog` and differing only in columns and form. Together they are a superset of the TUI, which can only list and delete rules (`rules_manager.rs`). Editing is **in a dialog, not an inline panel**: the rule form is tall enough (pattern, match type, category, vendor, priority, live test preview) that inline it would push the list off screen, delete is already a dialog, and `wa-dialog` brings the focus trap with it. Guardrails are rendered from `details.reason`, never from the server's sentence — `screens/manager-errors.ts` is the whole table (`has_transactions`, `has_active_rules`, `duplicate_name`, `already_inactive`), with the count formatted client-side, which is what makes the strings translatable; a `400` and an unrecognized 409 reason are the two deliberate exceptions and render the server's message, because `Invalid regex: unclosed group` names the offending value and anything re-derived would drift. A failed **save** renders in the dialog beside the field; a failed **delete** renders in the layout's alert region, because `confirmDialog()` resolves and removes itself before the request is sent. Every mutation refetches its list (no optimistic splicing: a priority edit reorders the rules, a rename reorders the categories, and a category rename changes the name on every rule row). Accounts has no transaction-count column (`GET /api/accounts` does not carry one and a screen may not add an endpoint — the number appears in the blocked delete) and enforces the 4-digit last-four rule that lives only in `account_manager.rs`. Categories edits send only changed fields (an all-omitted PATCH is a 400), and document the K-1 form-line vocabulary beside the field with a runtime-derived datalist plus a non-blocking warning for a value `resolve_k1_mapping` matches literally. Rules keeps the server's priority order (it is the semantics), debounces `POST /api/rules/test` at 250 ms into the review screen's own `wc-rule-test-preview` (immediately on a match-type change — a click is a decision, not typing), does **no** client-side regex validation (JS `RegExp` and the Rust `regex` crate accept different languages), and filters client-side on `#/rules?categoryId=12`, which is where the categories guardrail points. `@nigel/ui` gains `wc-manager-layout`, `wc-manager-table`, `wc-manager-dialog`, `wc-account-form`, `wc-category-form`, `wc-rule-form`, and the `wc-icon-plus`/`edit`/`trash` glyphs
- **SPA reconcile and undo:** `web/apps/app/src/screens/reconcile.ts` (`nigel-reconcile-screen`) is the web `reconcile_manager.rs` — account, month and statement balance, the reconciled-or-discrepancy verdict, and the history below it. The history is refetched after **every** submit, not only a matching one, because `POST /api/reconcile` records the attempt whichever way it came out and that record is how the history knows which months have been checked. The verdict is held with the request that produced it, so editing the month afterwards cannot relabel February's figures as March's. `screens/reconcile-data.ts` files a failure under the control that caused it: a 409 `no_transactions` on the month and a 404 on the account, both in our own words — the 404 deliberately **not** the server's sentence, which tells you to run `nigel accounts list`, good advice in a terminal and useless beside an account picker. The typed figures survive a failed submit, since this is the one screen where the number was copied off a paper statement. `screens/undo.ts` (`nigel-undo-screen`) supersets `undo_manager.rs`, which can only offer the most recent import because a terminal has nothing to point at: `DELETE /api/imports/:id` has always taken an id, so the web lists every import and undoes the chosen one. Confirming names the count and the file; a 404 (another tab got there first) is reported rather than passed off as success, and either outcome refetches rather than splicing, because every other row's count is the server's to state. AC-level freshness needs no invalidation wiring — `src/__tests__/screen-freshness.test.ts` drives the whole app, undoes an import and navigates to the register and the dashboard, asserting the refetch lands after the delete; the property holds because there is no global cache and each screen is a distinct element Lit tears down on a route change. `@nigel/ui` gains `wc-reconcile-form` (which carries the app's only currency input: a rendered `$` prefix, `inputmode="decimal"`, commas stripped exactly as `reconcile_manager.rs` strips them, a tidy on blur through `Intl`, and `wa-input type="month"` — confirmed to survive jsdom, with the `YYYY-MM` check kept anyway because Safari degrades the control to text), `wc-reconcile-result` (the difference gets its own emphasised row rather than leaning on the red alone, the same reasoning that makes `wc-money` always print its sign, and it shows the statement figure beside the calculated one where the TUI prints only the calculated), `wc-import-history` and `wc-reconciliation-history` (a null balance renders as an em dash, never an invented `$0.00`)
- **SPA invoicing:** `web/apps/app/src/screens/clients.ts` (`nigel-clients-screen`) is the fourth instance of the manager pattern — `wc-manager-layout`/`wc-manager-table`/`wc-manager-dialog` over `wc-client-form`, plus delete and archive. The archive filter is a route (`#/clients?archived=1`), because a filtered list is a URL, and the row's verb is Archive or Unarchive depending on its own state — which is what `ManagerRow.actions` exists for, since one array for the whole table cannot say it. An archived row carries a `wc-row-badge` in its name cell rather than a column that would be empty on nearly every row. `wc-client-form` has **no Email field**: it carries the whole contact list as a repeater with a radio choosing the billing recipient and up/down buttons to reorder (a drag handle has no keyboard equivalent that passes axe without building them anyway — `wc-line-items`' reasoning), because the route accepts `email` *or* `contacts` and a form with both controls would have to decide which wins. Opening Edit therefore fetches `GET /api/clients/{id}` first, with a loading state in the dialog and a load failure rendered beside the form rather than as the whole screen: the list row is a bare `Client`, and putting contacts on every row would cost the one screen that must stay one query and one cheap payload. It has **no invoice-count or outstanding column**: `GET /api/clients` answers bare `Client` rows, and a screen may neither invent an endpoint nor fan out one request per row, which is `accounts.ts`'s reasoning exactly — the count appears in the blocked delete, whose "Show those invoices" action is the `guardrailAction` precedent pointing at `#/invoices?clientId=N`. `screens/invoices.ts` (`nigel-invoices-screen`) is one screen with four views keyed off `ctx.params` (`number`, `edit`, `new`, none), the reports screen's arrangement since the router has no path segments; status filters are links, so a filtered list is a URL. Three deliberate departures from the rest of the app: the **invoice editor is a full view, not a dialog** (an invoice with eight line items inside `wa-dialog` is a scrolling box inside a scrolling page, where the client form fits the dialog even with a contacts repeater in it); the **send dialog survives its own request** and resolves on Close rather than on Send, because the step trace only means anything beside the thing it describes and `confirmDialog()` removes itself before a request is sent; and the **`can*` flags come from the server**, never re-derived from `status`, because an edit is blocked by recorded payments as well as by status, and a delete by either. Delete is the one detail action with nothing to refresh afterwards: it navigates back to the list and says so in a toast, because a route change drops the notice state a success message would have lived in. A failed *load* and a refused *action* are separate states: a load failure is the whole view, because there is nothing else to show, while a 409 on void or pay is a normal answer and lands in a dismissable notice above the invoice it is about — rendering it through the empty state would blank the very invoice being explained. A void's `teardownWarnings` land in warning notices *beside* that danger notice rather than in it: the void succeeded, and what is still live is somebody's next task, not this request's error. The send dialog additionally `preventDefault()`s `wa-hide` while a send is in flight: `wa-hide` is a request that `requestClose` honours unless it is prevented, so merely ignoring it would let Escape or the backdrop hide the dialog while `open` stayed true, and the `?open` binding would never reopen it. A/R aging is **not** on this screen — it is the ninth entry in `reports-data.ts`'s catalog (`AGING_DEF`, granularity none, no export links because `/api/exports` carries the same eight slugs `/api/reports` does), reached from the invoice list's `wc-aging-bars` strip. `screens/invoice-data.ts` holds the pure half — `invoiceListParams` (omits `all`, which is not one of the server's status words), `newInvoiceRequest`/`invoicePatch` (only changed fields, `dueDate: null` to clear, `items` whole-list or absent), `payRequest`, `clientPatch`, `sendStepViews` (every step, so a trace that stopped early shows what did not happen) and `invoiceTableRows` (a void invoice's balance becomes null, which the table renders as an em dash rather than an invented `$0.00`). `screens/invoicing-errors.ts` is `manager-errors.ts` for this domain: a reason→sentence table with the figures formatted client-side, the same two deliberate fallbacks (a 400 and an unrecognized 409 reason render the server's message), and `sendFailureMessage`, which gives our words for *what* failed and the upstream's own verbatim for *why* — `retryable` is false only for a refusal that would refuse identically and for a failure after the email went out. `@nigel/ui` gains `wc-invoice-status`, `wc-invoice-table`, `wc-invoice-summary`, `wc-payment-list`, `wc-aging-bars`, `wc-document-frame`, `wc-invoice-preview`, `wc-line-items`, `wc-invoice-form`, `wc-payment-form`, `wc-client-form`, `wc-send-dialog` and the `wc-icon-invoice`/`wc-icon-clients` glyphs. `wc-document-frame` owns every iframe in the app and the one `PREVIEW_SANDBOX` constant, taking either a `srcdoc` or a `src`; `wc-invoice-preview` delegates to it. The send dialog frames the page as **`srcdoc`**, fetched through `ApiClient.invoicePreviewHtml`, because an iframe cannot report a failure and a broken custom template has to arrive as the server's sentence rather than as an error envelope drawn in a box — the sandbox attribute, not the route's header, is what contains a `srcdoc` anyway. `wc-invoice-preview` is **collapsed by default** (it is a second render of the whole document per detail view, and expanded it pushes the actions off the first screenful) and its `sandbox` deliberately omits `allow-same-origin`: the document is served from the SPA's own origin, so granting it would hand a page rendered from invoice data the app's cookies and storage. `wc-line-items` reorders with up/down buttons rather than drag and drop, since a drag handle has no keyboard equivalent that passes axe without building the buttons anyway. `wc-invoice-form` takes both dates through native pickers (`wa-input type="date"`, `wc-reconcile-form`'s `type="month"` precedent — jsdom implements it, and the `YYYY-MM-DD` check stays because Safari degrades the control to text), and its **due date is chosen as terms rather than as a calendar**: `DUE_TERM_VALUES` is None / Net 7 / Net 14 / Net 30 / Custom, `InvoiceFormValue.dueTerm` carries the choice, and the picker appears only for Custom. A new invoice opens on `DEFAULT_DUE_TERM` (Net 14) through `newInvoiceForm(issueDate)`, which is `withIssueDate` over the empty form so the seeded term arrives with the due date it implies — a form showing Net 14 over an empty due date would raise an invoice that never goes overdue. It writes no terms text: the form labels a period only for a choice made in the session that makes it, and a default nobody has touched is not one, which is what keeps the provenance rule below a rule about this form's own writes rather than a guess at a value's origin. The behaviour is pure — `netDueDateFor`/`addDays` (UTC, so a daylight-saving boundary cannot cost a day; the year is set back after construction because `Date.UTC` maps 0-99 to 1900-1999 and a mistyped `0026` must not book a date nineteen centuries out; a computed date is always zero-padded), `withIssueDate`, `withDueTerm`, `prefilledTerms` and `dueTermFor` — so the form's handlers are three lines each and the arithmetic is tested without a DOM. `netDueDateFor` answers a `DueDateOutcome` rather than a string, because "no date" has three causes the field's hint has to tell apart — no issue date yet, an issue date that is not a day, and a period landing past year 9999 — and only the last is a choice the form refuses to keep. **Nothing is inferred on load**: `dueTermFor` opens a stored due date as Custom and an absent one as None, since a date thirty days out may be a Net 30 or a coincidence, and reading it as a preset would move a stored due date the moment the issue date was edited
- **Report figure parity:** `web/apps/app/src/screens/reports-parity.test.ts` compares every money figure the browser renders against every money figure in the CLI's own text export, per report, on absolute values (`wc-money` always renders the sign; the text report prints magnitudes and lets colour carry direction). Both sides are captured from one seeded database by `src/server/fixture_capture.rs` — an `#[ignore]`d test, run with `cargo test --features serve capture_web_report_fixtures -- --ignored`, writing `.json`/`.txt`/`manifest.json` into `web/apps/app/src/__fixtures__/reports/` plus a `needs-mapping-k1` pair from a second database that carries an unmapped category. It is a test rather than a script because a script driving `nigel serve` would have to run `nigel init --data-dir`, rewriting the developer's real settings.json
- **Invoicing figure parity (the test side):** `web/apps/app/src/screens/invoicing-parity.test.ts` drives the invoices, clients and reports screens with a `FakeApiClient` primed only from the committed JSON and compares every money figure they render against the money in the CLI's own text. It is not a plain equality like `reports-parity.test.ts`: the browser's list carries a Balance column `format_invoice_list` does not, and the detail's line-item table carries its own Subtotal and Total where `format_invoice_show` prints the total once in its header. Those extras are **derived from the same response the browser rendered**, never written down, so the comparison stays exact — a dropped balance or a doubled subtotal still fails. The `wc-aging-bars` strip is skipped in the walk for the reason `reports-parity` skips `wc-bar-chart`: it is a second view of the aging report, not a second set of the list's numbers. The clients view compares zero against zero, which is the claim itself — the screen may not grow a figure the endpoint does not carry
- **Invoicing figure parity:** the same file captures four invoicing view pairs into `web/apps/app/src/__fixtures__/invoicing/` (`invoices`, `invoice-1250`, `aging`, `clients`) with `capture_web_invoicing_fixtures`. The JSON side is a real router response with a real session; the text side calls `cli::invoice::format_invoice_list`/`format_invoice_show`, `reports::text::format_aging` and `cli::client::format_client_list` directly, because there is no invoice export route to fetch it from. The capture runs under a `TempConfigDir` so a developer's configured `public_base_url` cannot write a live address into a committed fixture, and aging is captured as of `testutil::AS_OF` (`2026-03-15`) for the reason the report fixtures fix their year. A non-ignored guard test fails when a fixture is missing, unparseable, or out of step with the manifest. `testutil::seed_invoicing` — three clients, one of them without an email, and invoices 1247-1252 covering all six statuses — lives in the shared `seeded_db()` seed rather than beside the invoicing tests, because `DATA_ROUTES` names `/api/clients/1` and `/api/invoices/1248` by hand and a detail route with nothing behind it would 404 in the very test that proves the locked guard lets it through
- **Modules:** `categorizer.rs` (rules engine), `reviewer.rs` (review + recategorize data layer; `set_transaction_flag` sets an explicit state and `toggle_transaction_flag` is expressed in terms of it), `reports/mod.rs` (P&L, expenses, tax, cashflow, balance, flagged, register, K-1 prep), `browser.rs` (interactive register browser via ratatui with row selection, inline category/vendor editing, flag toggling, scroll navigation, text wrapping, and incremental text search), `reconciler.rs` (monthly reconciliation), `pdf.rs` (PDF rendering via printpdf, feature-gated; the shared table machinery — `table_header`, `table_row`, `table_row_wrapped`, `hline`/`vline`, `COL_PAD` — is what the nine report renderers draw through, and the invoice deliberately draws through **its own**: `item_table_header`, `item_row`, `wrap_item_cells`/`draw_item_cells`, `ITEM_COL_PAD`, `CELL_PAD_Y`, and a `rule_color` on the writer that defaults to black and only the invoice sets. Reaching for a shared constant to adjust the invoice re-lays-out nine other documents — `wrap_text` measures against `col.width - COL_PAD`, so a wider gutter also narrows every report column's wrap width — which `mod shared_machinery_tests` pins against; `extract_text` is its `cfg(test)` assertion seam — printpdf re-exports `lopdf`, whose `extract_text` is available under the `pom_parser` feature printpdf already enables, so a test asserts what a document says rather than that it starts with `%PDF`, with no dev-dependency; `image_xobjects` is the same seam for what a document *carries*, which is how the embedded logo and its `/SMask`-free colour type are asserted without reaching into `lopdf` outside this module)
- **Migrations:** `migrations.rs` — sequential schema migration runner; `MIGRATIONS` array of `(version, description, up_fn)`; runs inside `init_db()` after table creation, which `main.rs` invokes in its dispatch pre-flight for every subcommand except `init`, `demo`, `load`, `update`, `password`, `completions`, and `restore`, and which the dashboard invokes in its own pre-flight, so every normal use of the app brings the schema up to date; each migration executes in a savepoint transaction; version tracked in `metadata` table under `schema_version` key; v1 is the no-op baseline for existing 0.1.x databases; v2 adds `csv_profiles` table for generic CSV column mappings; v3 backfills `form_line` on the stock chart-of-accounts categories; v4 adds the invoicing tables (`clients`, `invoices`, `invoice_line_items`, `invoice_payments`); v5 adds `voided_at` to `invoices` so void is derived rather than hand-set; v6 normalizes stored invoice dates (issue/due/published/voided/paid) to zero-padded ISO through `invoices::validate_date` itself, leaving values that rule cannot read untouched, then re-derives the status of every non-void invoice whose dates moved — padding a due date changes what `is_overdue` answers, so the stored status no longer follows from the row without it; v7 adds `archived_at` to `clients`; v8 creates `client_contacts` (one row per address, a partial unique index for at most one `is_billing` per client and an expression index for per-client `lower(email)` uniqueness), backfills it from `clients.email` and drops that column; v9 seeds `payment_instructions` with the bank-transfer sentence the stock page used to hardcode, so removing that paragraph is not a silent regression for an installation that was relying on it — the address comes from the same `contact_email`-then-`from_email` fallback `{{CONTACT}}` interpolated, and it writes nothing when the key is already set (including deliberately empty), when there is no contact address to name, or when the database has never invoiced, which is what keeps a fresh `nigel init` free of it
- **Data flow:** CSV/XLSX import → automatic pre-import DB snapshot (`<data_dir>/snapshots/`) → format auto-detect via `ImporterKind::detect()` → duplicate detection → auto-categorize via rules → flag unknowns for review → generate reports
- **Accounting model:** Cash-basis, single-entry. Negative amounts = expenses, positive = income. Categories map to IRS Schedule C / Form 1120-S line items via `tax_line` and `form_line` columns.
- **Settings:** `~/.config/nigel/settings.json` — stores `data_dir`, `user_name`, `update_check` (bool, default true), `last_update_check` (ISO 8601 timestamp), and the invoicing keys `stripe_secret_key`, `mailgun_api_key`, `mailgun_domain`, `from_email`, `from_name`, `reply_to_email`, `contact_email`, `r2_account_id`, `r2_access_key`, `r2_secret_key`, `r2_bucket`, `public_base_url`; `settings::invoicing_config()` resolves each invoicing value from its `NIGEL_*` env var first, then the file (`NIGEL_STRIPE_SECRET_KEY`, `NIGEL_R2_BUCKET`, …). `nigel load` switches between existing data directories without reinitializing. Per-database settings (e.g. `company_name`) are stored in the `metadata` table. Database password is runtime-only (never persisted to disk).
- **Password Manager:** `cli/password_manager.rs` — TUI screen for managing database encryption; detects current encryption state and shows set/change/remove options; masked password input with confirmation; used as sub-screen within Settings Manager
- **Onboarding:** `cli/onboarding.rs` — full-screen TUI shown on first launch (when settings.json doesn't exist); collects user name, business name, and optional password (masked input), then offers demo/fresh/load options
- **Data directory:** `~/Documents/nigel/` by default, configurable via `nigel init --data-dir`; switch with `nigel load <path>`. Contains `backups/` (manual backups) and `snapshots/` (automatic pre-import snapshots)
- **Demo:** `nigel demo` dynamically generates 18 months of sample transactions (counting backwards from the current date) + 10 rules directly into the DB (no CSV files), then runs categorization; dates are computed at runtime so reports always show current-year data. `insert_demo_invoicing` adds three clients and four invoices covering paid, partial, sent and draft, dated relative to today the same way, so the invoicing screens are not empty on the one database meant for exploring. No seeded invoice carries a payment link, so the launch sync's `stripe_payment_link_id IS NOT NULL` query skips all of them

## Commands

```bash
cargo build                                       # Debug build
cargo build --release                             # Release build
cargo test -- --test-threads=1                    # Run all tests (serial — the DB password is a process global)
cargo test --no-default-features -- --test-threads=1   # Test without gusto/pdf features
nigel                                             # Interactive dashboard (default)
nigel --help                                      # CLI help
nigel init                                        # Initialize (prompts for data dir on first run)
nigel init --data-dir ~/my-books                  # Initialize with custom data dir
nigel init --profile personal                     # Seed the personal chart of accounts (default: business)
nigel demo                                        # Load sample data to explore
nigel import <file> --account <name>              # Import CSV/XLSX (auto-detects format)
nigel import <file> --account <name> --format bofa_checking  # Import with explicit format
nigel import <file> --account <name> --dry-run           # Preview without importing
nigel import <file> --account <name> --date-col 0 --desc-col 1 --amount-col 3  # Generic CSV
nigel import <file> --account <name> --date-col 0 --desc-col 1 --amount-col 3 --save-profile chase  # Save profile
nigel import <file> --account <name> --format chase      # Use saved profile
nigel undo                                        # Undo the last import (with confirmation)
nigel accounts rename 1 "New Name"                # Rename account by ID
nigel accounts delete 3                           # Delete account by ID (blocked if has transactions)
nigel categories list                             # List all categories
nigel categories add "Consulting" --type income   # Add a category
nigel categories rename 5 "Professional Fees"     # Rename a category
nigel categories update 5 "Fees" --type income --tax-line "Gross receipts"  # Update all fields
nigel categories delete 30                        # Soft-delete a category
nigel rules test "ADOBE" --match-type contains    # Test pattern against transactions (dry run)
nigel rules update 1 --priority 10                # Update a rule field
nigel rules update 5 --category "Rent / Lease"    # Reassign rule category
nigel rules delete 3                              # Deactivate a rule (soft-delete)
nigel categorize                                  # Re-run rules on uncategorized
nigel recategorize 185 212 --category "Travel"    # Bulk move by IDs (applies immediately)
nigel recategorize --from-category "Cost of Goods Sold" --year 2025 --category "Supplies" --yes
                                                  # Bulk move by filters (--dry-run to preview; --yes to apply)
nigel review                                      # Interactive review
nigel review --id 185                             # Re-review a specific transaction by ID
nigel report pnl --year 2025                      # Interactive view (ratatui)
nigel report expenses --month 2025-03             # Expense breakdown
nigel report tax --year 2025                      # Tax summary
nigel report cashflow                             # Cash flow
nigel report balance                              # Cash position
nigel report register --year 2025                 # Interactive register browser
nigel report register --account "BofA Checking"   # Filter by account
nigel report register --category "Taxes & Licenses"  # Filter by category
nigel report register --uncategorized             # Only transactions with no category
nigel report flagged                              # Flagged transactions
nigel report k1 --year 2025                       # K-1 prep worksheet (1120-S)
nigel report aging                                # A/R aging buckets and open invoices
nigel report pnl --year 2025 --mode export        # Export as PDF
nigel report pnl --year 2025 --mode export --format text  # Export as text file
nigel report pnl --year 2025 --output ~/report.pdf  # --output implies export
nigel report all --year 2025                      # Bulk export all reports (PDF)
nigel report all --year 2025 --format text        # Bulk export as text files
nigel report all --year 2025 --output-dir ~/exports/  # Custom output directory
nigel browse register                            # All transactions, starts at today
nigel browse register --year 2025                 # Filter to a specific year
nigel browse register --account "BofA Checking"   # Browse filtered by account
nigel browse register --category "Taxes & Licenses"  # Browse filtered by category
nigel browse register --uncategorized             # Browse transactions with no category
nigel client add "Acme Co" --email ap@acme.test        # Add an invoicing client
nigel client list                                 # List clients with their IDs
nigel client show 1                               # One client: details plus invoice history
nigel client edit 1 --email ap@acme.test          # Update a client's name/email/address/notes
nigel client edit 1 --contact "ap@acme.test:Ada:AP" --contact "dana@acme.test"
                                                  # Replace the contact list (first = billed, rest cc'd)
nigel client delete 7 --yes                       # Delete (refused while any invoice bills them)
nigel client archive 7                            # Hide a finished client; touches no invoice
nigel client unarchive 7                          # Bring it back to the working list
nigel client list --all                           # Include archived clients, with the date
nigel invoice new --client 1 --issue 2026-08-04 --item "Consulting:10:150"  # Draft (--item repeatable)
nigel invoice new … --notes "Thanks" --terms "Net 30"  # Rendered on the invoice page and the PDF
nigel invoice edit 1248 --due 2026-09-30          # Edit a draft (published invoices refuse)
nigel invoice edit 1248 --clear-due               # Drop the due date, so it never goes overdue
nigel invoice void 1248                           # Cancel an invoice (confirms; --yes to skip)
nigel invoice delete 1252                         # Delete an unsent draft (confirms; --yes to skip)
nigel invoice list                                # Number, status, client, total, due date
nigel invoice show 1248                           # Line items, paid amount, payment link
nigel invoice preview 1248                        # Render HTML/PDF locally, no network (<data_dir>/previews)
nigel invoice preview 1248 --output-dir /tmp      # Write the preview somewhere else
nigel invoice send 1248                                 # Render, write the preview files, confirm, then publish and email
nigel invoice send 1248 --yes                           # Skip the confirmation and the files
nigel invoice sync                                # Pull Stripe payments and record them
nigel invoice pay 1248 --date 2026-08-20          # Record a manual payment (default: full balance)
nigel invoice pay 1248 --date 2026-08-20 --amount 500 --method ach  # Partial/other method
nigel invoice aging                               # A/R aging buckets
nigel invoice import --from-invoiceshelf ~/is.sqlite  # One-time InvoiceShelf import
nigel invoice template export                     # Write the built-in page to <data_dir>/templates/invoice.html
nigel invoice template export --output ~/mine.html --force  # Somewhere else / overwrite
nigel invoice template path                       # Where Nigel looks, and whether an override is in effect
nigel reconcile "BofA Checking" --month 2025-03 --balance 12345.67
nigel serve                                       # Web UI + JSON API on 127.0.0.1:5731 (opens a browser)
nigel serve --port 8080                           # Bind a different port (0 = ephemeral)
nigel serve --no-open                             # Print the tokenized URL instead of opening a browser
nigel status                                      # Show active DB and summary stats
nigel load ~/other-books                          # Switch to a different data directory
nigel backup                                      # Back up DB to <data_dir>/backups/
nigel backup --output /tmp/nigel-backup.db        # Back up to custom path
nigel restore ~/backups/nigel-20250301-120000.db  # Restore from a backup file
nigel password set                                # Encrypt an unencrypted database
nigel password change                             # Change password on encrypted database
nigel password remove                             # Decrypt database (remove password)
nigel update                                      # Check for and install the latest version
nigel completions bash                            # Generate shell completions (bash, zsh, fish, powershell)
```

### Web UI

Requires Node 20.19+ (22 recommended). All commands run from `web/`.

```bash
npm ci                                            # Install (committed lockfile)
npm run build                                     # theme -> ui -> app, output to web/dist
npm test                                          # vitest across all three packages
npm run lint                                      # eslint across all three packages
npm run typecheck                                 # tsc --noEmit across all three packages
npm run dev                                       # Vite dev server on :5173 (proxies to :5731)
npm run preview                                   # Component preview harness on :9090
```

Dev loop — run the backend and the dev server side by side, then open the
token URL **on the vite origin** so the session cookie lands there:

```bash
cargo run -- serve --no-open                      # terminal 1, prints /auth?token=<hex>
cd web && npm run dev                             # terminal 2
# browser: http://localhost:5173/auth?token=<hex>
```

`cargo build` works without node — `build.rs` seeds `web/dist` from
`web/placeholder/index.html` and the binary serves a "SPA not built" page. Run
`npm run build` in `web/` before `cargo build --release` to embed the real app.

## Component-First UI Workflow (MANDATORY)

Every visual change ships through `@nigel/ui`:

1. **The component lives in `web/packages/ui/src/components/`** as `wc-foo.ts`.
2. **A preview is co-located.** `wc-foo.preview.ts` covers the visible states (default, hover, disabled, loading, empty, dense — whichever apply).
3. **A11y passes.** `wc-foo.test.ts` calls `describePreviewA11y(preview)`, which runs `axe.run()` over every state the preview declares with zero violations. Adding a state adds its a11y test automatically — do not restate the states inside the test.
4. **Then it is consumed.** `web/apps/app` imports from `@nigel/ui`. **No bespoke component implementations in `web/apps/app/src/components/`** beyond the `nigel-app` root container.

The preview harness boots with `npm run preview` in `web/` at http://localhost:9090.

### Pre-merge checklist (visual changes)

- [ ] `wc-foo.preview.ts` exists with all visible states
- [ ] `describePreviewA11y` runs and passes with zero violations
- [ ] The component reads tokens from `@nigel/theme` — no inline brand values
- [ ] No styling logic for primitives lives in `web/apps/app/`

Pure logic, state, and service work is exempt.

### Component selection

- Use Web Awesome `<wa-*>` primitives unless behavior demands custom. Import them cherry-picked (`@awesome.me/webawesome/dist/components/<x>/<x>.js`) — never the autoloader, and never the WA stylesheet.
- A `wc-*` wrapper reads `@nigel/theme` tokens and exposes them as cascading variables; it never duplicates a brand value inline.
- A component that renders a `wa-*` primitive adopts `controlsCss`: `static styles = [controlsCss, css`…`]`. The theme's `::part()` treatment reaches one shadow boundary down from the tree the rule is in, so it has to be adopted by the component hosting the primitive — the document sheet is several boundaries away. `controls-adoption.test.ts` in each package fails the build when a file imports a `wa-*` module without it. This applies to app screens too; adopting the shared sheet is not the same as putting styling logic in `apps/app`.

## Documentation Policy

Every feature change must update the relevant documentation before the work is considered complete:

- **CLAUDE.md** — update Architecture, Commands, Project Structure, and Key Design Constraints sections when adding/changing CLI commands, modules, data flow, or settings
- **README.md** — update Quick Start, Features, and Configuration sections for user-facing changes

Do not merge or mark work complete if docs are stale.

## Key Design Constraints

- `tests/layering.rs` fails the build if anything on the core side reaches into `src/cli/`. Its scope is stated as an **exclusion**: everything under `src/` is guarded except `src/cli`, `src/tui.rs`, `src/browser.rs`, `src/effects.rs` and `src/main.rs`, so a module added to the core side is guarded from the moment it exists rather than when somebody remembers to list it. A second test asserts every excluded path exists, because a path that has been renamed away excludes nothing while still looking like it does. The scan matches the braced `use crate::{cli…}` forms as well as a bare `crate::cli::`, since a nested `use` group contains neither. Every `cli/<x>.rs` is a printing wrapper over the data layer that lives at the top level, and the invoicing wiring the HTTP layer needs is `invoicing::wiring`, which keeps the "invoicing never reads settings" rule by taking config as a parameter. The boundary exists so a desktop client can link the router without linking a terminal UI: `src/cli/` is clap and ratatui, so anything on the core side that names `crate::cli::` drags both in. It is still a text scan and cannot see coupling that arrives without a matching string — a trait impl, a closure the CLI hands in, an inherent impl left behind by a moved type — so it is a floor, not a proof; the compiler is the only real check, which is what task-33.9 exists to add
- All financial modifications require user confirmation — auto-categorizes but never silently changes confirmed data
- `recategorize` confirmation asymmetry: explicit IDs apply immediately (typing the IDs is the confirmation); filter selections print the matched rows and require `--yes`, or a y/N prompt on a TTY. Filter mode with zero filters is an error, and a malformed `--month` is a hard error — never a silently widened selection
- Interactive review supports back navigation: Esc goes back to re-review the previous transaction (undoing its categorization and any created rule), Tab skips forward
- Duplicate detection uses file checksums (imports table) and transaction-level matching (date + amount + description + account)
- Rules are ordered by priority DESC; first match wins
- Categories with `form_line = 'excluded'` (the stock `Transfer` category in both charts) are money movement, not income or spending: P&L, expense breakdown, cash flow (including its running balance), and the balance report's `ytd_net_income` skip them (`EXCLUDE_TRANSFERS` in `reports/mod.rs`); the register and per-account balances keep them, because per account the cash really moved
- The books profile matters at seed time and on a handful of surfaces: personal books drop K-1 from the dashboard's View Reports picker (`report_picker_items`/`canonical_report_idx`), from `report all` bulk exports, and from the SPA report directory; relabel "Business Name" to "Household Name" (TUI settings, SPA settings, onboarding); reword the SPA categories screen description; and refuse `nigel demo`, whose data is a business and whose rules name business categories. The K-1 command, route, and export stay reachable by name on any profile. `/api/status` reports `profile`, defaulting to `business` while locked or uninitialized
- Gusto imports extract only aggregate totals, never individual employee data
- Bank CSV formats vary by account type (checking, credit_card, line_of_credit) — each has its own variant in `ImporterKind`
- `ImporterKind::detect()` inspects file headers for format auto-detection; `--format` CLI flag overrides auto-detect
- Demo data is generated dynamically (18 months of transactions counting back from today, plus clients and invoices dated the same way) and inserted directly into the DB (no CSV files); `seed_demo` guards each half on what it writes — the `BofA Checking` account for the ledger, any invoice row for the invoicing — because the two cannot share a transaction (`create_invoice` opens its own and SQLite has no nested `BEGIN`), so a failure between them would otherwise be permanent, and the summary is read back from the database rather than tallied. The demo's invoice statuses are derived by `mark_published`/`record_payment` calling `refresh_status`, never written. The committed web report fixtures are unaffected by any of it: `server::fixture_capture` seeds from `server::testutil::seeded_db`, not from `cli::demo`
- Cash amounts are plain `f64` — negative = expense, positive = income. This is a known precision limitation: `f64` is not suitable for sub-cent accuracy, but is acceptable for the cash-basis bookkeeping use case where all amounts are rounded to cents on import
- Date filters `--from`/`--to` must be supplied as a pair; providing only one is a hard error
- Browse register and reports with no date flags show all transactions (no implicit year filter); the browse view scrolls to today on load
- Register filters (`--account`, `--category`, `--uncategorized`) are shared by `report register` and `browse register` via the `RegisterFilterArgs` clap struct in `cli/mod.rs`; `--category` and `--uncategorized` are mutually exclusive (enforced by clap). `RegisterFilters::resolve()` in `reports/mod.rs` validates the category name against the database before any query runs — an unknown name is `NigelError::UnknownCategory`, and other rusqlite errors propagate unchanged. Account names are not validated: an account with no transactions yields an empty selection, matching the pre-existing `--account` behavior
- Active register filters are surfaced everywhere the report is: the browser footer, the text/PDF report header (via `register_subtitle()`), and the default export filename, which appends slugified filter fragments (`register-bofa-checking-taxes-licenses-<date>.txt`). An explicit `--output` always takes precedence
- Database row deserialization errors are propagated, never silently discarded
- Database password is never persisted to disk — stored only in runtime `Mutex<Option<String>>`; for the dashboard, password is collected inline on the splash screen (TUI masked input); for CLI subcommands, prompted via rpassword
- Demo databases are always unencrypted; `init` and `demo` subcommands skip password detection
- Backups and snapshots preserve the encryption state of the source database
- Cross-encryption-state operations (encrypt/decrypt) use `sqlcipher_export` via ATTACH DATABASE; same-encryption operations (backup, rekey) use SQLite backup API or `PRAGMA rekey`
- Schema migrations run in the `dispatch()` pre-flight — after the initialization check and the password prompt — for every subcommand except `init`, `demo`, `load`, `update`, `password`, `completions`, `serve`, `invoice template`, and `restore`; the dashboard runs the same pre-flight before its render loop. The exemptions: `init`, `demo`, and `restore` call `init_db()` themselves on the database they create or replace; `load` only rewrites `settings.json`; `update` needs no database; `password`, `completions`, and `serve` are password-exempt and so cannot open an encrypted database; `invoice template` only reads and writes a file in the data directory, so exporting a template works on a machine that has never run `nigel init`. A failed migration aborts the command. Each migration is transactional (savepoint); to add a migration: append to `MIGRATIONS` array in `migrations.rs`, bump `LATEST_VERSION`, implement `up()` function with SQL statements
- Generic CSV profiles are stored in `csv_profiles` table; `--format <name>` resolves built-in importers first, then csv_profiles; generic CSV is never auto-detected
- `--dry-run` skips snapshot creation, imports table insertion, and transaction insertion; still runs full parse and duplicate detection
- Auto-update check runs once per 24 hours on launch (both dashboard and CLI); respects `update_check: false` in settings.json; silently skips on network failure; `nigel update` command always checks and can be exempt from init/password checks
- Invoice money is plain `f64` dollars; cents exist only at the Stripe boundary (`to_cents`/`amount_total`) and the InvoiceShelf import boundary. Paid-in-full comparisons carry half a cent of slack
- Every invoicing date is normalized by the writer, never by the caller: `validate_date` returns the padded form and all five functions that write a date column (`create_invoice`, `update_invoice`, `record_payment`, `void_invoice`, `mark_published`) store that, so the CLI, the TUI and the API cannot disagree and `refresh_status`'s string comparison stays correct. Each also validates the reference day it derives a status against, `update_invoice` included. A four-digit year is part of the rule, not chrono's business: `%Y` reads `26-8-9` as the year 26 AD, which would book a date two millennia off and hand `refresh_status` that day, so the year is counted before chrono is consulted. `record_payment` validates its own `paid_date` for the reason it validates its own method — a caller reaching the data layer directly cannot get past either. The HTTP API stays stricter still: `routes::reports::parse_date` requires ten characters, so `2026-4-1` is a 400 over HTTP and a normalized `2026-04-01` at a terminal. A stored date the rule cannot read is left alone rather than guessed at — by v6, by the InvoiceShelf import (which counts them into `ImportSummary::unparsed_dates` so the operator hears about it), and by `ar_aging_detail`'s `unwrap_or(today)` alike
- `update_invoice` takes the reference day as a trailing parameter like `void_invoice` and `record_payment`; nothing under `src/invoicing/` reads the clock, so every derived status is deterministic in tests and correct against the wall clock in production. The CLI and the API both pass `clock::today()`
- Invoice status is derived, never set by hand: `refresh_status` recomputes draft/sent/partial/paid/overdue from `published_at`, the payment total, and the due date, and `void` from `voided_at` — `void_invoice` writes that timestamp and lets the same function derive the status, so no code path sets `status` directly. `void` is terminal and blocks send, pay, and edit, and the guard lives in the data layer (`send_invoice` and `record_payment` call `ensure_not_void` themselves) rather than only in the CLI wrapper, so a front end that reaches the data layer directly cannot bypass it
- An invoice's `token` is `#[serde(skip_serializing)]`, so it never crosses the wire. It is the only access control on a published invoice, and a list endpoint carrying one token per row would put every invoice's access control into devtools history and any future response cache. What a client needs is the address, not the secret, so a response that wants one carries a computed URL instead
- Invoice edit is draft-only, and refused outright once any payment is recorded — which is what makes changing the currency under a settled amount unreachable. Void is likewise refused for an invoice with payments: cancelling money already received belongs in the transaction register, not in a credit-note model Nigel does not have. Void tears down what the invoice published — the Stripe payment link is deactivated and the published page is replaced with a static voided notice, the PDF left where it is so the token URL keeps resolving to something honest. Both run *after* the void has committed and neither can undo it: a Stripe outage is not a reason for an invoice to stay open in the books, so a failure prints a warning naming the link's URL for manual cleanup instead. Nothing is required — an unset key is a warning at the end rather than a refusal at the start, and voiding an ordinary draft (no link, never published) says nothing at all. An edit that moves the total or the currency clears a stale payment link, so the next send creates one at the right amount. Editing a client affects the next send, never an already-published page
- Delete is for the invoice that should never have existed; void is for the one that has. **Only a draft that was never published and carries no payments can be deleted** — published, paid and void all refuse, because each means somebody outside this machine has seen it: a token URL and an emailed PDF are in a client's hands, or the row is a record that something happened. `invoices::delete_blocker` is the one guard, in the `clients::delete_blocker`/`DeleteBlock` mold, and `NigelError::Blocked`'s `Display` is the one sentence the CLI, the TUI and the API all print. The *advice* under that sentence is not shared, because it is only honest when it names something that would work: `cli::invoice::delete_alternative` and the SPA's `not_deletable` branch both offer void only where `ensure_voidable` allows it, say nothing to an already-void invoice, and tell a paid one it stays on the books — a "Cannot delete" literal about an invoice anywhere else is a second copy of a rule with one home. `delete_invoice` runs that guard **inside** its own transaction, alongside the line-item cascade, so a delete races nothing; payments are asserted rather than cascaded, because the guard means a deletable invoice has none. That guard is **row existence**, not a sum: `has_payments` is one `EXISTS` asked by `delete_blocker` and re-asserted by `delete_invoice`, so a row of exactly `0.00` — or two that cancel out — cannot make every pre-flight say deletable while the write refuses. It is also why `detail_for` no longer pays for a second `SUM` it already has in hand. **The invoice number is not reused**: `next_invoice_number` is untouched, so the gap stays and `invoice new` carries on from where it was — reissuing a number that may already have been exported or quoted is the failure this avoids, and it is pinned by `deleting_the_newest_draft_does_not_move_the_invoice_number_counter`. Delete makes no network call, which is what keeps it a plain confirmation on every surface where send and void need a two-phase frame
- `invoice preview` renders through the same `invoicing::render::render_invoice` seam `send` publishes through, so the two cannot drift; it makes no network call (it is in `main.rs`'s launch-sync skip list), needs no invoicing config, and writes nothing to the database. There are exactly **three** differences from a published invoice, each deliberate: the Pay placeholder on an unsent draft, the absent PDF in a build without the `pdf` feature, and **where the logo's bytes come from** — a preview carries the `data:` URI inline, a published page points its `<img src>` at the hosted object. The third is deliberate on both sides: the published page *is* the email body and Gmail strips a `data:` URI out of one, while a preview that pointed at a hosted object would be a broken image on a machine that has never sent anything, and pointing at one would cost preview both of its invariants (no network call, no configuration). `Branding.logo_url` is the whole mechanism: `invoicing::wiring::company_profile(...).branding(...)` leaves it `None`, and only `send` and a republish set it, through `with_logo_url`
- A custom invoice page lives at `<data_dir>/templates/invoice.html` and is validated when it is **loaded**, not when it is rendered, so `invoice preview` and `invoice template path` catch a typo before a client does. An override that exists but is empty, oversized, missing one of `{{NUMBER}}`/`{{CLIENT}}`/`{{ROWS}}`/`{{TOTAL}}`, or using a `{{KEY}}` outside `PLACEHOLDERS` is an error naming the path — never a silent fallback to the stock page, which would put a document the operator never approved in a client's inbox. `send` loads it before the Stripe link is created, so a broken template costs no link, no upload and no email. Values are escaped on the way in (`esc`) and `expand` stays single-pass, so a client named `Acme {{ROWS}} Co` is literal text; the template itself is never sanitized, because whoever can write that file already runs programs as the operator. The PDF has no template: its customization is the operator's letterhead, which `render_invoice` threads out of the same `Branding` the HTML page reads, and `company_name` is repeated in the document's Info title. **Both documents carry the real logo.** `printpdf`'s `embedded_images` is enabled and its price is a measured number rather than an estimate: nine crates (`image`, `png`, `gif`, `jpeg-decoder`, `tiff`, `color_quant`, `fdeflate`, `bytemuck`, `bitflags` — its `image` dependency hard-enables every format, so PNG alone is not on offer) and **84,496 bytes** of release binary for the feature alone, measured on identical source with the same SPA embedded on both sides. printpdf's soft-mask path sizes a transparent image's mask from the image's *width*, so a wide transparent wordmark would embed wrong — **nothing hands printpdf an RGBA image**: `pdf::prepare_logo` composites any alpha onto white and is the only thing that builds what printpdf receives, so the defective path is unreachable rather than avoided, and a test asserts it. Any unusable logo — bad prefix, wrong type, bad magic bytes, over `MAX_LOGO_BYTES`, cut off before its `IEND`/`FFD9` end marker, zero-sized, or simply refusing to decode — degrades to the text wordmark on the PDF and to no `<img>` at all on the page, and **never fails a render or a send**: a logo is decoration on a document about money. Which of those it is, is decided **once, in `render_invoice`, above both renderers**, and the verdict is forced onto the page by clearing `Branding.logo` rather than left for it to re-derive: `document::parse_logo` is everything checkable without a decoder (and so holds identically in a build with no `pdf` feature), and `pdf::logo_is_embeddable` is the decode, which exists only where the feature brought `image` in. A file the page would happily put in an `<img>` and the PDF could not embed is the disagreement that split them, and it is unrepresentable now
- The TUI sends an invoice on the main thread and paints the screen first. The dashboard loop is draw → blocking `event::read()` → `handle_key()`, so a multi-second `send_invoice` called from the key handler would freeze the terminal showing the *confirmation dialog*, as if the keypress had been dropped. `InvoiceManager::handle_key` therefore returns `InvoiceAction::Perform`, the dashboard draws once more (painting the "Sending…" frame, which says the terminal is unresponsive) and only then calls `perform_pending`, which drains buffered input afterwards so a user who mashed Enter during the wait does not dismiss the result unread. Void goes through the same two phases for the same reason, since its teardown makes up to two more network calls. A worker thread plus a spinner was rejected: `Connection` is `!Sync` and `send_invoice` takes `&Connection`, so it would need a second handle on a possibly-SQLCipher database and a second writer racing the main one for the `mark_published` UPDATE
- `clients.name` uniqueness is **advisory**: `clients::name_taken` refuses a duplicate in `add_client`/`update_client`, and the column carries no `UNIQUE` index. The schema constrains machine-generated identity (`invoices.number`, `invoices.token`, `invoice_payments.stripe_checkout_session_id`, `csv_profiles.name`) and leaves user-typed names to the data layer, as `accounts.name` and `categories.name` already do — a `UNIQUE` index there would be wrong for categories, which soft-delete, so a retired `Travel` and a new one must coexist. Nothing resolves a client by name (invoices carry `client_id`; the only `WHERE name = ?` in production is `name_taken` itself), and `import_invoiceshelf` intentionally mirrors a source that does not guarantee unique names, so a constraint would abort a one-time migration over a cosmetic duplicate — `two_source_customers_with_the_same_name_import_as_two_clients` pins that. Two racing web writes can still both insert; the cure is a rename on the clients screen
- The page and the PDF print the same money lines because both ask `MoneySummary::lines()` — one function decides which lines exist for one invoice, so the attachment and the address a client is given cannot describe the same invoice differently. A balance is never negative: `invoices::is_settled` (the *same* inclusive test `refresh_status` uses to derive `paid`, over the one exported `invoices::CENT_SLACK`) zeroes a settled one, and an overpayment becomes a `Credit` line. The rows the payment block introduced — Paid, Balance due, Credit — carry `MoneyLine::payment_row` and render as `USD 60.00` on **both** documents, since they are new to both and a bare `$` cannot name a currency; the pre-existing Subtotal/Tax/Total rows keep each document's own style. `document::address_lines` clamps a billing address to `MAX_ADDRESS_LINES` with `ADDRESS_TRUNCATED` — the PDF has no page-break logic under that block — and clamping above both renderers is what keeps them agreeing; `document::email_line` is the same idea for blank-is-absent. A null or empty value omits its block rather than printing an empty label: on the page that is a *fragment* placeholder expanding to nothing (a single-pass expander with no conditionals can only omit a block by being handed an empty one), and in the PDF it is a row that is not drawn. And `REQUIRED` never grows — a template exported before a vocabulary change keeps loading and keeps rendering exactly what it did, gaining nothing until it is edited
- The invoice PDF carries no payment link **and no page URL** — no URL of any kind, asserted by `no_live_payment_link_reaches_the_pdf` and `the_pdf_prints_no_url_at_all`. An emailed attachment cannot be recalled or republished, so a live charge link in one would survive the settlement it was created for — the same reasoning that makes void deactivate the Stripe link — and nothing deactivates a link on settlement. Printing the page's `{base}/{token}/index.html` instead was considered and rejected: a tokenized address as unclickable text is sixty characters of noise beside the figure that matters, and the email already carries the link. Paying online is the published page's job, because the page is the one artifact a republish can correct. `{{PAY}}`/`{{PAY_URL}}` stay page-only, and `page_url` never joins `Branding`
- Payment instructions are one configurable block (`payment_instructions`), rendered under the foot rule on **both** documents or on neither — `document::payment_lines` decides the lines and both renderers draw them, so the page and the PDF cannot disagree about how to pay. Nothing about bank transfers is hardcoded: an installation that takes none prints no heading and no block. `{{CONTACT}}` keeps its exact meaning for templates that use it and simply leaves the stock documents. Removing the hardcoded paragraph is not allowed to be *silent*, which costs two things: migration v9 seeds the old sentence for a database that was already invoicing, and `cli::invoice::payment_instructions_notice` prints one stderr line from `preview` and `send` when a document would go out with no way to pay on it — unset instructions **and** no template override, since an operator who owns their page owns what it says about paying
- Neither document carries a title line. The letterhead is the masthead and the metadata band carries the identifier, so a heading reading `Invoice #1248` over a row reading `Invoice ID  1248` would say it twice; `{{NUMBER}}` stays in the page's `<title>` (which is what keeps it satisfying `REQUIRED`) and in the PDF's Info title, both of which are file metadata rather than visible layout. Line-item rows are ruled and every other one is tinted on both documents, from `document::row_is_shaded` — on the page a `:nth-child(even)` rule, in the PDF a filled band drawn *before* the row's cells, after `ensure_space` has already decided which page the row lands on, which is what carries the striping and the grid correctly across a page break. Every structural rule on both documents is `BORDER_GRAY`; the stock template is a static file, so a test reads the hex back out of it rather than trusting the two to stay in step
- The page and the PDF draw the From block, the metadata rows, the client block, the money rows, the terms block and the payment block from the same `document.rs` functions, so the two cannot disagree about the same invoice. A missing value omits its block rather than printing an empty label, on both. `REQUIRED` never grows — `{{NUMBER}}`, `{{CLIENT}}`, `{{ROWS}}`, `{{TOTAL}}` (or `{{TOTALS}}`) remain the whole requirement — so every template exported before the house layout keeps loading and keeps rendering exactly what it rendered, gaining nothing until its author edits the new keys in
- A payment against a **published** invoice republishes its page and PDF, best-effort, on void's terms: `record_payment` commits first and nothing afterwards can undo it, so an unconfigured publisher and an R2 outage are both warnings naming the invoice rather than failures. Every front end goes through `cli::invoice::republish_after_payment` (which resolves the branding `src/invoicing/` may not read) — `invoice pay`, `invoice sync`, the launch sync, the TUI's `p`, and `POST /api/invoices/{n}/pay`, which therefore reaches the network and answers `republishWarnings` beside the refreshed detail, the shape void's `teardownWarnings` established. It takes the `InvoicingConfig` and the data directory as **arguments**, the way `begin_send` takes them: each surface resolves its own settings at its own call site, so a caller that passes no config cannot reach a bucket and a test that omits one does not compile. `republish_with` is the same function with the publisher injected — the `send_with`/`void_with` seam — taking the invoice the caller already holds, and `republish_all_with` is the sync loop over it; both are what the HTTP layer calls, so the sentences a failed republish earns have one home rather than a copy per front end. **The two HTTP handlers resolve that config and data directory *inside* the `with_conn_api` closure**, which is `routes/mod.rs`'s read-after-gate rule and not an optional style: a data-directory switch holds `db_gate` for writing, and values resolved before the wait would record the payment in the new database while the republish loaded its template and its bucket from the old directory — a wrongly branded page published without a word. `state.data_dir()` is the source, the one `send_with` already reads, because it follows the `db_path` the switch rebinds under that guard. `SyncReport.recorded_invoices` is what a sync republishes, and the only way a browser can say which invoices a run moved. The TUI paints a `Republishing` frame before the uploads for the reason send and void paint theirs; an unpublished invoice reaches nothing and stays a plain write
- Send is confirmed on every surface, and every confirmation shows the document the client will get, rendered through `render_invoice` — the one seam. `nigel invoice send` renders, writes the preview artifacts at `invoice preview`'s own paths, states the recipient and the total, and asks, with `--yes` to skip both the prompt and the files (a scripted send has nobody to look at them) and a non-TTY refusal before either happens; the browser frames the page the preview route serves. Neither path constructs a gateway, so a broken custom template is caught while the operator is deciding rather than three steps into a send. A build without the `pdf` feature cannot send at all — the PDF is attached to the email — and both surfaces say so up front while still rendering the page
- Invoice payments are keyed by Stripe checkout session ID, so `invoice sync` is idempotent; Stripe reconciliation is pull-based (no webhook endpoint)
- Sending an invoice over HTTP is a blocking request, not a job. `with_conn_api` already runs on `spawn_blocking` and the three gateway clients are synchronous `reqwest::blocking`, so the work lands on that pool either way; more to the point the invoice row **is** the job record — `published_at` and `stripe_payment_link_url` are the durable state a job store would duplicate, and two sources of truth for "did this go out" is how they drift. The costs are paid explicitly: every outbound invoicing call is bounded (10s connect, 30s total), and a send makes five of them — two Stripe (price, payment link), two R2 (HTML, PDF), one Mailgun — so a send that hangs everywhere it can is about 150s plus rendering rather than an open socket. There is deliberately **no deadline over the orchestration as a whole**: the per-call bound and the step trace are the design, and a run cut off part-way would leave the caller unable to say which steps had happened. `POST /api/invoices/sync` is the opposite case and does carry one — a 60s budget checked before each invoice, with the invoices it did not reach returned as failures — because N open invoices is N × 30s of a request holding `db_gate` against an encrypt, a decrypt or a data-directory switch. The response carries the step trace on success as well as on failure; and **nothing retries automatically**, on either side — a failure at `record` after `email` succeeded means the client already has the invoice, so a retry is a fresh confirmation
- `POST /api/invoices/{number}/send` requires `{"confirm": true}` in the body and does nothing without it. AC-level "send requires explicit confirmation" is a property of the endpoint rather than of whichever screen calls it, which is what keeps it from being a convention the next screen forgets — and makes an accidental `curl` a no-op. `pay` and `sync` do not require it: one writes a row a person typed, and the other is idempotent by checkout session id and already runs at every CLI launch
- The upstream's own message is what a send failure reports. `r2 403: SignatureDoesNotMatch` is the only information anyone has about why R2 refused; a sentence reconstructed from the status would be a worse bug report. What the API adds is structure, never a substitute: the step, the service, the steps completed, `emailSent`, and the invoice's status. No response carries a setting's value — `send_not_configured` names the unset keys and nothing else
- A/R aging is always as of today: it takes no date parameters, its view has no period navigation, and the as-of date rides in the title. `nigel invoice aging` prints `reports::text::aging` — the report module's own table, header and all — so the two commands cannot drift; it stays a one-shot print and never routes through `report::dispatch`. The HTTP API serves aging at `GET /api/invoices/aging` and nowhere else — `/api/reports` and `/api/exports` still carry eight reports where the CLI has nine, because `ReportKind` has no aging variant and giving it one would mean touching the vocabulary eight endpoints and two front ends share. That route does take an optional `asOf`, the one place aging is not "as of today": a committed figure-parity fixture cannot survive a bucket boundary crossed overnight, which is the same reason the report fixtures fix their year
- The letterhead logo is published as its own object beside the pages, and that object is **content-addressed and immutable**. `r2::logo_object` names it `logo-<first 8 of the sha256>.<ext>` under `i/`, beside the token directories rather than inside one — it is the operator's own mark, carries no client data, and a per-token copy would write the same bytes again for every invoice ever sent. Nothing overwrites it and **nothing deletes it**: a page that has been delivered points at that address, and a delivered document may not change afterwards, so a rebrand writes a *different* object and the invoice a client is looking at keeps the mark it was sent with. That is the correct outcome, not a stale cache; the price is that old objects accumulate, one per rebrand at `MAX_LOGO_BYTES` each, and they are load-bearing, so there is deliberately no cleanup. Because the type is part of the name, a PNG replaced by a JPEG is a new object rather than a stale one under the wrong extension. `invoicing::logo` owns the rest: `AssetPublisher::logo_url` answers the address without uploading (pure, which is what lets the page be rendered against it *before* the object exists), the fingerprint and that URL are recorded together in the `published_logo` metadata key, and `logo::publish` uploads only when the pair differs — an unchanged logo costs nothing, and a `public_base_url` repointed at another bucket re-uploads the same image, since a stale address is as wrong as a stale picture. The upload runs **after** the render succeeds, in both `send` and a republish, so a broken custom template costs the bucket no object at all. Only a logo `render::usable_logo` passes is ever uploaded — the verdict is reached once per document and threaded into `render_with_logo`, rather than decoded again per renderer. A failed upload is a **warning, never a failed send or a failed republish**: the document is re-rendered carrying the bytes inline (a page may only point at an object that is there), and the sentence travels as data the way republish warnings do — `SendOutcome.warnings`, `RepublishOutcome::warnings()`, `warnings` on the send response. `logo::set_company_logo` is the one writer both settings screens go through, because clearing the stored logo must clear the record with it: the objects stay where delivered pages point, but a document published afterwards must not still carry a mark the operator removed. A void's replacement notice keeps the letterhead by reading `logo::published_logo_url` back out of that record — guarded on the current `public_base_url`, so after a bucket move the notice omits the image rather than naming a decommissioned domain — which is what lets `voided_page_html` still need no settings and only ever point at an object a send actually put there
- Published invoices are static R2 objects at `i/{token}/index.html` and `i/{token}/invoice.pdf`, served under `public_base_url` (required for `invoice send`, no default; e.g. `https://billing.example.com/i`); the 16-character random token is the only access control. The address Nigel prints, returns and reports names the `index.html` object, not its directory — an edge rewrite that makes the directory form resolve is an option, never a requirement, because a link that depends on one is a 404 in a client's inbox and nothing in Nigel can see whether it is configured. `public_base_url` is checked where both send paths pass, `cli::invoice::build_clients`: no http(s) scheme or no host is a refusal by name before any client is constructed, and a path that does not end in `/i` is a warning computed once in `settings::invoicing_status` (so the CLI's `notice:` and `/api/status`'s `publicBaseUrlWarning` are one sentence). `optional_publisher` stays lenient — void and republish only need the upload, and refusing there would leave a live payment link up to protect the formatting of a URL neither prints
- No invoicing config has a built-in default: `invoice send` requires `mailgun_domain`, `from_email`, and `public_base_url` by name alongside the secrets. The three envelope keys are optional and each has an honest fallback, so none of them appears in `invoicing_status.missing`: `from_email` is the Mailgun From and nothing else, must be a **bare address** (a `name-addr` like `Acme LLC <billing@…>` is refused up front, because `format_address` composes the name and a nested one would fail at Mailgun *after* the Stripe link and the upload), and one off `mailgun_domain` **warns and sends** — a sending domain need not equal the address's domain, and only Mailgun knows which senders are verified. That warning is **data**, not a print: `build_clients` returns `SendClients { stripe, r2, mail, warnings }` and each surface renders it once per send (CLI stderr, TUI status line, `configWarnings` on the send response), because `eprintln!` inside a ratatui alternate screen corrupts the display. `from_name` is the display name and falls back to the database's `company_name`; `reply_to_email` is unconstrained and absent by default; `contact_email` is the page's direct-deposit line and falls back to `from_email`. A display name is header-encoded by `mailgun::format_address` (RFC 5322 `name-addr`, UTF-8 passed through because Mailgun's API is UTF-8). `validate_header_value` refuses every ASCII and C1 control plus `U+2028`/`U+2029` in **every** value composed into a header — the from address included — and names the value's actual source, so a control character in the business name blames the business name rather than a `from_name` the operator never set. It is header injection, answered over HTTP as a `409 send_misconfigured` beside `send_not_configured`. `src/invoicing/` never reads settings itself, so the CLI layer resolves all of this into an `EmailEnvelope` and passes the contact address into `send_invoice`
- A client's addresses live in `client_contacts`, one row per contact with a name and a title, exactly one flagged `is_billing`: a partial unique index gives *at most* one and the normalize step every write runs gives *at least* one whenever there is any, so "exactly one address is the billing recipient" is true by construction. `clients.email` was dropped and `Client.email` is a correlated-subquery projection of the billing contact, which is what keeps `require_email`, `{{CLIENT_EMAIL}}`, `format_client_list`, the wire shape and the parity fixtures unchanged; `set_billing_email` is the one writer `add_client` and `update_client` go through; the InvoiceShelf importer takes its `_unchecked` twin, because refusing somebody's years-old address would abort a whole migration over a value they cannot edit until it is imported — it copies verbatim and counts, which is what this importer already does with a date it cannot parse and what v8 does with the column it backfills. Each surface writes the client row and its addresses in **one** transaction (`add_client_within`/`update_client_within`/`set_contacts_within`, the `sync_all_report_within` split applied to writes), so a refused contact list leaves no client row and no half-applied rename. A comma-separated column was rejected: `require_email` and the Mailgun `To` have to know *which* address is which, and the source data carries a name and a title per address that a string column would discard. Writes are whole-list (`set_contacts`, the shape `update_invoice` uses for `items`), and `email` and `contacts` in one request is refused on both surfaces because applying both would make their order visible — on **presence**, not value, since `email: null` is a write too. Every recipient gets the same document and can pay it — one render, one message, `To` plus a comma-joined `cc` — while the published page names the billing contact alone, because it is a static object on a public URL
- A client's lifecycle is two operations with two sentences and no cascade between them. **Archive** is `clients.archived_at`, a nullable timestamp following `voided_at`'s derived-state precedent: hidden by default on all three lists (`ClientScope::{Active,All}`, `nigel client list --all`, the TUI's `A`, `GET /api/clients?includeArchived=true`), invisible to every invoice query and every report — `list_invoices` and `ar_aging_detail` are deliberately untouched, so archiving can never make money disappear from the aging report — and refusing only a **new** invoice, as `Conflict { code: "client_archived" }` from `ensure_client_active`, which `create_invoice` calls before its transaction opens — and which is also its existence check, since it reads the row. **Delete** is refused while any invoice of any status names the client, and all three front ends print `delete_blocker`'s own sentence through `NigelError::Blocked`'s `Display`: nothing in the invoicing modules writes that sentence itself, so a "Cannot delete" literal under `src/invoicing/`, `cli/client*.rs` or `server/routes/clients.rs` is a second copy of a rule that has one home. `cli::confirm_or_refuse` is the one confirmation all three destructive commands (`invoice void`, `invoice delete`, `client delete`) ask through, so they behave identically on a pipe: without `--yes` and without a terminal, they fail rather than guessing
- Invoicing external clients (`stripe.rs`, `r2.rs`, `mailgun.rs`) split request building/response parsing from transport so tests cover them without network access
- Platform binary detection: macOS = `nigel-universal-apple-darwin`, Linux x86_64 = `nigel-x86_64-unknown-linux-gnu`, Windows x86_64 = `nigel-x86_64-pc-windows-msvc.exe`
- `serve` is password-exempt in the dispatch pre-flight — it has no stdin to prompt on. It runs migrations itself at startup, but only when the database is unencrypted; an encrypted database stays locked until a client unlocks it over HTTP, and migrations run then
- `security_headers` sets `X-Content-Type-Options: nosniff` and `X-Frame-Options: DENY` on every response as **defaults** (`HeaderMap::entry().or_insert`, not `insert`), so a handler that set either one keeps its value. `DENY` refuses same-origin framing as well as cross-origin, and the invoice preview is a document the SPA renders in a sandboxed `<iframe>` of its own page, so `GET /api/invoices/{number}/preview` answers `SAMEORIGIN` for itself. Deciding that at the one route that needs it beats loosening the blanket value for every response; a test pins both halves
- Localhost is not a trust boundary for `serve`. Three layers, in middleware order: bind 127.0.0.1 only; reject any request whose `Host` (or `Origin`, when present) is not exactly `localhost`/`127.0.0.1`/`[::1]` on any port (403, blocks DNS rebinding); require the `nigel_session` cookie on every `/api` route (401). The per-run 32-byte token is compared in constant time, is never persisted, and never appears in a response body. Static assets are session-exempt because they carry no data
- An encrypted database leaves `serve` locked: `GET /api/ping`, `GET /api/status`, and `POST /api/unlock` answer; every other `/api` path — including the 404 fallback — returns 423 `locked` until `POST /api/unlock` validates the password (`db::validate_password`), adopts it (`db::set_db_password`), and runs the deferred migrations (`init_db`). A migration failure re-locks the process rather than leaving it half-unlocked. Unlock is process-wide, so one server run serves one database. The guard is layered over the whole `/api` router and exempts the three gate paths by name (`UNGATED_PATHS`), so a new endpoint is guarded by default rather than by remembering where to mount it. It probes `is_encrypted` per request rather than caching it at startup, because password management over HTTP can change that state mid-run
- `GET /api/status` reports invoicing configuration as key names only (`sendConfigured`, `syncConfigured`, `missing`) and **omits the whole object while the database is locked**. `/api/status` is one of the three ungated paths, so the block would otherwise tell an unauthenticated-but-local caller which integrations this installation has configured before anyone has passed the gate. The names are already public in `docs/invoicing.md`; the values never leave the process, and a unit test renders a fully-populated `InvoicingConfig`'s status and asserts none of them appears
- Failed unlock attempts are counted in memory (`AppState.unlock`): `attemptsRemaining` counts 3 down to 0 with no hard lockout, and from the third failure the server delays its response 1s, 2s, 4s… capped at 30s (`tokio::time::sleep`, never a thread sleep). `retryAfterMs` reports the delay already applied to that response. A success resets the counter; nothing is persisted
- Every `/api/settings/*` route is behind the locked guard, including `GET/PUT /api/settings/app`, which never opens the database. Nothing on the unlock screen reads app settings, and `password/change`/`password/remove` take the current password in the body, so an exemption would make them an unthrottled password oracle reachable without passing the gate. A wrong `currentPassword` draws down the *same* `AppState.unlock` budget as a failed unlock, so guessing costs the same either way. Forgotten-password recovery stays `nigel load` plus a restart
- Encrypting, decrypting, and switching data directory take `AppState.db_gate` for writing; every connection is opened under it for reading (`routes::with_conn`, and `routes::imports`'s own `blocking` helper, which opens connections itself). `encrypt_database`/`decrypt_database` finish by renaming the database file and deleting the `-wal`/`-shm` sidecars, which a live connection elsewhere would not survive. `AppState.db_path` is behind an `RwLock` for the same feature: a data-directory switch rebinds the running server, because rewriting settings.json alone would leave it serving the old books under the new directory's name. The switch also clears the password global (an encrypted target must come up locked), resets the unlock budget, and migrates an unencrypted target
- Guardrail failures carry a machine-readable reason alongside the human message. `NigelError::Blocked`/`DuplicateName`/`Conflict`/`NoTransactions` become 409s whose `details.reason` is one of `has_transactions`, `has_active_rules`, `duplicate_name`, `already_inactive`, `no_transactions`, plus the invoicing codes `has_invoices`, `void`, `not_draft`, `has_payments`, `already_void`, `no_balance`, `not_deletable`, with `count`/`name`/`month`/`status`/`total`/`paid` where those apply — `count` is omitted rather than zeroed for a reason that counts nothing; `NotFound` is 404 and `Invalid` is 400. The `Display` text is unchanged, so the CLI and TUI print exactly what they always did — the structure is additive, and clients render from the code rather than parsing the sentence
- The API's flag edit is idempotent where the TUI's is a toggle: `PATCH /api/transactions/:id` takes `flag: bool` and settles on that state, because a toggle sent twice over an unreliable connection lands somewhere nobody chose. `reviewer::toggle_transaction_flag` (the register's `f` key) is now expressed in terms of `set_transaction_flag`
- A browser import is three calls — `POST /api/imports/upload` (multipart, 25 MB `DefaultBodyLimit`, `.csv`/`.xlsx`/`.xls` only), `preview` (`import_file` with `dry_run`), `confirm` (snapshot → import → categorize → optional `saveProfile`, the same sequence as `import_manager.rs`). Uploads spool to `<data_dir>/tmp/uploads/<32 hex>/<sanitized name>` (dirs 0700, file 0600) so the `imports` row records the user's filename, not an id; they are purged after an hour at startup and on each upload, deleted after a successful confirm, and kept after a failed one so the same `uploadId` can be retried. `format` and `mapping` are mutually exclusive (400); a duplicate file and malformed rows are data, not errors; a missing cargo feature named as a format is 501, and the routes map parse failures (`NigelError::Csv`/`Other`) to 400 rather than the global mapping's 500
- PATCH bodies distinguish an absent field from an explicit `null` (`double_option`): absent leaves a column alone, `null` clears it. `categoryId: null` is the exception — a 400, since uncategorizing is what review undo is for
- Rule `match_type` and `regex` patterns are validated in the data layer (`rules::validate_match_type`), so `nigel rules add --match-type bogus` and an uncompilable regex now error instead of saving a rule the categorizer can never match. `accounts::add_account` likewise validates the account type and rejects duplicate names for the CLI, which previously bypassed both by inserting directly
- The HTTP API is stricter with date parameters than the CLI is. `cli::parse_month_opt` answers a malformed `--month` with `(None, None)`, which on a report endpoint would silently widen the query to the whole database; over HTTP `month` must be `YYYY-MM`, `from`/`to` must be zero-padded `YYYY-MM-DD`, and anything else is a 400. A parameter a route does not support (`from` on expenses, `month` on tax, `account` on anything but the register) is also a 400 rather than ignored. `from`/`to` remain a pair, enforced in the route layer so the failure is a 400 and not `date_filter`'s error surfacing as a 500
- Unknown `account` on `/api/reports/register` is a 404. `reports::get_register` itself reports an unknown account as an empty register, which over HTTP is indistinguishable from an account with no transactions, so the route checks existence first
- The database password never reaches a log, an error message, or `Debug` output: request bodies hold it in `server::secret::Secret` (redacted `Debug`, zeroized on drop), and errors from opening the database on the unlock path are replaced with a fixed message because rusqlite renders `PRAGMA key = '<password>'` as literal SQL inside `SqlInputError`
- The session middleware wraps the `/api` router with `.layer`, not `.route_layer`, so it also covers that router's 404 fallback — otherwise unauthenticated requests to unknown `/api` paths would skip the check
- Web assets are embedded with rust-embed's `debug-embed` on, so debug and release builds behave identically and neither reads `web/dist` at runtime. `web/dist` is generated by the vite build and gitignored; `build.rs` seeds it from the committed `web/placeholder/index.html` when no `index.html` is present, so `cargo build` works without node
- `build.rs` also emits `cargo:rerun-if-changed` for `web/dist`. This is load bearing, not decorative: `debug-embed` controls *when* assets are baked in, not when cargo reconsiders them, and rust-embed's proc macro cannot emit the key itself — without the build script a fresh `npm run build` followed by `cargo run` serves the previously embedded bytes. Both CI and the release workflow build `web/` before any cargo step for the same reason; a release built without node would ship the placeholder
- Every visual change ships through `@nigel/ui` (see "Component-First UI Workflow" below). All server access goes through `web/apps/app/src/api/` — a guard test enforces it. No `window.confirm`: confirmations use `wc-confirm`/`confirmDialog()`
- A 401 carrying `invalid_password` is a mistyped database password, not a dead session, and must never raise `appUnauthorized` — that would show a "session expired" banner to someone who simply typed the wrong key
- `wc-password-form` is **one** password operation, not a password panel: a `fieldset` under a `legend` carrying the operation's name (as an `h3`, which `legend`'s content model allows, so the operation is one thing to heading navigation and to form grouping rather than two that must be kept in step), its own sentence about what it does, its own message line, and its own submit. The settings screen stacks two of them on an encrypted database, and change and remove each collect a field called "Current password" — in a flat stack only position tells them apart, and position is not something a screen reader conveys. `mode="remove"` carries the destructive treatment (danger border, danger heading, danger button) and the screen puts `confirmDialog({ variant: 'danger' })` in front of it; change is not confirmed, being reversible by changing it back. The screen holds a failure as one `passwordFailure { mode, message }` — the message and the operation it belongs to are only ever correct together — and words a non-`ApiError` failure per mode, since a single "Could not change the password." under the Remove form is the confusion the split exists to remove. It renders under the operation that produced it, falling back to the first form when that operation is no longer on screen (another session encrypted or decrypted the books): a stale-but-visible failure beats a vanished one
- The web UI is IBM Plex Mono throughout; the **client-facing invoice keeps a text face** and this is deliberate, not an oversight to tidy up. `AssetPublisher` uploads `index.html` and `invoice.pdf` and nothing else, so the published page has nowhere to get a font; a CDN link would make someone else's browser fetch from a third party when they open a bill from us; `pdf.rs` renders through printpdf's built-in faces, so changing the HTML half alone would split a document whose two halves come from one seam (`invoicing::render::render_invoice`); and the invoice is read by people who have never seen the CLI, where a terminal face is a statement about us in a document that is about them. `web/placeholder/index.html` keeps its system stack too — it is served when no `web/dist` exists, so there is no font to reference
- IBM Plex Mono has no glyph for `✗ ⟳ ◑ ● ◆ ▲ ⊘ ◻` (a property of the upstream font, not of our subset, so no subset range can carry them). None of the eight is typed as a character: `wc-invoice-status` draws the six statuses as `wc-icon-status-*`, `wc-send-dialog`'s step trace and `wc-reconciliation-history`'s result column draw theirs from `wc-icon-check`/`wc-icon-close`/`wc-icon-refresh`/`wc-icon-dot`. All of them are `WcIconBase` subclasses drawn with its `inline` attribute — the base's own 1em mode, so a mark set in text asks for it the same way everywhere rather than each component repeating a `--nc-icon-size: 1em` line, and an icon without the attribute is the unchanged 20px token an empty state and the gallery still use — and they inherit `currentColor`, so each mark tracks the type beside it and keeps its state's colour. All are decorative: the word beside each one (the status, the step's `sr-only` state, `Reconciled`/`Discrepancy`) is what announces. Each mark is a **template**, not a tag name resolved at render time, so Lit updates it in place — a send polls and an invoice row re-renders with its list, and rebuilding the element each tick would re-upgrade a custom element somebody is watching — and the status lookup is a `Map`, because `invoices.status` has no CHECK constraint and an object indexed by `constructor` answers with an inherited function. `packages/ui/src/__tests__/mono-glyph-coverage.test.ts` sweeps `packages/ui/{src,preview}`, `packages/theme/src` and `apps/app/src` across every extension that reaches a browser (`.ts`, `.js`, `.mjs`, `.cjs`, `.css`, `.html`, `.json`, `.txt`) and names the icon that replaces each character, because one typed back in renders plausibly on the author's machine and breaks no other test
- The print palette wins by **specificity, not source order**. `printCss` redefines the tokens under `:root:root` (0,2,0) because the two dark selectors — `:root:not(.light-mode)` and `:root.dark-mode` — are also (0,2,0), and a bare `:root` (0,1,0) loses to them wherever it sits in the sheet. Composing print last only settles a tie. Simplifying the doubled selector back puts dark pages on the printer for anyone whose OS prefers dark, which is what it did before TASK-75
- A net preset on the invoice form fills the terms field, and the paid date takes the same picker with no presets. The reference invoices print `2026-09-05 (Net 30)` beside the due date, so a Net 30 that left the terms empty would raise an invoice whose page contradicts the form; the prefill is **non-destructive** — `prefilledTerms` writes only over an empty field or the exact text this form put there itself, which the component tracks as provenance rather than recognising by value: a `Net 30` loaded off the invoice or typed by hand matches the label the form would have written and is still not the form's to rewrite, and editing the field by hand ends the claim. Switching to a custom date or to none clears the form's own label rather than leaving a period the dates no longer describe. `wc-payment-form`'s date is the same `type="date"` control and deliberately has **no** term presets: a payment landed on the day it landed, and there is no period to count it from
- `wc-toast` pins its region to the **bottom-right corner** of the viewport with both inline insets set (`inset-inline: <gutter>`) and no translate, so the region is the viewport minus its gutters, a `%` width inside it is viewport-relative, and the placement never depends on the element's static position — which, for a fixed box whose insets are all `auto` inside the shell's flex container, is the container's top-left corner, over the sidebar. Both other corners are occupied: the sidebar owns the left edge, the header the top. Up to `MAX_VISIBLE_TOASTS` (3) share the column, newest at the bottom, each on its own timer, and past that the oldest goes. A long message wraps against `min(360px, 100%)` rather than widening the chip off-screen. A toast that neither expires nor carries an action — `duration: 0` with no action, which is what `nigel-app`'s status error dispatches — carries a close button, because with a stack there is no next toast to replace it and it would otherwise hold a slot and intercept clicks for the session; `dismiss(id?)` closes one or all. The region is the **polite** live region and stays polite whatever is in it: a danger toast is its own `role="alert"`, so it announces assertively without escalating the info toasts beside it, and there is no `aria-atomic`, which would re-read the whole column on every arrival. The region is promoted into the top layer on **arrival only** — an expiry leaves it where it is, so toasts already on screen when a modal opened stay behind it until the next arrival, which is the price of never lifting a toast the user has already read above a modal it never covered. Where the toasts land is asserted as geometry, not as declarations: jsdom has no layout engine, so `preview/css-geometry.ts` resolves a stylesheet's cascade, shorthands, `var()` fallbacks and `calc()`/`min()`/`max()` into viewport pixels and the tests assert the resulting boxes
- `wc-money` renders the sign as a literal `-` rather than conveying it by color alone as `tui::money_span` does. A terminal can rely on red-versus-green; a browser cannot (WCAG 1.4.1), and red/green is the pair color-vision deficiency flattens most. Its formatting is tested against the same vectors `src/fmt.rs` asserts, so the two front ends cannot disagree about how an amount reads
- **The content area is a column, and so is every screen in it.** `wc-app-shell` stretches whatever is in its default slot to the whole content area, each screen stacks its children in a flex column, and a component that should own the height nothing else claimed says `flex: 1 1 auto` — `wc-empty-state`, `wc-manager-layout`, `wc-register-table`. That is the whole of how an empty state ends up in the middle of the page: it centres itself in the box it is given, and the box is the leftover height rather than the height of its own text. A screen's reading width therefore belongs to the things being read — the review card, the form, the summary panel — and never to the screen's own box, or an empty state inherits the column and sits left of centre. `apps/app/src/__tests__/screen-layout.test.ts` fails a screen that stacks its children some other way, so a new screen gets the centring without asking for it. Where the box is content-sized — a panel body, a table's own empty row — the same element grows into nothing and reads exactly as it did. **On paper every one of those boxes is a block again**: a flex container is not required to fragment across sheets, and Safari and older Chromium slice through a row rather than break between two. The shell's print block does it for every screen at once through `.content ::slotted(*)` — a normal declaration in the outer tree beats the inner tree's own `:host` — and `wc-manager-layout` and `wc-register-table` carry their own, being a shadow root deeper than the shell can reach
- The sidebar is one boolean with two appearances, and the width decides which. `sidebarCollapsed` is **nigel-app's** — the sidebar is its slotted child and only it can pass `collapsed` down — so `wc-app-shell` renders the toggle, asks by `nc-sidebar-toggle`, and re-renders when the answer comes back as a property; a shell that also flipped its own copy would be a second source of truth for one boolean. Above `48rem` collapsed is the 56px rail `wc-nav-sidebar` already drew. Below it the rail would still be wrong — 232px of a 390px viewport leaves a 40-character column, and 56px of icons is a poor trade on a phone — so the shell lifts the slotted sidebar out of the flow (`position: fixed`, `translateX(-100%)` while collapsed, a backdrop over the content while open) and the sidebar cancels its own rail styling at the same width, which is what makes the thing that slides in the full nav with its words. Escape, the backdrop and choosing a screen all close the drawer; on a wide viewport none of them touches the docked sidebar, because it covers nothing. The breakpoint is named once (`NARROW_QUERY`) and `narrowViewport()` is how the app seeds the state without repeating the width. jsdom's `matchMedia` answers false to everything, so the query is injectable — `color-mode.ts`'s own arrangement — and the CSS is asserted by reading the adopted stylesheet, as the register's fill rules are

## Project Structure

```
src/
  lib.rs                # Library root — exposes all modules as the `nigel` crate
  main.rs               # Binary entry point: clap parse, panic hook, command dispatch
  cli/                  # CLI subcommands
    mod.rs              # Clap structs (Cli, Commands, subcommands), shared helpers
    dashboard.rs        # nigel (no args) — interactive dashboard with inline screen transitions
    init.rs             # nigel init
    demo.rs             # nigel demo (sample data + setup_demo for isolated demo DB)
    onboarding.rs       # First-run onboarding TUI (animated logo, name collection, action picker)
    account_manager.rs  # TUI account management screen (list, add, rename, delete)
    accounts.rs         # nigel accounts add/list/rename/delete — a printing wrapper over top-level `accounts`
    categories.rs       # nigel categories list/add/rename/delete — a printing wrapper over top-level `categories`
    category_manager.rs # TUI category management screen (list, add, edit, delete)
    import.rs           # nigel import
    import_manager.rs   # TUI import screen (file path + account selector + result)
    undo.rs             # nigel undo (undo last import) — a printing wrapper over top-level `imports`
    undo_manager.rs     # TUI undo screen (confirm + execute from dashboard)
    categorize.rs       # nigel categorize
    recategorize.rs     # nigel recategorize (bulk category reassignment by IDs or filters)
    rules.rs            # nigel rules add/list/update/delete/test — a printing wrapper over top-level `rules`
    rules_manager.rs    # TUI rules screen (scrollable list + delete)
    password.rs         # nigel password set/change/remove — a printing wrapper over top-level `password`
    password_manager.rs # TUI password management screen (set/change/remove via settings)
    settings_manager.rs # TUI settings screen (business name + password management)
    reconcile_manager.rs # TUI reconcile screen (account/month/balance form + result)
    load_manager.rs     # TUI load screen (data directory switcher with reload)
    review.rs           # nigel review
    report/             # nigel report (unified view/export command)
      mod.rs            # Dispatch: view vs export, TTY detection, text export
      view.rs           # Ratatui interactive report views (scrollable, colored)
    browse.rs           # nigel browse (interactive browsers)
    client.rs           # nigel client add/list/show/edit/delete/archive/unarchive (+ --contact)
    client_manager.rs   # TUI client screen (list, add, edit, delete, archive, contacts)
    invoice.rs          # nigel invoice new/edit/void/list/show/preview/send/sync/pay/aging/import/template
    invoice_manager.rs  # TUI invoice screen (list, detail, send/pay/void)
    snake.rs            # Snake game easter egg (ratatui, accessible from dashboard)
    splash.rs           # Splash screen (1.5s animated logo + particles, shown on launch)
    goodbye.rs          # Goodbye screen (reverse logo animation + particles, shown on quit)
    export.rs           # PDF export helpers (per-function feature-gated behind "pdf")
    reconcile.rs        # nigel reconcile
    load.rs             # nigel load (switch data directory)
    backup.rs           # nigel backup (database backup)
    restore.rs          # nigel restore (restore database from backup)
    serve.rs            # nigel serve (feature gate + pre-flight, delegates to src/server/)
    status.rs           # nigel status (show active DB + stats)
    update.rs           # nigel update — a printing wrapper over top-level `updater` (version check + self-replace from GitHub Releases)
  server/               # Web server (feature-gated behind "serve")
    mod.rs              # Tokio runtime, router assembly, middleware order, graceful shutdown
    auth.rs             # Session token, Host/Origin validation, cookie parsing, /auth handler
    error.rs            # ApiError + ApiErrorCode, JSON error envelope, status mapping
    extract.rs          # ApiJson/ApiPath: axum extractors whose rejections use the error envelope
    state.rs            # AppState (db path, session token, build features, unlock gate, db file gate)
    secret.rs           # Secret: redacted-Debug, zeroize-on-drop password wrapper
    uploads.rs          # Spooled browser uploads: sanitize, store 0600/0700, resolve by id, purge
    static_files.rs     # rust-embed hosting of web/dist with SPA index fallback
    fixture_capture.rs  # cfg(test) only — captures the web UI's report and invoicing fixtures
    testutil.rs         # Test-only: temp/seeded databases, session router, JSON request helpers
    routes/
      mod.rs            # API router; GET /api/ping, JSON 404 fallback, guarded data_router(), with_conn()
      status.rs         # GET /api/status, POST /api/unlock, locked guard middleware
      reports.rs        # The eight GET /api/reports/* endpoints + query param validation
      exports.rs        # The eight GET /api/exports/* downloads (format=pdf|text)
      accounts.rs       # GET/POST /api/accounts, PATCH/DELETE /api/accounts/:id
      categories.rs     # GET/POST /api/categories, PATCH/DELETE /api/categories/:id
      rules.rs          # GET/POST /api/rules, PATCH/DELETE /api/rules/:id, POST /api/rules/test
      imports.rs        # GET /api/imports, /api/imports/formats, /api/csv-profiles; DELETE /api/imports/:id; POST upload/preview/confirm
      transactions.rs   # PATCH /api/transactions/:id, POST /api/categorize
      review.rs         # GET /api/review/queue, GET /api/review/:id, POST apply/undo
      reconcile.rs      # POST /api/reconcile, GET /api/reconciliations
      clients.rs        # GET/POST /api/clients, GET/PATCH/DELETE /api/clients/:id
      invoices.rs       # GET/POST /api/invoices, GET/PATCH /api/invoices/:number(+/void, /pay,
                        #   /send, /preview[.pdf]), /aging, /next-number, POST /api/invoices/sync
      settings.rs       # GET/PUT /api/settings/app, PUT company-name, POST data-dir, POST password/{set,change,remove}
  db.rs                 # SQLite schema, connection, category seeding
  migrations.rs          # Schema migration runner (version tracking, sequential up() functions)
  models.rs             # Structs (Account, Transaction, Rule, ParsedRow, etc.)
  importer.rs           # ImporterKind enum, format detection, CSV/XLSX parsing
  accounts.rs           # Account data layer (add/list/rename/delete, delete_blocker)
  categories.rs         # Category (chart of accounts) data layer (add/list/rename/delete, delete_blocker)
  rules.rs              # Categorization rules data layer (list/add/update/deactivate/test)
  imports.rs            # Import history data layer (list_imports, get_last_import)
  backup.rs             # Database backup/restore (rusqlite Backup API)
  password.rs           # Database encryption data layer (set/change/remove, rekey)
  updater.rs            # GitHub Releases version check + self-replace
  clock.rs              # The app's one clock read (`today()`)
  invoicing/            # Invoicing (A/R): clients, invoices, publish, email, payment sync
    mod.rs              # Module declarations
    clients.rs          # Client data layer
    invoices.rs         # Invoice/line-item/payment data layer, numbering, status, aging
    document.rs         # MoneySummary/MoneyLine + address_lines — what both documents say, decided once
    gateway.rs          # PaymentGateway / AssetPublisher / Mailer traits + shared types
    stripe.rs           # Stripe payment links and paid checkout sessions
    r2.rs               # Cloudflare R2 publisher (S3 API via rusty-s3)
    mailgun.rs          # Mailgun sender (HTML body + PDF attachment)
    render_html.rs      # Invoice HTML rendering ({{KEY}} expansion, PayButton, Branding, template loading)
    logo.rs             # The letterhead logo as an object beside the page (once per distinct image)
    render.rs           # render_invoice — the HTML+PDF pair send publishes and preview writes
    republish.rs        # Best-effort re-publish of a paid invoice's page and PDF
    templates/          # invoice.html
    send.rs             # Send orchestration (link → render → publish → email → publish mark)
    sync.rs             # Pull Stripe payments into invoice_payments
    void.rs             # Void + best-effort teardown (deactivate link, republish voided page)
    import_invoiceshelf.rs # One-time InvoiceShelf SQLite import
    wiring.rs           # Assembles a send/republish from settings — the one place invoicing meets `settings`
  categorizer.rs        # Rules engine (categorize_transactions)
  reviewer.rs           # Interactive review flow
  reports/              # Report data functions (pnl, expenses, tax, cashflow, balance, flagged, k1_prep) + RegisterFilters
    mod.rs              # Report data functions, ReportKind/DateGranularity, register_range_label/register_subtitle
    text.rs             # comfy_table text formatters (used for stdout + text file export + HTTP exports)
  browser.rs            # Interactive register browser (ratatui, row selection, inline editing, flag toggle, scroll navigation)
  effects.rs            # Shared gradient/particle effects (used by splash, onboarding, snake)
  tui.rs                # Shared ratatui helpers (styles, money_span, wrap_text, ReportView trait, run_report_view)
  pdf.rs                # PDF rendering engine (feature-gated behind "pdf")
  reconciler.rs         # Monthly reconciliation
  settings.rs           # Settings management (~/.config/nigel/)
  fmt.rs                # Number formatting helpers
  error.rs              # Error types
build.rs                # Seeds web/dist from the placeholder; rerun-if-changed for rust-embed
web/                    # npm workspace for the SPA (see web/README.md)
  package.json          # Workspace root: packages/*, apps/* — no turbo
  placeholder/
    index.html          # Committed "SPA not built" fallback (build.rs copies it into dist/)
  dist/                 # Generated by `npm run build`, gitignored, embedded by rust-embed
  packages/
    theme/              # @nigel/theme — token modules composed into one CSSResult + a plain .css
      src/tokens/       # color, gradient, typography, spacing, radius, shadow, motion
      src/global.ts     # ::part() overrides for wa-* primitives (document-level)
      src/print.ts      # @media print — composed last so it wins over dark mode
      scripts/build-css.js
    ui/                 # @nigel/ui — wc-* components + preview harness
      src/components/   # wc-app-shell, wc-nav-sidebar, wc-toast, wc-confirm, wc-money,
                        #   wc-empty-state, wc-spinner, wc-panel, the register, review,
                        #   import, report and manager families (wc-report-table,
                        #   wc-export-links, wc-link-grid, wc-manager-*), the reconcile
                        #   family (wc-reconcile-form, wc-reconcile-result,
                        #   wc-reconciliation-history, wc-import-history),
                        #   wc-category-picker, wc-shortcut-help, and the invoicing family
                        #   (wc-invoice-status, wc-invoice-table,
                        #   wc-invoice-summary, wc-payment-list, wc-aging-bars,
                        #   wc-invoice-preview, wc-line-items, wc-invoice-form,
                        #   wc-payment-form, wc-client-form, wc-send-dialog),
                        #   plus wc-snake + snake-engine.ts (the easter egg)
                        #   (each with .preview.ts + .test.ts)
      src/icons/        # WcIconBase + the icon set (screens, actions, invoice statuses)
      preview/          # Glob manifest, query-string router, axe/print/controls/layout
                        #   suites (port 9090), css-geometry.ts (stylesheet -> viewport
                        #   pixels, for layout assertions)
  apps/
    app/                # @nigel/app — composition only
      src/api/          # types.ts, client.ts — the ONLY module that talks to the server
      src/screens/      # registry.ts (Record<ScreenId, ScreenDef>), context.ts, hash-route.ts,
                        #   one module per screen (+ a *-data.ts sibling for its pure logic)
      src/state/        # app-store.ts (status/locked/company signals)
      src/__fixtures__/ # Report and invoicing fixtures captured from a seeded DB (figure-parity tests)
      src/mixins/       # signal-watcher.ts — the @lit-labs/signals seam
      src/snake-trigger.ts # The hidden `s` key and the focus guards on it
      src/components/   # nigel-app.ts — root container
docs/
  api.md                # HTTP API inventory (endpoints, error envelope, security model)
  importers.md          # Importer format specifications and authoring guide
  invoicing.md          # Invoicing setup (secrets, R2/Cloudflare routing) and command reference
  walkthrough.md        # Guided tour using demo data
  skills.md             # Claude skills documentation
```

<!-- BACKLOG.MD GUIDELINES START -->
# Instructions for the usage of Backlog.md CLI Tool

## Backlog.md: Comprehensive Project Management Tool via CLI

### Assistant Objective

Efficiently manage all project tasks, status, and documentation using the Backlog.md CLI, ensuring all project metadata
remains fully synchronized and up-to-date.

### Core Capabilities

- ✅ **Task Management**: Create, edit, assign, prioritize, and track tasks with full metadata
- ✅ **Search**: Fuzzy search across tasks, documents, and decisions with `backlog search`
- ✅ **Acceptance Criteria**: Granular control with add/remove/check/uncheck by index
- ✅ **Definition of Done checklists**: Per-task DoD items with add/remove/check/uncheck
- ✅ **Board Visualization**: Terminal-based Kanban board (`backlog board`) and web UI (`backlog browser`)
- ✅ **Git Integration**: Automatic tracking of task states across branches
- ✅ **Dependencies**: Task relationships and subtask hierarchies
- ✅ **Documentation & Decisions**: Structured docs and architectural decision records
- ✅ **Export & Reporting**: Generate markdown reports and board snapshots
- ✅ **AI-Optimized**: `--plain` flag provides clean text output for AI processing

### Why This Matters to You (AI Agent)

1. **Comprehensive system** - Full project management capabilities through CLI
2. **The CLI is the interface** - All operations go through `backlog` commands
3. **Unified interaction model** - You can use CLI for both reading (`backlog task 1 --plain`) and writing (
   `backlog task edit 1`)
4. **Metadata stays synchronized** - The CLI handles all the complex relationships

### Key Understanding

- **Tasks** live in `backlog/tasks/` as `task-<id> - <title>.md` files
- **You interact via CLI only**: `backlog task create`, `backlog task edit`, etc.
- **Use `--plain` flag** for AI-friendly output when viewing/listing
- **Never bypass the CLI** - It handles Git, metadata, file naming, and relationships

---

# ⚠️ CRITICAL: NEVER EDIT TASK FILES DIRECTLY. Edit Only via CLI

**ALL task operations MUST use the Backlog.md CLI commands**

- ✅ **DO**: Use `backlog task edit` and other CLI commands
- ✅ **DO**: Use `backlog task create` to create new tasks
- ✅ **DO**: Use `backlog task edit <id> --check-ac <index>` to mark acceptance criteria
- ❌ **DON'T**: Edit markdown files directly
- ❌ **DON'T**: Manually change checkboxes in files
- ❌ **DON'T**: Add or modify text in task files without using CLI

**Why?** Direct file editing breaks metadata synchronization, Git tracking, and task relationships.

---

## 1. Source of Truth & File Structure

### 📖 **UNDERSTANDING** (What you'll see when reading)

- Markdown task files live under **`backlog/tasks/`** (drafts under **`backlog/drafts/`**)
- Files are named: `task-<id> - <title>.md` (e.g., `task-42 - Add GraphQL resolver.md`)
- Project documentation is in **`backlog/docs/`**
- Project decisions are in **`backlog/decisions/`**

### 🔧 **ACTING** (How to change things)

- **All task operations MUST use the Backlog.md CLI tool**
- This ensures metadata is correctly updated and the project stays in sync
- **Always use `--plain` flag** when listing or viewing tasks for AI-friendly text output

---

## 2. Common Mistakes to Avoid

### ❌ **WRONG: Direct File Editing**

```markdown
# DON'T DO THIS:

1. Open backlog/tasks/task-7 - Feature.md in editor
2. Change "- [ ]" to "- [x]" manually
3. Add notes or final summary directly to the file
4. Save the file
```

### ✅ **CORRECT: Using CLI Commands**

```bash
# DO THIS INSTEAD:
backlog task edit 7 --check-ac 1  # Mark AC #1 as complete
backlog task edit 7 --notes "Implementation complete"  # Add notes
backlog task edit 7 --final-summary "PR-style summary"  # Add final summary
backlog task edit 7 -s "In Progress" -a @agent-k  # Multiple commands: change status and assign the task when you start working on the task
```

---

## 3. Understanding Task Format (Read-Only Reference)

⚠️ **FORMAT REFERENCE ONLY** - The following sections show what you'll SEE in task files.
**Never edit these directly! Use CLI commands to make changes.**

### Task Structure You'll See

```markdown
---
id: task-42
title: Add GraphQL resolver
status: To Do
assignee: [@sara]
labels: [backend, api]
---

## Description

Brief explanation of the task purpose.

## Acceptance Criteria

<!-- AC:BEGIN -->

- [ ] #1 First criterion
- [x] #2 Second criterion (completed)
- [ ] #3 Third criterion

<!-- AC:END -->

## Definition of Done

<!-- DOD:BEGIN -->

- [ ] #1 Tests pass
- [ ] #2 Docs updated

<!-- DOD:END -->

## Implementation Plan

1. Research approach
2. Implement solution

## Implementation Notes

Progress notes captured during implementation.

## Final Summary

PR-style summary of what was implemented.
```

### How to Modify Each Section

| What You Want to Change | CLI Command to Use                                       |
|-------------------------|----------------------------------------------------------|
| Title                   | `backlog task edit 42 -t "New Title"`                    |
| Status                  | `backlog task edit 42 -s "In Progress"`                  |
| Assignee                | `backlog task edit 42 -a @sara`                          |
| Labels                  | `backlog task edit 42 -l backend,api`                    |
| Description             | `backlog task edit 42 -d "New description"`              |
| Add AC                  | `backlog task edit 42 --ac "New criterion"`              |
| Add DoD                 | `backlog task edit 42 --dod "Ship notes"`                |
| Check AC #1             | `backlog task edit 42 --check-ac 1`                      |
| Check DoD #1            | `backlog task edit 42 --check-dod 1`                     |
| Uncheck AC #2           | `backlog task edit 42 --uncheck-ac 2`                    |
| Uncheck DoD #2          | `backlog task edit 42 --uncheck-dod 2`                   |
| Remove AC #3            | `backlog task edit 42 --remove-ac 3`                     |
| Remove DoD #3           | `backlog task edit 42 --remove-dod 3`                    |
| Add Plan                | `backlog task edit 42 --plan "1. Step one\n2. Step two"` |
| Add Notes (replace)     | `backlog task edit 42 --notes "What I did"`              |
| Append Notes            | `backlog task edit 42 --append-notes "Another note"` |
| Add Final Summary       | `backlog task edit 42 --final-summary "PR-style summary"` |
| Append Final Summary    | `backlog task edit 42 --append-final-summary "Another detail"` |
| Clear Final Summary     | `backlog task edit 42 --clear-final-summary` |

---

## 4. Defining Tasks

### Creating New Tasks

**Always use CLI to create tasks:**

```bash
# Example
backlog task create "Task title" -d "Description" --ac "First criterion" --ac "Second criterion"
```

### Title (one liner)

Use a clear brief title that summarizes the task.

### Description (The "why")

Provide a concise summary of the task purpose and its goal. Explains the context without implementation details.

### Acceptance Criteria (The "what")

**Understanding the Format:**

- Acceptance criteria appear as numbered checkboxes in the markdown files
- Format: `- [ ] #1 Criterion text` (unchecked) or `- [x] #1 Criterion text` (checked)

**Managing Acceptance Criteria via CLI:**

⚠️ **IMPORTANT: How AC Commands Work**

- **Adding criteria (`--ac`)** accepts multiple flags: `--ac "First" --ac "Second"` ✅
- **Checking/unchecking/removing** accept multiple flags too: `--check-ac 1 --check-ac 2` ✅
- **Mixed operations** work in a single command: `--check-ac 1 --uncheck-ac 2 --remove-ac 3` ✅

```bash
# Examples

# Add new criteria (MULTIPLE values allowed)
backlog task edit 42 --ac "User can login" --ac "Session persists"

# Check specific criteria by index (MULTIPLE values supported)
backlog task edit 42 --check-ac 1 --check-ac 2 --check-ac 3  # Check multiple ACs
# Or check them individually if you prefer:
backlog task edit 42 --check-ac 1    # Mark #1 as complete
backlog task edit 42 --check-ac 2    # Mark #2 as complete

# Mixed operations in single command
backlog task edit 42 --check-ac 1 --uncheck-ac 2 --remove-ac 3

# ❌ STILL WRONG - These formats don't work:
# backlog task edit 42 --check-ac 1,2,3  # No comma-separated values
# backlog task edit 42 --check-ac 1-3    # No ranges
# backlog task edit 42 --check 1         # Wrong flag name

# Multiple operations of same type
backlog task edit 42 --uncheck-ac 1 --uncheck-ac 2  # Uncheck multiple ACs
backlog task edit 42 --remove-ac 2 --remove-ac 4    # Remove multiple ACs (processed high-to-low)
```

### Definition of Done checklist (per-task)

Definition of Done items are a second checklist in each task. Defaults come from `definition_of_done` in the project config file (`backlog/config.yml`, `.backlog/config.yml`, or `backlog.config.yml`) or from Web UI Settings, and can be disabled per task.

**Managing Definition of Done via CLI:**

```bash
# Add DoD items (MULTIPLE values allowed)
backlog task edit 42 --dod "Run tests" --dod "Update docs"

# Check/uncheck DoD items by index (MULTIPLE values supported)
backlog task edit 42 --check-dod 1 --check-dod 2
backlog task edit 42 --uncheck-dod 1

# Remove DoD items by index
backlog task edit 42 --remove-dod 2

# Create without defaults
backlog task create "Feature" --no-dod-defaults
```

**Key Principles for Good ACs:**

- **Outcome-Oriented:** Focus on the result, not the method.
- **Testable/Verifiable:** Each criterion should be objectively testable
- **Clear and Concise:** Unambiguous language
- **Complete:** Collectively cover the task scope
- **User-Focused:** Frame from end-user or system behavior perspective

Good Examples:

- "User can successfully log in with valid credentials"
- "System processes 1000 requests per second without errors"
- "CLI preserves literal newlines in description/plan/notes/final summary; `\\n` sequences are not auto‑converted"

Bad Example (Implementation Step):

- "Add a new function handleLogin() in auth.ts"
- "Define expected behavior and document supported input patterns"

### Task Breakdown Strategy

1. Identify foundational components first
2. Create tasks in dependency order (foundations before features)
3. Ensure each task delivers value independently
4. Avoid creating tasks that block each other

### Task Requirements

- Tasks must be **atomic** and **testable** or **verifiable**
- Each task should represent a single unit of work for one PR
- **Never** reference future tasks (only tasks with id < current task id)
- Ensure tasks are **independent** and don't depend on future work

---

## 5. Implementing Tasks

### 5.1. First step when implementing a task

The very first things you must do when you take over a task are:

* set the task in progress
* assign it to yourself

```bash
# Example
backlog task edit 42 -s "In Progress" -a @{myself}
```

### 5.2. Review Task References and Documentation

Before planning, check if the task has any attached `references` or `documentation`:
- **References**: Related code files, GitHub issues, or URLs relevant to the implementation
- **Documentation**: Design docs, API specs, or other materials for understanding context

These are visible in the task view output. Review them to understand the full context before drafting your plan.

### 5.3. Create an Implementation Plan (The "how")

Previously created tasks contain the why and the what. Once you are familiar with that part you should think about a
plan on **HOW** to tackle the task and all its acceptance criteria. This is your **Implementation Plan**.
First do a quick check to see if all the tools that you are planning to use are available in the environment you are
working in.
When you are ready, write it down in the task so that you can refer to it later.

```bash
# Example
backlog task edit 42 --plan "1. Research codebase for references\n2Research on internet for similar cases\n3. Implement\n4. Test"
```

## 5.4. Implementation

Once you have a plan, you can start implementing the task. This is where you write code, run tests, and make sure
everything works as expected. Follow the acceptance criteria one by one and MARK THEM AS COMPLETE as soon as you
finish them.

### 5.5 Implementation Notes (Progress log)

Use Implementation Notes to log progress, decisions, and blockers as you work.
Append notes progressively during implementation using `--append-notes`:

```
backlog task edit 42 --append-notes "Investigated root cause" --append-notes "Added tests for edge case"
```

```bash
# Example
backlog task edit 42 --notes "Initial implementation done; pending integration tests"
```

### 5.6 Final Summary (PR description)

When you are done implementing a task you need to prepare a PR description for it.
Because you cannot create PRs directly, write the PR as a clean summary in the Final Summary field.

**Quality bar:** Write it like a reviewer will see it. A one‑liner is rarely enough unless the change is truly trivial.
Include the key scope so someone can understand the impact without reading the whole diff.

```bash
# Example
backlog task edit 42 --final-summary "Implemented pattern X because Reason Y; updated files Z and W; added tests"
```

**IMPORTANT**: Do NOT include an Implementation Plan when creating a task. The plan is added only after you start the
implementation.

- Creation phase: provide Title, Description, Acceptance Criteria, and optionally labels/priority/assignee.
- When you begin work, switch to edit, set the task in progress and assign to yourself
  `backlog task edit <id> -s "In Progress" -a "..."`.
- Think about how you would solve the task and add the plan: `backlog task edit <id> --plan "..."`.
- After updating the plan, share it with the user and ask for confirmation. Do not begin coding until the user approves the plan or explicitly tells you to skip the review.
- Append Implementation Notes during implementation using `--append-notes` as progress is made.
- Add Final Summary only after completing the work: `backlog task edit <id> --final-summary "..."` (replace) or append using `--append-final-summary`.

## Phase discipline: What goes where

- Creation: Title, Description, Acceptance Criteria, labels/priority/assignee.
- Implementation: Implementation Plan (after moving to In Progress and assigning to yourself) + Implementation Notes (progress log, appended as you work).
- Wrap-up: Final Summary (PR description), verify AC and Definition of Done checks.

**IMPORTANT**: Only implement what's in the Acceptance Criteria. If you need to do more, either:

1. Update the AC first: `backlog task edit 42 --ac "New requirement"`
2. Or create a new follow up task: `backlog task create "Additional feature"`

---

## 6. Typical Workflow

```bash
# 1. Identify work
backlog task list -s "To Do" --plain

# 2. Read task details
backlog task 42 --plain

# 3. Start work: assign yourself & change status
backlog task edit 42 -s "In Progress" -a @myself

# 4. Add implementation plan
backlog task edit 42 --plan "1. Analyze\n2. Refactor\n3. Test"

# 5. Share the plan with the user and wait for approval (do not write code yet)

# 6. Work on the task (write code, test, etc.)

# 7. Mark acceptance criteria as complete (supports multiple in one command)
backlog task edit 42 --check-ac 1 --check-ac 2 --check-ac 3  # Check all at once
# Or check them individually if preferred:
# backlog task edit 42 --check-ac 1
# backlog task edit 42 --check-ac 2
# backlog task edit 42 --check-ac 3

# 8. Add Final Summary (PR Description)
backlog task edit 42 --final-summary "Refactored using strategy pattern, updated tests"

# 9. Mark task as done
backlog task edit 42 -s Done
```

---

## 7. Definition of Done (DoD)

A task is **Done** only when **ALL** of the following are complete:

### ✅ Via CLI Commands:

1. **All acceptance criteria checked**: Use `backlog task edit <id> --check-ac <index>` for each
2. **All Definition of Done items checked**: Use `backlog task edit <id> --check-dod <index>` for each
3. **Final Summary added**: Use `backlog task edit <id> --final-summary "..."`
4. **Status set to Done**: Use `backlog task edit <id> -s Done`

### ✅ Via Code/Testing:

5. **Tests pass**: Run test suite and linting
6. **Documentation updated**: Update relevant docs if needed
7. **Code reviewed**: Self-review your changes
8. **No regressions**: Performance, security checks pass

⚠️ **NEVER mark a task as Done without completing ALL items above**

---

## 8. Finding Tasks and Content with Search

When users ask you to find tasks related to a topic, use the `backlog search` command with `--plain` flag:

```bash
# Search for tasks about authentication
backlog search "auth" --plain

# Search only in tasks (not docs/decisions)
backlog search "login" --type task --plain

# Search with filters
backlog search "api" --status "In Progress" --plain
backlog search "bug" --priority high --plain
```

**Key points:**
- Uses fuzzy matching - finds "authentication" when searching "auth"
- Searches task titles, descriptions, and content
- Also searches documents and decisions unless filtered with `--type task`
- Always use `--plain` flag for AI-readable output

---

## 9. Quick Reference: DO vs DON'T

### Viewing and Finding Tasks

| Task         | ✅ DO                        | ❌ DON'T                         |
|--------------|-----------------------------|---------------------------------|
| View task    | `backlog task 42 --plain`   | Open and read .md file directly |
| List tasks   | `backlog task list --plain` | Browse backlog/tasks folder     |
| Check status | `backlog task 42 --plain`   | Look at file content            |
| Find by topic| `backlog search "auth" --plain` | Manually grep through files |

### Modifying Tasks

| Task          | ✅ DO                                 | ❌ DON'T                           |
|---------------|--------------------------------------|-----------------------------------|
| Check AC      | `backlog task edit 42 --check-ac 1`  | Change `- [ ]` to `- [x]` in file |
| Add notes     | `backlog task edit 42 --notes "..."` | Type notes into .md file          |
| Add final summary | `backlog task edit 42 --final-summary "..."` | Type summary into .md file |
| Change status | `backlog task edit 42 -s Done`       | Edit status in frontmatter        |
| Add AC        | `backlog task edit 42 --ac "New"`    | Add `- [ ] New` to file           |

---

## 10. Complete CLI Command Reference

### Task Creation

| Action           | Command                                                                             |
|------------------|-------------------------------------------------------------------------------------|
| Create task      | `backlog task create "Title"`                                                       |
| With description | `backlog task create "Title" -d "Description"`                                      |
| With AC          | `backlog task create "Title" --ac "Criterion 1" --ac "Criterion 2"`                 |
| With final summary | `backlog task create "Title" --final-summary "PR-style summary"`                 |
| With references  | `backlog task create "Title" --ref src/api.ts --ref https://github.com/issue/123`   |
| With documentation | `backlog task create "Title" --doc https://design-docs.example.com`               |
| With all options | `backlog task create "Title" -d "Desc" -a @sara -s "To Do" -l auth --priority high --ref src/api.ts --doc docs/spec.md` |
| Create draft     | `backlog task create "Title" --draft`                                               |
| Create subtask   | `backlog task create "Title" -p 42`                                                 |

### Task Modification

| Action           | Command                                     |
|------------------|---------------------------------------------|
| Edit title       | `backlog task edit 42 -t "New Title"`       |
| Edit description | `backlog task edit 42 -d "New description"` |
| Change status    | `backlog task edit 42 -s "In Progress"`     |
| Assign           | `backlog task edit 42 -a @sara`             |
| Add labels       | `backlog task edit 42 -l backend,api`       |
| Set priority     | `backlog task edit 42 --priority high`      |

### Acceptance Criteria Management

| Action              | Command                                                                     |
|---------------------|-----------------------------------------------------------------------------|
| Add AC              | `backlog task edit 42 --ac "New criterion" --ac "Another"`                  |
| Remove AC #2        | `backlog task edit 42 --remove-ac 2`                                        |
| Remove multiple ACs | `backlog task edit 42 --remove-ac 2 --remove-ac 4`                          |
| Check AC #1         | `backlog task edit 42 --check-ac 1`                                         |
| Check multiple ACs  | `backlog task edit 42 --check-ac 1 --check-ac 3`                            |
| Uncheck AC #3       | `backlog task edit 42 --uncheck-ac 3`                                       |
| Mixed operations    | `backlog task edit 42 --check-ac 1 --uncheck-ac 2 --remove-ac 3 --ac "New"` |

### Task Content

| Action           | Command                                                  |
|------------------|----------------------------------------------------------|
| Add plan         | `backlog task edit 42 --plan "1. Step one\n2. Step two"` |
| Add notes        | `backlog task edit 42 --notes "Implementation details"`  |
| Add final summary | `backlog task edit 42 --final-summary "PR-style summary"` |
| Append final summary | `backlog task edit 42 --append-final-summary "More details"` |
| Clear final summary | `backlog task edit 42 --clear-final-summary` |
| Add dependencies | `backlog task edit 42 --dep task-1 --dep task-2`         |
| Add references   | `backlog task edit 42 --ref src/api.ts --ref https://github.com/issue/123` |
| Add documentation | `backlog task edit 42 --doc https://design-docs.example.com --doc docs/spec.md` |

### Multi‑line Input (Description/Plan/Notes/Final Summary)

The CLI preserves input literally. Shells do not convert `\n` inside normal quotes. Use one of the following to insert real newlines:

- Bash/Zsh (ANSI‑C quoting):
  - Description: `backlog task edit 42 --desc $'Line1\nLine2\n\nFinal'`
  - Plan: `backlog task edit 42 --plan $'1. A\n2. B'`
  - Notes: `backlog task edit 42 --notes $'Done A\nDoing B'`
  - Append notes: `backlog task edit 42 --append-notes $'Progress update line 1\nLine 2'`
  - Final summary: `backlog task edit 42 --final-summary $'Shipped A\nAdded B'`
  - Append final summary: `backlog task edit 42 --append-final-summary $'Added X\nAdded Y'`
- POSIX portable (printf):
  - `backlog task edit 42 --notes "$(printf 'Line1\nLine2')"`
- PowerShell (backtick n):
  - `backlog task edit 42 --notes "Line1`nLine2"`

Do not expect `"...\n..."` to become a newline. That passes the literal backslash + n to the CLI by design.

Descriptions support literal newlines; shell examples may show escaped `\\n`, but enter a single `\n` to create a newline.

### Implementation Notes Formatting

- Keep implementation notes concise and time-ordered; focus on progress, decisions, and blockers.
- Use short paragraphs or bullet lists instead of a single long line.
- Use Markdown bullets (`-` for unordered, `1.` for ordered) for readability.
- When using CLI flags like `--append-notes`, remember to include explicit
  newlines. Example:

  ```bash
  backlog task edit 42 --append-notes $'- Added new API endpoint\n- Updated tests\n- TODO: monitor staging deploy'
  ```

### Final Summary Formatting

- Treat the Final Summary as a PR description: lead with the outcome, then add key changes and tests.
- Keep it clean and structured so it can be pasted directly into GitHub.
- Prefer short paragraphs or bullet lists and avoid raw progress logs.
- Aim to cover: **what changed**, **why**, **user impact**, **tests run**, and **risks/follow‑ups** when relevant.
- Avoid single‑line summaries unless the change is truly tiny.

**Example (good, not rigid):**
```
Added Final Summary support across CLI/MCP/Web/TUI to separate PR summaries from progress notes.

Changes:
- Added `finalSummary` to task types and markdown section parsing/serialization (ordered after notes).
- CLI/MCP/Web/TUI now render and edit Final Summary; plain output includes it.

Tests:
- bun test src/test/final-summary.test.ts
- bun test src/test/cli-final-summary.test.ts
```

### Task Operations

| Action             | Command                                      |
|--------------------|----------------------------------------------|
| View task          | `backlog task 42 --plain`                    |
| List tasks         | `backlog task list --plain`                  |
| Search tasks       | `backlog search "topic" --plain`              |
| Search with filter | `backlog search "api" --status "To Do" --plain` |
| Filter by status   | `backlog task list -s "In Progress" --plain` |
| Filter by assignee | `backlog task list -a @sara --plain`         |
| Archive task       | `backlog task archive 42`                    |
| Demote to draft    | `backlog task demote 42`                     |

---

## Common Issues

| Problem              | Solution                                                           |
|----------------------|--------------------------------------------------------------------|
| Task not found       | Check task ID with `backlog task list --plain`                     |
| AC won't check       | Use correct index: `backlog task 42 --plain` to see AC numbers     |
| Changes not saving   | Ensure you're using CLI, not editing files                         |
| Metadata out of sync | Re-edit via CLI to fix: `backlog task edit 42 -s <current-status>` |

---

## Remember: The Golden Rule

**🎯 If you want to change ANYTHING in a task, use the `backlog task edit` command.**
**📖 Use CLI to read tasks, exceptionally READ task files directly, never WRITE to them.**

Full help available: `backlog --help`

<!-- BACKLOG.MD GUIDELINES END -->
