# Move the Core Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `src/server/` build without `src/cli/`, so the desktop client can link the router without linking the terminal UI.

**Architecture:** Every `src/cli/<x>.rs` today holds two things: a data layer of pure `&Connection` functions, and a thin wrapper that resolves names and prints. The server calls the first half and drags the second along with it. Each task moves one module's data layer to a top-level module and leaves the printing wrapper in `cli/` calling into it. A source-scanning guard test is the spine: it starts red with every offending line named, and each task drives it toward empty.

**Tech Stack:** Rust 2021, rusqlite, cargo workspaces (later), no new dependencies.

**Spec:** `backlog/decisions/decision-1 - Desktop-transport-custom-URI-scheme-over-the-existing-axum-router.md` and TASK-33.1. The decision settles that the desktop shell serves the *same* `build_router()`; this plan is what makes that router linkable on its own.

## Global Constraints

- The `nigel` binary keeps its name, its features (`gusto`, `pdf`, `serve`), and its behaviour. No CLI output changes in this plan — not one word of one printed line.
- No behaviour changes at all. Every task is a move plus import updates. If a task tempts you to fix something you noticed, don't: file it and move on.
- `cargo test -- --test-threads=1` is the gate for every task. The DB password is a process global, so the suite is serial.
- Public re-exports stay: `pub use` the moved items from their old paths in `cli/` where a caller outside this repo could plausibly depend on them. Internal callers are updated to the new path.
- The repo is public and holds no real book data. `./scripts/check-no-real-data.sh --staged` must pass before every commit; judge it by exit status, never by grepping its output.
- Commit after every task. Small commits are the point — this plan is 12 reversible steps, not one big bang.

## File Structure

New top-level modules, each holding the data layer extracted from its `cli/` twin:

| New file | Holds | Extracted from |
|---|---|---|
| `src/accounts.rs` | account CRUD over `&Connection` | `src/cli/accounts.rs` |
| `src/categories.rs` | category CRUD, `CategoryRow` | `src/cli/categories.rs` |
| `src/rules.rs` | rule CRUD, pattern testing | `src/cli/rules.rs` |
| `src/imports.rs` | import history and delete | `src/cli/undo.rs` |
| `src/backup.rs` | snapshot functions | `src/cli/backup.rs` |
| `src/password.rs` | encrypt / decrypt / rekey | `src/cli/password.rs` |
| `src/updater.rs` | release check, version compare | `src/cli/update.rs` |
| `src/clock.rs` | `today()` | `src/cli/mod.rs:47` |
| `src/reports/mod.rs` | today's `src/reports.rs`, plus `PDF_DISABLED_MESSAGE` and `export_file_stem` | `src/reports.rs`, `src/cli/report/mod.rs` |
| `src/reports/text.rs` | report text formatters | `src/cli/report/text.rs` |
| `src/invoicing/wiring.rs` | the settings-to-invoicing wiring the server calls | `src/cli/invoice.rs` |
| `tests/layering.rs` | the guard test | new |

What stays in `src/cli/`: every `run()`, `add()`, `list()`, `rename()`, `delete()`, `test()` wrapper; the clap surface in `cli/mod.rs`; every `*_manager.rs` TUI screen; `cli/report/view.rs`; `cli/report/export.rs`; and `cli/invoice.rs`'s command surface.

**One constraint carries over from CLAUDE.md and must survive this plan:** nothing under `src/invoicing/` reads settings. `wiring.rs` sits inside that directory and keeps the rule by taking `InvoicingConfig` as a *parameter*, exactly as the functions do today. If a moved function reaches for `settings::` directly, it is in the wrong file.

---

### Task 1: The guard test

**Files:**
- Create: `tests/layering.rs`

**Interfaces:**
- Produces: nothing importable. This is the scoreboard every later task reads.

- [ ] **Step 1: Write the failing test**

```rust
//! What may reach into the terminal UI, and what may not.
//!
//! `src/server/` is the half a desktop client links without a terminal (see
//! backlog/decisions/decision-1). Every `crate::cli::` reference here is a
//! build error waiting for the workspace split in TASK-33.1, so it is one
//! here first, where the fix is cheap.

use std::fs;
use std::path::{Path, PathBuf};

/// Directories that must not reach into the CLI/TUI layer.
const CORE_DIRS: [&str; 1] = ["src/server"];

/// Test support that drives the CLI's own formatters on purpose: the figure
/// parity fixtures compare what a browser renders against what `nigel invoice
/// list` prints, which means naming both. Neither ships in a release binary.
/// Both move to the CLI crate when the workspace splits.
const TEST_SUPPORT: [&str; 2] = ["testutil.rs", "fixture_capture.rs"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if !TEST_SUPPORT.contains(&name.as_str()) {
                out.push(path);
            }
        }
    }
}

fn cli_references() -> Vec<String> {
    let mut files = Vec::new();
    for dir in CORE_DIRS {
        rust_files(Path::new(dir), &mut files);
    }
    files.sort();

    let mut hits = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("read source file");
        for (number, line) in text.lines().enumerate() {
            if line.contains("crate::cli::") {
                hits.push(format!("{}:{}: {}", file.display(), number + 1, line.trim()));
            }
        }
    }
    hits
}

#[test]
fn the_server_does_not_reach_into_the_cli_layer() {
    let hits = cli_references();
    assert!(
        hits.is_empty(),
        "the server still reaches into the CLI layer in {} place(s):\n{}",
        hits.len(),
        hits.join("\n")
    );
}
```

- [ ] **Step 2: Run it and read the list**

Run: `cargo test --test layering -- --test-threads=1`
Expected: FAIL, listing roughly 24 lines across `mod.rs`, `routes/accounts.rs`, `routes/categories.rs`, `routes/clients.rs`, `routes/exports.rs`, `routes/imports.rs`, `routes/invoices.rs`, `routes/review.rs`, `routes/rules.rs`, `routes/settings.rs`, `routes/transactions.rs`.

Copy that list somewhere. It is the plan's checklist, and the exact count is what each later task drives down.

- [ ] **Step 3: Mark the test as expected-to-fail for now**

Add directly above `fn the_server_does_not_reach_into_the_cli_layer`:

```rust
// Red until Task 11 lands. Each task in the plan removes its own module's
// references; running this test is how a task proves it finished.
#[ignore = "red until the boundary move completes (TASK-33.1)"]
```

- [ ] **Step 4: Verify the suite is green with the guard ignored**

Run: `cargo test -- --test-threads=1`
Expected: PASS, with `the_server_does_not_reach_into_the_cli_layer` reported as ignored.

- [ ] **Step 5: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add tests/layering.rs
git commit -m "Add the layering guard: the server may not reach into cli"
```

---

### Task 2: `accounts`

**Files:**
- Create: `src/accounts.rs`
- Modify: `src/cli/accounts.rs`, `src/lib.rs`, `src/server/routes/accounts.rs`

**Interfaces:**
- Produces: `crate::accounts::{ACCOUNT_TYPES, list_accounts, get_account, add_account, rename_account, transaction_count, account_names, delete_blocker, delete_account}` — signatures unchanged from `src/cli/accounts.rs:10-210`.

- [ ] **Step 1: Move the data layer**

Create `src/accounts.rs` containing, verbatim and in this order, from `src/cli/accounts.rs`:

`ACCOUNT_TYPES` (line 10), `list_accounts` (74), `get_account` (92), `add_account` (115), `rename_account` (149), `transaction_count` (174), `account_names` (183), `delete_blocker` (195), `delete_account` (203), plus any private helper they call and the `use` lines they need.

Leave in `src/cli/accounts.rs`: `add` (12), `list` (24), `rename` (56), `delete` (63) — the printing wrappers — and add at the top:

```rust
pub use crate::accounts::*;
```

so the wrappers keep compiling unchanged and any outside caller of `cli::accounts::add_account` still resolves.

- [ ] **Step 2: Register the module**

In `src/lib.rs`, add in alphabetical position among the existing `pub mod` lines:

```rust
pub mod accounts;
```

- [ ] **Step 3: Point the server at the new path**

In `src/server/routes/accounts.rs`, replace every `crate::cli::accounts` with `crate::accounts`.

- [ ] **Step 4: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS. Then:
Run: `cargo test --test layering -- --ignored --test-threads=1`
Expected: still FAIL, but `routes/accounts.rs` no longer appears in the list.

- [ ] **Step 5: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Move the account data layer out of cli"
```

---

### Task 3: `categories`

**Files:**
- Create: `src/categories.rs`
- Modify: `src/cli/categories.rs`, `src/lib.rs`, `src/server/routes/categories.rs`, `src/server/routes/review.rs`, `src/server/routes/transactions.rs`

**Interfaces:**
- Produces: `crate::categories::{CategoryRow, list_categories, get_category, ensure_category_exists, add_category, rename_category, update_category, delete_blocker, blocking_reason, delete_category, usage_count}` — signatures unchanged from `src/cli/categories.rs:11-270`.

- [ ] **Step 1: Move the data layer**

Create `src/categories.rs` with, from `src/cli/categories.rs`: `CategoryRow` (11), `list_categories` (81), `get_category` (102), `ensure_category_exists` (128), `add_category` (142), `rename_category` (180), `update_category` (204), `delete_blocker` (236), `blocking_reason` (248), `delete_category` (252), `usage_count` (266).

Leave `add` (19), `list` (31), `rename` (50), `update` (57), `delete` (70) behind, and add `pub use crate::categories::*;` at the top of `src/cli/categories.rs`.

- [ ] **Step 2: Register the module**

In `src/lib.rs`: `pub mod categories;`

- [ ] **Step 3: Point the server at the new path**

Three files: `routes/categories.rs`, `routes/review.rs`, `routes/transactions.rs` (the last uses `ensure_category_exists`). Replace `crate::cli::categories` with `crate::categories` in each.

- [ ] **Step 4: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS.
Run: `cargo test --test layering -- --ignored --test-threads=1`
Expected: FAIL with three fewer files listed.

- [ ] **Step 5: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Move the category data layer out of cli"
```

---

### Task 4: `rules`

**Files:**
- Create: `src/rules.rs`
- Modify: `src/cli/rules.rs`, `src/lib.rs`, `src/server/routes/rules.rs`

**Interfaces:**
- Produces: `crate::rules::{RuleRow, MATCH_TYPES, NewRule, RuleUpdate, RuleTestMatch, RuleTestResult, list_rules, validate_match_type, resolve_category_id, get_rule, add_rule, update_rule, deactivate_rule, test_pattern}` — signatures unchanged from `src/cli/rules.rs:17-300`.

- [ ] **Step 1: Move the data layer**

Create `src/rules.rs` with, from `src/cli/rules.rs`: `RuleRow` (17), `list_rules` (30), `MATCH_TYPES` (56), `NewRule` (59), `RuleUpdate` (70), `RuleTestMatch` (91), `RuleTestResult` (99), `validate_match_type` (107), `resolve_category_id` (121), `get_rule` (132), `add_rule` (174), `update_rule` (194), `deactivate_rule` (257), `test_pattern` (270).

Leave `add` (304), `list` (327), `update` (349), `delete` (378), `test` (389) behind, and add `pub use crate::rules::*;` at the top of `src/cli/rules.rs`.

- [ ] **Step 2: Register the module**

In `src/lib.rs`: `pub mod rules;`

- [ ] **Step 3: Point the server at the new path**

In `src/server/routes/rules.rs`, replace `crate::cli::rules` with `crate::rules`.

- [ ] **Step 4: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Move the rules data layer out of cli"
```

---

### Task 5: `imports` (from `cli::undo`)

**Files:**
- Create: `src/imports.rs`
- Modify: `src/cli/undo.rs`, `src/lib.rs`, `src/server/routes/imports.rs`

**Interfaces:**
- Produces: `crate::imports::{LastImport, ImportListItem, list_imports, get_last_import, import_exists, delete_import}` — signatures unchanged from `src/cli/undo.rs:13-92`.

The module is named for what it holds (import history) rather than for the command that reads it, because the server lists imports on a screen that has no undo on it.

- [ ] **Step 1: Move the data layer**

Create `src/imports.rs` with, from `src/cli/undo.rs`: `LastImport` (13), `ImportListItem` (24), `list_imports` (34), `get_last_import` (60), `import_exists` (75), `delete_import` (86).

Leave `run` (94) — which prompts and prints — in `src/cli/undo.rs`, and add `pub use crate::imports::*;` at its top.

- [ ] **Step 2: Register the module**

In `src/lib.rs`: `pub mod imports;`

- [ ] **Step 3: Point the server at the new path**

In `src/server/routes/imports.rs`, replace `crate::cli::undo` with `crate::imports`.

- [ ] **Step 4: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Move import history out of cli::undo"
```

---

### Task 6: `backup`

**Files:**
- Create: `src/backup.rs`
- Modify: `src/cli/backup.rs`, `src/lib.rs`, `src/server/routes/imports.rs`

**Interfaces:**
- Produces: `crate::backup::{snapshot, snapshot_with_password}` — signatures unchanged from `src/cli/backup.rs:13-38`.

- [ ] **Step 1: Move the data layer**

Create `src/backup.rs` with `snapshot` (13) and `snapshot_with_password` (19) from `src/cli/backup.rs`. Leave `run` (39) behind and add `pub use crate::backup::*;` at the top of `src/cli/backup.rs`.

- [ ] **Step 2: Register the module**

In `src/lib.rs`: `pub mod backup;`

- [ ] **Step 3: Point the server at the new path**

In `src/server/routes/imports.rs`, replace `crate::cli::backup` with `crate::backup`.

- [ ] **Step 4: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Move snapshot helpers out of cli::backup"
```

---

### Task 7: `password`

**Files:**
- Create: `src/password.rs`
- Modify: `src/cli/password.rs`, `src/lib.rs`, `src/server/routes/settings.rs`

**Interfaces:**
- Produces: `crate::password::{encrypt_database, decrypt_database, rekey_database}` — signatures unchanged from `src/cli/password.rs:9-76`, each taking `&Path` plus password strings.

- [ ] **Step 1: Move the data layer**

Create `src/password.rs` with `encrypt_database` (9), `decrypt_database` (30), `rekey_database` (51). Leave `run_set` (77), `run_change` (94), `run_remove` (112) — which prompt via rpassword — in `src/cli/password.rs`, and add `pub use crate::password::*;` at its top.

- [ ] **Step 2: Register the module**

In `src/lib.rs`: `pub mod password;`

- [ ] **Step 3: Point the server at the new path**

In `src/server/routes/settings.rs`, replace `crate::cli::password` with `crate::password`.

- [ ] **Step 4: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS. These three rewrite the database file, so read the failures carefully if any appear — a wrong path here is data loss in production.

- [ ] **Step 5: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Move encrypt, decrypt and rekey out of cli"
```

---

### Task 8: `updater`

**Files:**
- Create: `src/updater.rs`
- Modify: `src/cli/update.rs`, `src/lib.rs`, `src/server/mod.rs`

**Interfaces:**
- Produces: `crate::updater::{UpdateInfo, asset_name, check_for_update, check_with_cooldown, update_notice, check_and_notify, is_newer}` — signatures unchanged from `src/cli/update.rs:16-210`.

Named `updater` rather than `update` because `update` is a verb this codebase already uses for row edits, and `crate::update` beside `rules::update_rule` reads as one.

- [ ] **Step 1: Move the data layer**

Create `src/updater.rs` with `UpdateInfo` (16), `asset_name` (31), `check_for_update` (42), `check_with_cooldown` (165), `update_notice` (195), `check_and_notify` (201), `is_newer` (206). Leave `run` (127) — which downloads, prompts and self-replaces — in `src/cli/update.rs`, and add `pub use crate::updater::*;` at its top.

- [ ] **Step 2: Register the module**

In `src/lib.rs`: `pub mod updater;`

- [ ] **Step 3: Point the server at the new path**

In `src/server/mod.rs`, replace `crate::cli::update::check_with_cooldown` with `crate::updater::check_with_cooldown`.

- [ ] **Step 4: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Move the release check out of cli::update"
```

---

### Task 9: `clock`

**Files:**
- Create: `src/clock.rs`
- Modify: `src/cli/mod.rs:45-49`, `src/lib.rs`, `src/server/routes/clients.rs`, `src/server/routes/invoices.rs`

**Interfaces:**
- Produces: `crate::clock::today() -> String`.

- [ ] **Step 1: Create the module**

```rust
//! The app's one clock read.
//!
//! Nothing under `src/invoicing/` reads the clock — every derived status takes
//! its reference day as a parameter, which is what makes them deterministic in
//! tests and correct against the wall clock in production. This is where that
//! day comes from.

/// Today's local date as `YYYY-MM-DD` — the reference day every date-less
/// command ages, derives and reports against.
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}
```

- [ ] **Step 2: Delete the original and re-export**

Remove `today()` from `src/cli/mod.rs` (lines 45-49, doc comment included) and add near the top of that file:

```rust
pub use crate::clock::today;
```

Every existing `cli::today()` call site keeps working unchanged.

- [ ] **Step 3: Register the module**

In `src/lib.rs`: `pub mod clock;`

- [ ] **Step 4: Point the server at the new path**

In `src/server/routes/clients.rs` and `src/server/routes/invoices.rs`, replace `crate::cli::today()` with `crate::clock::today()`. There are five call sites across the two files.

- [ ] **Step 5: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Give today() a home outside the CLI"
```

---

### Task 10: `reports::text` and the export naming

**Files:**
- Create: `src/reports/mod.rs` (from `src/reports.rs`), `src/reports/text.rs` (from `src/cli/report/text.rs`)
- Delete: `src/reports.rs`, `src/cli/report/text.rs`
- Modify: `src/cli/report/mod.rs`, `src/server/routes/exports.rs`, `src/server/routes/invoices.rs`

**Interfaces:**
- Produces: `crate::reports::text::*` (every formatter, unchanged), `crate::reports::PDF_DISABLED_MESSAGE`, `crate::reports::export_file_stem(name: &str) -> String`.

`src/cli/report/text.rs` is the one file in this plan that moves wholesale: 647 lines, no clap, no ratatui, and not a single `println!`. It was already a library.

- [ ] **Step 1: Turn `reports.rs` into a directory**

```bash
mkdir -p src/reports
git mv src/reports.rs src/reports/mod.rs
git mv src/cli/report/text.rs src/reports/text.rs
```

Add to the top of `src/reports/mod.rs`:

```rust
pub mod text;
```

- [ ] **Step 2: Move the two export-naming items**

Move from `src/cli/report/mod.rs` into `src/reports/mod.rs`, verbatim with their doc comments: `PDF_DISABLED_MESSAGE` (line 40) and `export_file_stem` (line 46).

In `src/cli/report/mod.rs`, replace the `pub mod text;` declaration with a re-export and add re-exports for the two moved items:

```rust
pub use crate::reports::text;
pub use crate::reports::{export_file_stem, PDF_DISABLED_MESSAGE};
```

- [ ] **Step 3: Point the server at the new paths**

In `src/server/routes/exports.rs`: `crate::cli::report::text` → `crate::reports::text`, `crate::cli::report::export_file_stem` → `crate::reports::export_file_stem`, `crate::cli::report::PDF_DISABLED_MESSAGE` → `crate::reports::PDF_DISABLED_MESSAGE`.

In `src/server/routes/invoices.rs`: the one `crate::cli::report::PDF_DISABLED_MESSAGE` → `crate::reports::PDF_DISABLED_MESSAGE`.

- [ ] **Step 4: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS. The report figure-parity fixtures exercise these formatters heavily, so a green run here is strong evidence the move was clean.

- [ ] **Step 5: Verify the no-pdf build still compiles**

Run: `cargo build --no-default-features`
Expected: success. `PDF_DISABLED_MESSAGE` has to exist without the `pdf` feature — that is its whole job.

- [ ] **Step 6: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Move the report text formatters into reports"
```

---

### Task 11: `invoicing::wiring`

**Files:**
- Create: `src/invoicing/wiring.rs`
- Modify: `src/cli/invoice.rs`, `src/invoicing/mod.rs`, `src/server/routes/invoices.rs`

**Interfaces:**
- Produces, with signatures unchanged from `src/cli/invoice.rs` and visibility widened from `pub(crate)` to `pub`:
  - `company_name(conn: &Connection) -> String` (104)
  - `company_profile(conn: &Connection) -> CompanyProfile` (122)
  - `optional_gateway(cfg: &InvoicingConfig) -> Option<StripeClient>` (170)
  - `optional_publisher(cfg: &InvoicingConfig) -> Option<R2Publisher>` (178)
  - `build_clients(cfg: InvoicingConfig, company: &str) -> Result<SendClients>` (210)
  - `republish_with<P: AssetPublisher>(…)` (573)
  - `republish_all_with<P: AssetPublisher>(…)` (621)
  - `contact_email_for_preview(cfg: &InvoicingConfig) -> (String, bool)` (646)

This is the largest task and the only one with a rule to honour rather than just a move: **nothing under `src/invoicing/` reads settings.** Every function above already takes its `InvoicingConfig` as a parameter, which is why they can live here at all. If one of them reaches for `crate::settings::` while you are moving it, stop — it belongs in the CLI layer and the server should be passing the value in instead.

- [ ] **Step 1: Move the eight functions**

Create `src/invoicing/wiring.rs` with the eight items listed above, verbatim, `pub(crate)` changed to `pub`, plus the `use` lines they need and any private helper they call that no CLI command uses.

Open with a module doc that states the constraint, so the next person does not have to rediscover it:

```rust
//! Where settings meet invoicing.
//!
//! `src/invoicing/` never reads settings — every value arrives as a parameter,
//! resolved by whichever surface is calling. These functions are the wiring
//! that assembles a send or a republish out of those values, and they live here
//! rather than in the CLI because the HTTP layer needs them too. A function
//! here that reaches for `crate::settings` has broken the rule this module
//! exists to keep.
```

- [ ] **Step 2: Register the module**

In `src/invoicing/mod.rs`, add alongside the existing declarations:

```rust
pub mod wiring;
```

- [ ] **Step 3: Re-export from the old path**

At the top of `src/cli/invoice.rs`:

```rust
pub(crate) use crate::invoicing::wiring::*;
```

The CLI's own command surface — every `run_*`, the confirmation prompts, the printing — stays exactly where it is and keeps compiling against these names.

- [ ] **Step 4: Fix the one hit that is not a move**

`crate::cli::invoice::pay_button_for` in `src/server/routes/invoices.rs` is not defined in `cli/invoice.rs` at all — it is a re-export of `crate::invoicing::render::pay_button_for`. Point the server straight at the definition:

```rust
crate::invoicing::render::pay_button_for(&invoice)
```

- [ ] **Step 5: Point the server at the new path**

In `src/server/routes/invoices.rs`, replace the remaining `crate::cli::invoice::` references with `crate::invoicing::wiring::`. That is `build_clients`, `company_name`, `company_profile`, `contact_email_for_preview`, `optional_gateway`, `optional_publisher`, `republish_all_with`, `republish_with`.

- [ ] **Step 6: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS. The send and republish paths are fake-tested through the gateway traits, so no test here reaches the network — a green run means the wiring still assembles.

- [ ] **Step 7: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Move the invoicing wiring out of the CLI layer"
```

---

### Task 12: Close the boundary

**Files:**
- Modify: `tests/layering.rs`, `CLAUDE.md`

- [ ] **Step 1: Un-ignore the guard**

Delete the `#[ignore = "red until the boundary move completes (TASK-33.1)"]` line added in Task 1.

- [ ] **Step 2: Run it**

Run: `cargo test --test layering -- --test-threads=1`
Expected: PASS. If anything remains, the failure message names the file and line; fix it the way its neighbours were fixed, in its own commit.

- [ ] **Step 3: Run everything**

Run: `cargo test -- --test-threads=1`
Expected: PASS.
Run: `cargo test --no-default-features -- --test-threads=1`
Expected: PASS.
Run: `cargo build --release`
Expected: success.

- [ ] **Step 4: Document the boundary**

In `CLAUDE.md`, under **Key Design Constraints**, add:

```markdown
- `src/server/` does not reach into `src/cli/`, and `tests/layering.rs` fails the build if it starts to. Every `cli/<x>.rs` is a printing wrapper over a data layer that lives at the top level — `accounts`, `categories`, `rules`, `imports`, `backup`, `password`, `updater`, `clock`, `reports::text` — and the invoicing wiring the HTTP layer needs is `invoicing::wiring`, which keeps the "invoicing never reads settings" rule by taking config as a parameter. The reason is TASK-33.1: a desktop client links the router without linking a terminal UI, and this is the boundary that makes that possible before the crate is split at all.
```

Also update the **Project Structure** tree with the new top-level modules.

- [ ] **Step 5: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Close the core boundary and document it"
```

---

## What this plan deliberately does not do

- **No workspace.** No `Cargo.toml` restructure, no new crates, no `nigel-core`. That is TASK-33.1's second half and its own plan, and it is mechanical once nothing crosses this boundary.
- **No Tauri.** Nothing here needs a webview, and the download probe (TASK-33.2, AC #5) runs independently in CI.
- **No behaviour changes.** If a moved function has a bug, it keeps the bug and gets a task.
- **`fixture_capture.rs` and `testutil.rs` still call the CLI's formatters**, deliberately — the parity fixtures compare browser output against `nigel invoice list`'s own text, so they must name both. They are excluded from the guard and move to the CLI crate when the workspace splits.
