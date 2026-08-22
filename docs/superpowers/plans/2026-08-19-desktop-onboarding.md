# First-Run Onboarding in the Desktop App — Implementation Plan (TASK-33.17)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A machine that has never run Nigel gets a first-run experience in the desktop app — and in a browser against `nigel serve` — that is functionally equivalent to the CLI's onboarding TUI: profile, identity, optional password, then demo / start fresh / load existing books. Today that machine gets a broken dashboard over a zero-byte database.

**Architecture:** The CLI's inline setup consumption logic moves into `nigel_core::setup` and the demo seeding into `nigel_core::demo`, so one implementation serves the TUI and a new `POST /api/setup` route. `GET /api/status` tightens `initialized` to "exists **and** non-empty", `nigel serve` stops creating an absent database, and the SPA gains a `needs-setup` boot phase that renders a setup gate in the same shell-replacing `.gate` treatment the unlock screen uses. Two brand components — `wc-wordmark` and `wc-particle-field` — land in `@nigel/ui`, pinned to the TUI's `LOGO` and particle constants. The desktop shell only gains a minimum window size.

**Tech Stack:** Rust (axum, rusqlite/SQLCipher, serde), Tauri 2; TypeScript, Lit 3, Web Awesome, vitest, axe.

**Spec:** `docs/superpowers/specs/2026-08-19-desktop-onboarding-design.md`. That document is the binding authority; this plan implements it.

## Spec-vs-code notes

Four places where the spec's letter and the code as it stands do not fit together. Each is resolved here rather than silently.

1. **`setup::run` returns the database path, not `()`.** The spec writes `pub fn run(plan: &SetupPlan) -> Result<()>`. The HTTP route has to rebind `AppState` to the database it just created, and `AppState`'s path can already differ from `settings.json`'s (the data-directory switch rebinds one without the other). Re-reading `settings.json` inside the route to find out where the books went is the drift the switch route was written to avoid. So `run` answers `Result<PathBuf>` — the same sequence, with the path it initialized handed back. Strengthening, not a behaviour change.

2. **The `fresh` action rebinds `AppState` too.** The spec names the rebind only for `demo`. The route takes the write gate and calls `state.set_db_path` for both actions, because after `setup::run` the server has to be serving the file that now exists rather than the path it was constructed with.

3. **The load path cannot persist `userName`.** The spec says the identity step "has already run `setup`-adjacent persistence for `user_name`", but there is no web path that writes it: `PUT /api/settings/app` explicitly refuses `userName` (`app_settings_ignore_fields_the_web_may_not_write`), and `POST /api/setup` would create the books the user is trying to *avoid* creating. Resolution: the load path calls the data-directory switch only. The collected name is dropped, and the dashboard greets the un-named way — exactly what any browser-first user gets today. A follow-up may add a name-only write; it is not this task.

4. **There is no `unlock.test.ts`.** The spec's "screen-test patterns" reference resolves to `web/apps/app/src/screens/settings.test.ts`, which is the nearest and richest example (mount helper, `FakeApiClient`, `client.calls` assertions). Task 6 follows it.

## What already exists, and must not be rebuilt

- `crates/nigel/src/cli/onboarding.rs` — the TUI. `pub enum PostSetupAction { Demo, StartFresh, Import }`, `pub struct OnboardingResult { user_name, company_name, password: Option<String>, action, profile }`, `pub fn run() -> Result<Option<OnboardingResult>>`. **Its behaviour does not change.**
- `nigel_core::settings`: `load_settings()`, `save_settings(&Settings)`, `settings_file_exists()`, `get_data_dir() -> PathBuf`, `shellexpand_path(&str) -> String`, `restrict_dir_permissions(&Path)`, `migrate_company_name() -> Option<String>`, `TempConfigDir`.
- `nigel_core::db`: `Profile::{Business, Personal}` with `as_str()`/`parse()`, `set_db_password(Option<String>)`, `get_db_password()`, `get_connection(&Path)`, `open_connection(&Path, Option<&str>)`, `is_encrypted(&Path) -> Result<bool>` (false for absent **and** for files under 16 bytes), `init_db`, `init_db_with_profile`, `get_metadata`, `set_metadata`, `get_profile`.
- `nigel_core::server::state::AppState`: `db_path()`, `data_dir()`, `set_db_path(PathBuf)` (write gate only), `db_gate: Arc<TokioRwLock<()>>`, `is_locked()`, `unlock: Arc<UnlockGate>`.
- `routes/status.rs`: `pub(crate) struct StatusResponse`, `pub(crate) async fn current_status(&AppState) -> ApiResult<StatusResponse>`, `locked_guard` with `UNGATED_PATHS = ["/ping", "/status", "/unlock"]`.
- `routes/settings.rs::post_data_dir` — validates, migrates, rewrites `settings.json`, clears the password global, `set_db_path`, resets the unlock budget, answers `current_status`. **The SPA's load path calls this, unchanged.**
- `server/testutil.rs`: `TempConfig`, `temp_db()`, `seeded_db()`, `encrypt()`, `app_for()`, `post_json()`, `put_json()`, `ok_json()`, `send()`, `session_request()`.
- `@nigel/theme` `tokens/gradient.ts`: `NIGEL_PALETTE`, `NIGEL_PALETTE_INK`, `gradientColor(t)`, `brandRamp`, `brandCycleKeyframes`, `gradientCss` with `--nc-grad-brand`, `--nc-grad-brand-size`, `--nc-grad-brand-text`. `__tests__/palette-parity.test.ts` pins `NIGEL_PALETTE` to `effects.rs::GRADIENT`.
- `@nigel/ui` `snake-engine.ts`: `MAX_PARTICLES = 20`, `PARTICLE_CHARS`, `type Rng = () => number`. `wc-snake.ts` holds a private `seedParticles` + `.particle` CSS that Task 5 extracts.
- `web/packages/ui/preview/axe-suite.ts::describePreviewA11y(preview)` and `preview/types.ts` (`Preview`, `PreviewState`).
- `web/apps/app/src/screens/registry.ts` (`ScreenId`, `DEFS`, `screenDef`, `navItems`), `context.ts` (`ScreenContext`), `components/nigel-app.ts` (`.gate` branch), `state/app-store.ts` (`BootPhase`, `initializeAppStore`), `snake-trigger.ts::snakeAllowedOnBoot` (exhaustive switch on `BootPhase`).
- `web/apps/app/src/__mocks__/fake-api-client.ts`: `FakeApiClient`, `UNLOCKED_STATUS`, `LOCKED_STATUS`, `conflictError`, `notFoundError`, `client.calls`.

## Global Constraints

- **⛔ No real book data.** This repo is public. Fixture cast only — Acme, Cedar Systems, Juniper Labs, Harbor & Vale, Globex, Initech — with invented amounts. Statutory figures every filer shares are allowed. Before each commit run `./scripts/check-no-real-data.sh --staged` and **judge it by its exit status, never by grepping its output**. The pre-commit hook runs the same check; never bypass it.
- **Rust tests run serially:** `cargo test -- --test-threads=1`. The database password is a process global. `cargo test -p nigel-core -- --test-threads=1` for the core crate alone.
- **`crates/nigel-desktop` is its own workspace** (`exclude`d from the root `[workspace]`, so its `desktop` feature never unifies into the `nigel` binary). Its tests run as `cargo test --manifest-path crates/nigel-desktop/Cargo.toml -- --test-threads=1`, from the repo root, and it is **not** covered by a root `cargo test`.
- **`cargo fmt --check` and `cargo clippy -- -D warnings`** both pass before any commit. CI runs fmt first.
- **Web checks run from `web/`:** `npm test`, `npm run lint`, `npm run typecheck`. `npx vitest run <path>` does not work — there is no root vitest config. Scope with `npm test --workspace=@nigel/ui` / `--workspace=@nigel/app` / `--workspace=@nigel/theme`.
- **Component-first UI is mandatory.** A visual element lives in `web/packages/ui/src/components/wc-foo.ts`, with a co-located `wc-foo.preview.ts` covering every visible state and a `wc-foo.test.ts` that calls `describePreviewA11y(preview)` — **zero axe violations**, and the test never restates the states. No bespoke component implementations in `web/apps/app/src/components/` beyond `nigel-app`.
- **Web Awesome primitives are cherry-picked** (`@awesome.me/webawesome/dist/components/<x>/<x>.js`) — never the autoloader, never the WA stylesheet. **Any file importing a `wa-*` module must adopt `controlsCss`** (`static styles = [controlsCss, css\`…\`]`), app screens included; `controls-adoption.test.ts` fails the build otherwise.
- **Components read `@nigel/theme` tokens.** No inline brand values in a component.
- **Screens never spell an endpoint and never touch `__TAURI__`.** Everything server-shaped goes through the `ApiClient` seam; the desktop client is a subclass, so a new method is added once in `client.ts`, once in the `ApiClient` interface, and once in `FakeApiClient`.
- **No provenance comments.** No "added in", "was formerly", "renamed because", "don't change this back", in code or in docs. Describe the current state; `git log` is the audit trail.
- **CI order:** `check-no-real-data.sh`, `npm run lint`, `npm run typecheck`, `npm test`, `npm run build`, `cargo fmt --check`, `cargo clippy -- -D warnings`, then four `cargo test` variants (default, `--no-default-features`, `--no-default-features --features serve`, `-p nigel-core`).

---

### Task 1: `nigel_core::setup` — one setup engine, and the CLI on top of it

**Files:**
- Create: `crates/nigel-core/src/setup.rs`
- Modify: `crates/nigel-core/src/lib.rs`, `crates/nigel/src/cli/dashboard.rs`

**Interfaces:**
- Consumes: `settings::{load_settings, save_settings, restrict_dir_permissions}`, `db::{Profile, set_db_password, get_connection, init_db_with_profile, set_metadata}`.
- Produces:
  ```rust
  pub struct SetupPlan {
      pub user_name: String,
      pub company_name: String,
      pub profile: db::Profile,
      pub password: Option<String>,
  }
  pub fn run(plan: &SetupPlan) -> Result<PathBuf>;   // the database path it initialized
  ```

- [ ] **Step 1: Write the failing tests**

Create `crates/nigel-core/src/setup.rs` with only the test module for now (the file will not compile until Step 3; that is the point):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Every test here writes settings.json and creates the data directory it
    /// names, so every one needs the redirect.
    fn fixture() -> (crate::settings::TempConfigDir, tempfile::TempDir) {
        crate::db::set_db_password(None);
        let config = crate::settings::TempConfigDir::new();
        let dir = tempfile::tempdir().expect("tempdir");
        let mut settings = crate::settings::load_settings();
        settings.data_dir = dir.path().join("books").to_string_lossy().to_string();
        crate::settings::save_settings(&settings).expect("save settings");
        (config, dir)
    }

    fn plan(password: Option<&str>) -> SetupPlan {
        SetupPlan {
            user_name: "Marta".to_string(),
            company_name: "Cedar Systems".to_string(),
            profile: crate::db::Profile::Business,
            password: password.map(str::to_string),
        }
    }

    #[test]
    fn it_builds_the_whole_directory_tree() {
        let (_config, _dir) = fixture();

        let db_path = run(&plan(None)).expect("setup");

        let data_dir = db_path.parent().expect("parent");
        assert!(db_path.exists(), "no database at {}", db_path.display());
        for name in ["exports", "snapshots", "backups"] {
            assert!(data_dir.join(name).is_dir(), "missing {name}/");
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_directories_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let (_config, _dir) = fixture();

        let db_path = run(&plan(None)).expect("setup");
        let data_dir = db_path.parent().expect("parent");

        for dir in [
            data_dir.to_path_buf(),
            data_dir.join("exports"),
            data_dir.join("snapshots"),
            data_dir.join("backups"),
        ] {
            let mode = std::fs::metadata(&dir).expect("metadata").permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{} is not 0700", dir.display());
        }
    }

    #[test]
    fn it_saves_the_name_and_the_company_where_each_belongs() {
        let (_config, _dir) = fixture();

        let db_path = run(&plan(None)).expect("setup");

        assert_eq!(crate::settings::load_settings().user_name, "Marta");
        let conn = crate::db::get_connection(&db_path).expect("open");
        assert_eq!(
            crate::db::get_metadata(&conn, "company_name").as_deref(),
            Some("Cedar Systems")
        );
    }

    #[test]
    fn it_honours_the_chosen_profile() {
        let (_config, _dir) = fixture();
        let mut personal = plan(None);
        personal.profile = crate::db::Profile::Personal;

        let db_path = run(&personal).expect("setup");

        let conn = crate::db::get_connection(&db_path).expect("open");
        assert_eq!(crate::db::get_profile(&conn), crate::db::Profile::Personal);
    }

    #[test]
    fn a_password_encrypts_the_database_from_its_first_write() {
        // The point of setting the global *before* the file exists: SQLCipher's
        // PRAGMA key has to be in force for the very first page written, or the
        // books sit in plaintext with a password the user believes protects them.
        let (_config, _dir) = fixture();

        let db_path = run(&plan(Some("correct horse battery staple"))).expect("setup");

        assert!(crate::db::is_encrypted(&db_path).expect("probe"), "not encrypted");
        crate::db::set_db_password(None);
        assert!(
            crate::db::open_connection(&db_path, None).is_err(),
            "opened without the key"
        );
    }

    #[test]
    fn no_password_leaves_the_file_readable_without_one() {
        let (_config, _dir) = fixture();

        let db_path = run(&plan(None)).expect("setup");

        assert!(!crate::db::is_encrypted(&db_path).expect("probe"));
        crate::db::open_connection(&db_path, None).expect("open plaintext");
    }

    #[test]
    fn an_empty_name_leaves_the_stored_one_alone() {
        // The dashboard calls this on every launch, not only the first, and a
        // returning user's plan carries whatever settings.json already holds.
        let (_config, _dir) = fixture();
        run(&plan(None)).expect("first setup");

        let mut anonymous = plan(None);
        anonymous.user_name = String::new();
        anonymous.company_name = String::new();
        run(&anonymous).expect("second setup");

        assert_eq!(crate::settings::load_settings().user_name, "Marta");
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core setup:: -- --test-threads=1`
Expected: FAIL to compile — `crates/nigel-core/src/setup.rs` is not a module and `SetupPlan`/`run` do not exist.

- [ ] **Step 3: Write the engine**

Put this above the test module in `crates/nigel-core/src/setup.rs`:

```rust
//! Creating a set of books: the one implementation both front ends call.
//!
//! The terminal's onboarding and the web's `POST /api/setup` collect the same
//! four answers in very different ways and then have to do exactly the same
//! six things with them. Those six live here, in the order they have to happen
//! in: the password global is set *before* the database file is created, so
//! SQLCipher's `PRAGMA key` is in force for the first page written and the
//! books are never briefly in plaintext.

use std::path::PathBuf;

use crate::db;
use crate::error::Result;
use crate::settings;

/// The answers a set of books is created from.
pub struct SetupPlan {
    /// Who to greet. Empty leaves whatever `settings.json` already holds.
    pub user_name: String,
    /// The business or household name. Empty writes no metadata.
    pub company_name: String,
    /// Which chart of accounts. Only takes effect on a fresh database.
    pub profile: db::Profile,
    /// `Some` encrypts the database; `None` leaves the current key alone.
    pub password: Option<String>,
}

/// Create the data directory tree and the database, and answer where the
/// database landed.
///
/// Safe to call against books that already exist: the directory creation is
/// idempotent, `init_db_with_profile` migrates rather than reseeds, and an
/// empty name or company writes nothing.
pub fn run(plan: &SetupPlan) -> Result<PathBuf> {
    let mut stored = settings::load_settings();
    if !plan.user_name.is_empty() {
        stored.user_name = plan.user_name.clone();
    }
    settings::save_settings(&stored)?;

    if let Some(password) = plan.password.as_ref() {
        db::set_db_password(Some(password.clone()));
    }

    let data_dir = PathBuf::from(&stored.data_dir);
    for dir in [
        data_dir.clone(),
        data_dir.join("exports"),
        data_dir.join("snapshots"),
        data_dir.join("backups"),
    ] {
        std::fs::create_dir_all(&dir)?;
        settings::restrict_dir_permissions(&dir)?;
    }

    let db_path = data_dir.join("nigel.db");
    let conn = db::get_connection(&db_path)?;
    db::init_db_with_profile(&conn, plan.profile)?;
    if !plan.company_name.is_empty() {
        db::set_metadata(&conn, "company_name", &plan.company_name)?;
    }

    Ok(db_path)
}
```

Register the module in `crates/nigel-core/src/lib.rs`, in alphabetical position between `reviewer` and `rules`:

```rust
pub mod reviewer;
pub mod rules;
#[cfg(feature = "serve")]
pub mod server;
pub mod settings;
pub mod setup;
pub mod updater;
```

- [ ] **Step 4: Run them and watch them pass**

Run: `cargo test -p nigel-core setup:: -- --test-threads=1`
Expected: PASS — 7 tests (6 on non-unix).

- [ ] **Step 5: Refactor the CLI onto it**

In `crates/nigel/src/cli/dashboard.rs::run()`, replace the block that runs from `// First-run: show onboarding…` through `drop(conn);` with:

```rust
    // First-run: show onboarding, then ensure data dir + DB exist
    let mut post_setup_action = None;
    let stored = load_settings();
    let mut plan = nigel_core::setup::SetupPlan {
        user_name: stored.user_name.clone(),
        company_name: String::new(),
        profile: nigel_core::db::Profile::default(),
        password: None,
    };
    if is_first_run {
        if let Some(result) = super::onboarding::run()? {
            plan.user_name = result.user_name;
            plan.company_name = result.company_name;
            plan.profile = result.profile;
            plan.password = result.password;
            post_setup_action = Some(result.action);
        }
    }

    let db_path = nigel_core::setup::run(&plan)?;
    let conn = nigel_core::db::get_connection(&db_path)?;

    // The chosen profile only takes effect on a fresh database. If onboarding
    // ran against books that already exist (settings.json was deleted, or a
    // prior run was skipped after the database was created), say so — the
    // same courtesy `nigel init --profile` extends — instead of leaving the
    // user believing they switched charts.
    let mut profile_notice = None;
    if post_setup_action.is_some() {
        let seeded = nigel_core::db::get_profile(&conn);
        if seeded != plan.profile {
            profile_notice = Some(format!(
                "These books already keep {} records; the {} choice was ignored.",
                seeded.as_str(),
                plan.profile.as_str()
            ));
        }
    }

    // Migrate legacy company_name from settings.json → DB metadata
    if nigel_core::db::get_metadata(&conn, "company_name").is_none() {
        if let Some(company) = nigel_core::settings::migrate_company_name() {
            nigel_core::db::set_metadata(&conn, "company_name", &company)?;
        }
    }

    drop(conn);

    let settings = load_settings();
```

The `// Handle post-setup action from onboarding` match, the `user_name` binding, and everything after it stay exactly as they are.

- [ ] **Step 6: Prove the TUI is unchanged**

Run: `cargo test -- --test-threads=1`
Expected: PASS, whole tree. The onboarding and dashboard suites must be green without edits — if one needed changing, the refactor changed behaviour.

Run: `cargo fmt --check && cargo clippy -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 7: Commit**

`./scripts/check-no-real-data.sh --staged` (exit 0), then commit.

---

### Task 2: `nigel_core::demo` — the demo where a route can reach it

**Files:**
- Create: `crates/nigel-core/src/demo.rs`
- Modify: `crates/nigel-core/src/lib.rs`, `crates/nigel/src/cli/demo.rs`

**Interfaces:**
- Consumes: `nigel_core::{categorizer::categorize_transactions, db, invoicing, settings}` — all already in the core crate.
- Produces:
  ```rust
  pub const ACCOUNT_NAME: &str = "BofA Checking";
  pub struct DemoSummary {
      pub transactions: usize,
      pub rules: usize,
      pub categorized: usize,
      pub flagged: usize,
      pub clients: usize,
      pub invoices: usize,
  }
  pub fn seed_demo(conn: &Connection) -> Result<bool>;    // false when nothing was missing
  pub fn demo_summary(conn: &Connection) -> Result<DemoSummary>;
  pub fn setup_demo_dir() -> Result<PathBuf>;             // the demo database path
  ```

- [ ] **Step 1: Write the failing tests**

Create `crates/nigel-core/src/demo.rs` holding only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (tempfile::TempDir, Connection) {
        crate::db::set_db_password(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = crate::db::open_connection(&dir.path().join("nigel.db"), None).expect("open");
        crate::db::init_db(&conn).expect("init");
        (dir, conn)
    }

    #[test]
    fn seeding_writes_the_fixture_company() {
        let (_dir, conn) = test_db();

        assert!(seed_demo(&conn).expect("seed"), "first seed wrote nothing");

        assert_eq!(
            crate::db::get_metadata(&conn, "company_name").as_deref(),
            Some("Acme Consulting LLC")
        );
    }

    #[test]
    fn seeding_twice_is_a_no_op() {
        let (_dir, conn) = test_db();
        seed_demo(&conn).expect("first seed");

        assert!(!seed_demo(&conn).expect("second seed"), "seeded twice");

        let summary = demo_summary(&conn).expect("summary");
        let again = demo_summary(&conn).expect("summary");
        assert_eq!(summary.transactions, again.transactions);
    }

    #[test]
    fn setup_demo_dir_builds_its_own_books_and_repoints_settings() {
        crate::db::set_db_password(None);
        let _config = crate::settings::TempConfigDir::new();
        let base = tempfile::tempdir().expect("tempdir");
        let mut settings = crate::settings::load_settings();
        settings.data_dir = base.path().to_string_lossy().to_string();
        crate::settings::save_settings(&settings).expect("save");

        let db_path = setup_demo_dir().expect("demo dir");

        assert_eq!(db_path, base.path().join("demo").join("nigel.db"));
        assert!(db_path.exists(), "no demo database");
        assert_eq!(
            crate::settings::load_settings().data_dir,
            base.path().join("demo").to_string_lossy().to_string(),
            "settings.json was not repointed"
        );
        let conn = crate::db::get_connection(&db_path).expect("open");
        assert_eq!(
            crate::db::get_metadata(&conn, "company_name").as_deref(),
            Some("Acme Consulting LLC")
        );
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core demo:: -- --test-threads=1`
Expected: FAIL to compile — `demo` is not a module.

- [ ] **Step 3: Move the seeding into the core crate**

Move, verbatim, from `crates/nigel/src/cli/demo.rs` into `crates/nigel-core/src/demo.rs`: `ACCOUNT_NAME`, `RecurringTxn`/`RotatingTxn`/`DemoTxn`/`DemoRule`/`DemoInvoice`/`DemoSummary` and their tables (`RECURRING`, `ROTATING`, `MEALS`, `INCOME_BASES`, `MEAL_AMOUNTS`, `RULES`, `CLIENTS`, `INVOICES`, `DEMO_TERMS`), the helpers (`clamp_day`, `make_date`, `generate_transactions`, `offset_day`, `insert_demo_invoicing`, `insert_demo_data`, `demo_summary`, `seed_demo`), and the existing `#[cfg(test)] mod tests` cases that exercise them. **The fixture cast and every amount move unchanged** — they are invented data for fictional companies and stay that way.

Adjust visibility and paths as you move:

- `ACCOUNT_NAME`, `DemoSummary` and its fields, `demo_summary`, `seed_demo` become `pub`.
- `nigel_core::` prefixes become `crate::` (`crate::db`, `crate::categorizer::categorize_transactions`, `crate::invoicing::…`, `crate::settings::load_settings`).
- File header:

```rust
//! The demo books: eighteen months of invented transactions, rules, clients
//! and invoices for a fictional consultancy.
//!
//! It lives in the core crate rather than the CLI because the web's setup
//! route offers the same "show me the demo" exit the terminal's onboarding
//! does, and two seeders would drift.

use std::path::PathBuf;

use chrono::{Datelike, Local, NaiveDate};
use rusqlite::Connection;

use crate::categorizer::categorize_transactions;
use crate::db::{get_connection, init_db};
use crate::error::Result;
use crate::invoicing::clients::add_client;
use crate::settings::load_settings;
```

Then add `setup_demo_dir`, which is `cli/demo.rs::setup_demo` with a path handed back:

```rust
/// Build `<data_dir>/demo/` with its own seeded database and repoint
/// `settings.data_dir` at it, so the demo never touches the user's own books.
///
/// The demo is business books — its rules name business categories — so the
/// directory is initialized on the business chart whatever profile the caller
/// keeps next door.
pub fn setup_demo_dir() -> Result<PathBuf> {
    let mut settings = load_settings();
    let demo_dir = PathBuf::from(&settings.data_dir).join("demo");
    for dir in [demo_dir.clone(), demo_dir.join("exports")] {
        std::fs::create_dir_all(&dir)?;
        crate::settings::restrict_dir_permissions(&dir)?;
    }

    let db_path = demo_dir.join("nigel.db");
    let conn = get_connection(&db_path)?;
    init_db(&conn)?;
    seed_demo(&conn)?;
    drop(conn);

    settings.data_dir = demo_dir.to_string_lossy().to_string();
    crate::settings::save_settings(&settings)?;

    Ok(db_path)
}
```

Register it in `crates/nigel-core/src/lib.rs` between `db` and `error`:

```rust
pub mod db;
pub mod demo;
pub mod error;
```

- [ ] **Step 4: Reduce the CLI to its wrapper**

`crates/nigel/src/cli/demo.rs` becomes only the stdin/stdout concerns:

```rust
//! `nigel demo` — the CLI's front door to `nigel_core::demo`.

use std::path::PathBuf;

use nigel_core::db::{get_connection, init_db};
use nigel_core::demo::{demo_summary, seed_demo, ACCOUNT_NAME};
use nigel_core::error::Result;
use nigel_core::settings::load_settings;

pub fn run() -> Result<()> {
    let settings = load_settings();
    let db_path = PathBuf::from(&settings.data_dir).join("nigel.db");

    if !db_path.exists() {
        eprintln!("No database found. Run `nigel init` first.");
        std::process::exit(1);
    }

    let conn = get_connection(&db_path)?;
    init_db(&conn)?;

    // Demo data is business books: its rules name business categories, so on
    // a personal chart the inserts would fail partway through the category
    // lookups, leaving transactions with no import row for `nigel undo`.
    if nigel_core::db::get_profile(&conn) == nigel_core::db::Profile::Personal {
        eprintln!("These books are personal, and the demo data is a business (its rules");
        eprintln!("name business categories). Try it in its own directory instead:");
        eprintln!("  nigel init --data-dir ~/nigel-demo && nigel demo");
        std::process::exit(1);
    }

    if !seed_demo(&conn)? {
        println!(
            "Demo data already loaded (account '{}' exists).",
            ACCOUNT_NAME
        );
        return Ok(());
    }
    let summary = demo_summary(&conn)?;

    println!("Demo data loaded!");
    println!("  Account:      {ACCOUNT_NAME}");
    println!("  Transactions: {}", summary.transactions);
    println!("  Rules:        {}", summary.rules);
    println!("  Categorized:  {}", summary.categorized);
    println!("  Flagged:      {}", summary.flagged);
    println!("  Clients:      {}", summary.clients);
    println!("  Invoices:     {}", summary.invoices);
    println!();
    println!("Try these next:");
    println!("  nigel accounts list");
    println!("  nigel rules list");
    println!("  nigel report pnl");
    println!("  nigel report flagged");
    println!("  nigel review");
    println!("  nigel invoice list");

    Ok(())
}

/// Create a demo data directory and switch settings to point at it, so the
/// user's real books stay clean.
pub fn setup_demo() -> Result<()> {
    nigel_core::demo::setup_demo_dir()?;
    Ok(())
}
```

`cli/dashboard.rs` still calls `super::demo::setup_demo()` for `PostSetupAction::Demo` — unchanged.

- [ ] **Step 5: Run everything**

Run: `cargo test -p nigel-core demo:: -- --test-threads=1`
Expected: PASS — the three new cases plus every case that moved.

Run: `cargo test -- --test-threads=1`
Expected: PASS, whole tree.

Run: `cargo fmt --check && cargo clippy -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 6: Sweep and commit**

Run: `./scripts/check-no-real-data.sh --staged`
Expected: **exit 0.** This task stages a large block of moved fixture data, so read the exit status and nothing else. Every figure in it belongs to a fictional consultancy and every name is from the fixture cast; if the script hard-fails, something real got in and the commit stops.

---

### Task 3: The server tells the truth about an empty machine

**Files:**
- Modify: `crates/nigel-core/src/server/routes/status.rs`, `crates/nigel-core/src/server/routes/mod.rs`, `crates/nigel/src/cli/serve.rs`
- Create: `crates/nigel-core/src/server/routes/setup.rs`

**Interfaces:**
- Consumes: `crate::setup::{SetupPlan, run}`, `crate::demo::setup_demo_dir`, `AppState::{db_gate, db_path, set_db_path}`, `status::{current_status, StatusResponse}`, `ApiError::{bad_request, conflict, internal}`, `ApiJson`, `Secret`.
- Produces:
  ```rust
  // routes/status.rs
  pub(crate) fn initialized(db_path: &Path) -> bool;   // exists AND non-empty

  // routes/setup.rs
  pub fn routes() -> Router<AppState>;                 // POST /setup
  // body:   { userName, companyName, profile, password?, action: "fresh" | "demo" }
  // answer: 200 StatusResponse | 400 bad_request | 409 conflict{reason:"already_initialized"}

  // cli/serve.rs
  pub(crate) fn preflight(db_path: &Path) -> Result<()>;
  ```

- [ ] **Step 1: Write the failing tests**

Append to `crates/nigel-core/src/server/routes/status.rs`'s existing `mod tests`:

```rust
    #[tokio::test]
    async fn a_zero_byte_file_is_not_initialized() {
        // A stray connection leaves a zero-byte file behind. It is not books,
        // and calling it initialized is what strands a first run on a broken
        // dashboard instead of sending it to setup.
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("nigel.db");
        std::fs::write(&db_path, b"").expect("touch");
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/status", &token).await;

        assert_eq!(body["initialized"], false);
    }

    #[tokio::test]
    async fn an_absent_file_is_not_initialized() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (app, token) = app_for(&dir.path().join("nigel.db"));

        let body = ok_json(&app, "/api/status", &token).await;

        assert_eq!(body["initialized"], false);
        assert_eq!(body["locked"], false, "an absent database cannot be locked");
    }

    #[tokio::test]
    async fn a_seeded_database_is_initialized() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/status", &token).await;

        assert_eq!(body["initialized"], true);
    }
```

Create `crates/nigel-core/src/server/routes/setup.rs` holding only:

```rust
#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use axum::http::StatusCode;
    use serde_json::json;

    /// A data directory with no database in it, with settings.json redirected
    /// to a temporary config dir and pointed at that directory.
    fn empty_books() -> (TempConfig, tempfile::TempDir, std::path::PathBuf) {
        crate::db::set_db_password(None);
        let config = TempConfig::new();
        let dir = tempfile::tempdir().expect("tempdir");
        let mut settings = crate::settings::load_settings();
        settings.data_dir = dir.path().to_string_lossy().to_string();
        crate::settings::save_settings(&settings).expect("save settings");
        let db_path = dir.path().join("nigel.db");
        (config, dir, db_path)
    }

    fn fresh_body() -> serde_json::Value {
        json!({
            "userName": "Marta",
            "companyName": "Cedar Systems",
            "profile": "business",
            "action": "fresh"
        })
    }

    #[tokio::test]
    async fn a_fresh_setup_answers_ready_status() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(&app, "/api/setup", &token, &fresh_body()).await;

        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["initialized"], true);
        assert_eq!(body["locked"], false);
        assert_eq!(body["encrypted"], false);
        assert_eq!(body["companyName"], "Cedar Systems");
        assert_eq!(body["profile"], "business");
        assert_eq!(crate::settings::load_settings().user_name, "Marta");
    }

    #[tokio::test]
    async fn a_personal_setup_keeps_the_personal_chart() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["profile"] = json!("personal");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["profile"], "personal");
    }

    #[tokio::test]
    async fn a_demo_setup_rebinds_to_the_demo_books() {
        let (_config, dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["action"] = json!("demo");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["companyName"], "Acme Consulting LLC");
        let demo_dir = std::fs::canonicalize(dir.path().join("demo")).expect("canonicalize");
        assert_eq!(
            std::fs::canonicalize(answer["dataDir"].as_str().expect("dataDir")).expect("canonicalize"),
            demo_dir,
            "the server is still serving the empty books"
        );

        // The rebind is what makes the next read land on the demo: a rewritten
        // settings.json alone would leave every request on the old path.
        let accounts = ok_json(&app, "/api/accounts", &token).await;
        let names: Vec<&str> = accounts
            .as_array()
            .expect("array")
            .iter()
            .map(|a| a["name"].as_str().expect("name"))
            .collect();
        assert!(names.contains(&"BofA Checking"), "demo account missing: {names:?}");
    }

    #[tokio::test]
    async fn a_password_leaves_the_database_encrypted_and_unlocked() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["password"] = json!("correct horse battery staple");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["encrypted"], true);
        assert_eq!(answer["locked"], false, "setup locked the user straight back out");
        assert!(crate::db::is_encrypted(&db_path).expect("probe"));
        crate::db::set_db_password(None);
    }

    #[tokio::test]
    async fn a_password_never_appears_in_the_answer() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["password"] = json!("correct horse battery staple");

        let (_status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert!(!answer.to_string().contains("correct horse"), "{answer}");
        crate::db::set_db_password(None);
    }

    #[tokio::test]
    async fn setting_up_twice_is_a_conflict() {
        // Setup is not re-runnable, and the guard is the route's rather than
        // the client's: a second call must not walk over existing books.
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        post_json(&app, "/api/setup", &token, &fresh_body()).await;

        let (status, body) = post_json(&app, "/api/setup", &token, &fresh_body()).await;

        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "already_initialized");
    }

    #[tokio::test]
    async fn an_unknown_profile_is_refused() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["profile"] = json!("corporate");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{answer}");
        assert!(!db_path.exists(), "a bad profile still created books");
    }

    #[tokio::test]
    async fn an_unknown_field_is_refused() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["dataDir"] = json!("/somewhere/else");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{answer}");
    }

    #[tokio::test]
    async fn an_empty_password_is_treated_as_no_password() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["password"] = json!("   ");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["encrypted"], false);
    }

    #[tokio::test]
    async fn setup_needs_a_session_in_web_mode() {
        let (_config, _dir, db_path) = empty_books();
        let (app, _token) = app_for(&db_path);

        let (status, _body) = send(
            &app,
            session_request("POST", "/api/setup", "not-the-token", Some(&fresh_body().to_string())),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(!db_path.exists(), "an unauthenticated call created books");
    }
}
```

Append to `crates/nigel/src/cli/serve.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_leaves_an_absent_database_absent() {
        // A web-first user must reach setup rather than silently getting
        // default books nobody asked for.
        nigel_core::db::set_db_password(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("nigel.db");

        preflight(&db_path).expect("preflight");

        assert!(!db_path.exists(), "preflight created a database");
    }

    #[test]
    fn preflight_migrates_an_existing_plaintext_database() {
        nigel_core::db::set_db_password(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("nigel.db");
        let conn = nigel_core::db::open_connection(&db_path, None).expect("open");
        nigel_core::db::init_db(&conn).expect("init");
        drop(conn);

        preflight(&db_path).expect("preflight");

        let conn = nigel_core::db::open_connection(&db_path, None).expect("reopen");
        assert_eq!(
            nigel_core::db::get_profile(&conn),
            nigel_core::db::Profile::Business
        );
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core -- --test-threads=1 status:: setup::`
Expected: FAIL — `setup` is not a route module; the zero-byte case reports `initialized: true`.

Run: `cargo test -- --test-threads=1 serve::`
Expected: FAIL to compile — `preflight` does not exist.

- [ ] **Step 3: Tighten `initialized`**

In `crates/nigel-core/src/server/routes/status.rs`, add above `current_status`:

```rust
/// Whether there are books here: the file exists **and** has been written to.
///
/// A zero-byte file is what a stray connection leaves behind. Reading it as
/// initialized is what puts a first run in front of a dashboard over nothing.
pub(crate) fn initialized(db_path: &Path) -> bool {
    std::fs::metadata(db_path).is_ok_and(|meta| meta.len() > 0)
}
```

and inside `current_status`, replace `let initialized = db_path.exists();` with:

```rust
    let initialized = initialized(&db_path);
```

- [ ] **Step 4: Write the setup route**

Put this above the test module in `crates/nigel-core/src/server/routes/setup.rs`:

```rust
//! `POST /api/setup` — creating a set of books from the browser or the desktop
//! shell, the same four answers the terminal's onboarding collects.
//!
//! Setup runs once. The guard is here rather than in the client: a second call
//! is a conflict, so no client bug can walk over books that already exist. It
//! needs no exemption from the locked guard — an uninitialized database cannot
//! be locked — and in web mode the session guard applies as it does everywhere
//! else, since the user arrived through the token URL.

use std::path::PathBuf;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use crate::db;
use crate::setup::SetupPlan;

use super::super::error::{ApiError, ApiResult};
use super::super::extract::ApiJson;
use super::super::secret::Secret;
use super::super::state::AppState;
use super::status::{current_status, initialized, StatusResponse};

pub fn routes() -> Router<AppState> {
    Router::new().route("/setup", post(post_setup))
}

/// What to do once the books exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SetupAction {
    Fresh,
    Demo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetupRequest {
    user_name: String,
    company_name: String,
    profile: String,
    #[serde(default)]
    password: Option<Secret>,
    action: SetupAction,
}

async fn post_setup(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<SetupRequest>,
) -> ApiResult<Json<StatusResponse>> {
    let Some(profile) = db::Profile::parse(request.profile.trim()) else {
        return Err(ApiError::bad_request(format!(
            "Unknown profile '{}'. Expected 'business' or 'personal'.",
            request.profile.trim()
        )));
    };

    // Trimmed as the terminal's prompt trims, so a password set here can
    // always be typed back in. Control characters cannot survive the
    // `PRAGMA key = '…'` every later open applies, and would lock the owner
    // out permanently.
    let password = match request.password.as_ref().map(|secret| secret.expose().trim()) {
        None | Some("") => None,
        Some(value) if value.chars().any(char::is_control) => {
            return Err(ApiError::bad_request(
                "The password cannot contain control characters.",
            ))
        }
        Some(value) => Some(value.to_string()),
    };

    let plan = SetupPlan {
        user_name: request.user_name.trim().to_string(),
        company_name: request.company_name.trim().to_string(),
        profile,
        password,
    };
    let action = request.action;

    {
        // The write side, as the data-directory switch takes it: this creates
        // a database file and then rebinds the path every later request reads.
        let _gate = state.db_gate.write().await;

        if initialized(&state.db_path()) {
            return Err(ApiError::conflict(
                "These books are already set up.",
                serde_json::json!({ "reason": "already_initialized" }),
            ));
        }

        let db_path = tokio::task::spawn_blocking(move || -> ApiResult<PathBuf> {
            let fresh = crate::setup::run(&plan)?;
            match action {
                SetupAction::Fresh => Ok(fresh),
                // Its own directory and its own database, so the demo never
                // sits on top of the books the user is about to keep.
                SetupAction::Demo => Ok(crate::demo::setup_demo_dir()?),
            }
        })
        .await
        .map_err(ApiError::internal)??;

        state.set_db_path(db_path);
    }

    Ok(Json(current_status(&state).await?))
}
```

Register it in `crates/nigel-core/src/server/routes/mod.rs` — the module list, alphabetically after `settings`:

```rust
pub mod settings;
pub mod setup;
pub mod status;
```

and in `data_router()`, after the settings merge:

```rust
        .merge(settings::routes())
        .merge(setup::routes())
```

- [ ] **Step 5: Stop `nigel serve` creating an absent database**

Rewrite `crates/nigel/src/cli/serve.rs`'s body (keeping the `#[cfg(not(feature = "serve"))]` arm exactly as it is):

```rust
//! `nigel serve` — the dispatch seam for the web server.

use std::path::Path;

use nigel_core::error::Result;

/// Migrate a database that is already there; leave an absent one absent.
///
/// An encrypted file is skipped — `serve` is exempt from the stdin password
/// prompt, so it is still locked here and the unlock endpoint runs its
/// migrations once the key arrives. An absent one is left for the setup gate,
/// which is where a machine with no books belongs.
pub(crate) fn preflight(db_path: &Path) -> Result<()> {
    if !db_path.exists() || nigel_core::db::is_encrypted(db_path)? {
        return Ok(());
    }
    let conn = nigel_core::db::get_connection(db_path)?;
    nigel_core::db::init_db(&conn)
}

#[cfg(feature = "serve")]
pub fn run(port: u16, no_open: bool) -> Result<()> {
    preflight(&nigel_core::settings::get_data_dir().join("nigel.db"))?;
    nigel_core::server::run(port, no_open)
}
```

- [ ] **Step 6: Run them and watch them pass**

Run: `cargo test -p nigel-core -- --test-threads=1`
Expected: PASS, including 11 new `routes::setup::tests::*` and 3 new `routes::status::tests::*`.

Run: `cargo test -- --test-threads=1`
Expected: PASS, whole tree, including `cli::serve::tests::*`.

Run: `cargo test --no-default-features -- --test-threads=1`
Expected: PASS — `preflight` is outside the `serve` cfg, so it is still compiled and tested.

Run: `cargo fmt --check && cargo clippy -- -D warnings`
Expected: no output, exit 0.

- [ ] **Step 7: Commit**

`./scripts/check-no-real-data.sh --staged` (exit 0), then commit.

---

### Task 4: The SPA gains a `needs-setup` boot phase

**Files:**
- Modify: `web/apps/app/src/state/app-store.ts`, `web/apps/app/src/state/app-store.test.ts`, `web/apps/app/src/snake-trigger.ts`, `web/apps/app/src/__mocks__/fake-api-client.ts`

**Interfaces:**
- Consumes: `StatusResponse.initialized`, `appLocked`.
- Produces:
  ```ts
  export type BootPhase = 'starting' | 'locked' | 'needs-setup' | 'failed' | 'ready';
  export const UNINITIALIZED_STATUS: StatusResponse;   // fake-api-client
  ```

- [ ] **Step 1: Write the failing tests**

Add to the `describe('app store boot phase', …)` block in `web/apps/app/src/state/app-store.test.ts`:

```ts
  it('needs setup when the database has never been created', async () => {
    const client = new FakeApiClient();
    client.status = UNINITIALIZED_STATUS;
    const store = initializeAppStore(client);
    await store.refreshStatus();
    expect(store.boot.get()).toBe('needs-setup');
  });

  it('is locked rather than needing setup when both could be claimed', async () => {
    // An encrypted file exists, so the key is the question — not setup.
    // Offering setup here would be offering to overwrite somebody's books.
    const client = new FakeApiClient();
    client.status = { ...LOCKED_STATUS, initialized: false };
    const store = initializeAppStore(client);
    await store.refreshStatus();
    expect(store.boot.get()).toBe('locked');
  });

  it('is ready once setup has answered with initialized books', async () => {
    const client = new FakeApiClient();
    client.status = UNINITIALIZED_STATUS;
    const store = initializeAppStore(client);
    await store.refreshStatus();
    expect(store.boot.get()).toBe('needs-setup');

    await client.setup({
      userName: 'Marta',
      companyName: 'Cedar Systems',
      profile: 'business',
      action: 'fresh',
    });
    await store.refreshStatus();

    expect(store.boot.get()).toBe('ready');
  });
```

Update the import at the top of the file:

```ts
import {
  FakeApiClient,
  LOCKED_STATUS,
  UNINITIALIZED_STATUS,
} from '../__mocks__/fake-api-client.js';
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cd web && npm test --workspace=@nigel/app -- app-store`
Expected: FAIL — `UNINITIALIZED_STATUS` is not exported, `client.setup` is not a function.

- [ ] **Step 3: Add the phase and the fixture**

In `web/apps/app/src/state/app-store.ts`, extend the doc comment and the union:

```ts
/**
 * Where the app is in its boot sequence.
 *
 * `starting` — the first `/api/status` has not answered yet.
 * `locked` — the database is encrypted and this process has no key, so the
 * unlock gate is the only thing rendered and no screen exists to fetch data.
 * `needs-setup` — there is no database yet, so the setup gate collects the
 * four answers a set of books is created from.
 * `failed` — status could not be read at all.
 * `ready` — the app proper.
 */
export type BootPhase = 'starting' | 'locked' | 'needs-setup' | 'failed' | 'ready';
```

and the derivation:

```ts
    boot: computed((): BootPhase => {
      const locked = (_status.get()?.locked ?? false) || appLocked.get();
      // Locked wins: an encrypted file is somebody's books, and offering to
      // set up over it would be offering to replace them.
      if (locked) return 'locked';
      const status = _status.get();
      if (status && !status.initialized) return 'needs-setup';
      if (_statusError.get()) return 'failed';
      if (!status) return 'starting';
      return 'ready';
    }),
```

In `web/apps/app/src/snake-trigger.ts`, add the new phase to the exhaustive switch — the game needs a dashboard to cover:

```ts
    case 'starting':
    case 'locked':
    case 'needs-setup':
    case 'failed':
      return false;
```

In `web/apps/app/src/__mocks__/fake-api-client.ts`, after `LOCKED_STATUS`:

```ts
/** A machine that has never run Nigel: settings.json may exist, books do not. */
export const UNINITIALIZED_STATUS: StatusResponse = {
  ...UNLOCKED_STATUS,
  initialized: false,
  companyName: null,
};
```

The api seam gains its `setup` method here, in the four places a method exists — `types.ts`, the `ApiClient` interface, `FetchApiClient`, and `FakeApiClient` — so this task's third test compiles and Task 6 has something to call. The desktop client is a subclass of `FetchApiClient`, so it inherits it and no `__TAURI__` branch is involved.

On `FakeApiClient`:

```ts
  setupError: Error | null = null;

  async setup(input: SetupRequest): Promise<StatusResponse> {
    this.calls.push(`setup:${JSON.stringify(input)}`);
    if (this.setupError) throw this.setupError;
    this.status = {
      ...this.status,
      initialized: true,
      companyName: input.companyName || null,
      profile: input.profile,
    };
    return this.status;
  }
```

with `SetupRequest` added to the type import block from `'../api/types.js'`. The type itself, in `web/apps/app/src/api/types.ts`:

```ts
// web/apps/app/src/api/types.ts
/** `POST /api/setup` — what to do once the books exist. */
export type SetupAction = 'fresh' | 'demo';

/** `POST /api/setup` */
export interface SetupRequest {
  userName: string;
  companyName: string;
  profile: BooksProfile;
  /** Absent or empty leaves the database unencrypted. */
  password?: string;
  action: SetupAction;
}
```

```ts
// web/apps/app/src/api/client.ts — in `interface ApiClient`, beside unlock
  /** Create the books. Answers with the status of what it created. */
  setup(input: SetupRequest): Promise<StatusResponse>;
```

and the real implementation on `FetchApiClient`, beside `unlock`:

```ts
  async setup(input: SetupRequest): Promise<StatusResponse> {
    const status = await this.request<StatusResponse>('POST', '/setup', input);
    // A password in the plan encrypts the database; this answer is the
    // authority on the resulting lock state the same way getStatus is.
    appLocked.set(status.locked);
    return status;
  }
```

Add `type SetupRequest` to the type import list in `client.ts`.

- [ ] **Step 4: Run them and watch them pass**

Run: `cd web && npm test --workspace=@nigel/app -- app-store`
Expected: PASS — the three new cases plus every existing boot-phase case.

Run: `cd web && npm run typecheck && npm run lint`
Expected: clean. `snakeAllowedOnBoot`'s `never` arm would have failed the typecheck had the switch not been updated.

- [ ] **Step 5: Commit**

`./scripts/check-no-real-data.sh --staged` (exit 0), then commit.

---

### Task 5: `wc-wordmark` and `wc-particle-field` in `@nigel/ui`

**Files:**
- Modify: `web/packages/theme/src/tokens/gradient.ts`, `web/packages/theme/src/tokens/color.ts`, `web/packages/theme/src/print.ts`, `web/packages/theme/__tests__/nigel-theme.test.ts`, `web/packages/ui/src/components/wc-snake.ts`, `web/packages/ui/src/components/index.ts`
- Create: `web/packages/ui/src/components/particle-field.ts`, `wc-particle-field.ts`, `wc-particle-field.preview.ts`, `wc-particle-field.test.ts`, `wc-wordmark.ts`, `wc-wordmark.preview.ts`, `wc-wordmark.test.ts`, `wordmark-parity.test.ts`

**Interfaces:**
- Consumes: `NIGEL_PALETTE`, `MAX_PARTICLES`, `PARTICLE_CHARS`, `Rng` from `snake-engine.ts`.
- Produces:
  ```ts
  // theme
  // --nc-grad-brand-text-cycle : the wordmark ramp as a periodic image,
  //   sized by the existing --nc-grad-brand-size.

  // particle-field.ts
  export interface FieldParticle {
    left: string; rest: string; duration: string; delay: string;
    tint: string; brightness: string; glyph: string;
  }
  export function seedParticleField(rng?: Rng, density?: number): FieldParticle[];
  export function prefersReducedMotion(): boolean;

  // wc-particle-field.ts
  export class WcParticleField extends LitElement {
    density: number;          // capped at MAX_PARTICLES
    reducedMotion: boolean;   // reflected as `reduced-motion`
  }

  // wc-wordmark.ts
  export const WORDMARK_ART: readonly string[];   // pinned to effects.rs LOGO
  export class WcWordmark extends LitElement {
    animated: boolean;        // reflected
    reveal: number;           // 0..1, the fraction of characters shown
    label: string;            // the accessible name; default 'Nigel'
    reducedMotion: boolean;   // reflected as `reduced-motion`
  }
  ```

- [ ] **Step 1: Write the failing tests**

`web/packages/ui/src/components/wordmark-parity.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { WORDMARK_ART } from './wc-wordmark.js';

/**
 * The web wordmark is meant to *be* the TUI's wordmark, not a redrawing of it,
 * which is `palette-parity.test.ts`'s claim about the colours applied to the
 * shape. This reads the Rust source and fails if the two drift apart.
 */
const here = dirname(fileURLToPath(import.meta.url));
const effectsRs = resolve(here, '../../../../../crates/nigel/src/effects.rs');

function logoFromRust(): string[] {
  const source = readFileSync(effectsRs, 'utf8');
  const start = source.indexOf('pub const LOGO');
  expect(start, 'LOGO const not found in crates/nigel/src/effects.rs').toBeGreaterThan(-1);
  const end = source.indexOf('];', start);
  return [...source.slice(start, end).matchAll(/^\s*r"(.*)",$/gm)].map((m) => m[1]);
}

describe('wordmark parity with crates/nigel/src/effects.rs', () => {
  it('reads a non-empty logo out of the Rust source', () => {
    expect(logoFromRust().length).toBeGreaterThan(0);
  });

  it('matches WORDMARK_ART exactly, in order', () => {
    expect(logoFromRust()).toEqual([...WORDMARK_ART]);
  });
});
```

`web/packages/ui/src/components/wc-wordmark.test.ts`:

```ts
import { describe, it, expect, afterEach } from 'vitest';
import './wc-wordmark.js';
import { WORDMARK_ART, type WcWordmark } from './wc-wordmark.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-wordmark.preview.js';

async function mount(props: Partial<WcWordmark> = {}): Promise<WcWordmark> {
  const el = document.createElement('wc-wordmark');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const chars = (el: WcWordmark) => [...(el.shadowRoot?.querySelectorAll('.char') ?? [])];

describe('wc-wordmark', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one span per character of the art', async () => {
    const el = await mount();
    const expected = WORDMARK_ART.reduce((total, line) => total + line.length, 0);
    expect(chars(el)).toHaveLength(expected);
  });

  it('names itself for a screen reader instead of reading the ascii aloud', async () => {
    const el = await mount();
    const art = el.shadowRoot?.querySelector('.art');
    expect(art?.getAttribute('role')).toBe('img');
    expect(art?.getAttribute('aria-label')).toBe('Nigel');
  });

  it('takes a custom accessible name', async () => {
    const el = await mount({ label: 'Nigel — bookkeeping' });
    expect(el.shadowRoot?.querySelector('.art')?.getAttribute('aria-label')).toBe(
      'Nigel — bookkeeping',
    );
  });

  it('staggers each character so one ramp sweeps across the whole mark', async () => {
    const el = await mount({ animated: true });
    const delays = chars(el).map((c) => (c as HTMLElement).style.animationDelay);
    expect(new Set(delays).size).toBeGreaterThan(1);
    expect(delays.every((d) => d.startsWith('-') && d.endsWith('s'))).toBe(true);
  });

  it('shows everything at a full reveal', async () => {
    const el = await mount({ reveal: 1 });
    const hidden = chars(el).filter((c) => c.classList.contains('hidden'));
    expect(hidden).toHaveLength(0);
  });

  it('shows nothing at a zero reveal, and keeps the space it will take', async () => {
    // Hidden rather than absent: a wordmark that grows into place shoves the
    // form underneath it down mid-animation.
    const el = await mount({ reveal: 0 });
    const drawn = chars(el).filter(
      (c) => !c.classList.contains('hidden') && c.textContent?.trim(),
    );
    expect(drawn).toHaveLength(0);
    expect(chars(el).length).toBeGreaterThan(0);
  });

  it('reveals more characters as the reveal advances', async () => {
    const el = await mount({ reveal: 0.5 });
    const half = chars(el).filter((c) => !c.classList.contains('hidden')).length;
    el.reveal = 0.9;
    await el.updateComplete;
    const most = chars(el).filter((c) => !c.classList.contains('hidden')).length;
    expect(most).toBeGreaterThan(half);
  });

  it('reveals in a stable order as the fraction climbs', async () => {
    // The order is shuffled once per instance; a reshuffle per render would
    // make characters blink out again as the reveal advances.
    const el = await mount({ reveal: 0.4 });
    const shown = chars(el).map((c) => !c.classList.contains('hidden'));
    el.reveal = 0.8;
    await el.updateComplete;
    const later = chars(el).map((c) => !c.classList.contains('hidden'));
    shown.forEach((was, i) => {
      if (was) expect(later[i], `character ${i} went back into hiding`).toBe(true);
    });
  });

  it('does not animate when motion is unwelcome', async () => {
    const el = await mount({ animated: true, reducedMotion: true });
    expect(el.hasAttribute('reduced-motion')).toBe(true);
  });
});

describePreviewA11y(preview);
```

`web/packages/ui/src/components/wc-particle-field.test.ts`:

```ts
import { describe, it, expect, afterEach } from 'vitest';
import './wc-particle-field.js';
import type { WcParticleField } from './wc-particle-field.js';
import { MAX_PARTICLES } from './snake-engine.js';
import { seedParticleField } from './particle-field.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-particle-field.preview.js';

async function mount(props: Partial<WcParticleField> = {}): Promise<WcParticleField> {
  const el = document.createElement('wc-particle-field');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const specks = (el: WcParticleField) => [...(el.shadowRoot?.querySelectorAll('.particle') ?? [])];

describe('seedParticleField', () => {
  it('never exceeds the TUI cap however many are asked for', () => {
    expect(seedParticleField(() => 0.5, 500)).toHaveLength(MAX_PARTICLES);
  });

  it('draws every field from the shared glyph and palette sets', () => {
    const field = seedParticleField(() => 0.5, 4);
    expect(field).toHaveLength(4);
    for (const speck of field) {
      expect(speck.glyph).toMatch(/[·∘•◦]/);
      expect(speck.tint).toMatch(/^#[0-9a-f]{6}$/i);
    }
  });
});

describe('wc-particle-field', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the default density', async () => {
    const el = await mount();
    expect(specks(el)).toHaveLength(MAX_PARTICLES);
  });

  it('honours a lower density', async () => {
    const el = await mount({ density: 6 });
    expect(specks(el)).toHaveLength(6);
  });

  it("caps density at the TUI's own limit", async () => {
    const el = await mount({ density: 500 });
    expect(specks(el)).toHaveLength(MAX_PARTICLES);
  });

  it('is decoration and says so', async () => {
    // Drifting punctuation read aloud one glyph at a time is not information.
    const el = await mount();
    expect(el.getAttribute('aria-hidden')).toBe('true');
  });

  it('still renders the specks when motion is unwelcome, and stops them', async () => {
    const el = await mount({ reducedMotion: true });
    expect(specks(el)).toHaveLength(MAX_PARTICLES);
    expect(el.hasAttribute('reduced-motion')).toBe(true);
  });
});

describePreviewA11y(preview);
```

Add to `web/packages/theme/__tests__/nigel-theme.test.ts`:

```ts
  it('declares a periodic wordmark ramp in both modes', () => {
    const declared = declarationsOf('--nc-grad-brand-text-cycle');
    expect(declared.length).toBeGreaterThanOrEqual(2);
    for (const value of declared) {
      expect(value).toContain('repeating-linear-gradient');
    }
  });

  it('closes the wordmark ramp on the colour it opened with', () => {
    // A ramp whose ends disagree shows a seam once per cycle, which is the
    // whole reason the periodic form exists.
    const [light] = declarationsOf('--nc-grad-brand-text-cycle');
    const stops = [...light.matchAll(/#[0-9a-f]{6}/gi)].map((m) => m[0].toLowerCase());
    expect(stops.at(0)).toBe(stops.at(-1));
  });
```

with `declarationsOf` imported from `./token-resolution.js` if the file does not already import it.

- [ ] **Step 2: Run them and watch them fail**

Run: `cd web && npm test --workspace=@nigel/ui -- wordmark particle`
Expected: FAIL — none of the modules exist.

Run: `cd web && npm test --workspace=@nigel/theme`
Expected: FAIL — `--nc-grad-brand-text-cycle` is not declared.

- [ ] **Step 3: Generalize the ramp in the theme**

In `web/packages/theme/src/tokens/gradient.ts`, replace the block that builds `cycleStops`/`cycle`/`cycleSize` with a shared builder, and add the ink cycle:

```ts
/**
 * A ramp as a *periodic* image, so a drift across it can loop.
 *
 * Counted in the ramp's own step — seven stops across the element is six gaps,
 * so a step is a sixth of the element's width. One period is the ramp plus a
 * seventh step wrapping the last stop back to the first, and the image is a
 * period plus the ramp: thirteen steps. `--nc-grad-brand-size` says exactly
 * that, and it is the whole trick — `background-position: 100%` offsets by
 * image minus element, which is one period, so the loop has no seam.
 */
const GAPS = NIGEL_PALETTE.length - 1;
const PERIOD_STEPS = NIGEL_PALETTE.length;
const IMAGE_STEPS = GAPS + PERIOD_STEPS;
const at = (step: number): string => `${((step / IMAGE_STEPS) * 100).toFixed(4)}%`;

const cycleFrom = (palette: readonly string[]): string => {
  const stops = [
    ...palette.map((color, i) => `${color} ${at(i)}`),
    `${palette[0]} ${at(PERIOD_STEPS)}`,
  ].join(', ');
  return `repeating-linear-gradient(90deg, ${stops})`;
};

const cycle = unsafeCSS(cycleFrom(NIGEL_PALETTE));
const inkCycle = unsafeCSS(cycleFrom(NIGEL_PALETTE_INK));
const cycleSize = unsafeCSS(`${((IMAGE_STEPS / GAPS) * 100).toFixed(4)}% 100%`);
```

Both ramps have seven stops, so one `--nc-grad-brand-size` serves both. In `gradientCss`'s `:root`, beside `--nc-grad-brand-text`:

```css
    /* The same ramp made periodic, for the wordmark's drift. Sized by
       --nc-grad-brand-size like every other periodic image here. */
    --nc-grad-brand-text-cycle: ${inkCycle};
```

In `web/packages/theme/src/tokens/color.ts`'s `darkTokens`, beside the existing `--nc-grad-brand-text`:

```ts
  --nc-grad-brand-text: ${brandRamp};
  --nc-grad-brand-text-cycle: ${brandCycle};
```

exporting `brandCycle` from `gradient.ts` as `export const brandCycle = css\`${cycle}\`;` and importing it in `color.ts` alongside `brandRamp`. In `web/packages/theme/src/print.ts`, beside the other three:

```css
      --nc-grad-brand-text-cycle: none;
```

- [ ] **Step 4: Extract the particle field**

Create `web/packages/ui/src/components/particle-field.ts`:

```ts
import { NIGEL_PALETTE } from '@nigel/theme';
import { MAX_PARTICLES, PARTICLE_CHARS, type Rng } from './snake-engine.js';

/**
 * The drifting specks the TUI puts behind its splash, goodbye, onboarding and
 * Snake screens, as CSS a component can hand straight to `styleMap`.
 *
 * The constants are `snake-engine.ts`'s, which are `crates/nigel/src/effects.rs`'s: the cap
 * and the glyph set are the same in the terminal and the browser, and there is
 * one seeding function rather than one per screen that wants specks.
 */
export interface FieldParticle {
  /** Horizontal position, as a percentage of the field. */
  left: string;
  /** Where in its rise the speck starts, as a percentage. */
  rest: string;
  duration: string;
  delay: string;
  tint: string;
  brightness: string;
  glyph: string;
}

/** Seed a field. `density` is clamped to the TUI's own cap. */
export function seedParticleField(
  rng: Rng = Math.random,
  density: number = MAX_PARTICLES,
): FieldParticle[] {
  const count = Math.max(0, Math.min(Math.floor(density), MAX_PARTICLES));
  return Array.from({ length: count }, () => ({
    left: `${(rng() * 100).toFixed(2)}%`,
    rest: `${(rng() * 100).toFixed(2)}%`,
    duration: `${(9 + rng() * 12).toFixed(2)}s`,
    delay: `${(-rng() * 12).toFixed(2)}s`,
    tint: NIGEL_PALETTE[Math.floor(rng() * NIGEL_PALETTE.length)],
    brightness: (0.2 + rng() * 0.4).toFixed(2),
    glyph: PARTICLE_CHARS[Math.floor(rng() * PARTICLE_CHARS.length)],
  }));
}

/** Whether the viewer has asked for less movement. False where nobody asked. */
export function prefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  );
}
```

In `web/packages/ui/src/components/wc-snake.ts`, delete the local `interface Particle`, `seedParticles` and `prefersReducedMotion`, import them instead, and rename the two use sites:

```ts
import { seedParticleField, prefersReducedMotion } from './particle-field.js';
```

```ts
  private particles = seedParticleField(Math.random);
```

Its `.particle` CSS, the `rise` keyframes, `renderParticles` and the reduced-motion rules stay exactly where they are — the game owns its own board.

Create `web/packages/ui/src/components/wc-particle-field.ts`:

```ts
import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { styleMap } from 'lit/directives/style-map.js';

import { MAX_PARTICLES } from './snake-engine.js';
import {
  prefersReducedMotion,
  seedParticleField,
  type FieldParticle,
} from './particle-field.js';

/**
 * The ambient drift: pastel punctuation rising slowly through whatever it is
 * placed behind.
 *
 * Decoration and nothing else, so the host is `aria-hidden` and the specks are
 * never in the tab order or the accessibility tree. Under reduced motion they
 * are still drawn and simply stop — the field is part of the composition, and
 * removing it would move everything laid out over it.
 */
@customElement('wc-particle-field')
export class WcParticleField extends LitElement {
  static styles = css`
    :host {
      display: block;
      position: relative;
      overflow: hidden;
      pointer-events: none;
    }

    .particle {
      position: absolute;
      top: 0;
      height: 100%;
      line-height: 1;
      font-family: var(--wa-font-family-mono);
      font-size: var(--wa-font-size-s, 13px);
      color: var(--tint);
      opacity: var(--brightness);
      transform: translateY(var(--rest));
      animation: nc-particle-rise var(--duration) linear var(--delay) infinite;
    }

    /* The glyph sits at the top of a field-height box, so one field height of
       travel in each direction carries it across and off. */
    @keyframes nc-particle-rise {
      from {
        transform: translateY(100%);
      }
      to {
        transform: translateY(-100%);
      }
    }

    :host([reduced-motion]) .particle {
      animation: none;
    }

    @media (prefers-reduced-motion: reduce) {
      .particle {
        animation: none;
      }
    }
  `;

  /** How many specks. Clamped to the cap the TUI draws under. */
  @property({ type: Number })
  density = MAX_PARTICLES;

  @property({ type: Boolean, reflect: true, attribute: 'reduced-motion' })
  reducedMotion = prefersReducedMotion();

  private motionQuery: MediaQueryList | null = null;
  private field: FieldParticle[] = [];
  private seededFor = -1;

  connectedCallback(): void {
    super.connectedCallback();
    this.setAttribute('aria-hidden', 'true');
    if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
      this.motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
      this.motionQuery.addEventListener('change', this.handleMotionChange);
    }
  }

  disconnectedCallback(): void {
    this.motionQuery?.removeEventListener('change', this.handleMotionChange);
    this.motionQuery = null;
    super.disconnectedCallback();
  }

  private handleMotionChange = (event: MediaQueryListEvent): void => {
    this.reducedMotion = event.matches;
  };

  render() {
    // Reseeded only when the count changes: a fresh field on every render
    // would teleport every speck each time the host updates.
    if (this.seededFor !== this.density) {
      this.field = seedParticleField(Math.random, this.density);
      this.seededFor = this.density;
    }

    return this.field.map(
      (speck) => html`<span
        class="particle"
        style=${styleMap({
          left: speck.left,
          '--rest': speck.rest,
          '--tint': speck.tint,
          '--brightness': speck.brightness,
          '--duration': speck.duration,
          '--delay': speck.delay,
        })}
        >${speck.glyph}</span
      >`,
    );
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-particle-field': WcParticleField;
  }
}
```

- [ ] **Step 5: Write the wordmark**

Create `web/packages/ui/src/components/wc-wordmark.ts`:

```ts
import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { styleMap } from 'lit/directives/style-map.js';
import { classMap } from 'lit/directives/class-map.js';

import { prefersReducedMotion } from './particle-field.js';

/**
 * The ASCII wordmark, exactly as `crates/nigel/src/effects.rs` declares it.
 *
 * Pinned to the Rust source by `wordmark-parity.test.ts`, so the terminal and
 * the browser cannot end up drawing two different marks.
 */
export const WORDMARK_ART: readonly string[] = [
  '  /$$   /$$ /$$                     /$$',
  ' | $$$ | $$|__/                    | $$',
  ' | $$$$| $$ /$$  /$$$$$$   /$$$$$$ | $$',
  ' | $$ $$ $$| $$ /$$__  $$ /$$__  $$| $$',
  ' | $$  $$$$| $$| $$  \\ $$| $$$$$$$$| $$',
  ' | $$\\  $$$| $$| $$  | $$| $$_____/| $$',
  ' | $$ \\  $$| $$|  $$$$$$$|  $$$$$$$| $$',
  ' |__/  \\__/|__/ \\____  $$ \\_______/|__/',
  '                /$$  \\ $$',
  '               |  $$$$$$/',
  '                \\______/',
] as const;

/** Every drawable position, row-major — spaces carry no colour and no reveal. */
function drawablePositions(): number[] {
  const positions: number[] = [];
  let index = 0;
  for (const line of WORDMARK_ART) {
    for (const char of line) {
      if (char !== ' ') positions.push(index);
      index += 1;
    }
  }
  return positions;
}

/** The TUI's shuffled reveal order, so the mark assembles rather than wipes. */
function shuffled(positions: number[]): number[] {
  const order = [...positions];
  for (let i = order.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1));
    [order[i], order[j]] = [order[j], order[i]];
  }
  return order;
}

/**
 * Nigel's wordmark: the terminal's ASCII art, drawn as per-character spans
 * sharing one animated gradient with staggered delays.
 *
 * Every colour comes from `@nigel/theme` — `--nc-grad-brand-text` static,
 * `--nc-grad-brand-text-cycle` when it drifts, both of which are the ink ramp
 * on a light surface and the pastels on a dark one. Reduced motion renders the
 * static mark at full reveal: the wordmark is the identity, not the animation.
 */
@customElement('wc-wordmark')
export class WcWordmark extends LitElement {
  static styles = css`
    :host {
      display: inline-block;
    }

    .art {
      margin: 0;
      font-family: var(--wa-font-family-mono);
      font-size: var(--nc-wordmark-size, var(--wa-font-size-s, 13px));
      line-height: 1.05;
      white-space: pre;
      font-weight: var(--wa-font-weight-bold, 700);
    }

    .char {
      display: inline;
      background-image: var(--nc-grad-brand-text);
      background-size: 100% 100%;
      -webkit-background-clip: text;
      background-clip: text;
      -webkit-text-fill-color: transparent;
      color: transparent;
    }

    .hidden {
      visibility: hidden;
    }

    :host([animated]) .char {
      background-image: var(--nc-grad-brand-text-cycle);
      background-size: var(--nc-grad-brand-size);
      animation: nc-wordmark-cycle var(--nc-wordmark-duration, 3.5s) linear infinite;
    }

    @keyframes nc-wordmark-cycle {
      from {
        background-position: 0% 50%;
      }
      to {
        background-position: 100% 50%;
      }
    }

    :host([reduced-motion]) .char {
      background-image: var(--nc-grad-brand-text);
      background-size: 100% 100%;
      animation: none;
    }

    @media (prefers-reduced-motion: reduce) {
      .char {
        animation: none;
      }
    }
  `;

  /** Whether the gradient drifts along the mark. */
  @property({ type: Boolean, reflect: true })
  animated = false;

  /** How much of the mark is drawn, 0 to 1. Characters appear in a shuffled order. */
  @property({ type: Number })
  reveal = 1;

  /** The accessible name. The ascii itself is never read aloud. */
  @property({ type: String })
  label = 'Nigel';

  @property({ type: Boolean, reflect: true, attribute: 'reduced-motion' })
  reducedMotion = prefersReducedMotion();

  /** Shuffled once per instance: a reshuffle per render un-draws characters. */
  private readonly order = shuffled(drawablePositions());

  private motionQuery: MediaQueryList | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
      this.motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
      this.motionQuery.addEventListener('change', this.handleMotionChange);
    }
  }

  disconnectedCallback(): void {
    this.motionQuery?.removeEventListener('change', this.handleMotionChange);
    this.motionQuery = null;
    super.disconnectedCallback();
  }

  private handleMotionChange = (event: MediaQueryListEvent): void => {
    this.reducedMotion = event.matches;
  };

  private visiblePositions(): Set<number> {
    // Motion is additive: somebody who asked for less of it gets the whole
    // mark rather than a partial one frozen mid-assembly.
    const fraction = this.reducedMotion ? 1 : Math.max(0, Math.min(this.reveal, 1));
    return new Set(this.order.slice(0, Math.round(this.order.length * fraction)));
  }

  /** Spaces are never hidden — they are blank either way, and hiding them
   * would make a full reveal still carry the hidden class. */
  private isHidden(char: string, index: number, visible: Set<number>): boolean {
    return char !== ' ' && !visible.has(index);
  }

  render() {
    const visible = this.visiblePositions();
    let index = -1;

    return html`
      <pre class="art" role="img" aria-label=${this.label}>${WORDMARK_ART.map(
        (line, row) =>
          html`<span class="line"
              >${[...line].map((char, col) => {
                index += 1;
                return html`<span
                  class=${classMap({ char: true, hidden: this.isHidden(char, index, visible) })}
                  style=${styleMap({
                    animationDelay: `-${(row * 0.15 + col * 0.03).toFixed(2)}s`,
                  })}
                  >${char === ' ' ? '\u00a0' : char}</span
                >`;
              })}</span
            >${row < WORDMARK_ART.length - 1 ? '\n' : ''}`,
      )}</pre>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-wordmark': WcWordmark;
  }
}
```

- [ ] **Step 6: Write the previews**

`web/packages/ui/src/components/wc-wordmark.preview.ts`:

```ts
import { html } from 'lit';
import './wc-wordmark.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-wordmark',
  title: 'Wordmark',
  group: 'Brand',
  description:
    "Nigel's ASCII wordmark, per-character spans sharing one gradient. The art is the TUI's LOGO and a parity test pins the two together.",
  layout: 'stack',
  states: [
    { name: 'static', render: () => html`<wc-wordmark></wc-wordmark>` },
    { name: 'animated', render: () => html`<wc-wordmark animated></wc-wordmark>` },
    {
      name: 'revealing',
      render: () => html`<wc-wordmark animated .reveal=${0.45}></wc-wordmark>`,
    },
    {
      name: 'reduced-motion',
      render: () => html`<wc-wordmark animated reduced-motion .reveal=${0.2}></wc-wordmark>`,
    },
    {
      name: 'on-dark',
      background: 'inverse',
      render: () => html`<wc-wordmark animated></wc-wordmark>`,
    },
    {
      name: 'labelled',
      render: () => html`<wc-wordmark label="Nigel — bookkeeping"></wc-wordmark>`,
    },
  ],
};

export default preview;
```

`web/packages/ui/src/components/wc-particle-field.preview.ts`:

```ts
import { html } from 'lit';
import './wc-particle-field.js';
import type { Preview } from '../../preview/types.js';

const box = (content: unknown) =>
  html`<div style="position: relative; height: 240px; background: var(--nc-color-arcade-bg, #1f1f28);">
    ${content}
  </div>`;

const preview: Preview = {
  id: 'wc-particle-field',
  title: 'Particle Field',
  group: 'Brand',
  description:
    "The TUI's drifting specks, capped at the same twenty. Decorative, aria-hidden, still and silent under reduced motion.",
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () => box(html`<wc-particle-field style="position:absolute;inset:0"></wc-particle-field>`),
    },
    {
      name: 'sparse',
      render: () =>
        box(html`<wc-particle-field density="6" style="position:absolute;inset:0"></wc-particle-field>`),
    },
    {
      name: 'reduced-motion',
      render: () =>
        box(
          html`<wc-particle-field reduced-motion style="position:absolute;inset:0"></wc-particle-field>`,
        ),
    },
  ],
};

export default preview;
```

Export both from `web/packages/ui/src/components/index.ts`, beside `WcSnake`:

```ts
export { WcSnake } from './wc-snake.js';
export { WcParticleField } from './wc-particle-field.js';
export { seedParticleField, prefersReducedMotion, type FieldParticle } from './particle-field.js';
export { WcWordmark, WORDMARK_ART } from './wc-wordmark.js';
```

- [ ] **Step 7: Run them and watch them pass**

Run: `cd web && npm test --workspace=@nigel/theme`
Expected: PASS, including the two new ramp cases and every existing contrast case — the new token is name-resolved, so no existing assertion shifts.

Run: `cd web && npm test --workspace=@nigel/ui`
Expected: PASS — the wordmark suite, the parity suite, the particle suite, `describePreviewA11y` over nine preview states with **zero axe violations**, and the whole existing `wc-snake` suite still green after the extraction.

Run: `cd web && npm run lint && npm run typecheck && npm run build`
Expected: clean.

Optionally check the harness by eye: `cd web && npm run preview`, then http://localhost:9090/?p=wc-wordmark and `?p=wc-particle-field`.

- [ ] **Step 8: Commit**

`./scripts/check-no-real-data.sh --staged` (exit 0), then commit.

---

### Task 6: The setup screen

**Files:**
- Create: `web/apps/app/src/screens/setup.ts`, `web/apps/app/src/screens/setup.test.ts`
- Modify: `web/apps/app/src/screens/registry.ts`, `web/apps/app/src/components/nigel-app.ts`, `web/apps/app/src/state/app-store.ts`, `web/apps/app/src/state/app-store.test.ts`

**Interfaces:**
- Consumes: `client.setup(input: SetupRequest)`, `store.switchDataDir(path)`, `store.refreshStatus()`, `wc-wordmark`, `wc-particle-field`, `wc-panel`, `wa-input`, `wa-button`, `controlsCss`.
- Produces:
  ```ts
  // app-store.ts
  export type SetupOutcome = { ok: true } | { ok: false; message: string };
  // on AppStore:
  runSetup(input: SetupRequest): Promise<SetupOutcome>;

  // screens/setup.ts
  export class NigelSetupScreen extends LitElement { client: ApiClient }
  export function renderSetup(ctx: ScreenContext): TemplateResult;
  // registry.ts: ScreenId gains 'setup'
  ```

#### The copy

Written out because it is part of the deliverable. Nigel's register — dry, concrete, the same person who says "Kettle's on." and "The spreadsheets send their regards."

**Arrival**
- Wordmark, drifting specks.
- Heading: `Hello. I'm Nigel.`
- Body: `I keep books. Cash-basis, single-entry, on this machine and nowhere else. Four questions and we can start.`
- Primary button: `Right then`
- Skip hint (visible during the intro): `Click anywhere to skip the theatrics.`

**Step 1 — Profile.** Heading: `What are we keeping books for?`
- Card **Business**: title `A business`, body `Schedule C or 1120-S chart of accounts, with the tax lines already mapped. Invoices, clients, the lot.`
- Card **Personal**: title `Personal finances`, body `A household chart. No tax mapping, no invoices to chase.`
- Footnote: `This picks the chart of accounts, and it's decided once — when the books are created.`

**Step 2 — Identity.** Heading: `Who am I working for?`
- `Your name` — hint: `So I know who I'm greeting. First name is plenty.`
- Business: `Business name` / Personal: `Household name` — hint: `It goes on the books, and on invoices if you send any.`
- `Password (optional)` — hint: `Encrypts the database file. There is no recovery: lose it and the books are gone. Leave it blank and the file stays plain.`
- `Type it again` (appears only once the password is non-empty) — mismatch error: `Those two don't match. Have another go.`
- Back: `Back`, forward: `Carry on`

**Step 3 — First move.** Heading: `How shall we start?`
- Card **Demo**: title `Show me the demo`, body `Eighteen months of invented books for a fictional consultancy. Its own directory, so it never touches yours.`, button `Load the demo`
- Card **Fresh**: title `Start from scratch`, body `An empty ledger and a chart of accounts. Import a statement when you're ready.`, button `Start fresh`
- Card **Load**: title `Load books I already have`, body `Point me at a directory with a nigel.db in it.`, field `Data directory` (placeholder `~/Documents/nigel`), button `Load them`

**Working / failure**
- Busy button labels: `Setting up…` / `Loading the demo…` / `Looking…`
- Setup failure: `That didn't take. ${message}`
- Load failure: the server's own sentence, verbatim — `No database found at /nope/nigel.db. Run \`nigel init --data-dir /nope\` to create one.`
- Empty path: `I need a directory to look in.`

- [ ] **Step 1: Write the failing tests**

`web/apps/app/src/screens/setup.test.ts`:

```ts
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import './setup.js';
import type { NigelSetupScreen } from './setup.js';
import { ApiError, appLocked } from '../api/index.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import { FakeApiClient, UNINITIALIZED_STATUS } from '../__mocks__/fake-api-client.js';

/**
 * The setup screen driven entirely by FakeApiClient — no network, no server,
 * and every assertion is about which api method the screen chose to call with
 * what.
 */
let reloads = 0;

async function mount(client = new FakeApiClient()): Promise<{
  el: NigelSetupScreen;
  client: FakeApiClient;
}> {
  reloads = 0;
  client.status = UNINITIALIZED_STATUS;
  const store = initializeAppStore(client, { reload: () => (reloads += 1) });
  await store.refreshStatus();
  client.calls.length = 0;

  const el = document.createElement('nigel-setup-screen');
  el.client = client;
  document.body.appendChild(el);
  await el.updateComplete;
  return { el, client };
}

async function settle(el: NigelSetupScreen): Promise<void> {
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
}

const button = (el: NigelSetupScreen, text: string) =>
  [...(el.shadowRoot?.querySelectorAll('wa-button') ?? [])].find((b) =>
    b.textContent?.includes(text),
  ) as HTMLElement | undefined;

const field = (el: NigelSetupScreen, label: string) =>
  [...(el.shadowRoot?.querySelectorAll('wa-input') ?? [])].find(
    (i) => i.getAttribute('label') === label,
  ) as (HTMLElement & { value: string }) | undefined;

/** Type into a field the way a user does — the screen reads the input event. */
async function typeInto(el: NigelSetupScreen, label: string, value: string): Promise<void> {
  const input = field(el, label);
  if (!input) throw new Error(`no field labelled ${label}`);
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
  await el.updateComplete;
}

/** Walk arrival -> profile, choosing business or personal. */
async function toIdentity(el: NigelSetupScreen, profile: 'business' | 'personal') {
  button(el, 'Right then')?.click();
  await el.updateComplete;
  (el.shadowRoot?.querySelector(`[data-profile="${profile}"]`) as HTMLElement)?.click();
  await el.updateComplete;
}

async function toFirstMove(el: NigelSetupScreen) {
  await toIdentity(el, 'business');
  await typeInto(el, 'Your name', 'Marta');
  await typeInto(el, 'Business name', 'Cedar Systems');
  button(el, 'Carry on')?.click();
  await el.updateComplete;
}

describe('setup screen', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it("opens on the arrival, in Nigel's voice", async () => {
    const { el } = await mount();
    expect(el.shadowRoot?.textContent).toContain("Hello. I'm Nigel.");
    expect(el.shadowRoot?.querySelector('wc-wordmark')).not.toBeNull();
  });

  it('skips the intro on a click anywhere', async () => {
    // Delight is additive. Somebody who has seen it once should not have to
    // sit through it again.
    const { el } = await mount();
    el.shadowRoot?.querySelector('.stage')?.dispatchEvent(
      new MouseEvent('click', { bubbles: true, composed: true }),
    );
    await el.updateComplete;
    expect(el.shadowRoot?.textContent).toContain('What are we keeping books for?');
  });

  it('asks nothing of the server before the last step', async () => {
    const { el, client } = await mount();
    await toFirstMove(el);
    expect(client.calls).toEqual([]);
  });

  it('labels the company field from the profile', async () => {
    const { el } = await mount();
    await toIdentity(el, 'business');
    expect(field(el, 'Business name')).toBeDefined();
  });

  it('labels it for a household on personal books', async () => {
    const { el } = await mount();
    await toIdentity(el, 'personal');
    expect(field(el, 'Household name')).toBeDefined();
    expect(field(el, 'Business name')).toBeUndefined();
  });

  it('asks for the password twice only once one has been typed', async () => {
    const { el } = await mount();
    await toIdentity(el, 'business');
    expect(field(el, 'Type it again')).toBeUndefined();

    await typeInto(el, 'Password (optional)', 'hunter2');

    expect(field(el, 'Type it again')).toBeDefined();
  });

  it('refuses to move on when the two passwords differ', async () => {
    const { el } = await mount();
    await toIdentity(el, 'business');
    await typeInto(el, 'Your name', 'Marta');
    await typeInto(el, 'Business name', 'Cedar Systems');
    await typeInto(el, 'Password (optional)', 'hunter2');
    await typeInto(el, 'Type it again', 'hunter3');

    button(el, 'Carry on')?.click();
    await el.updateComplete;

    expect(el.shadowRoot?.textContent).toContain("Those two don't match");
    expect(el.shadowRoot?.textContent).not.toContain('How shall we start?');
  });

  it('sends the fresh plan exactly as the route expects it', async () => {
    const { el, client } = await mount();
    await toFirstMove(el);

    button(el, 'Start fresh')?.click();
    await settle(el);

    expect(client.calls).toContain(
      `setup:${JSON.stringify({
        userName: 'Marta',
        companyName: 'Cedar Systems',
        profile: 'business',
        action: 'fresh',
      })}`,
    );
  });

  it('sends the demo plan with the demo action', async () => {
    const { el, client } = await mount();
    await toFirstMove(el);

    button(el, 'Load the demo')?.click();
    await settle(el);

    expect(client.calls.some((c) => c.startsWith('setup:') && c.includes('"action":"demo"'))).toBe(
      true,
    );
  });

  it('carries the password through when one was set', async () => {
    const { el, client } = await mount();
    await toIdentity(el, 'business');
    await typeInto(el, 'Your name', 'Marta');
    await typeInto(el, 'Business name', 'Cedar Systems');
    await typeInto(el, 'Password (optional)', 'hunter2');
    await typeInto(el, 'Type it again', 'hunter2');
    button(el, 'Carry on')?.click();
    await el.updateComplete;

    button(el, 'Start fresh')?.click();
    await settle(el);

    expect(client.calls.some((c) => c.includes('"password":"hunter2"'))).toBe(true);
  });

  it('delegates the load path to the data-directory switch', async () => {
    // Loading existing books is the settings screen's switch, not a second
    // implementation of it: it already validates, migrates and rebinds.
    const { el, client } = await mount();
    await toFirstMove(el);

    await typeInto(el, 'Data directory', '~/Documents/nigel');
    button(el, 'Load them')?.click();
    await settle(el);

    expect(client.calls).toContain('setDataDir');
    expect(client.calls.some((c) => c.startsWith('setup:'))).toBe(false);
    expect(reloads).toBe(1);
  });

  it('asks for a path rather than posting an empty one', async () => {
    const { el, client } = await mount();
    await toFirstMove(el);

    button(el, 'Load them')?.click();
    await settle(el);

    expect(el.shadowRoot?.textContent).toContain('I need a directory to look in.');
    expect(client.calls).not.toContain('setDataDir');
  });

  it('surfaces a refused setup and stays where it is', async () => {
    const { el, client } = await mount();
    client.setupError = new ApiError({
      code: 'conflict',
      rawCode: 'conflict',
      message: 'These books are already set up.',
      status: 409,
      details: { reason: 'already_initialized' },
    });
    await toFirstMove(el);

    button(el, 'Start fresh')?.click();
    await settle(el);

    expect(el.shadowRoot?.textContent).toContain('These books are already set up.');
    expect(el.shadowRoot?.textContent).toContain('How shall we start?');
  });

  it("surfaces the server's own sentence when a directory has no books", async () => {
    const { el, client } = await mount();
    client.settingsError = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'No database found at /nope/nigel.db.',
      status: 400,
    });
    await toFirstMove(el);

    await typeInto(el, 'Data directory', '/nope');
    button(el, 'Load them')?.click();
    await settle(el);

    expect(el.shadowRoot?.textContent).toContain('No database found at /nope/nigel.db.');
    expect(reloads).toBe(0);
  });

  it('goes back a step without losing what was typed', async () => {
    const { el } = await mount();
    await toIdentity(el, 'business');
    await typeInto(el, 'Your name', 'Marta');
    await typeInto(el, 'Business name', 'Cedar Systems');
    button(el, 'Carry on')?.click();
    await el.updateComplete;

    button(el, 'Back')?.click();
    await el.updateComplete;

    expect(field(el, 'Your name')!.value).toBe('Marta');
  });
});
```

Add the store action's tests to `web/apps/app/src/state/app-store.test.ts`:

```ts
describe('running setup', () => {
  beforeEach(() => {
    resetAppStore();
    appLocked.set(false);
  });

  const plan = {
    userName: 'Marta',
    companyName: 'Cedar Systems',
    profile: 'business',
    action: 'fresh',
  } as const;

  it('creates the books and lands on ready', async () => {
    const client = new FakeApiClient();
    client.status = UNINITIALIZED_STATUS;
    const store = initializeAppStore(client);
    await store.refreshStatus();

    const outcome = await store.runSetup(plan);

    expect(outcome).toEqual({ ok: true });
    expect(store.boot.get()).toBe('ready');
    expect(store.companyName.get()).toBe('Cedar Systems');
  });

  it('reports a refusal without changing the phase', async () => {
    const client = new FakeApiClient();
    client.status = UNINITIALIZED_STATUS;
    client.setupError = new ApiError({
      code: 'conflict',
      rawCode: 'conflict',
      message: 'These books are already set up.',
      status: 409,
    });
    const store = initializeAppStore(client);
    await store.refreshStatus();

    const outcome = await store.runSetup(plan);

    expect(outcome).toEqual({ ok: false, message: 'These books are already set up.' });
    expect(store.boot.get()).toBe('needs-setup');
  });
});
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cd web && npm test --workspace=@nigel/app -- setup app-store`
Expected: FAIL — `nigel-setup-screen` is not defined, `store.runSetup` is not a function.

- [ ] **Step 3: Add the store action**

In `web/apps/app/src/state/app-store.ts`, beside `SwitchOutcome`:

```ts
/** What a setup attempt reported. */
export type SetupOutcome = { ok: true } | { ok: false; message: string };
```

on the `AppStore` interface, beside `unlock`:

```ts
  runSetup(input: SetupRequest): Promise<SetupOutcome>;
```

and the implementation, beside `switchDataDir`:

```ts
  const runSetup = async (input: SetupRequest): Promise<SetupOutcome> => {
    try {
      _status.set(await client.setup(input));
      _statusError.set(null);
    } catch (error) {
      return {
        ok: false,
        message: error instanceof ApiError ? error.message : String(error),
      };
    }
    // The route answers the fresh status, so the phase moves without a second
    // round trip; the refresh is what picks up the background update check.
    await refreshStatus();
    return { ok: true };
  };
```

wiring `runSetup` into the returned object and importing `type SetupRequest` from `'../api/types.js'`.

- [ ] **Step 4: Write the screen**

Create `web/apps/app/src/screens/setup.ts`:

```ts
import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, state, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@nigel/ui';
import { controlsCss } from '@nigel/theme';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import type { ApiClient } from '../api/index.js';
import type { BooksProfile, SetupAction } from '../api/types.js';
import type { ScreenContext } from './context.js';

/** How long the wordmark takes to assemble before the first question arrives. */
const REVEAL_MS = 1200;
const REVEAL_TICK_MS = 40;

type Step = 'arrival' | 'profile' | 'identity' | 'first-move';

/**
 * The first-run gate: the four answers a set of books is created from.
 *
 * Rendered instead of the app shell, not inside it — with no sidebar and no
 * screen there is nothing that could fetch data from a database that does not
 * exist yet. One question is visible at a time, in the terminal onboarding's
 * order, and nothing reaches the server until the last step: the first three
 * are entirely local, so a wrong turn costs a click rather than a set of books.
 */
@customElement('nigel-setup-screen')
export class NigelSetupScreen extends SignalWatcher(LitElement) {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        min-height: 100vh;
        background: var(--wa-color-bg);
        color: var(--wa-color-text);
        font-family: var(--wa-font-family-sans);
      }

      .stage {
        position: relative;
        display: grid;
        place-items: center;
        min-height: 100vh;
        padding: var(--wa-space-xl, 24px);
        overflow: hidden;
      }

      wc-particle-field {
        position: absolute;
        inset: 0;
      }

      .panel {
        position: relative;
        width: 100%;
        max-width: 42rem;
        display: grid;
        gap: var(--wa-space-l, 16px);
        justify-items: center;
        text-align: center;
      }

      wc-wordmark {
        --nc-wordmark-size: var(--wa-font-size-s, 13px);
      }

      h1 {
        margin: 0;
        font-size: var(--wa-font-size-2xl, 24px);
        font-weight: var(--wa-font-weight-semibold, 600);
      }

      p {
        margin: 0;
        color: var(--wa-color-muted);
        max-width: 34rem;
      }

      .cards {
        display: grid;
        gap: var(--wa-space-m, 12px);
        width: 100%;
        text-align: left;
      }

      .card {
        display: grid;
        gap: var(--wa-space-2xs, 4px);
        padding: var(--wa-space-l, 16px);
        background: var(--wa-color-surface);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-l, 12px);
        cursor: pointer;
        font: inherit;
        color: inherit;
        text-align: left;
      }

      .card:hover,
      .card:focus-visible {
        border-color: var(--wa-color-brand);
      }

      .card strong {
        font-size: var(--wa-font-size-l, 15px);
      }

      .card span {
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
      }

      .form {
        display: grid;
        gap: var(--wa-space-m, 12px);
        width: 100%;
        max-width: 26rem;
        text-align: left;
      }

      .actions {
        display: flex;
        gap: var(--wa-space-s, 8px);
        justify-content: center;
      }

      .footnote,
      .skip {
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
      }

      .error {
        color: var(--wa-color-danger);
        font-size: var(--wa-font-size-s, 13px);
        margin: 0;
      }
    `,
  ];

  /**
   * Overridable so tests can drive the screen the way every other screen is
   * driven. The screen itself reaches the server only through the store.
   */
  @property({ attribute: false })
  client: ApiClient | null = null;

  @state() private step: Step = 'arrival';
  @state() private reveal = 0;
  @state() private profile: BooksProfile = 'business';
  @state() private userName = '';
  @state() private companyName = '';
  @state() private password = '';
  @state() private confirm = '';
  @state() private dataDir = '';
  @state() private error = '';
  @state() private busy: SetupAction | 'load' | null = null;

  private store: AppStore = getAppStore();
  private ticker: ReturnType<typeof setInterval> | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    this.ticker = setInterval(() => {
      this.reveal = Math.min(1, this.reveal + REVEAL_TICK_MS / REVEAL_MS);
      if (this.reveal >= 1) this.stopReveal();
    }, REVEAL_TICK_MS);
  }

  disconnectedCallback(): void {
    this.stopReveal();
    super.disconnectedCallback();
  }

  private stopReveal(): void {
    if (this.ticker !== null) clearInterval(this.ticker);
    this.ticker = null;
  }

  /** Any click during the arrival goes straight to the first question. */
  private skipIntro = (): void => {
    if (this.step !== 'arrival') return;
    this.stopReveal();
    this.reveal = 1;
    this.step = 'profile';
  };

  private chooseProfile(profile: BooksProfile): void {
    this.profile = profile;
    this.step = 'identity';
  }

  private readField(event: Event, key: 'userName' | 'companyName' | 'password' | 'confirm' | 'dataDir'): void {
    this[key] = (event.target as HTMLInputElement).value;
  }

  private submitIdentity(): void {
    if (this.password && this.password !== this.confirm) {
      this.error = "Those two don't match. Have another go.";
      return;
    }
    this.error = '';
    this.step = 'first-move';
  }

  private plan(action: SetupAction) {
    return {
      userName: this.userName.trim(),
      companyName: this.companyName.trim(),
      profile: this.profile,
      ...(this.password ? { password: this.password } : {}),
      action,
    };
  }

  private runSetup = async (action: SetupAction): Promise<void> => {
    this.busy = action;
    this.error = '';
    const outcome = await this.store.runSetup(this.plan(action));
    this.busy = null;
    // A success unmounts this screen: the boot phase moves to `ready` and the
    // shell takes over, so there is nothing to render here afterwards.
    if (!outcome.ok) this.error = `That didn't take. ${outcome.message}`;
  };

  private loadExisting = async (): Promise<void> => {
    const path = this.dataDir.trim();
    if (!path) {
      this.error = 'I need a directory to look in.';
      return;
    }
    this.busy = 'load';
    this.error = '';
    const outcome = await this.store.switchDataDir(path);
    this.busy = null;
    if (!outcome.ok) this.error = outcome.message;
  };

  render() {
    return html`
      <div class="stage" @click=${this.skipIntro}>
        <wc-particle-field></wc-particle-field>
        <div class="panel">
          <wc-wordmark animated .reveal=${this.reveal}></wc-wordmark>
          ${this.renderStep()}
          ${this.error ? html`<p class="error" role="alert">${this.error}</p>` : nothing}
        </div>
      </div>
    `;
  }

  private renderStep() {
    switch (this.step) {
      case 'arrival':
        return html`
          <h1>Hello. I'm Nigel.</h1>
          <p>
            I keep books. Cash-basis, single-entry, on this machine and nowhere
            else. Four questions and we can start.
          </p>
          <wa-button variant="brand" @click=${this.skipIntro}>Right then</wa-button>
          <p class="skip">Click anywhere to skip the theatrics.</p>
        `;
      case 'profile':
        return html`
          <h1>What are we keeping books for?</h1>
          <div class="cards">
            <button class="card" data-profile="business" @click=${() => this.chooseProfile('business')}>
              <strong>A business</strong>
              <span>
                Schedule C or 1120-S chart of accounts, with the tax lines already
                mapped. Invoices, clients, the lot.
              </span>
            </button>
            <button class="card" data-profile="personal" @click=${() => this.chooseProfile('personal')}>
              <strong>Personal finances</strong>
              <span>A household chart. No tax mapping, no invoices to chase.</span>
            </button>
          </div>
          <p class="footnote">
            This picks the chart of accounts, and it's decided once — when the
            books are created.
          </p>
        `;
      case 'identity':
        return this.renderIdentity();
      case 'first-move':
        return this.renderFirstMove();
    }
  }

  private renderIdentity() {
    const companyLabel = this.profile === 'personal' ? 'Household name' : 'Business name';
    return html`
      <h1>Who am I working for?</h1>
      <div class="form" @click=${(e: Event) => e.stopPropagation()}>
        <wa-input
          label="Your name"
          hint="So I know who I'm greeting. First name is plenty."
          .value=${this.userName}
          @input=${(e: Event) => this.readField(e, 'userName')}
        ></wa-input>
        <wa-input
          label=${companyLabel}
          hint="It goes on the books, and on invoices if you send any."
          .value=${this.companyName}
          @input=${(e: Event) => this.readField(e, 'companyName')}
        ></wa-input>
        <wa-input
          type="password"
          label="Password (optional)"
          autocomplete="new-password"
          password-toggle
          hint="Encrypts the database file. There is no recovery: lose it and the books are gone. Leave it blank and the file stays plain."
          .value=${this.password}
          @input=${(e: Event) => this.readField(e, 'password')}
        ></wa-input>
        ${this.password
          ? html`<wa-input
              type="password"
              label="Type it again"
              autocomplete="new-password"
              .value=${this.confirm}
              @input=${(e: Event) => this.readField(e, 'confirm')}
            ></wa-input>`
          : nothing}
      </div>
      <div class="actions">
        <wa-button appearance="outlined" @click=${() => (this.step = 'profile')}>Back</wa-button>
        <wa-button variant="brand" @click=${() => this.submitIdentity()}>Carry on</wa-button>
      </div>
    `;
  }

  private renderFirstMove() {
    return html`
      <h1>How shall we start?</h1>
      <div class="cards" @click=${(e: Event) => e.stopPropagation()}>
        <div class="card">
          <strong>Show me the demo</strong>
          <span>
            Eighteen months of invented books for a fictional consultancy. Its
            own directory, so it never touches yours.
          </span>
          <wa-button
            variant="brand"
            ?disabled=${this.busy !== null}
            @click=${() => this.runSetup('demo')}
            >${this.busy === 'demo' ? 'Loading the demo…' : 'Load the demo'}</wa-button
          >
        </div>
        <div class="card">
          <strong>Start from scratch</strong>
          <span>
            An empty ledger and a chart of accounts. Import a statement when
            you're ready.
          </span>
          <wa-button
            variant="brand"
            ?disabled=${this.busy !== null}
            @click=${() => this.runSetup('fresh')}
            >${this.busy === 'fresh' ? 'Setting up…' : 'Start fresh'}</wa-button
          >
        </div>
        <div class="card">
          <strong>Load books I already have</strong>
          <span>Point me at a directory with a nigel.db in it.</span>
          <wa-input
            label="Data directory"
            placeholder="~/Documents/nigel"
            .value=${this.dataDir}
            @input=${(e: Event) => this.readField(e, 'dataDir')}
          ></wa-input>
          <wa-button ?disabled=${this.busy !== null} @click=${this.loadExisting}
            >${this.busy === 'load' ? 'Looking…' : 'Load them'}</wa-button
          >
        </div>
      </div>
      <div class="actions">
        <wa-button appearance="outlined" @click=${() => (this.step = 'identity')}>Back</wa-button>
      </div>
    `;
  }
}

export function renderSetup(_ctx: ScreenContext): TemplateResult {
  return html`<nigel-setup-screen></nigel-setup-screen>`;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-setup-screen': NigelSetupScreen;
  }
}
```

- [ ] **Step 5: Register the screen and the gate**

`web/apps/app/src/screens/registry.ts` — add the import, the union member, and the entry:

```ts
import { renderSetup } from './setup.js';
```

```ts
  | 'settings'
  | 'setup'
  | 'unlock';
```

```ts
  setup: {
    id: 'setup',
    title: 'Setup',
    navLabel: 'Setup',
    icon: 'wc-icon-settings',
    // Reached only through the first-run gate, never by choice.
    inNav: false,
    render: renderSetup,
  },
```

`web/apps/app/src/components/nigel-app.ts` — a second gate branch, directly after the locked one:

```ts
    // Same treatment as the unlock gate, and for the same reason: with no
    // database there is no screen that could fetch anything.
    if (boot === 'needs-setup') {
      const gate = screenDef('setup');
      document.title = `${gate.title} · ${store.companyName.get()}`;
      return html`<div class="gate">${gate.render(ctx)}</div>`;
    }
```

- [ ] **Step 6: Run them and watch them pass**

Run: `cd web && npm test --workspace=@nigel/app`
Expected: PASS — the 16 setup-screen cases, the two store cases, the registry suite (a new `ScreenId` with an entry), and everything already green.

Run: `cd web && npm run lint && npm run typecheck && npm run build`
Expected: clean. `controls-adoption.test.ts` passes because the screen adopts `controlsCss`.

- [ ] **Step 7: Commit**

`./scripts/check-no-real-data.sh --staged` (exit 0), then commit.

---

### Task 7: A window that cannot be crushed

**Files:**
- Modify: `crates/nigel-desktop/src/main.rs`
- Create: `crates/nigel-desktop/tests/window_size.rs`

**Interfaces:**
- Consumes: `tauri::WebviewWindowBuilder`.
- Produces: nothing other crates read. `MIN_WINDOW: (f64, f64) = (900.0, 700.0)` in `main.rs`.

- [ ] **Step 1: Write the failing test**

`crates/nigel-desktop/tests/window_size.rs`:

```rust
//! The shell asks for a floor under the window size. Neither the setup gate's
//! four steps nor the shell's sidebar-plus-table survives a 400px window, and
//! a webview will happily be dragged to one.
//!
//! Read off the source rather than a running window: building one needs a
//! display server, which CI has not got.

use std::fs;

#[test]
fn the_window_declares_a_minimum_size() {
    let main = fs::read_to_string("src/main.rs").expect("read main.rs");

    assert!(
        main.contains(".min_inner_size(900.0, 700.0)"),
        "src/main.rs does not set a minimum inner size"
    );
    let min_at = main.find(".min_inner_size").expect("min_inner_size");
    let build_at = main.find(".build()?").expect("build()");
    assert!(
        min_at < build_at,
        "min_inner_size is not applied to the window builder"
    );
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test --manifest-path crates/nigel-desktop/Cargo.toml --test window_size -- --test-threads=1`
Expected: FAIL — `src/main.rs does not set a minimum inner size`.

- [ ] **Step 3: Set the floor**

In `crates/nigel-desktop/src/main.rs`, inside the `.setup` closure:

```rust
            .title("Nigel")
            .inner_size(1200.0, 820.0)
            .min_inner_size(900.0, 700.0)
            .build()?;
```

- [ ] **Step 4: Run it and watch it pass**

Run: `cargo test --manifest-path crates/nigel-desktop/Cargo.toml -- --test-threads=1`
Expected: PASS — the new case plus `desktop_router` and `no_deep_link`.

Run: `cargo fmt --check && cargo clippy -- -D warnings`
Expected: no output, exit 0. (The desktop crate is its own workspace; run `cargo clippy --manifest-path crates/nigel-desktop/Cargo.toml -- -D warnings` as well.)

- [ ] **Step 5: Commit**

`./scripts/check-no-real-data.sh --staged` (exit 0), then commit.

---

### Task 8: Documentation

**Files:**
- Modify: `docs/api.md`, `docs/architecture.md`, `README.md`

**Interfaces:**
- Consumes: everything the seven tasks above produced.
- Produces: nothing code reads.

There is no test cycle here — the check is that a reader who has not seen the diff can find the endpoint and the boot states. Do it last, when the shapes are settled.

- [ ] **Step 1: `docs/api.md`**

In `### GET /api/status`, replace the sentence defining `initialized`:

> `initialized` is whether there are books here — the database file exists **and** has been written to. A zero-byte file, which is what a stray connection leaves behind, reads as uninitialized, and a client that sees `false` shows the setup gate rather than a dashboard over nothing.

Add a new section immediately after `### POST /api/unlock`:

````markdown
### `POST /api/setup`

Creates a set of books on a machine that has none. The web and desktop
equivalent of the terminal's onboarding: the same four answers, the same three
exits.

```json
{
  "userName": "Marta",
  "companyName": "Cedar Systems",
  "profile": "business",
  "password": "…",
  "action": "fresh"
}
```

`profile` is `business` or `personal` and picks the chart of accounts.
`password` is optional; when present the database is encrypted from its first
write, and the process stays unlocked afterwards. `action` is `fresh` — an
empty ledger — or `demo`, which additionally builds `<data_dir>/demo/` with its
own seeded database for a fictional consultancy and points the server at it.
Unknown fields are refused.

The answer is a full [`StatusResponse`](#get-apistatus) for the database that
now exists, so a client needs no second round trip before it renders.

Setup runs once. A call against books that already exist is `409` with
`details.reason` of `already_initialized` — the guard is the route's, not the
client's, so no client bug can write over somebody's ledger. An unknown
`profile`, or a password holding a control character, is `400`.

It is not exempt from the locked guard and needs no exemption: an
uninitialized database cannot be locked. In web mode the session guard applies
as it does to every other endpoint.

**Loading books that already exist is not this endpoint.** That is
[`POST /api/settings/data-dir`](#switching-data-directory), which already
validates, migrates and rebinds, and which relocks the server when the target
turns out to be encrypted.
````

- [ ] **Step 2: `docs/architecture.md`**

In the **Web server** bullet, add `setup` to the list of route modules and note the truth `initialized` now tells. In the **Web UI (SPA)** bullet, add the boot states:

> The SPA's boot phases are `starting | locked | needs-setup | failed | ready`, derived in `state/app-store.ts` from one `/api/status`. `locked` wins first — an encrypted file is somebody's books, and offering setup over it would be offering to replace them — then `needs-setup` when the database has never been created, then the error, the pre-answer and the app proper. `locked` and `needs-setup` both replace the shell entirely (`nigel-app`'s `.gate`) rather than rendering inside it: with no key or no database there is no screen that could fetch anything. `snakeAllowedOnBoot` is an exhaustive switch over the union, so a phase added later fails the typecheck rather than defaulting the easter egg open.

Add a **Setup** bullet beside the others:

> **Setup:** `nigel_core::setup` is the one implementation of "create a set of books" — `SetupPlan` in, the database path out. It saves `user_name`, sets the password global *before* the file exists so SQLCipher encrypts from the first page written, builds the data directory with `exports/`, `snapshots/` and `backups/` at 0700, runs `init_db_with_profile`, and writes `company_name` into metadata. `cli/dashboard.rs` calls it after the onboarding TUI; `server/routes/setup.rs` calls it from `POST /api/setup`. `nigel_core::demo` holds the demo fixtures and `setup_demo_dir()`, which builds `<data_dir>/demo/` with its own seeded database so the demo never sits on top of real books; `cli/demo.rs` is the CLI's stdout wrapper over it. `nigel serve`'s pre-flight migrates a database that is already there and leaves an absent one absent, so a web-first user reaches the setup gate instead of silently getting default books.

Add `setup.rs`, `demo.rs` and `routes/setup.rs` to the on-disk tree in **Project Structure**.

- [ ] **Step 3: `README.md`**

Setup is now user-facing in three places, so Quick Start gains a browser/desktop path beside the terminal one:

> **First run.** `nigel` opens the onboarding in your terminal. `nigel serve` and the desktop app open the same four questions in a browser: what the books are for, who they belong to, an optional password, and whether to start with the demo, an empty ledger, or books you already have. Whichever you use, the books land in your data directory and nothing leaves the machine.

Add the password sentence to Configuration if it is not already there: an optional password encrypts the database with SQLCipher and **there is no recovery**.

- [ ] **Step 4: Verify and commit**

Run: `./scripts/check-no-real-data.sh --staged`
Expected: **exit 0.** Docs are one of the files the rule names explicitly; sweep by hand as well. Every name in the new prose is fixture cast.

Run, one last time from a clean tree:

```
cargo fmt --check
cargo clippy -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test --no-default-features --features serve -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
cargo test --manifest-path crates/nigel-desktop/Cargo.toml -- --test-threads=1
cd web && npm run lint && npm run typecheck && npm test && npm run build
```

Expected: every one green.

---

## Manual verification (operator, macOS)

State the steps; never the figures.

1. Under a clean `HOME`, launch the desktop app. The wordmark assembles over drifting specks, then the first question arrives. Click during the intro — it jumps straight to the question.
2. Turn on Reduce Motion in System Settings and relaunch: the wordmark is drawn whole and still, the specks are present and stationary, and every step still works.
3. Walk each of the three exits in a fresh `HOME`: demo (fixture books for the fictional consultancy, in `<data_dir>/demo/`), fresh (an empty ledger, the dashboard greeting you by name), and load (point it at an existing directory).
4. Run setup with a password, quit, relaunch: the unlock gate appears and the password opens it.
5. Drag the window as small as it will go: it stops at 900×700 and neither the gate nor the shell is crushed.
6. With no books, run `nigel serve` and open the token URL: the same gate appears, and no database was created by starting the server.
