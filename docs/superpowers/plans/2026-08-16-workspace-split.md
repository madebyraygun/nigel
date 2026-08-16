# Workspace Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the single `nigel` crate into a cargo workspace whose core builds with no clap, no ratatui and no crossterm, so a desktop client can link it without a terminal UI.

**Architecture:** The boundary already holds — `tests/layering.rs` proves no core module names `crate::cli::`. This plan makes that a crate line instead of a text scan: `crates/nigel-core` (the 22 core modules), `crates/nigel` (the binary, the 4 TUI modules, `main.rs`), and the workspace root that ties them together. The compiler becomes the guard, which is what closes the class of coupling a grep cannot see.

**Tech Stack:** Rust 2021, cargo workspaces, rust-embed, no new dependencies.

**Spec:** TASK-33.1 (second half) and `backlog/decisions/decision-1`. `docs/design-constraints.md` carries the rules the split must not break.

## Global Constraints

- The `nigel` binary keeps its name, its path, its three features (`gusto`, `pdf`, `serve`) and its behaviour. Not one word of printed output changes.
- `cargo test -- --test-threads=1` is the gate — serial, because the DB password is a process global. `cargo test --no-default-features -- --test-threads=1` must also pass.
- `cargo fmt --check` is CI's first step. Run it before every commit; the boundary-move branch failed CI on formatting alone.
- `./scripts/check-no-real-data.sh --staged` before every commit, judged by **exit status**, never by grepping its output.
- Release CI must still produce the same CLI artifacts for all three platforms.
- Commit after every task. Each task leaves the workspace building and the suite green.

## What the survey established

Facts this plan is built on, measured rather than assumed:

- **22 core modules**: `accounts`, `backup`, `categories`, `categorizer`, `clock`, `db`, `error`, `fmt`, `importer`, `imports`, `invoicing`, `migrations`, `models`, `password`, `pdf`, `reconciler`, `reports`, `reviewer`, `rules`, `server`, `settings`, `updater`.
- **4 CLI/TUI modules**: `cli`, `tui`, `browser`, `effects`, plus `main.rs`.
- **Cleanly CLI-only dependencies**: `ratatui` (22 files), `crossterm` (20), `clap` (3), `self_replace` (1). None appears in a core module.
- **`comfy_table` is a core dependency**, not a CLI one: `reports/text.rs` uses it, and the server's export routes render through those formatters.
- **Two hazards the task does not mention**, each with its own task below: `db.rs:714` prompts on stdin through `rpassword`, and `server/fixture_capture.rs` names `crate::cli` three times.

## File Structure

```
Cargo.toml                    # workspace root: members, shared [workspace.dependencies]
crates/
  nigel-core/
    Cargo.toml                # the 22 core modules' dependencies; features gusto, pdf, serve
    build.rs                  # moved: seeds web/dist, rerun-if-changed (rust-embed lives here)
    src/lib.rs                # the 22 core modules
    src/…                     # moved verbatim from src/
  nigel/
    Cargo.toml                # depends on nigel-core; clap, ratatui, crossterm, self_replace
    src/main.rs               # moved verbatim
    src/cli/ src/tui.rs src/browser.rs src/effects.rs
tests/                        # stays at the workspace root where possible; see Task 8
web/                          # unmoved — build.rs paths change instead
```

---

### Task 1: Workspace skeleton with the crate still whole

**Files:**
- Create: `Cargo.toml` (workspace root), `crates/nigel/Cargo.toml`
- Move: everything under `src/` to `crates/nigel/src/`, `build.rs` to `crates/nigel/build.rs`

**Interfaces:**
- Produces: a workspace with ONE member, `crates/nigel`, that builds and tests exactly as the single crate did.

Doing the move before the split keeps the two risky things apart: this task proves the paths, the build script and the embedded assets survive relocation, and nothing else changes.

- [ ] **Step 1: Move the crate under `crates/nigel`**

```bash
mkdir -p crates/nigel
git mv src crates/nigel/src
git mv build.rs crates/nigel/build.rs
git mv Cargo.toml crates/nigel/Cargo.toml
```

- [ ] **Step 2: Write the workspace root**

Create `Cargo.toml`:

```toml
[workspace]
members = ["crates/nigel"]
resolver = "2"
```

- [ ] **Step 3: Fix the paths the move broke**

`crates/nigel/build.rs` resolves `web/dist` and `web/placeholder` relative to `CARGO_MANIFEST_DIR`, which is now `crates/nigel`. The `web/` directory did not move. Change both paths to reach the workspace root (`../../web/dist`, `../../web/placeholder/index.html`), and the same for the `cargo:rerun-if-changed` key.

`rust-embed`'s `#[folder = "web/dist"]` in `crates/nigel/src/server/static_files.rs` is resolved against the same variable and needs the same treatment.

`build.rs` also points `core.hooksPath` at the tracked `.githooks/`, which is at the repo root — check whether it computes that path relative to the manifest and fix it if so.

- [ ] **Step 4: Verify the binary is unchanged**

Run: `cargo build --release`
Expected: success, and `target/release/nigel` exists at the WORKSPACE root target dir.

Run: `cargo test -- --test-threads=1`
Expected: PASS, same counts as before the move.

Run: `cargo test --no-default-features -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Prove the SPA is still embedded**

Run: `cd web && npm ci && npm run build && cd ..`
Run: `cargo build --release`
Run: `target/release/nigel serve --port 0 --no-open` and confirm it prints a URL, then stop it.

If the embed path is wrong the binary serves the "SPA not built" placeholder rather than failing, so this step is what catches it. A `grep -c 'assets/index' ` over the built binary is a cheaper equivalent.

- [ ] **Step 6: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Move the crate under crates/nigel with a workspace root"
```

---

### Task 2: Move the stdin prompt out of the core

**Files:**
- Modify: `crates/nigel/src/db.rs:714`, and every caller of `prompt_password_if_needed`

**Interfaces:**
- Produces: `db` with no `rpassword` dependency; the prompt lives in the CLI layer and the password arrives as a parameter.

`db.rs:714` calls `rpassword::prompt_password("Database password: ")`. A core crate cannot prompt on stdin: the desktop client has no terminal, the server answers 423 and waits for `POST /api/unlock`, and a library that blocks on a TTY is unusable from either. This is the same shape as the settings rule invoicing already keeps — the value arrives as a parameter, resolved by whichever surface can ask for it.

- [ ] **Step 1: Find every caller**

Run: `grep -rn "prompt_password_if_needed" crates/nigel/src/`

Note which callers are CLI dispatch and which are anything else. Report anything outside `cli/` and `main.rs` before changing it — that would mean a second surface depends on prompting, which changes this task.

- [ ] **Step 2: Move the prompt**

Move the prompting function into `crates/nigel/src/cli/password.rs` (the wrapper module, beside the existing `run_set`/`run_change`/`run_remove` prompts). `db` keeps only the functions that take a password it is given.

- [ ] **Step 3: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS. The encrypted-database tests exercise these paths.

Run: `grep -rn "rpassword" crates/nigel/src/db.rs`
Expected: no output.

- [ ] **Step 4: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Prompt for the database password in the CLI, not in db"
```

---

### Task 3: Move the fixture capture to where it belongs

**Files:**
- Move: `crates/nigel/src/server/fixture_capture.rs` → `crates/nigel/src/cli/fixture_capture.rs` (or a `tests/` integration target)
- Modify: `crates/nigel/src/server/mod.rs`, `crates/nigel/src/cli/mod.rs`

**Interfaces:**
- Produces: `server` with no reference to `crate::cli`, test-support included.

`fixture_capture.rs` names `crate::cli` three times, deliberately: the figure-parity fixtures compare what a browser renders against what `nigel invoice list` prints, so it must drive both. `tests/layering.rs` excludes it by name for exactly that reason. That exclusion cannot survive a crate split — the file would sit in `nigel-core` and name a crate that depends on `nigel-core`, which is a cycle.

It captures fixtures by driving the real router with a real session, so it needs both sides. The CLI crate depends on core, so it is the side that can see both.

- [ ] **Step 1: Move it and re-point**

Move the file, update the module declarations, and fix its imports: what it reached as `crate::server::…` becomes `nigel_core::server::…` once Task 4 renames the crate — for now it is still one crate, so only the module path changes.

- [ ] **Step 2: Verify the capture still runs**

Run: `cargo test --features serve capture_web_report_fixtures -- --ignored --test-threads=1`
Expected: PASS, and `git status --porcelain web/apps/app/src/__fixtures__/` shows no change — the fixtures it regenerates must be byte-identical to the committed ones. A diff here means the move changed what it captures.

- [ ] **Step 3: Drop the exclusion**

Remove `src/server/fixture_capture.rs` from `TEST_SUPPORT` in `tests/layering.rs` (leaving `testutil.rs`), and update the doc comment above it to describe only what remains.

- [ ] **Step 4: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS, including the layering guard with one fewer exclusion.

- [ ] **Step 5: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Capture web fixtures from the CLI side, not from the server"
```

---

### Task 4: Split the crate in two

**Files:**
- Create: `crates/nigel-core/Cargo.toml`, `crates/nigel-core/src/lib.rs`
- Move: the 22 core modules and `build.rs` into `crates/nigel-core/`
- Modify: `crates/nigel/Cargo.toml`, `crates/nigel/src/main.rs`, every `crate::` path in the CLI modules that now names core

**Interfaces:**
- Produces: `nigel_core` as a library crate; `nigel` as a binary crate depending on it.

- [ ] **Step 1: Create the core crate and move its modules**

```bash
mkdir -p crates/nigel-core/src
for m in accounts backup categories categorizer clock db error fmt importer imports \
         invoicing migrations models password pdf reconciler reports reviewer rules \
         server settings updater; do
  git mv "crates/nigel/src/$m" "crates/nigel-core/src/$m" 2>/dev/null || \
  git mv "crates/nigel/src/$m.rs" "crates/nigel-core/src/$m.rs"
done
git mv crates/nigel/build.rs crates/nigel-core/build.rs
```

`build.rs` moves with the server, because rust-embed's folder is resolved from the manifest directory of the crate that contains the `#[derive(RustEmbed)]`.

- [ ] **Step 2: Write `crates/nigel-core/src/lib.rs`**

One `pub mod` line per module above, in the order `src/lib.rs` had them, carrying over its doc comment.

- [ ] **Step 3: Split the dependencies**

`crates/nigel-core/Cargo.toml` takes everything the core modules use — including `comfy_table`, which `reports/text.rs` needs and the server's export routes render through. It must NOT list `clap`, `ratatui`, `crossterm`, `self_replace` or `rpassword`.

`crates/nigel/Cargo.toml` takes those four plus `nigel-core = { path = "../nigel-core" }`.

The three features (`gusto`, `pdf`, `serve`) are declared on `nigel-core` and **forwarded** from `nigel`, so `cargo build --no-default-features` at the workspace root still means what it meant.

- [ ] **Step 4: Fix the paths**

In the CLI crate, `crate::db`, `crate::reports`, `crate::invoicing`… become `nigel_core::…`. In the core crate, nothing should reference `crate::cli` — if the compiler says otherwise, that is a real finding: stop and report it rather than re-exporting to make it build.

- [ ] **Step 5: Verify the boundary by compiler**

Run: `cargo build -p nigel-core --no-default-features`
Expected: success. This is the moment the epic exists for.

Run: `cargo tree -p nigel-core | grep -E "clap|ratatui|crossterm"`
Expected: no output. If any appears, a dependency is arriving transitively and the tree shows through what.

- [ ] **Step 6: Verify nothing else moved**

Run: `cargo test -- --test-threads=1`
Expected: PASS, same counts.

Run: `cargo test --no-default-features -- --test-threads=1`
Expected: PASS.

Run: `cargo build --release && ls -la target/release/nigel`
Expected: the binary exists under the same name.

- [ ] **Step 7: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Split nigel-core out of the binary crate"
```

---

### Task 5: Widen only what the compiler demands

**Files:**
- Modify: whichever core items the CLI crate can no longer reach

**Interfaces:**
- Produces: the core crate's public API, decided by what the split actually needs.

This is TASK-33.10 answered rather than guessed. Until now every `pub(crate)` was reachable from the CLI because they were one crate; the compiler now names each item that is not.

- [ ] **Step 1: Collect the list**

Run: `cargo build 2>&1 | grep -E "^error\[E0603\]|is private" | sort -u`

Every line is an item the CLI needs across the crate line.

- [ ] **Step 2: Widen each one, individually**

For each, change `pub(crate)` to `pub` **only on that item**, and add a one-line doc comment saying what it is for if it has none. Do not widen a module wholesale to fix its members.

Two things stay narrow deliberately: an item only the core uses stays `pub(crate)`, and a type whose fields carry an unenforced invariant — `CompanyProfile`, `SendClients`, `RegisterFilters` — gets a doc line saying it is a DTO with no invariant, since `pub` fields now promise that to anyone.

- [ ] **Step 3: Verify**

Run: `cargo build --release`
Expected: success.

Run: `cargo test -- --test-threads=1`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Widen the core API to what the split needs, and no further"
```

---

### Task 6: Retire the text guard in favour of the compiler

**Files:**
- Modify: `tests/layering.rs`

**Interfaces:**
- Produces: a guard that states what now enforces the boundary.

`cargo build -p nigel-core` cannot succeed if core names the CLI — the crate does not depend on it. That is stronger than the grep, which could not see a trait impl, a closure or an inherent impl left behind.

- [ ] **Step 1: Decide its fate, and say why in the file**

Keep the test, narrowed to what the compiler does not check: the CLI crate is allowed to depend on core, so nothing in `crates/nigel-core/src/` may name `nigel_cli` or `crate::cli`. Replace the module doc with a statement that the crate boundary is now the enforcement and this test is a fast local signal for the same rule.

If the test can say nothing the compiler does not already say, delete it and record that in the commit message — a test that cannot fail is worse than no test.

- [ ] **Step 2: Verify**

Run: `cargo test -- --test-threads=1`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Let the crate boundary enforce what the text guard approximated"
```

---

### Task 7: Release CI still ships the same binaries

**Files:**
- Modify: `.github/workflows/` — the check and release workflows

**Interfaces:**
- Produces: unchanged artifacts from a changed source layout.

- [ ] **Step 1: Read what the workflows assume**

Run: `grep -rn "cargo\|web/dist\|npm" .github/workflows/*.yml`

Note every path that assumes a crate root: `--manifest-path`, `working-directory`, artifact paths, the `web/` build step that must precede any cargo step.

- [ ] **Step 2: Update them**

Workspace-root `cargo build --release` still produces `target/release/nigel`, so most steps need no change — but any step naming `src/` or `Cargo.toml` directly does.

- [ ] **Step 3: Verify**

Run: `cargo build --release --locked`
Expected: success — `--locked` is what CI uses, and the lockfile changed shape when the workspace appeared.

- [ ] **Step 4: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Point CI at the workspace layout"
```

---

### Task 8: Documentation

**Files:**
- Modify: `docs/architecture.md`, `CLAUDE.md` (only if a command or pointer changed)

- [ ] **Step 1: Update the architecture doc**

The Project Structure tree in `docs/architecture.md` describes `src/…`. Rewrite it for `crates/nigel-core/src/…` and `crates/nigel/src/…`, and add a short section stating which crate owns what and why the line falls there.

- [ ] **Step 2: Touch CLAUDE.md only if a rule or command changed**

Per its own policy: architecture goes in `docs/architecture.md`, and CLAUDE.md changes only for a command, a rule or a pointer. If `cargo test` still works verbatim from the root, CLAUDE.md needs nothing. Check the size budget still holds if you do edit it.

- [ ] **Step 3: Commit**

```bash
./scripts/check-no-real-data.sh --staged
git add -A && git commit -m "Document the workspace layout"
```

---

## What this plan deliberately does not do

- **No `nigel-desktop` crate.** The workspace has two members. The desktop shell is TASK-33.2 and needs the download probe answered first.
- **No behaviour changes** beyond moving the password prompt and the fixture capture, each of which is a task with its own verification.
- **No dependency upgrades.** A version bump inside a layout change makes a bisect useless.
- **TASK-33.9 becomes redundant here** and should be closed as superseded when this lands: it asked for a *trial* split in CI to get a compiler-level check, and this is the real one.
