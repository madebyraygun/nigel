# Multiuser PR 1: Users Data Layer and CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the books a `users` and `sessions` store, an `identity` data layer (Argon2id passwords with NIST rules, sessions stored as hashes, RFC 6238 TOTP with a replay guard, TOTP secrets sealed under a per-machine `server.key`), the `nigel user` commands, and `NIGEL_DB_PASSWORD_FILE` — without changing anything `nigel serve` does.

**Architecture:** Migration 17 adds two tables. A new `nigel_core::identity` module (`mod.rs`, `passwords.rs`, `totp.rs`, `seal.rs`, `sessions.rs`, `users.rs`) owns every read and write of them; every function that stamps or judges a time takes `now: DateTime<Utc>` from the caller (`clock::now_utc()`), so tests pin it. `crates/nigel/src/cli/user.rs` is a printing wrapper over `identity::users`, dispatched from `main.rs` through the normal password and migration pre-flight. `db::env_password_if_set` learns `NIGEL_DB_PASSWORD_FILE`, which every command inherits. Nothing under `crates/nigel-core/src/server/` changes: PR 2 wires the data layer to HTTP.

**Tech Stack:** Rust 2021, rusqlite (SQLCipher), clap 4, `argon2` 0.5, `chacha20poly1305` 0.10, `hmac` 0.12 + `sha1` 0.10, `unicode-normalization` 0.1, `data-encoding` 2, `subtle` 2.6, `assert_cmd` for end-to-end CLI tests.

**Spec:** `docs/superpowers/specs/2026-10-23-multiuser-phase-1-design.md` — sections 1, 2, 3, 4.3, 4.5, 6, 11.4, 15, 16 and the PR 1 row of 17. Backlog: TASK-32.1.

---

## Global Constraints

Copied from the spec; every task's requirements include these.

- One migration, **version 17**, appended to `MIGRATIONS` in `crates/nigel-core/src/migrations.rs`, idempotent like the ones before it. (v16 is the last on `main` today; if another migration lands first, take the next free number — `migration_versions_are_contiguous_from_one` enforces it.)
- `users` and `sessions` exactly as spec section 2 writes them: `uid` 32 hex chars, `email` `UNIQUE COLLATE NOCASE`, `role` `CHECK (role IN ('admin', 'bookkeeper', 'read_only'))`, `totp_secret BLOB` (sealed; NULL = no two-step), `totp_last_step INTEGER`, `must_change_password INTEGER NOT NULL DEFAULT 0`, `disabled_at TEXT` (NULL = enabled); `sessions.token_hash BLOB NOT NULL UNIQUE`, `kind CHECK (kind IN ('cookie', 'bearer'))`, `user_agent` truncated to 256 chars, indexes `idx_sessions_user`, `idx_sessions_expires`.
- Email is trimmed and lower-cased before it is stored and before every lookup; shape check: one `@`, non-empty local part and domain, no whitespace.
- Timestamps are UTC RFC 3339, read through one clock seam so tests can pin it.
- Users are never deleted; `disabled_at` is the off switch.
- Invariant: books with users always have at least one enabled admin. The first user must be an admin; the last enabled admin cannot be disabled or demoted. The CLI and the API go through the same function.
- Data-layer module: `crates/nigel-core/src/identity/` (`users.rs`, `sessions.rs`, `passwords.rs`, `totp.rs`, `seal.rs`). Not under `server/`. The existing `password.rs` (books encryption) is left alone.
- User password: Argon2id PHC string, `argon2` crate, v19, m = 19 MiB, t = 2, p = 1, 16-byte random salt per hash.
- Session token: SHA-256 of the 32-byte random token; no salt.
- Temporary password: hashed like any password, shown once, never stored in plaintext. 20 characters from an unambiguous base32 alphabet in four groups of five (about 100 bits), e.g. `K7QXM-2RTPA-W9HCN-4FJDE`; sets `must_change_password`.
- TOTP secret: sealed with XChaCha20-Poly1305 under the **server key**, the user's `uid` as associated data.
- Server key: 32 random bytes in `<config_dir>/server.key`, created on first need, mode `0600` in a `0700` directory; never in the data directory, so never in a backup.
- Never logged: passwords, temporary passwords, TOTP secrets and codes, session tokens. The `identity` module keeps secrets out of its error messages.
- TOTP: RFC 6238, HMAC-SHA1, 6 digits, 30-second step, 160-bit secret from the CSPRNG; current step ± 1 accepted; a code is accepted only if its step is greater than `totp_last_step`, which is then written.
- Password rules (NIST SP 800-63B): at least 12 and at most 256 characters, Unicode NFC-normalized before hashing; not on a bundled list of the 10,000 most common passwords (compiled in, compared case-insensitively); not equal to the email, its local part, or the display name (case-insensitive); a new password must differ from the current one. One function enforces them for the CLI and the API; failure reasons `too_short`, `too_long`, `common`, `matches_identity`, `unchanged`.
- Session lifetime: 30 days idle, 90 days absolute; renewal at most once an hour, `expires_at = min(now + 30 days, created_at + 90 days)`.
- CLI: `nigel user add <email> --name <name> (--admin | --role <admin|bookkeeper|read_only>)`, `list`, `role <email> <role>`, `disable <email>`, `enable <email>`, `reset-password <email> [--clear-totp]`. Exactly one of `--admin`/`--role`; no default role. `add` and `reset-password` print the temporary password once with "Give this to <name>. They will choose their own password when they first sign in." `add` creating the first user on unencrypted books prints: "These books are not encrypted. Anyone who can read the file can read the books. Encrypt them with `nigel password set` before you give anyone else access." Unknown emails are refused with a clear error. `list` never prints hashes or secrets.
- `NIGEL_DB_PASSWORD_FILE` is added to `db::env_password_if_set`, so every command accepts it. Setting both variables is an error. The file's content is the password with one trailing newline stripped.
- **PR 1 changes no HTTP behaviour and leaves local mode exactly as it is.** No file under `crates/nigel-core/src/server/` changes.
- Repository rules: no real book data, names or hostnames — fixtures use Pat Example, Sam Example, `example.com`; `./scripts/check-no-real-data.sh --staged` judged by **exit status**; no comment that justifies an edit, explains a config entry, or warns against reverting; docs describe the current state; never edit `backlog/`.
- Rust tests run serially: `cargo test -- --test-threads=1`, `cargo test --no-default-features -- --test-threads=1`, `cargo test -p nigel-core -- --test-threads=1`; `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.

## Review Focus

Five inputs the spec implies but no happy-path test meets, most likely first. Each is pinned by a named test in the task that owns the code.

1. **A password file written by `echo` or an editor** — a trailing `\n`, or `\r\n` from a Windows editor, must unlock; a file with two trailing newlines must not silently strip both (that would change the password); an empty file, a missing file and a wrong password must each fail fast naming `NIGEL_DB_PASSWORD_FILE` and never quoting the file. Pinned in Task 9: `a_password_file_unlocks_with_one_trailing_newline_stripped`, `only_one_newline_is_stripped`, `a_missing_empty_or_wrong_password_file_is_fatal_and_quotes_nothing`.
2. **A damaged or truncated `server.key`** (a partial copy, a stray edit) — must be an error naming the file and the `--clear-totp` recovery, and must never be overwritten with a fresh key, which would silently orphan every enrolled secret. Pinned in Task 5: `a_damaged_key_file_is_an_error_and_is_never_replaced`.
3. **An email typed with different case or stray spaces** (`" Pat@Example.com "`, `SAM@example.com`) — must find the same user on every command and must not create a second account. Pinned in Task 7: `a_second_spelling_of_an_email_is_a_duplicate`, `a_created_user_has_a_temporary_password_and_must_change_it`, and Task 10: `user_list_role_disable_enable_and_the_last_admin` (`enable SAM@example.com`).
4. **A TOTP code typed the way authenticator apps display it** (`123 456`) or with junk (full-width digits, a trailing newline, five or seven digits) — the spaced form must match; junk must never match or panic. Pinned in Task 4: `a_code_typed_with_a_space_still_matches_and_junk_never_does`.
5. **A password the same in NFC and NFD** (`café` typed on macOS vs Linux) — must verify either way, and the 12-character minimum counts characters after normalization, not code points before it. Pinned in Task 3: `nfc_and_nfd_spellings_verify_against_each_other`, `length_counts_characters_after_normalization`.

## Rulings on what the spec left open

These are decisions this plan makes; each is small and reversible, and the reviewer should confirm them.

- **Crates.** `argon2` 0.5.3, `chacha20poly1305` 0.10.1, `hmac` 0.12.1, `sha1` 0.10, `unicode-normalization` 0.1, `data-encoding` 2 — all RustCrypto or long-standing MIT/Apache crates. The 0.5/0.10 generation is chosen over the newer 0.6/0.11 because it shares `digest` 0.10 and `rand_core` 0.6 with the `sha2` 0.10 and `rand` 0.8 nigel-core already uses for every token it generates, so `rand::rngs::OsRng` feeds all of them and no second generation of trait crates enters the tree. TOTP is hand-written over `hmac` + `sha1` (about 30 lines, pinned to the RFC vectors) rather than taking `totp-rs`, which brings its own base32 and URL handling and a newer MSRV for no gain. `subtle` stops being `serve`-only, because the TOTP comparison lives outside `server/`.
- **`unicode-normalization`'s licence** is `(MIT OR Apache-2.0) AND Unicode-3.0`; Unicode-3.0 is a permissive data licence compatible with MIT.
- **The common-password list** is SecLists `Passwords/Common-Credentials/10k-most-common.txt` (MIT, Daniel Miessler), bundled verbatim (73 KB, sha256 `68782d6a4a19a4768d5f15dd66bd534e7a33055cc755411e33f16d18c50fdcce`) with its licence notice. Only 10 of its entries are 12 characters or longer, so beside the length rule it adds little; it also contains profanity, as every such list does. Both are flagged for the operator; a longer-password list is a possible follow-up, not this PR.
- **Rule order:** length, then the common list, then identity, then unchanged. The first failure is the reason reported.
- **Display name:** trimmed, 1–100 characters, no control characters. `--name` is required on `nigel user add`.
- **The first non-admin user** is refused as `Conflict { code: "last_admin" }` — the invariant's own reason code — with "The first user must be an admin. Add them with --admin."
- **Wrong-key or unknown-uid TOTP** is an `Err`, not a `Rejected`, so PR 2 can log which users are affected when `server.key` is lost (spec section 3).
- **The enrolling code cannot sign in:** `set_totp_secret` takes the confirming code's step and stores it as `totp_last_step`.
- **TOTP input** has ASCII spaces removed before the six-digit check.
- **Disable and enable are idempotent:** disabling a disabled user keeps the original `disabled_at` and the CLI says "was already disabled".
- **`Role`** lives in `identity/mod.rs` (not `users.rs`), because `sessions.rs` reads it and `users.rs` depends on `sessions.rs`.
- **`read-only`** is accepted on the command line as an alias of `read_only`.
- **Both password variables set** is an error, never a precedence rule. Spec section 11.4 says the file "wins" and also that "setting both variables is an error"; the stricter reading is taken.
- **`\r\n`** counts as the one trailing newline stripped from a password file.
- **`nigel user` skips the launch-time Stripe and document syncs** (`launch_sync_allowed`): account administration on a server should not make network calls first.
- **"Nigel will now require sign-in…"** (spec section 6) is **not** printed in PR 1. Until PR 2 lands, `nigel serve` ignores users, and the sentence would be false; PR 2 adds it beside the mode derivation. `docs/commands.md` says plainly that `nigel serve` does not read users yet.
- **Argon2 in debug builds** is slow enough to cost ~25 s across the suite; `[profile.dev.package.argon2]` and `blake2` get `opt-level = 3` in the root `Cargo.toml` (measured: identity tests 26 s → 3 s).
- **Docs beyond `docs/commands.md`:** the PR also adds the `identity` module to `docs/architecture.md`, the identity invariants to `docs/design-constraints.md`, one `NIGEL_DB_PASSWORD_FILE` sentence to the README's "Automated backups", and a CHANGELOG entry — the repository's documentation policy requires each.

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `Cargo.toml` | modify | Optimize `argon2`/`blake2` in dev builds |
| `Cargo.lock`, `crates/nigel-desktop/Cargo.lock`, `crates/nigel-ffi/Cargo.lock` | modify | New dependencies (additions only) |
| `crates/nigel-core/Cargo.toml` | modify | Crypto dependencies; `subtle` unconditional |
| `crates/nigel-core/src/lib.rs` | modify | `pub mod identity;` |
| `crates/nigel-core/src/clock.rs` | modify | `now_utc()` — the one instant read |
| `crates/nigel-core/src/migrations.rs` | modify | v17: `users`, `sessions` |
| `crates/nigel-core/src/identity/mod.rs` | create | `Role`, `stamp`, test helpers |
| `crates/nigel-core/src/identity/passwords.rs` | create | Rules, Argon2id, dummy verify, temporary passwords |
| `crates/nigel-core/src/identity/common-passwords.txt` | create | The bundled 10k list |
| `crates/nigel-core/src/identity/totp.rs` | create | HOTP/TOTP, skew window, base32, otpauth URI |
| `crates/nigel-core/src/identity/seal.rs` | create | `ServerKey`, `server.key`, seal/open |
| `crates/nigel-core/src/identity/sessions.rs` | create | Create/find/renew/end/sweep sessions |
| `crates/nigel-core/src/identity/users.rs` | create | Users, credentials, password change, TOTP storage |
| `crates/nigel-core/src/db.rs` | modify | `NIGEL_DB_PASSWORD_FILE` |
| `crates/nigel/src/cli/password.rs` | modify | Never-prompt message names both variables |
| `crates/nigel/src/cli/mod.rs` | modify | `Commands::User`, `UserCommands`, launch-sync skip |
| `crates/nigel/src/cli/user.rs` | create | `nigel user` printing wrapper |
| `crates/nigel/src/main.rs` | modify | Dispatch `nigel user` |
| `crates/nigel/tests/cli_dispatch.rs` | modify | End-to-end tests |
| `docs/commands.md`, `docs/architecture.md`, `docs/design-constraints.md`, `README.md`, `CHANGELOG.md` | modify | Documentation |

### Interfaces at a glance

```rust
// identity/mod.rs
pub enum Role { ReadOnly, Bookkeeper, Admin }        // Ord: ReadOnly < Bookkeeper < Admin
impl Role { pub const ALL: [Role; 3]; fn as_str(self) -> &'static str; fn parse(&str) -> Option<Role>; fn label(self) -> &'static str }
pub(crate) fn stamp(now: DateTime<Utc>) -> String;    // "2026-10-24T07:30:15Z"

// identity/passwords.rs
pub enum PasswordRule { TooShort, TooLong, Common, MatchesIdentity, Unchanged }  // reason_code(), Display, Into<NigelError>
pub struct PasswordOwner<'a> { email: &'a str, display_name: &'a str, current_hash: Option<&'a str> }
pub fn check_new_password(password: &str, owner: &PasswordOwner<'_>) -> Result<(), PasswordRule>;
pub fn hash_password(password: &str) -> nigel_core::error::Result<String>;
pub fn verify_password(password: &str, phc: &str) -> bool;
pub fn verify_dummy(password: &str);
pub fn temporary_password() -> Zeroizing<String>;

// identity/totp.rs
pub fn generate_secret() -> Zeroizing<[u8; 20]>;
pub fn step_at(now: DateTime<Utc>) -> i64;
pub fn hotp(secret: &[u8], counter: u64, digits: u32) -> u32;
pub fn code_for_step(secret: &[u8], step: i64) -> String;
pub fn matching_step(secret: &[u8], code: &str, now: DateTime<Utc>) -> Option<i64>;
pub fn secret_base32(secret: &[u8]) -> Zeroizing<String>;
pub fn otpauth_uri(email: &str, secret: &[u8]) -> Zeroizing<String>;

// identity/seal.rs
pub struct ServerKey;                                  // redacted Debug, zeroized
pub fn key_path() -> PathBuf;                          // <config_dir>/server.key
impl ServerKey { fn load(&Path) -> Result<Option<ServerKey>>; fn load_or_create(&Path) -> Result<ServerKey>;
                 fn seal(&self, uid: &str, plaintext: &[u8]) -> Result<Vec<u8>>; fn open(&self, uid: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>>;
                 #[cfg(any(test, feature = "testutil"))] fn from_bytes([u8; 32]) -> ServerKey }

// identity/sessions.rs
pub enum SessionKind { Cookie, Bearer }
pub struct IssuedSession { id: i64, token: Zeroizing<String>, expires_at: String }
pub struct ActiveSession { id, user_id, uid, role: Role, must_change_password, kind, created_at, last_seen_at, expires_at }
pub fn token_hash(token_hex: &str) -> Option<[u8; 32]>;
pub fn create_session(conn, user_id: i64, kind, user_agent: Option<&str>, ip: Option<&str>, now) -> Result<IssuedSession>;
pub fn find_session(conn, token_hex: &str, now) -> Result<Option<ActiveSession>>;
pub fn renew_session(conn, session: &ActiveSession, now) -> Result<Option<String>>;
pub fn end_session(conn, id: i64) -> Result<bool>;
pub fn end_user_sessions(conn, user_id: i64, except: Option<i64>) -> Result<usize>;
pub fn sweep_expired(conn, now) -> Result<usize>;

// identity/users.rs
pub struct User { id, uid, email, display_name, role, totp_enabled, must_change_password, password_changed_at, disabled_at, created_at, updated_at }
pub struct UserListing { user: User, last_seen_at: Option<String>, active_sessions: i64 }
pub struct NewUser<'a> { email: &'a str, display_name: &'a str, role: Role }
pub struct CreatedUser { user: User, temporary_password: Zeroizing<String>, first_user: bool }
pub enum Credentials { Valid(User), Invalid }
pub enum PasswordChangeError { WrongCurrentPassword, Rule(PasswordRule), Failed(NigelError) }
pub enum TotpCheck { Accepted, Rejected, NotEnrolled }
pub fn normalize_email(raw: &str) -> Result<String>;
pub fn has_users(conn) -> Result<bool>;
pub fn get_user(conn, id) -> Result<Option<User>>;
pub fn find_by_email(conn, email) -> Result<Option<User>>;
pub fn require_by_email(conn, email) -> Result<User>;
pub fn list_users(conn, now) -> Result<Vec<UserListing>>;
pub fn create_user(conn, &NewUser, now) -> Result<CreatedUser>;
pub fn set_role(conn, id, Role, now) -> Result<User>;
pub fn disable_user(conn, id, now) -> Result<User>;
pub fn enable_user(conn, id, now) -> Result<User>;
pub fn reset_password(conn, id, clear_totp: bool, now) -> Result<Zeroizing<String>>;
pub fn verify_credentials(conn, email, password) -> Result<Credentials>;
pub fn change_password(conn, id, current, new, keep_session: Option<i64>, now) -> Result<(), PasswordChangeError>;
pub fn set_totp_secret(conn, id, &ServerKey, secret: &[u8], confirmed_step: i64, now) -> Result<()>;
pub fn clear_totp(conn, id, now) -> Result<()>;
pub fn verify_totp(conn, id, &ServerKey, code: &str, now) -> Result<TotpCheck>;
```

`Result` is `nigel_core::error::Result` throughout. Refusals use existing `NigelError` variants, so `server/error.rs` needs no change: `Conflict { code: "last_admin" | "duplicate_email" }` (409 with `details.reason` = the code), `NotFound` (404), `Invalid` (400).

---

### Task 1: Dependencies, the clock seam and the `identity` module with `Role`

**Files:**
- Modify: `Cargo.toml` (dev profile for `argon2`/`blake2`)
- Modify: `crates/nigel-core/Cargo.toml`
- Modify: `crates/nigel-core/src/lib.rs`, `crates/nigel-core/src/clock.rs`
- Create: `crates/nigel-core/src/identity/mod.rs`
- Modify (generated): `Cargo.lock`, `crates/nigel-desktop/Cargo.lock`, `crates/nigel-ffi/Cargo.lock`

**Interfaces:**
- Consumes: nothing.
- Produces: `nigel_core::identity::Role` (`ReadOnly < Bookkeeper < Admin`, `ALL`, `as_str`, `parse`, `label`); `nigel_core::clock::now_utc() -> chrono::DateTime<chrono::Utc>`; the crates every later task imports.

- [ ] **Step 1: Add the dependencies**

In `crates/nigel-core/Cargo.toml`, take `subtle` out of the `serve` feature and make it unconditional, and add the five crypto crates after `zeroize`:

```toml
serve = ["dep:axum", "dep:tokio", "dep:tower", "dep:rust-embed", "dep:open"]
```

```toml
zeroize = "1.8.2"
argon2 = { version = "0.5.3", features = ["std"] }
chacha20poly1305 = "0.10.1"
hmac = "0.12.1"
sha1 = "0.10.6"
data-encoding = "2.6"
unicode-normalization = "0.1.24"
```

and replace `subtle = { version = "2.6", optional = true }` with:

```toml
subtle = "2.6"
```

In the root `Cargo.toml`, append (Argon2id at 19 MiB is ~0.5 s per hash unoptimized; the suite hashes dozens of times):

```toml
[profile.dev.package.argon2]
opt-level = 3

[profile.dev.package.blake2]
opt-level = 3
```

Licences (all MIT/Apache-compatible): `argon2`, `hmac`, `sha1` — MIT OR Apache-2.0; `chacha20poly1305` — Apache-2.0 OR MIT; `data-encoding` — MIT; `unicode-normalization` — (MIT OR Apache-2.0) AND Unicode-3.0. Check with `cargo info <crate>@<version> | grep license` if in doubt.

- [ ] **Step 2: Refresh the three lockfiles (additions only)**

Run:
```bash
cargo build -p nigel-core
(cd crates/nigel-desktop && cargo update -p nigel-core)
(cd crates/nigel-ffi && cargo update -p nigel-core)
git diff --stat -- Cargo.lock crates/nigel-desktop/Cargo.lock crates/nigel-ffi/Cargo.lock
```
Expected: each `cargo update` prints only `Adding …` lines (argon2 0.5.3, base64ct, blake2, chacha20, chacha20poly1305, cipher, data-encoding, inout, opaque-debug, password-hash, poly1305, sha1, unicode-normalization, universal-hash) and the diff stat shows insertions only. The desktop and FFI shells are separate workspaces that CI builds with `--locked`; without this their CI fails.

- [ ] **Step 3: Write the failing test**

Add `pub mod identity;` to `crates/nigel-core/src/lib.rs` after `pub mod fmt;`, and create `crates/nigel-core/src/identity/mod.rs` with only the module doc and the test:

```rust
//! Who may sign in to a shared `nigel serve`: users, their sessions, their
//! passwords and their second factor.
//!
//! Not under `server/`, because the CLI creates and manages users too. The
//! books' own encryption password is `crate::password`, a different thing.
//!
//! Every function that writes or judges a time takes `now` as a parameter;
//! `clock::now_utc()` is the one read, at the caller.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_order_and_round_trip() {
        assert!(Role::ReadOnly < Role::Bookkeeper && Role::Bookkeeper < Role::Admin);
        for role in Role::ALL {
            assert_eq!(Role::parse(role.as_str()), Some(role));
        }
        assert_eq!(Role::parse("owner"), None);
        assert_eq!(Role::ReadOnly.label(), "Read-only");
    }
}
```

- [ ] **Step 4: Run it to see it fail**

Run: `cargo test -p nigel-core identity::tests -- --test-threads=1`
Expected: compile error, `failed to resolve: use of undeclared type Role`.

- [ ] **Step 5: Implement `Role` and the clock read**

Insert between the module doc and the tests in `identity/mod.rs`:

```rust
/// Ordered `ReadOnly < Bookkeeper < Admin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    ReadOnly,
    Bookkeeper,
    Admin,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::ReadOnly, Role::Bookkeeper, Role::Admin];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::ReadOnly => "read_only",
            Role::Bookkeeper => "bookkeeper",
            Role::Admin => "admin",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read_only" => Some(Role::ReadOnly),
            "bookkeeper" => Some(Role::Bookkeeper),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }

    /// The word a person reads: Admin, Bookkeeper, Read-only.
    pub fn label(self) -> &'static str {
        match self {
            Role::ReadOnly => "Read-only",
            Role::Bookkeeper => "Bookkeeper",
            Role::Admin => "Admin",
        }
    }
}
```

In `crates/nigel-core/src/clock.rs`, after `today()`:

```rust
/// The current instant in UTC — the reference time every identity write
/// stamps and every session expiry is judged against. The `identity` data
/// layer takes it as a parameter rather than reading it, so tests pin it.
pub fn now_utc() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}
```

- [ ] **Step 6: Run it to see it pass, and the checks**

Run: `cargo test -p nigel-core identity::tests -- --test-threads=1 && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: `roles_order_and_round_trip ... ok`; clippy and fmt clean.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/nigel-desktop/Cargo.lock crates/nigel-ffi/Cargo.lock \
  crates/nigel-core/Cargo.toml crates/nigel-core/src/lib.rs crates/nigel-core/src/clock.rs \
  crates/nigel-core/src/identity/mod.rs
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "identity: add the module, Role, the UTC clock read and the crypto crates"
```

---

### Task 2: Migration 17 — `users` and `sessions`

**Files:**
- Modify: `crates/nigel-core/src/migrations.rs` (append to `MIGRATIONS`; tests in its `mod tests`)

**Interfaces:**
- Consumes: the migration runner (`apply_migrations`, savepoint per migration) and the test helpers already in `migrations.rs`'s tests: `test_db()`, `table_exists(conn, name)`, `set_metadata`, `run_migrations`, `get_schema_version`, `LATEST_VERSION`.
- Produces: tables `users`, `sessions` with the columns in Global Constraints; `LATEST_VERSION == 17`.

First confirm the number: `grep -n "version: 1[0-9]," crates/nigel-core/src/migrations.rs | tail -1` must show `version: 16`. If it shows a higher number, use the next one everywhere below (the test names, the `"16"` stamps) — `migration_versions_are_contiguous_from_one` fails on a gap or a duplicate.

- [ ] **Step 1: Write the failing tests**

In `migrations.rs`'s `mod tests`, before `fn v16_is_replayable`:

```rust
    #[test]
    fn v17_creates_users_and_sessions_on_a_fresh_database() {
        let (_dir, conn) = test_db();
        assert!(table_exists(&conn, "users"));
        assert!(table_exists(&conn, "sessions"));
        let users: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
            .unwrap();
        assert_eq!(users, 0, "a fresh database has no users, so it stays local");
    }

    #[test]
    fn v17_migrates_a_v16_database_and_is_replayable() {
        let (_dir, conn) = test_db();
        conn.execute_batch("DROP TABLE sessions; DROP TABLE users;")
            .unwrap();
        set_metadata(&conn, "schema_version", "16").unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
        assert!(table_exists(&conn, "users"));
        assert!(table_exists(&conn, "sessions"));
        set_metadata(&conn, "schema_version", "16").unwrap();
        run_migrations(&conn).unwrap();
        assert_eq!(get_schema_version(&conn).unwrap(), LATEST_VERSION);
    }

    #[test]
    fn v17_refuses_an_unknown_role_and_a_second_spelling_of_an_email() {
        let (_dir, conn) = test_db();
        let insert = |uid: &str, email: &str, role: &str| {
            conn.execute(
                "INSERT INTO users (uid, email, display_name, password_hash, role,
                                    password_changed_at, created_at, updated_at)
                 VALUES (?1, ?2, 'Pat Example', 'x', ?3, 't', 't', 't')",
                rusqlite::params![uid, email, role],
            )
        };
        insert("a", "pat@example.com", "admin").unwrap();
        assert!(insert("b", "PAT@example.com", "admin").is_err());
        assert!(insert("c", "sam@example.com", "owner").is_err());
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p nigel-core migrations::tests::v17 -- --test-threads=1`
Expected: FAIL — `v17_creates_users_and_sessions_on_a_fresh_database` panics on `assert!(table_exists(&conn, "users"))`; the others fail on `no such table: users`.

- [ ] **Step 3: Append the migration**

After the `version: 16` entry, before the closing `];` of `MIGRATIONS`:

```rust
    Migration {
        version: 17,
        description: "users and sessions for signing in to a shared server",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS users (
                    id                    INTEGER PRIMARY KEY,
                    uid                   TEXT NOT NULL UNIQUE,
                    email                 TEXT NOT NULL UNIQUE COLLATE NOCASE,
                    display_name          TEXT NOT NULL,
                    password_hash         TEXT NOT NULL,
                    role                  TEXT NOT NULL
                                          CHECK (role IN ('admin', 'bookkeeper', 'read_only')),
                    totp_secret           BLOB,
                    totp_last_step        INTEGER,
                    must_change_password  INTEGER NOT NULL DEFAULT 0,
                    password_changed_at   TEXT NOT NULL,
                    disabled_at           TEXT,
                    created_at            TEXT NOT NULL,
                    updated_at            TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS sessions (
                    id            INTEGER PRIMARY KEY,
                    token_hash    BLOB NOT NULL UNIQUE,
                    user_id       INTEGER NOT NULL REFERENCES users(id),
                    kind          TEXT NOT NULL CHECK (kind IN ('cookie', 'bearer')),
                    created_at    TEXT NOT NULL,
                    last_seen_at  TEXT NOT NULL,
                    expires_at    TEXT NOT NULL,
                    user_agent    TEXT,
                    ip            TEXT
                 );
                 CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
                 CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);",
            )?;
            Ok(())
        },
    },
```

- [ ] **Step 4: Run the migration tests to see them pass**

Run: `cargo test -p nigel-core migrations -- --test-threads=1`
Expected: every test passes, including `migration_versions_are_contiguous_from_one`, `test_idempotent_rerun` and the three `v17_` tests.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/migrations.rs
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "Migration 17: users and sessions"
```

---

### Task 3: `identity::passwords` — rules, Argon2id, temporary passwords

**Files:**
- Create: `crates/nigel-core/src/identity/passwords.rs`
- Create: `crates/nigel-core/src/identity/common-passwords.txt`
- Modify: `crates/nigel-core/src/identity/mod.rs` (`pub mod passwords;`)

**Interfaces:**
- Consumes: `crate::error::{NigelError, Result}`.
- Produces: `PasswordRule` (`TooShort`, `TooLong`, `Common`, `MatchesIdentity`, `Unchanged`; `reason_code()`; `Display`; `From<PasswordRule> for NigelError` → `Invalid`), `PasswordOwner<'a> { email, display_name, current_hash: Option<&str> }`, `MIN_LENGTH = 12`, `MAX_LENGTH = 256`, `normalize(&str) -> Zeroizing<String>`, `check_new_password(&str, &PasswordOwner) -> Result<(), PasswordRule>`, `hash_password(&str) -> Result<String>`, `verify_password(&str, &str) -> bool`, `verify_dummy(&str)`, `temporary_password() -> Zeroizing<String>`.

- [ ] **Step 1: Bundle the common-password list**

```bash
curl -fsSL -o crates/nigel-core/src/identity/common-passwords.txt \
  "https://raw.githubusercontent.com/danielmiessler/SecLists/master/Passwords/Common-Credentials/10k-most-common.txt"
sha256sum crates/nigel-core/src/identity/common-passwords.txt
wc -l crates/nigel-core/src/identity/common-passwords.txt
./scripts/check-no-real-data.sh; echo "exit=$?"
```
Expected: sha256 `68782d6a4a19a4768d5f15dd66bd534e7a33055cc755411e33f16d18c50fdcce`, about 10,000 lines, check exit 0. If upstream has changed and the hash differs, the file is still acceptable if it is the same list (one password per line, MIT-licensed SecLists); record the new hash in the commit message. If the check script ever fails on an entry, delete that line. The licence notice is the `COMMON_PASSWORDS` doc comment below.

- [ ] **Step 2: Write the failing tests**

Add `pub mod passwords;` to `identity/mod.rs` (above `/// Ordered`), and create `identity/passwords.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> PasswordOwner<'static> {
        PasswordOwner {
            email: "pat@example.com",
            display_name: "Pat Example",
            current_hash: None,
        }
    }

    #[test]
    fn hashes_are_argon2id_with_the_pinned_parameters() {
        let phc = hash_password("correct horse battery").unwrap();
        assert!(phc.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"), "{phc}");
        let salt_b64 = phc.split('$').nth(4).unwrap();
        assert_eq!(salt_b64.len(), 22, "a 16-byte salt is 22 base64 chars");
    }

    #[test]
    fn the_same_password_hashes_differently_each_time() {
        let a = hash_password("correct horse battery").unwrap();
        let b = hash_password("correct horse battery").unwrap();
        assert_ne!(a, b);
        assert!(verify_password("correct horse battery", &a));
        assert!(verify_password("correct horse battery", &b));
    }

    #[test]
    fn verify_refuses_a_wrong_password_and_garbage() {
        let phc = hash_password("correct horse battery").unwrap();
        assert!(!verify_password("correct horse batter", &phc));
        assert!(!verify_password("correct horse battery", "not a hash"));
        assert!(!verify_password("correct horse battery", ""));
    }

    #[test]
    fn nfc_and_nfd_spellings_verify_against_each_other() {
        let nfc = "caf\u{e9} au lait daily";
        let nfd = "cafe\u{301} au lait daily";
        let phc = hash_password(nfc).unwrap();
        assert!(verify_password(nfd, &phc));
    }

    #[test]
    fn the_dummy_path_runs_and_matches_nothing() {
        verify_dummy("anything at all here");
        verify_dummy("");
    }

    #[test]
    fn length_counts_characters_after_normalization() {
        assert_eq!(
            check_new_password("elevenchars", &owner()),
            Err(PasswordRule::TooShort)
        );
        assert_eq!(check_new_password("twelve chars", &owner()), Ok(()));
        // Eleven characters, twelve code points before NFC.
        let nfd = "cafe\u{301}-au-lai";
        assert_eq!(nfd.chars().count(), 12);
        assert_eq!(
            check_new_password(nfd, &owner()),
            Err(PasswordRule::TooShort)
        );
        assert_eq!(
            check_new_password(&"x".repeat(257), &owner()),
            Err(PasswordRule::TooLong)
        );
        assert_eq!(check_new_password(&"x".repeat(256), &owner()), Ok(()));
    }

    #[test]
    fn a_common_password_is_refused_whatever_its_case() {
        assert_eq!(
            check_new_password("unbelievable", &owner()),
            Err(PasswordRule::Common)
        );
        assert_eq!(
            check_new_password("UnBelievable", &owner()),
            Err(PasswordRule::Common)
        );
    }

    #[test]
    fn the_list_is_bundled_whole() {
        assert!(
            common_passwords().len() >= 9_900,
            "{}",
            common_passwords().len()
        );
        assert!(common_passwords().contains("password"));
    }

    #[test]
    fn the_email_its_local_part_and_the_name_are_refused() {
        let owner = PasswordOwner {
            email: "patexample12@example.com",
            display_name: "Pat Example Jones",
            current_hash: None,
        };
        for candidate in [
            "PatExample12@example.com",
            "patexample12",
            "pat example jones",
        ] {
            assert_eq!(
                check_new_password(candidate, &owner),
                Err(PasswordRule::MatchesIdentity),
                "{candidate}"
            );
        }
    }

    #[test]
    fn the_current_password_is_refused_as_unchanged() {
        let current = hash_password("correct horse battery").unwrap();
        let owner = PasswordOwner {
            current_hash: Some(&current),
            ..owner()
        };
        assert_eq!(
            check_new_password("correct horse battery", &owner),
            Err(PasswordRule::Unchanged)
        );
        assert_eq!(check_new_password("a different one", &owner), Ok(()));
    }

    #[test]
    fn each_rule_has_its_wire_code() {
        let codes: Vec<_> = [
            PasswordRule::TooShort,
            PasswordRule::TooLong,
            PasswordRule::Common,
            PasswordRule::MatchesIdentity,
            PasswordRule::Unchanged,
        ]
        .iter()
        .map(|r| r.reason_code())
        .collect();
        assert_eq!(
            codes,
            [
                "too_short",
                "too_long",
                "common",
                "matches_identity",
                "unchanged"
            ]
        );
    }

    #[test]
    fn temporary_passwords_are_four_groups_of_five_unambiguous_characters() {
        let pw = temporary_password();
        assert_eq!(pw.len(), 23, "{}", pw.as_str());
        let groups: Vec<&str> = pw.split('-').collect();
        assert_eq!(groups.len(), 4);
        for group in groups {
            assert_eq!(group.len(), 5);
            assert!(group.bytes().all(|b| TEMPORARY_ALPHABET.contains(&b)));
        }
        assert_ne!(temporary_password().as_str(), pw.as_str());
    }

    #[test]
    fn a_temporary_password_passes_the_rules_it_never_has_to_meet() {
        let pw = temporary_password();
        assert_eq!(check_new_password(&pw, &owner()), Ok(()));
    }
}
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test -p nigel-core identity::passwords -- --test-threads=1`
Expected: compile errors — `cannot find type PasswordOwner`, `cannot find function hash_password`, etc.

- [ ] **Step 4: Implement**

Put this above the tests in `passwords.rs`:

```rust
//! User passwords: the rules a new one must meet, Argon2id hashing, and the
//! temporary passwords an admin hands out.
//!
//! This is sign-in identity. The books' encryption password is `crate::password`
//! and has nothing to do with it.

use std::collections::HashSet;
use std::fmt;
use std::sync::OnceLock;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::Rng;
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use crate::error::{NigelError, Result};

pub const MIN_LENGTH: usize = 12;
pub const MAX_LENGTH: usize = 256;

/// Argon2id memory cost in KiB (19 MiB), passes and lanes. Pinned rather than
/// taken from the crate's defaults, so a dependency bump cannot change them.
const M_COST_KIB: u32 = 19 * 1024;
const T_COST: u32 = 2;
const P_COST: u32 = 1;
const OUTPUT_LEN: usize = 32;

/// The 10,000 most common passwords, one per line (SecLists, MIT licence).
const COMMON_PASSWORDS: &str = include_str!("common-passwords.txt");

/// Crockford's base32 alphabet: no I, L, O or U, so nothing on paper reads as
/// two different characters.
const TEMPORARY_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Why a new password was refused. `reason_code` is the API's `details.reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordRule {
    TooShort,
    TooLong,
    Common,
    MatchesIdentity,
    Unchanged,
}

impl PasswordRule {
    pub fn reason_code(self) -> &'static str {
        match self {
            PasswordRule::TooShort => "too_short",
            PasswordRule::TooLong => "too_long",
            PasswordRule::Common => "common",
            PasswordRule::MatchesIdentity => "matches_identity",
            PasswordRule::Unchanged => "unchanged",
        }
    }
}

impl fmt::Display for PasswordRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PasswordRule::TooShort => {
                write!(f, "Use at least {MIN_LENGTH} characters.")
            }
            PasswordRule::TooLong => write!(f, "Use at most {MAX_LENGTH} characters."),
            PasswordRule::Common => write!(
                f,
                "That password is on a list of common passwords. Choose another."
            ),
            PasswordRule::MatchesIdentity => write!(
                f,
                "Don't use your email address or your name as your password."
            ),
            PasswordRule::Unchanged => {
                write!(f, "Choose a password different from your current one.")
            }
        }
    }
}

impl std::error::Error for PasswordRule {}

impl From<PasswordRule> for NigelError {
    fn from(rule: PasswordRule) -> Self {
        NigelError::Invalid(rule.to_string())
    }
}

/// Who the password belongs to, for the identity rule, and the hash it would
/// replace, for the unchanged rule.
pub struct PasswordOwner<'a> {
    pub email: &'a str,
    pub display_name: &'a str,
    pub current_hash: Option<&'a str>,
}

/// NFC, so the same password typed on two keyboards hashes the same.
pub fn normalize(password: &str) -> Zeroizing<String> {
    Zeroizing::new(password.nfc().collect())
}

fn common_passwords() -> &'static HashSet<String> {
    static SET: OnceLock<HashSet<String>> = OnceLock::new();
    SET.get_or_init(|| {
        COMMON_PASSWORDS
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_lowercase)
            .collect()
    })
}

/// The one check every new password passes, from the CLI and the API alike.
/// Checked in this order: length, the common list, identity, unchanged.
pub fn check_new_password(
    password: &str,
    owner: &PasswordOwner<'_>,
) -> std::result::Result<(), PasswordRule> {
    let normalized = normalize(password);
    let length = normalized.chars().count();
    if length < MIN_LENGTH {
        return Err(PasswordRule::TooShort);
    }
    if length > MAX_LENGTH {
        return Err(PasswordRule::TooLong);
    }
    let folded = Zeroizing::new(normalized.to_lowercase());
    if common_passwords().contains(folded.as_str()) {
        return Err(PasswordRule::Common);
    }
    let email = owner.email.trim().to_lowercase();
    let local_part = email.split('@').next().unwrap_or("").to_string();
    let name = owner.display_name.trim().to_lowercase();
    if [email, local_part, name]
        .iter()
        .any(|identity| !identity.is_empty() && *identity == *folded)
    {
        return Err(PasswordRule::MatchesIdentity);
    }
    if let Some(current) = owner.current_hash {
        if verify_password(password, current) {
            return Err(PasswordRule::Unchanged);
        }
    }
    Ok(())
}

fn hasher() -> Argon2<'static> {
    // unwrap: the constants above are within Argon2's documented bounds
    let params = Params::new(M_COST_KIB, T_COST, P_COST, Some(OUTPUT_LEN)).unwrap();
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// An Argon2id PHC string for `password`, NFC-normalized, with a fresh
/// 16-byte salt.
pub fn hash_password(password: &str) -> Result<String> {
    let normalized = normalize(password);
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    hasher()
        .hash_password(normalized.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| NigelError::Other("Could not hash the password.".into()))
}

/// Whether `password` matches the PHC string. A string that does not parse is
/// a mismatch, never a panic.
pub fn verify_password(password: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return false;
    };
    let normalized = normalize(password);
    hasher()
        .verify_password(normalized.as_bytes(), &parsed)
        .is_ok()
}

/// One verification against a fixed hash, for an email that matches no user,
/// so an unknown address costs the same time as a known one.
pub fn verify_dummy(password: &str) {
    static DUMMY: OnceLock<String> = OnceLock::new();
    let phc = DUMMY.get_or_init(|| {
        // unwrap: hashing a fixed string with valid parameters cannot fail
        hash_password("nigel-dummy-password-never-matches").unwrap()
    });
    let _ = verify_password(password, phc);
}

/// Twenty characters from [`TEMPORARY_ALPHABET`] in four groups of five, about
/// 100 bits: `K7QXM-2RTPA-W9HCN-4FJDE`.
pub fn temporary_password() -> Zeroizing<String> {
    let mut rng = rand::rngs::OsRng;
    let mut out = Zeroizing::new(String::with_capacity(23));
    for i in 0..20 {
        if i > 0 && i % 5 == 0 {
            out.push('-');
        }
        out.push(TEMPORARY_ALPHABET[rng.gen_range(0..32)] as char);
    }
    out
}
```

- [ ] **Step 5: Run them to see them pass**

Run: `cargo test -p nigel-core identity::passwords -- --test-threads=1`
Expected: 13 passed, in a few seconds (the dev-profile override from Task 1 keeps Argon2 fast).

- [ ] **Step 6: Commit**

```bash
git add crates/nigel-core/src/identity/
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "identity: password rules, Argon2id hashing and temporary passwords

The common-password list is SecLists 10k-most-common.txt (MIT)."
```

---

### Task 4: `identity::totp` — RFC 6238 codes and the skew window

**Files:**
- Create: `crates/nigel-core/src/identity/totp.rs`
- Modify: `crates/nigel-core/src/identity/mod.rs` (`pub mod totp;`)

**Interfaces:**
- Consumes: `hmac`, `sha1`, `subtle`, `data-encoding`, `rand`.
- Produces: `SECRET_LEN = 20`, `STEP_SECONDS = 30`, `DIGITS = 6`, `SKEW_STEPS = 1`, `generate_secret() -> Zeroizing<[u8; 20]>`, `step_at(DateTime<Utc>) -> i64`, `hotp(&[u8], u64, u32) -> u32`, `code_for_step(&[u8], i64) -> String`, `matching_step(&[u8], &str, DateTime<Utc>) -> Option<i64>`, `secret_base32(&[u8]) -> Zeroizing<String>`, `otpauth_uri(&str, &[u8]) -> Zeroizing<String>`. Task 8 uses `matching_step` and `code_for_step`; PR 2 uses the rest for enrollment.

- [ ] **Step 1: Write the failing tests**

Add `pub mod totp;` to `identity/mod.rs`, and create `identity/totp.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const RFC_SECRET: &[u8] = b"12345678901234567890";

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).unwrap()
    }

    /// RFC 6238 appendix B, SHA-1 column, eight digits.
    #[test]
    fn matches_the_rfc_6238_sha1_vectors() {
        let vectors = [
            (59, 94287082),
            (1111111109, 7081804),
            (1111111111, 14050471),
            (1234567890, 89005924),
            (2000000000, 69279037),
            (20000000000, 65353130),
        ];
        for (seconds, expected) in vectors {
            let step = step_at(at(seconds)) as u64;
            assert_eq!(hotp(RFC_SECRET, step, 8), expected, "T={seconds}");
        }
    }

    #[test]
    fn six_digit_codes_are_the_low_digits_zero_padded() {
        assert_eq!(code_for_step(RFC_SECRET, step_at(at(1111111109))), "081804");
        assert_eq!(code_for_step(RFC_SECRET, step_at(at(1234567890))), "005924");
    }

    #[test]
    fn one_step_either_side_is_accepted_and_two_is_not() {
        let now = at(1_800_000_000);
        let current = step_at(now);
        for offset in [-1, 0, 1] {
            let code = code_for_step(RFC_SECRET, current + offset);
            assert_eq!(
                matching_step(RFC_SECRET, &code, now),
                Some(current + offset),
                "offset {offset}"
            );
        }
        for offset in [-2, 2] {
            let code = code_for_step(RFC_SECRET, current + offset);
            assert_eq!(
                matching_step(RFC_SECRET, &code, now),
                None,
                "offset {offset}"
            );
        }
    }

    #[test]
    fn a_code_typed_with_a_space_still_matches_and_junk_never_does() {
        let now = at(1_800_000_000);
        let code = code_for_step(RFC_SECRET, step_at(now));
        let spaced = format!("{} {}", &code[..3], &code[3..]);
        assert!(matching_step(RFC_SECRET, &spaced, now).is_some());
        for junk in ["", "12345", "1234567", "abcdef", "１２３４５６", "12345\n"] {
            assert_eq!(matching_step(RFC_SECRET, junk, now), None, "{junk:?}");
        }
    }

    #[test]
    fn a_generated_secret_is_160_bits_and_not_repeated() {
        let a = generate_secret();
        let b = generate_secret();
        assert_eq!(a.len(), 20);
        assert_ne!(*a, *b);
    }

    #[test]
    fn the_uri_carries_the_base32_secret_and_the_issuer() {
        let uri = otpauth_uri("pat+books@example.com", RFC_SECRET);
        assert_eq!(
            uri.as_str(),
            "otpauth://totp/Nigel:pat%2Bbooks@example.com?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Nigel"
        );
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p nigel-core identity::totp -- --test-threads=1`
Expected: compile errors — `cannot find function hotp`, `step_at`, …

- [ ] **Step 3: Implement**

Above the tests:

```rust
//! RFC 6238 time-based one-time passwords: HMAC-SHA1, six digits, a 30-second
//! step, a 160-bit secret, and one step of skew either side.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub const SECRET_LEN: usize = 20;
pub const STEP_SECONDS: i64 = 30;
pub const DIGITS: u32 = 6;
/// Steps accepted either side of the current one.
pub const SKEW_STEPS: i64 = 1;

/// A fresh 160-bit secret from the operating system's CSPRNG.
pub fn generate_secret() -> Zeroizing<[u8; SECRET_LEN]> {
    let mut secret = Zeroizing::new([0u8; SECRET_LEN]);
    rand::rngs::OsRng.fill_bytes(secret.as_mut());
    secret
}

/// The RFC 6238 time step `now` falls in.
pub fn step_at(now: DateTime<Utc>) -> i64 {
    now.timestamp().div_euclid(STEP_SECONDS)
}

/// RFC 4226 HOTP: the `digits`-digit code for `counter`.
pub fn hotp(secret: &[u8], counter: u64, digits: u32) -> u32 {
    // unwrap: HMAC accepts a key of any length
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).unwrap();
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    binary % 10u32.pow(digits)
}

/// The six-digit code for a step, zero-padded.
pub fn code_for_step(secret: &[u8], step: i64) -> String {
    format!(
        "{:0width$}",
        hotp(secret, step as u64, DIGITS),
        width = DIGITS as usize
    )
}

/// The code as typed, reduced to its digits: spaces are dropped (authenticator
/// apps show `123 456`), and anything else that is not exactly six ASCII
/// digits is `None`.
fn clean_code(code: &str) -> Option<String> {
    let digits: String = code.chars().filter(|c| *c != ' ').collect();
    (digits.len() == DIGITS as usize && digits.bytes().all(|b| b.is_ascii_digit()))
        .then_some(digits)
}

/// The step a code matches within the skew window around `now`, newest first,
/// or `None`. Every candidate is compared in constant time, and all three are
/// always computed.
pub fn matching_step(secret: &[u8], code: &str, now: DateTime<Utc>) -> Option<i64> {
    let code = clean_code(code)?;
    let current = step_at(now);
    let mut found = None;
    for step in (current - SKEW_STEPS..=current + SKEW_STEPS).rev() {
        let expected = code_for_step(secret, step);
        if bool::from(expected.as_bytes().ct_eq(code.as_bytes())) && found.is_none() {
            found = Some(step);
        }
    }
    found
}

/// The secret as unpadded RFC 4648 base32, for typing in by hand.
pub fn secret_base32(secret: &[u8]) -> Zeroizing<String> {
    Zeroizing::new(data_encoding::BASE32_NOPAD.encode(secret))
}

/// `otpauth://totp/Nigel:<email>?secret=…&issuer=Nigel`, the URI a QR code
/// carries. The email is percent-encoded as a path segment.
pub fn otpauth_uri(email: &str, secret: &[u8]) -> Zeroizing<String> {
    let mut label = String::new();
    for byte in email.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'@' => {
                label.push(byte as char)
            }
            _ => label.push_str(&format!("%{byte:02X}")),
        }
    }
    Zeroizing::new(format!(
        "otpauth://totp/Nigel:{label}?secret={}&issuer=Nigel",
        secret_base32(secret).as_str()
    ))
}
```

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p nigel-core identity::totp -- --test-threads=1`
Expected: 6 passed, including `matches_the_rfc_6238_sha1_vectors`.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/identity/
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "identity: RFC 6238 TOTP with a one-step skew window"
```

---

### Task 5: `identity::seal` — the server key and sealed secrets

**Files:**
- Create: `crates/nigel-core/src/identity/seal.rs`
- Modify: `crates/nigel-core/src/identity/mod.rs` (`pub mod seal;`)

**Interfaces:**
- Consumes: `crate::settings::{config_dir, restrict_dir_permissions, TempConfigDir, get_data_dir}`, `chacha20poly1305`, `rand`.
- Produces: `KEY_FILE = "server.key"`, `key_path() -> PathBuf`, `ServerKey` with `load(&Path) -> Result<Option<ServerKey>>`, `load_or_create(&Path) -> Result<ServerKey>`, `seal(&self, uid, plaintext) -> Result<Vec<u8>>`, `open(&self, uid, sealed) -> Result<Zeroizing<Vec<u8>>>`, and `from_bytes([u8; 32])` behind `#[cfg(any(test, feature = "testutil"))]` (PR 2's server tests need it through `testutil`). Sealed layout: `0x01 || 24-byte nonce || ciphertext+tag`.

- [ ] **Step 1: Write the failing tests**

Add `pub mod seal;` to `identity/mod.rs`, and create `identity/seal.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const UID: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn a_sealed_secret_round_trips() {
        let key = ServerKey::from_bytes([7; 32]);
        let sealed = key.seal(UID, b"twenty byte secret!!").unwrap();
        assert_eq!(sealed[0], 1);
        assert_eq!(sealed.len(), 1 + 24 + 20 + 16);
        assert_eq!(
            key.open(UID, &sealed).unwrap().as_slice(),
            b"twenty byte secret!!"
        );
    }

    #[test]
    fn sealing_twice_gives_different_bytes() {
        let key = ServerKey::from_bytes([7; 32]);
        assert_ne!(key.seal(UID, b"x").unwrap(), key.seal(UID, b"x").unwrap());
    }

    #[test]
    fn another_users_uid_cannot_open_it() {
        let key = ServerKey::from_bytes([7; 32]);
        let sealed = key.seal(UID, b"secret").unwrap();
        assert!(key
            .open("ffffffffffffffffffffffffffffffff", &sealed)
            .is_err());
    }

    #[test]
    fn another_key_cannot_open_it() {
        let sealed = ServerKey::from_bytes([7; 32]).seal(UID, b"secret").unwrap();
        assert!(ServerKey::from_bytes([8; 32]).open(UID, &sealed).is_err());
    }

    #[test]
    fn truncated_tampered_and_unknown_versions_are_refused() {
        let key = ServerKey::from_bytes([7; 32]);
        let sealed = key.seal(UID, b"secret").unwrap();
        assert!(key.open(UID, &sealed[..10]).is_err());
        assert!(key.open(UID, &[]).is_err());
        let mut flipped = sealed.clone();
        *flipped.last_mut().unwrap() ^= 1;
        assert!(key.open(UID, &flipped).is_err());
        let mut versioned = sealed;
        versioned[0] = 2;
        assert!(key.open(UID, &versioned).is_err());
    }

    #[test]
    fn debug_never_prints_the_key() {
        let key = ServerKey::from_bytes([0xAB; 32]);
        let printed = format!("{key:?}");
        assert_eq!(printed, "ServerKey(<redacted>)");
    }

    #[test]
    fn the_key_file_is_created_once_owner_only_and_reread() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config").join(KEY_FILE);
        assert!(ServerKey::load(&path).unwrap().is_none());
        let created = ServerKey::load_or_create(&path).unwrap();
        let sealed = created.seal(UID, b"secret").unwrap();
        let reread = ServerKey::load_or_create(&path).unwrap();
        assert_eq!(reread.open(UID, &sealed).unwrap().as_slice(), b"secret");
        assert_eq!(std::fs::read(&path).unwrap().len(), 32);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            let dir_mode = std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600);
            assert_eq!(dir_mode, 0o700);
        }
    }

    #[test]
    fn a_damaged_key_file_is_an_error_and_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEY_FILE);
        std::fs::write(&path, b"short").unwrap();
        let err = ServerKey::load_or_create(&path).unwrap_err().to_string();
        assert!(err.contains("5 bytes"), "{err}");
        assert!(err.contains("--clear-totp"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), b"short");
    }

    #[test]
    fn the_key_lives_in_the_config_directory_not_the_data_directory() {
        let _config = crate::settings::TempConfigDir::new();
        assert_eq!(key_path(), crate::settings::config_dir().join("server.key"));
        assert!(!key_path().starts_with(crate::settings::get_data_dir()));
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p nigel-core identity::seal -- --test-threads=1`
Expected: compile errors — `cannot find type ServerKey`, `cannot find value KEY_FILE`.

- [ ] **Step 3: Implement**

Above the tests:

```rust
//! Sealing TOTP secrets under the per-machine server key.
//!
//! The key is 32 random bytes in `<config_dir>/server.key`, never in the data
//! directory, so a backup or a copied database carries no usable second
//! factor. A sealed value is `0x01 || 24-byte nonce || ciphertext+tag`,
//! XChaCha20-Poly1305 with the user's `uid` as associated data, so it cannot
//! be moved to another user's row.

use std::fmt;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use zeroize::Zeroizing;

use crate::error::{NigelError, Result};

pub const KEY_FILE: &str = "server.key";
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const SEAL_VERSION: u8 = 1;

/// The server key. `Debug` is redacted and the bytes are wiped on drop.
pub struct ServerKey(Zeroizing<[u8; KEY_LEN]>);

impl fmt::Debug for ServerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ServerKey(<redacted>)")
    }
}

/// Where the key lives: beside `settings.json`.
pub fn key_path() -> PathBuf {
    crate::settings::config_dir().join(KEY_FILE)
}

impl ServerKey {
    #[cfg(any(test, feature = "testutil"))]
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// The key at `path`, or `None` when there is no file. A file of the wrong
    /// length is an error and is left exactly as it is.
    pub fn load(path: &Path) -> Result<Option<Self>> {
        let mut file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut bytes = Zeroizing::new(Vec::with_capacity(KEY_LEN));
        file.read_to_end(&mut bytes)?;
        if bytes.len() != KEY_LEN {
            return Err(NigelError::Other(format!(
                "{} is not a Nigel server key ({} bytes, expected {KEY_LEN}). \
                 It has been left as it is. Move it aside to create a new one; \
                 every user with two-step verification will then need \
                 `nigel user reset-password <email> --clear-totp`.",
                path.display(),
                bytes.len()
            )));
        }
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        key.copy_from_slice(&bytes);
        Ok(Some(Self(key)))
    }

    /// The key at `path`, created first when there is none: mode 0600 in a
    /// 0700 directory. Two processes racing to create it agree on one file,
    /// because creation is `create_new` and the loser reads the winner's.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if let Some(key) = Self::load(path)? {
            return Ok(key);
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
            crate::settings::restrict_dir_permissions(dir)?;
        }
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        rand::rngs::OsRng.fill_bytes(key.as_mut());
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(path) {
            Ok(mut file) => {
                file.write_all(key.as_ref())?;
                file.sync_all()?;
                Ok(Self(key))
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Self::load(path)?
                .ok_or_else(|| NigelError::Other(format!("{} vanished", path.display()))),
            Err(e) => Err(e.into()),
        }
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(Key::from_slice(self.0.as_ref()))
    }

    /// Seal `plaintext` for the user `uid`.
    pub fn seal(&self, uid: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher()
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: uid.as_bytes(),
                },
            )
            .map_err(|_| NigelError::Other("Could not seal the secret.".into()))?;
        let mut sealed = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
        sealed.push(SEAL_VERSION);
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(&ciphertext);
        Ok(sealed)
    }

    /// Open a value sealed for `uid`. The wrong key, the wrong `uid`, a
    /// truncated value and an unknown version are all the same error, which
    /// names none of the bytes.
    pub fn open(&self, uid: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let refused = || {
            NigelError::Other(
                "A two-step secret could not be opened with this machine's server key.".into(),
            )
        };
        if sealed.len() < 1 + NONCE_LEN || sealed[0] != SEAL_VERSION {
            return Err(refused());
        }
        let (nonce, ciphertext) = sealed[1..].split_at(NONCE_LEN);
        self.cipher()
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: uid.as_bytes(),
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| refused())
    }
}
```

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p nigel-core identity::seal -- --test-threads=1`
Expected: 9 passed, including `a_damaged_key_file_is_an_error_and_is_never_replaced` and (on Unix) the 0600/0700 assertions.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/identity/
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "identity: seal TOTP secrets under a per-machine server.key"
```

---

### Task 6: `identity::sessions` — session rows stored as hashes

**Files:**
- Create: `crates/nigel-core/src/identity/sessions.rs`
- Modify: `crates/nigel-core/src/identity/mod.rs` (`pub mod sessions;`, `stamp`, the `testing` helpers, a test)

**Interfaces:**
- Consumes: `Role` (Task 1), the `users`/`sessions` tables (Task 2), `sha2`, `hex`, `rand`.
- Produces: `pub(crate) fn stamp(DateTime<Utc>) -> String` in `identity/mod.rs`; `#[cfg(test)] identity::testing::{db, at, insert_user}`; and in `sessions.rs`: `IDLE_DAYS = 30`, `ABSOLUTE_DAYS = 90`, `RENEW_AFTER_SECONDS = 3600`, `USER_AGENT_MAX_CHARS = 256`, `SessionKind { Cookie, Bearer }` (`as_str`, `parse`), `IssuedSession { id, token, expires_at }`, `ActiveSession { id, user_id, uid, role, must_change_password, kind, created_at, last_seen_at, expires_at }`, `token_hash`, `create_session`, `find_session`, `renew_session`, `end_session`, `end_user_sessions(conn, user_id, except: Option<i64>) -> Result<usize>`, `sweep_expired`. Task 7 calls `end_user_sessions`; PR 2 calls the rest.

- [ ] **Step 1: Add `stamp` and the test helpers to `identity/mod.rs`**

Add `pub mod sessions;` beside the other `pub mod` lines and `use chrono::{DateTime, SecondsFormat, Utc};` below them. Between `impl Role { … }` and `#[cfg(test)] mod tests`, add:

```rust
/// The stored form of an instant: UTC RFC 3339 to the second, `Z`-suffixed,
/// so stored values compare correctly as strings.
pub(crate) fn stamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
pub(crate) mod testing {
    use chrono::{DateTime, Utc};
    use rusqlite::Connection;

    pub fn db() -> (tempfile::TempDir, Connection) {
        crate::db::set_db_password_for_tests(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = crate::db::open_connection(&dir.path().join("nigel.db"), None).expect("open");
        crate::db::init_db(&conn).expect("init");
        (dir, conn)
    }

    pub fn at(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .expect("timestamp")
            .with_timezone(&Utc)
    }

    /// A user row written directly, for tests of the layers below `users.rs`.
    /// `must_change_password` is set, as it is for every new user.
    pub fn insert_user(conn: &Connection, email: &str, role: &str) -> i64 {
        conn.execute(
            "INSERT INTO users (uid, email, display_name, password_hash, role,
                                must_change_password, password_changed_at, created_at, updated_at)
             VALUES (lower(hex(randomblob(16))), ?1, 'Pat Example', 'unused', ?2, 1,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![email, role],
        )
        .expect("insert user");
        conn.last_insert_rowid()
    }
}
```

and inside `mod tests`, after `roles_order_and_round_trip`:

```rust
    #[test]
    fn stamps_are_utc_seconds_with_a_z() {
        assert_eq!(
            stamp(testing::at("2026-10-24T09:30:15.987+02:00")),
            "2026-10-24T07:30:15Z"
        );
    }
```

- [ ] **Step 2: Write the failing session tests**

Create `identity/sessions.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::testing::{at, db, insert_user};

    fn admin(conn: &Connection) -> i64 {
        insert_user(conn, "pat@example.com", "admin")
    }

    #[test]
    fn the_token_is_64_hex_and_only_its_hash_is_stored() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let issued = create_session(
            &conn,
            user,
            SessionKind::Cookie,
            None,
            None,
            at("2026-01-01T00:00:00Z"),
        )
        .unwrap();
        assert_eq!(issued.token.len(), 64);
        assert!(issued.token.bytes().all(|b| b.is_ascii_hexdigit()));
        let stored: Vec<u8> = conn
            .query_row("SELECT token_hash FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            stored.as_slice(),
            token_hash(&issued.token).unwrap().as_slice()
        );
        assert_ne!(stored, hex::decode(issued.token.as_str()).unwrap());
        assert_eq!(issued.expires_at, "2026-01-31T00:00:00Z");
    }

    #[test]
    fn a_token_finds_its_session_and_nothing_else_does() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let now = at("2026-01-01T00:00:00Z");
        let issued = create_session(&conn, user, SessionKind::Bearer, None, None, now).unwrap();
        let found = find_session(&conn, &issued.token, now).unwrap().unwrap();
        assert_eq!(found.user_id, user);
        assert_eq!(found.role, Role::Admin);
        assert_eq!(found.kind, SessionKind::Bearer);
        assert!(found.must_change_password);
        assert!(find_session(&conn, &"0".repeat(64), now).unwrap().is_none());
        assert!(find_session(&conn, "not hex", now).unwrap().is_none());
        assert!(find_session(&conn, &issued.token[..62], now)
            .unwrap()
            .is_none());
    }

    #[test]
    fn an_expired_session_or_a_disabled_user_finds_nothing() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let sam = insert_user(&conn, "sam@example.com", "bookkeeper");
        let start = at("2026-01-01T00:00:00Z");
        let mine = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        assert!(find_session(&conn, &mine.token, at("2026-01-31T00:00:00Z"))
            .unwrap()
            .is_none());
        let theirs = create_session(&conn, sam, SessionKind::Cookie, None, None, start).unwrap();
        conn.execute("UPDATE users SET disabled_at = 'x' WHERE id = ?1", [sam])
            .unwrap();
        assert!(find_session(&conn, &theirs.token, start).unwrap().is_none());
    }

    #[test]
    fn renewal_waits_an_hour_slides_thirty_days_and_never_passes_ninety() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let start = at("2026-01-01T00:00:00Z");
        let issued = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        let session = |now| find_session(&conn, &issued.token, now).unwrap().unwrap();

        let soon = at("2026-01-01T00:59:00Z");
        assert_eq!(renew_session(&conn, &session(soon), soon).unwrap(), None);

        let later = at("2026-01-20T00:00:00Z");
        assert_eq!(
            renew_session(&conn, &session(later), later)
                .unwrap()
                .as_deref(),
            Some("2026-02-19T00:00:00Z")
        );

        let near_the_cap = at("2026-03-15T00:00:00Z");
        conn.execute(
            "UPDATE sessions SET last_seen_at = '2026-03-01T00:00:00Z', expires_at = '2026-03-31T00:00:00Z'",
            [],
        )
        .unwrap();
        assert_eq!(
            renew_session(&conn, &session(near_the_cap), near_the_cap)
                .unwrap()
                .as_deref(),
            Some("2026-04-01T00:00:00Z"),
            "created_at + 90 days caps the renewal"
        );
        assert!(
            find_session(&conn, &issued.token, at("2026-04-01T00:00:00Z"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn the_user_agent_is_cut_to_256_characters() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let long = "é".repeat(300);
        create_session(
            &conn,
            user,
            SessionKind::Cookie,
            Some(&long),
            Some("203.0.113.9"),
            at("2026-01-01T00:00:00Z"),
        )
        .unwrap();
        let (ua, ip): (String, String) = conn
            .query_row("SELECT user_agent, ip FROM sessions", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(ua.chars().count(), 256);
        assert_eq!(ip, "203.0.113.9");
    }

    #[test]
    fn ending_one_all_or_all_but_one_and_sweeping() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let start = at("2026-01-01T00:00:00Z");
        let a = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        let b = create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        let _c = create_session(&conn, user, SessionKind::Bearer, None, None, start).unwrap();
        assert!(end_session(&conn, a.id).unwrap());
        assert!(!end_session(&conn, a.id).unwrap());
        assert_eq!(end_user_sessions(&conn, user, Some(b.id)).unwrap(), 1);
        assert_eq!(end_user_sessions(&conn, user, None).unwrap(), 1);

        create_session(&conn, user, SessionKind::Cookie, None, None, start).unwrap();
        assert_eq!(sweep_expired(&conn, at("2026-01-30T23:59:59Z")).unwrap(), 0);
        assert_eq!(sweep_expired(&conn, at("2026-01-31T00:00:00Z")).unwrap(), 1);
    }
}
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test -p nigel-core identity::sessions -- --test-threads=1`
Expected: compile errors — `cannot find function create_session`, `cannot find type SessionKind`.

- [ ] **Step 4: Implement**

Above the tests:

```rust
//! Sessions: a random 32-byte token handed to the client once, stored only as
//! its SHA-256. Idle for 30 days, or 90 days after creation, ends a session.

use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::{stamp, Role};
use crate::error::{NigelError, Result};

pub const IDLE_DAYS: i64 = 30;
pub const ABSOLUTE_DAYS: i64 = 90;
pub const RENEW_AFTER_SECONDS: i64 = 3600;
pub const USER_AGENT_MAX_CHARS: usize = 256;
const TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Cookie,
    Bearer,
}

impl SessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionKind::Cookie => "cookie",
            SessionKind::Bearer => "bearer",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "cookie" => Some(SessionKind::Cookie),
            "bearer" => Some(SessionKind::Bearer),
            _ => None,
        }
    }
}

/// A session just created. `token` is the only copy of the credential: it is
/// returned to the client once and never stored.
pub struct IssuedSession {
    pub id: i64,
    pub token: Zeroizing<String>,
    pub expires_at: String,
}

/// A live session joined to its enabled user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSession {
    pub id: i64,
    pub user_id: i64,
    pub uid: String,
    pub role: Role,
    pub must_change_password: bool,
    pub kind: SessionKind,
    pub created_at: String,
    pub last_seen_at: String,
    pub expires_at: String,
}

/// The stored hash of a presented token: hex-decoded to exactly 32 bytes, then
/// SHA-256. Anything that is not 64 hex characters is `None`.
pub fn token_hash(token_hex: &str) -> Option<[u8; 32]> {
    let raw = Zeroizing::new(hex::decode(token_hex).ok()?);
    if raw.len() != TOKEN_BYTES {
        return None;
    }
    Some(Sha256::digest(raw.as_slice()).into())
}

fn truncate_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn parse_stamp(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| NigelError::Other(format!("unreadable session timestamp: {value}")))
}

pub fn create_session(
    conn: &Connection,
    user_id: i64,
    kind: SessionKind,
    user_agent: Option<&str>,
    ip: Option<&str>,
    now: DateTime<Utc>,
) -> Result<IssuedSession> {
    let mut raw = Zeroizing::new([0u8; TOKEN_BYTES]);
    rand::rngs::OsRng.fill_bytes(raw.as_mut());
    let token = Zeroizing::new(hex::encode(raw.as_ref()));
    let hash: [u8; 32] = Sha256::digest(raw.as_ref()).into();
    let expires_at = stamp(now + Duration::days(IDLE_DAYS));
    conn.execute(
        "INSERT INTO sessions
             (token_hash, user_id, kind, created_at, last_seen_at, expires_at, user_agent, ip)
         VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7)",
        params![
            hash.as_slice(),
            user_id,
            kind.as_str(),
            stamp(now),
            expires_at,
            user_agent.map(|ua| truncate_chars(ua, USER_AGENT_MAX_CHARS)),
            ip,
        ],
    )?;
    Ok(IssuedSession {
        id: conn.last_insert_rowid(),
        token,
        expires_at,
    })
}

/// The live session a token names: unexpired at `now`, its user enabled.
pub fn find_session(
    conn: &Connection,
    token_hex: &str,
    now: DateTime<Utc>,
) -> Result<Option<ActiveSession>> {
    let Some(hash) = token_hash(token_hex) else {
        return Ok(None);
    };
    let row = conn
        .query_row(
            "SELECT s.id, s.user_id, u.uid, u.role, u.must_change_password, s.kind,
                    s.created_at, s.last_seen_at, s.expires_at
               FROM sessions s JOIN users u ON u.id = s.user_id
              WHERE s.token_hash = ?1 AND s.expires_at > ?2 AND u.disabled_at IS NULL",
            params![hash.as_slice(), stamp(now)],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, bool>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((id, user_id, uid, role, must_change, kind, created, seen, expires)) = row else {
        return Ok(None);
    };
    let role = Role::parse(&role)
        .ok_or_else(|| NigelError::Other(format!("unknown role in users table: {role}")))?;
    let kind = SessionKind::parse(&kind)
        .ok_or_else(|| NigelError::Other(format!("unknown session kind: {kind}")))?;
    Ok(Some(ActiveSession {
        id,
        user_id,
        uid,
        role,
        must_change_password: must_change,
        kind,
        created_at: created,
        last_seen_at: seen,
        expires_at: expires,
    }))
}

/// Slide a session's expiry forward once its last use is over an hour old:
/// `expires_at = min(now + 30 days, created_at + 90 days)`. Answers the new
/// expiry when it wrote one, `None` when the session was used within the hour.
pub fn renew_session(
    conn: &Connection,
    session: &ActiveSession,
    now: DateTime<Utc>,
) -> Result<Option<String>> {
    let last_seen = parse_stamp(&session.last_seen_at)?;
    if now - last_seen <= Duration::seconds(RENEW_AFTER_SECONDS) {
        return Ok(None);
    }
    let absolute = parse_stamp(&session.created_at)? + Duration::days(ABSOLUTE_DAYS);
    let expires_at = stamp((now + Duration::days(IDLE_DAYS)).min(absolute));
    conn.execute(
        "UPDATE sessions SET last_seen_at = ?1, expires_at = ?2 WHERE id = ?3",
        params![stamp(now), expires_at, session.id],
    )?;
    Ok(Some(expires_at))
}

pub fn end_session(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn.execute("DELETE FROM sessions WHERE id = ?1", [id])? > 0)
}

/// End every session of a user, or every one but `except`. Answers how many.
pub fn end_user_sessions(conn: &Connection, user_id: i64, except: Option<i64>) -> Result<usize> {
    Ok(conn.execute(
        "DELETE FROM sessions WHERE user_id = ?1 AND (?2 IS NULL OR id != ?2)",
        params![user_id, except],
    )?)
}

pub fn sweep_expired(conn: &Connection, now: DateTime<Utc>) -> Result<usize> {
    Ok(conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", [stamp(now)])?)
}
```

- [ ] **Step 5: Run them to see them pass**

Run: `cargo test -p nigel-core identity -- --test-threads=1 && cargo clippy --all-targets -- -D warnings`
Expected: the six session tests and `stamps_are_utc_seconds_with_a_z` pass; clippy clean (`stamp` and `testing::db` are now used).

- [ ] **Step 6: Commit**

```bash
git add crates/nigel-core/src/identity/
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "identity: sessions stored as SHA-256 hashes with idle and absolute expiry"
```

---

### Task 7: `identity::users` — users, the last-admin invariant, credentials, password change

**Files:**
- Create: `crates/nigel-core/src/identity/users.rs`
- Modify: `crates/nigel-core/src/identity/mod.rs` (`pub mod users;`)

**Interfaces:**
- Consumes: `Role`, `stamp` (mod.rs); `passwords::{check_new_password, hash_password, temporary_password, verify_dummy, verify_password, PasswordOwner, PasswordRule}` (Task 3); `sessions::{end_user_sessions, create_session, SessionKind}` (Task 6); `testing::{db, at}`.
- Produces: `User`, `UserListing`, `NewUser`, `CreatedUser`, `Credentials`, `PasswordChangeError` (`Display`, `From<NigelError>`, `From<rusqlite::Error>`, `Into<NigelError>`), `DISPLAY_NAME_MAX_CHARS = 100`, and `normalize_email`, `has_users`, `get_user`, `find_by_email`, `require_by_email`, `list_users`, `create_user`, `set_role`, `disable_user`, `enable_user`, `reset_password`, `verify_credentials`, `change_password` with the signatures in "Interfaces at a glance". Task 8 appends TOTP storage; Task 10 calls the CLI-facing functions; PR 2/3 call the rest.

- [ ] **Step 1: Write the failing tests**

Add `pub mod users;` to `identity/mod.rs`, and create `identity/users.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::sessions::{create_session, SessionKind};
    use crate::identity::testing::{at, db};

    const T0: &str = "2026-01-01T00:00:00Z";

    fn add(conn: &Connection, email: &str, role: Role) -> CreatedUser {
        create_user(
            conn,
            &NewUser {
                email,
                display_name: "Pat Example",
                role,
            },
            at(T0),
        )
        .unwrap()
    }

    fn conflict_code(err: NigelError) -> &'static str {
        match err {
            NigelError::Conflict { code, .. } => code,
            other => panic!("expected a conflict, got {other:?}"),
        }
    }

    #[test]
    fn emails_are_trimmed_lowercased_and_shape_checked() {
        assert_eq!(
            normalize_email("  Pat@Example.COM ").unwrap(),
            "pat@example.com"
        );
        for bad in [
            "",
            "pat",
            "@example.com",
            "pat@",
            "pat@@example.com",
            "pat x@example.com",
        ] {
            assert!(normalize_email(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_first_user_must_be_an_admin() {
        let (_dir, conn) = db();
        let err = create_user(
            &conn,
            &NewUser {
                email: "sam@example.com",
                display_name: "Sam Example",
                role: Role::Bookkeeper,
            },
            at(T0),
        )
        .err()
        .unwrap();
        assert_eq!(conflict_code(err), "last_admin");
        assert!(!has_users(&conn).unwrap());
    }

    #[test]
    fn a_created_user_has_a_temporary_password_and_must_change_it() {
        let (_dir, conn) = db();
        let created = add(&conn, " Pat@Example.com ", Role::Admin);
        assert!(created.first_user);
        let user = &created.user;
        assert_eq!(user.email, "pat@example.com");
        assert_eq!(user.uid.len(), 32);
        assert!(user.must_change_password);
        assert!(!user.totp_enabled);
        assert_eq!(user.created_at, T0);
        assert!(matches!(
            verify_credentials(&conn, "PAT@example.com", &created.temporary_password).unwrap(),
            Credentials::Valid(_)
        ));
        let stored: String = conn
            .query_row("SELECT password_hash FROM users", [], |r| r.get(0))
            .unwrap();
        assert!(stored.starts_with("$argon2id$"));
        assert!(!stored.contains(created.temporary_password.as_str()));
        assert!(!add(&conn, "sam@example.com", Role::Bookkeeper).first_user);
    }

    #[test]
    fn a_second_spelling_of_an_email_is_a_duplicate() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        let err = create_user(
            &conn,
            &NewUser {
                email: "PAT@example.com",
                display_name: "Pat Again",
                role: Role::Admin,
            },
            at(T0),
        )
        .err()
        .unwrap();
        assert_eq!(conflict_code(err), "duplicate_email");
    }

    #[test]
    fn a_blank_or_overlong_name_is_refused() {
        let (_dir, conn) = db();
        for name in ["   ".to_string(), "x".repeat(101), "Pat\u{7}".to_string()] {
            let result = create_user(
                &conn,
                &NewUser {
                    email: "pat@example.com",
                    display_name: &name,
                    role: Role::Admin,
                },
                at(T0),
            );
            assert!(matches!(result, Err(NigelError::Invalid(_))), "{name:?}");
        }
    }

    #[test]
    fn the_last_enabled_admin_cannot_be_demoted_or_disabled() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper).user.id;
        assert_eq!(
            conflict_code(set_role(&conn, pat, Role::Bookkeeper, at(T0)).unwrap_err()),
            "last_admin"
        );
        assert_eq!(
            conflict_code(disable_user(&conn, pat, at(T0)).unwrap_err()),
            "last_admin"
        );
        set_role(&conn, sam, Role::Admin, at(T0)).unwrap();
        disable_user(&conn, pat, at(T0)).unwrap();
        assert_eq!(
            conflict_code(set_role(&conn, sam, Role::ReadOnly, at(T0)).unwrap_err()),
            "last_admin",
            "a disabled admin does not count"
        );
        set_role(&conn, pat, Role::ReadOnly, at(T0)).unwrap();
    }

    #[test]
    fn disable_and_enable_are_idempotent() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper).user.id;
        let first = disable_user(&conn, sam, at(T0)).unwrap();
        let again = disable_user(&conn, sam, at("2026-02-01T00:00:00Z")).unwrap();
        assert_eq!(first.disabled_at.as_deref(), Some(T0));
        assert_eq!(again.disabled_at.as_deref(), Some(T0));
        assert!(enable_user(&conn, sam, at(T0)).unwrap().is_enabled());
        assert!(enable_user(&conn, sam, at(T0)).unwrap().is_enabled());
    }

    #[test]
    fn a_disabled_user_cannot_sign_in() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper);
        disable_user(&conn, sam.user.id, at(T0)).unwrap();
        assert!(matches!(
            verify_credentials(&conn, "sam@example.com", &sam.temporary_password).unwrap(),
            Credentials::Invalid
        ));
    }

    #[test]
    fn unknown_wrong_and_malformed_credentials_are_all_invalid() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        for (email, password) in [
            ("nobody@example.com", "whatever password"),
            ("pat@example.com", "wrong password here"),
            ("not an email", "whatever password"),
        ] {
            assert!(
                matches!(
                    verify_credentials(&conn, email, password).unwrap(),
                    Credentials::Invalid
                ),
                "{email}"
            );
        }
    }

    #[test]
    fn reset_issues_a_new_password_and_ends_every_session() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin);
        create_session(&conn, pat.user.id, SessionKind::Cookie, None, None, at(T0)).unwrap();
        let fresh = reset_password(&conn, pat.user.id, false, at("2026-02-01T00:00:00Z")).unwrap();
        assert_ne!(fresh.as_str(), pat.temporary_password.as_str());
        assert!(matches!(
            verify_credentials(&conn, "pat@example.com", &pat.temporary_password).unwrap(),
            Credentials::Invalid
        ));
        assert!(matches!(
            verify_credentials(&conn, "pat@example.com", &fresh).unwrap(),
            Credentials::Valid(u) if u.must_change_password
                && u.password_changed_at == "2026-02-01T00:00:00Z"
        ));
        let sessions: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sessions, 0);
    }

    #[test]
    fn reset_with_clear_totp_removes_the_second_factor() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        conn.execute(
            "UPDATE users SET totp_secret = x'0102', totp_last_step = 5 WHERE id = ?1",
            [pat],
        )
        .unwrap();
        reset_password(&conn, pat, false, at(T0)).unwrap();
        assert!(get_user(&conn, pat).unwrap().unwrap().totp_enabled);
        reset_password(&conn, pat, true, at(T0)).unwrap();
        let (secret, step): (Option<Vec<u8>>, Option<i64>) = conn
            .query_row(
                "SELECT totp_secret, totp_last_step FROM users WHERE id = ?1",
                [pat],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((secret, step), (None, None));
    }

    #[test]
    fn changing_a_password_checks_the_current_one_and_the_rules() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin);
        let id = pat.user.id;
        let temp = pat.temporary_password.as_str();
        assert!(matches!(
            change_password(
                &conn,
                id,
                "not it at all",
                "a new long password",
                None,
                at(T0)
            ),
            Err(PasswordChangeError::WrongCurrentPassword)
        ));
        assert!(matches!(
            change_password(&conn, id, temp, "short", None, at(T0)),
            Err(PasswordChangeError::Rule(PasswordRule::TooShort))
        ));
        assert!(matches!(
            change_password(&conn, id, temp, temp, None, at(T0)),
            Err(PasswordChangeError::Rule(PasswordRule::Unchanged))
        ));
        let keep = create_session(&conn, id, SessionKind::Cookie, None, None, at(T0)).unwrap();
        create_session(&conn, id, SessionKind::Bearer, None, None, at(T0)).unwrap();
        change_password(
            &conn,
            id,
            temp,
            "a new long password",
            Some(keep.id),
            at(T0),
        )
        .unwrap();
        let user = get_user(&conn, id).unwrap().unwrap();
        assert!(!user.must_change_password);
        let left: Vec<i64> = conn
            .prepare("SELECT id FROM sessions")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(left, [keep.id]);
        assert!(matches!(
            verify_credentials(&conn, "pat@example.com", "a new long password").unwrap(),
            Credentials::Valid(_)
        ));
    }

    #[test]
    fn the_listing_reports_last_seen_and_live_sessions_and_no_secrets() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        add(&conn, "sam@example.com", Role::Bookkeeper);
        create_session(&conn, pat, SessionKind::Cookie, None, None, at(T0)).unwrap();
        create_session(
            &conn,
            pat,
            SessionKind::Cookie,
            None,
            None,
            at("2026-01-10T00:00:00Z"),
        )
        .unwrap();
        let rows = list_users(&conn, at("2026-02-05T00:00:00Z")).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].user.email, "pat@example.com");
        assert_eq!(
            rows[0].last_seen_at.as_deref(),
            Some("2026-01-10T00:00:00Z")
        );
        assert_eq!(
            rows[0].active_sessions, 1,
            "the first expired on 2026-01-31"
        );
        assert_eq!(rows[1].last_seen_at, None);
        assert!(!format!("{rows:?}").contains("argon2"));
    }

    #[test]
    fn unknown_ids_and_emails_are_not_found() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        assert!(matches!(
            set_role(&conn, 999, Role::Admin, at(T0)),
            Err(NigelError::NotFound(_))
        ));
        assert!(matches!(
            disable_user(&conn, 999, at(T0)),
            Err(NigelError::NotFound(_))
        ));
        assert!(matches!(
            enable_user(&conn, 999, at(T0)),
            Err(NigelError::NotFound(_))
        ));
        assert!(matches!(
            reset_password(&conn, 999, false, at(T0)),
            Err(NigelError::NotFound(_))
        ));
        let err = require_by_email(&conn, "nobody@example.com").unwrap_err();
        assert!(err.to_string().contains("nobody@example.com"), "{err}");
    }

    #[test]
    fn disabling_a_user_ends_their_sessions_and_no_one_elses() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        let sam = add(&conn, "sam@example.com", Role::Admin).user.id;
        create_session(&conn, sam, SessionKind::Cookie, None, None, at(T0)).unwrap();
        create_session(&conn, pat, SessionKind::Cookie, None, None, at(T0)).unwrap();
        disable_user(&conn, sam, at(T0)).unwrap();
        let count = |id: i64| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE user_id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(count(sam), 0);
        assert_eq!(count(pat), 1);
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p nigel-core identity::users -- --test-threads=1`
Expected: compile errors — `cannot find function create_user`, `cannot find type CreatedUser`, …

- [ ] **Step 3: Implement**

Above the tests:

```rust
//! Users: created, listed, re-roled, disabled and enabled, never deleted.
//!
//! Invariant: books that have users always have at least one enabled admin.
//! The first user must be an admin, and the last enabled admin cannot be
//! disabled or demoted. Every write that could break it checks inside one
//! `BEGIN IMMEDIATE` transaction, and the CLI and the API both come here.

use std::fmt;

use chrono::{DateTime, Utc};
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use zeroize::Zeroizing;

use super::passwords::{
    check_new_password, hash_password, temporary_password, verify_dummy, verify_password,
    PasswordOwner, PasswordRule,
};
use super::{sessions, stamp, Role};
use crate::error::{NigelError, Result};

pub const DISPLAY_NAME_MAX_CHARS: usize = 100;

/// A user as every caller sees one. No hash and no secret: those never leave
/// this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub uid: String,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub totp_enabled: bool,
    pub must_change_password: bool,
    pub password_changed_at: String,
    pub disabled_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl User {
    pub fn is_enabled(&self) -> bool {
        self.disabled_at.is_none()
    }
}

/// A row of `nigel user list` and `GET /api/users`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserListing {
    pub user: User,
    pub last_seen_at: Option<String>,
    pub active_sessions: i64,
}

pub struct NewUser<'a> {
    pub email: &'a str,
    pub display_name: &'a str,
    pub role: Role,
}

/// A created user and the temporary password to hand them, once.
pub struct CreatedUser {
    pub user: User,
    pub temporary_password: Zeroizing<String>,
    /// Whether this was the books' first user.
    pub first_user: bool,
}

/// What a sign-in attempt proved. A wrong email, a wrong password and a
/// disabled user are all `Invalid`, and all cost one Argon2id verification.
#[derive(Debug)]
pub enum Credentials {
    Valid(User),
    Invalid,
}

/// Why a password change was refused.
#[derive(Debug)]
pub enum PasswordChangeError {
    WrongCurrentPassword,
    Rule(PasswordRule),
    Failed(NigelError),
}

impl fmt::Display for PasswordChangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PasswordChangeError::WrongCurrentPassword => {
                write!(f, "The current password is not right.")
            }
            PasswordChangeError::Rule(rule) => write!(f, "{rule}"),
            PasswordChangeError::Failed(e) => write!(f, "{e}"),
        }
    }
}

impl From<NigelError> for PasswordChangeError {
    fn from(e: NigelError) -> Self {
        PasswordChangeError::Failed(e)
    }
}

impl From<rusqlite::Error> for PasswordChangeError {
    fn from(e: rusqlite::Error) -> Self {
        PasswordChangeError::Failed(e.into())
    }
}

impl From<PasswordChangeError> for NigelError {
    fn from(e: PasswordChangeError) -> Self {
        match e {
            PasswordChangeError::Failed(inner) => inner,
            other => NigelError::Invalid(other.to_string()),
        }
    }
}

const USER_COLUMNS: &str = "id, uid, email, display_name, role, totp_secret IS NOT NULL, \
     must_change_password, password_changed_at, disabled_at, created_at, updated_at";

fn user_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    let role: String = r.get(4)?;
    let role = Role::parse(&role).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            format!("unknown role {role:?}").into(),
        )
    })?;
    Ok(User {
        id: r.get(0)?,
        uid: r.get(1)?,
        email: r.get(2)?,
        display_name: r.get(3)?,
        role,
        totp_enabled: r.get(5)?,
        must_change_password: r.get(6)?,
        password_changed_at: r.get(7)?,
        disabled_at: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
    })
}

/// Trimmed and lower-cased, with one `@`, a non-empty local part and domain,
/// and no whitespace.
pub fn normalize_email(raw: &str) -> Result<String> {
    let email = raw.trim().to_lowercase();
    let mut parts = email.split('@');
    let (local, domain) = (parts.next().unwrap_or(""), parts.next());
    let well_formed = matches!(domain, Some(d) if !d.is_empty())
        && parts.next().is_none()
        && !local.is_empty()
        && !email.chars().any(char::is_whitespace);
    if well_formed {
        Ok(email)
    } else {
        Err(NigelError::Invalid(format!(
            "{:?} is not an email address.",
            raw.trim()
        )))
    }
}

fn clean_display_name(raw: &str) -> Result<String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(NigelError::Invalid("A user needs a name.".into()));
    }
    if name.chars().count() > DISPLAY_NAME_MAX_CHARS {
        return Err(NigelError::Invalid(format!(
            "Use at most {DISPLAY_NAME_MAX_CHARS} characters for a name."
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(NigelError::Invalid(
            "A name cannot contain control characters.".into(),
        ));
    }
    Ok(name.to_string())
}

fn new_uid() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn immediate(conn: &Connection) -> Result<Transaction<'_>> {
    Ok(Transaction::new_unchecked(
        conn,
        TransactionBehavior::Immediate,
    )?)
}

fn last_admin(message: &str) -> NigelError {
    NigelError::Conflict {
        code: "last_admin",
        message: message.to_string(),
    }
}

fn not_found(id: i64) -> NigelError {
    NigelError::NotFound(format!("User not found: id {id}"))
}

fn enabled_admins_other_than(conn: &Connection, id: i64) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM users
          WHERE role = 'admin' AND disabled_at IS NULL AND id != ?1",
        [id],
        |r| r.get(0),
    )?)
}

/// Whether the books have any user at all — what decides local or shared mode.
pub fn has_users(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM users)", [], |r| r.get(0))?)
}

pub fn get_user(conn: &Connection, id: i64) -> Result<Option<User>> {
    Ok(conn
        .query_row(
            &format!("SELECT {USER_COLUMNS} FROM users WHERE id = ?1"),
            [id],
            user_from_row,
        )
        .optional()?)
}

pub fn find_by_email(conn: &Connection, email: &str) -> Result<Option<User>> {
    let email = normalize_email(email)?;
    Ok(conn
        .query_row(
            &format!("SELECT {USER_COLUMNS} FROM users WHERE email = ?1"),
            [email],
            user_from_row,
        )
        .optional()?)
}

/// [`find_by_email`], with an unknown address as an error naming it.
pub fn require_by_email(conn: &Connection, email: &str) -> Result<User> {
    find_by_email(conn, email)?.ok_or_else(|| {
        NigelError::NotFound(format!(
            "No user has the email {}. Run `nigel user list` to see who does.",
            email.trim()
        ))
    })
}

/// Every user, oldest first, with when each was last seen and how many
/// unexpired sessions each holds at `now`.
pub fn list_users(conn: &Connection, now: DateTime<Utc>) -> Result<Vec<UserListing>> {
    let sql = format!(
        "SELECT {USER_COLUMNS},
                (SELECT MAX(last_seen_at) FROM sessions s WHERE s.user_id = users.id),
                (SELECT COUNT(*) FROM sessions s
                  WHERE s.user_id = users.id AND s.expires_at > ?1)
           FROM users ORDER BY id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([stamp(now)], |r| {
        Ok(UserListing {
            user: user_from_row(r)?,
            last_seen_at: r.get(11)?,
            active_sessions: r.get(12)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Create a user with a temporary password they must change at first sign-in.
pub fn create_user(
    conn: &Connection,
    new: &NewUser<'_>,
    now: DateTime<Utc>,
) -> Result<CreatedUser> {
    let email = normalize_email(new.email)?;
    let display_name = clean_display_name(new.display_name)?;
    let temporary = temporary_password();
    let hash = hash_password(&temporary)?;

    let tx = immediate(conn)?;
    let first_user = !has_users(&tx)?;
    if first_user && new.role != Role::Admin {
        return Err(last_admin(
            "The first user must be an admin. Add them with --admin.",
        ));
    }
    let taken: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = ?1)",
        [&email],
        |r| r.get(0),
    )?;
    if taken {
        return Err(NigelError::Conflict {
            code: "duplicate_email",
            message: format!("A user with the email {email} already exists."),
        });
    }
    let ts = stamp(now);
    tx.execute(
        "INSERT INTO users (uid, email, display_name, password_hash, role,
                            must_change_password, password_changed_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6, ?6)",
        params![new_uid(), email, display_name, hash, new.role.as_str(), ts],
    )?;
    let id = tx.last_insert_rowid();
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    tx.commit()?;
    Ok(CreatedUser {
        user,
        temporary_password: temporary,
        first_user,
    })
}

pub fn set_role(conn: &Connection, id: i64, role: Role, now: DateTime<Utc>) -> Result<User> {
    let tx = immediate(conn)?;
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    if user.role == Role::Admin
        && role != Role::Admin
        && user.is_enabled()
        && enabled_admins_other_than(&tx, id)? == 0
    {
        return Err(last_admin(&format!(
            "{} is the only enabled admin. Make someone else an admin first.",
            user.email
        )));
    }
    tx.execute(
        "UPDATE users SET role = ?1, updated_at = ?2 WHERE id = ?3",
        params![role.as_str(), stamp(now), id],
    )?;
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    tx.commit()?;
    Ok(user)
}

/// Disable a user and end every session they hold. Disabling a disabled user
/// changes nothing and keeps the original `disabled_at`.
pub fn disable_user(conn: &Connection, id: i64, now: DateTime<Utc>) -> Result<User> {
    let tx = immediate(conn)?;
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    if !user.is_enabled() {
        return Ok(user);
    }
    if user.role == Role::Admin && enabled_admins_other_than(&tx, id)? == 0 {
        return Err(last_admin(&format!(
            "{} is the only enabled admin and cannot be disabled.",
            user.email
        )));
    }
    let ts = stamp(now);
    tx.execute(
        "UPDATE users SET disabled_at = ?1, updated_at = ?1 WHERE id = ?2",
        params![ts, id],
    )?;
    sessions::end_user_sessions(&tx, id, None)?;
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    tx.commit()?;
    Ok(user)
}

pub fn enable_user(conn: &Connection, id: i64, now: DateTime<Utc>) -> Result<User> {
    let tx = immediate(conn)?;
    get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    tx.execute(
        "UPDATE users SET disabled_at = NULL, updated_at = ?1
          WHERE id = ?2 AND disabled_at IS NOT NULL",
        params![stamp(now), id],
    )?;
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    tx.commit()?;
    Ok(user)
}

/// A new temporary password: `must_change_password` set, every session ended,
/// and with `clear_totp` the second factor removed too (a lost device).
pub fn reset_password(
    conn: &Connection,
    id: i64,
    clear_totp: bool,
    now: DateTime<Utc>,
) -> Result<Zeroizing<String>> {
    let temporary = temporary_password();
    let hash = hash_password(&temporary)?;
    let tx = immediate(conn)?;
    get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    let ts = stamp(now);
    tx.execute(
        "UPDATE users SET password_hash = ?1, must_change_password = 1,
                          password_changed_at = ?2, updated_at = ?2
          WHERE id = ?3",
        params![hash, ts, id],
    )?;
    if clear_totp {
        tx.execute(
            "UPDATE users SET totp_secret = NULL, totp_last_step = NULL WHERE id = ?1",
            [id],
        )?;
    }
    sessions::end_user_sessions(&tx, id, None)?;
    tx.commit()?;
    Ok(temporary)
}

/// Check an email and password. An address that matches no user, or is not an
/// address at all, still runs one Argon2id verification against a fixed hash.
pub fn verify_credentials(conn: &Connection, email: &str, password: &str) -> Result<Credentials> {
    let Ok(email) = normalize_email(email) else {
        verify_dummy(password);
        return Ok(Credentials::Invalid);
    };
    let row = conn
        .query_row(
            &format!("SELECT {USER_COLUMNS}, password_hash FROM users WHERE email = ?1"),
            [email],
            |r| Ok((user_from_row(r)?, r.get::<_, String>(11)?)),
        )
        .optional()?;
    let Some((user, hash)) = row else {
        verify_dummy(password);
        return Ok(Credentials::Invalid);
    };
    if verify_password(password, &hash) && user.is_enabled() {
        Ok(Credentials::Valid(user))
    } else {
        Ok(Credentials::Invalid)
    }
}

/// Change a user's own password. The current one must verify, the new one must
/// pass the rules; on success `must_change_password` is cleared and every
/// session but `keep_session` ends.
pub fn change_password(
    conn: &Connection,
    id: i64,
    current: &str,
    new: &str,
    keep_session: Option<i64>,
    now: DateTime<Utc>,
) -> std::result::Result<(), PasswordChangeError> {
    let (user, hash) = conn
        .query_row(
            &format!("SELECT {USER_COLUMNS}, password_hash FROM users WHERE id = ?1"),
            [id],
            |r| Ok((user_from_row(r)?, r.get::<_, String>(11)?)),
        )
        .optional()?
        .ok_or_else(|| not_found(id))?;
    if !verify_password(current, &hash) {
        return Err(PasswordChangeError::WrongCurrentPassword);
    }
    check_new_password(
        new,
        &PasswordOwner {
            email: &user.email,
            display_name: &user.display_name,
            current_hash: Some(&hash),
        },
    )
    .map_err(PasswordChangeError::Rule)?;
    let new_hash = hash_password(new)?;
    let tx = immediate(conn)?;
    let ts = stamp(now);
    tx.execute(
        "UPDATE users SET password_hash = ?1, must_change_password = 0,
                          password_changed_at = ?2, updated_at = ?2
          WHERE id = ?3",
        params![new_hash, ts, id],
    )?;
    sessions::end_user_sessions(&tx, id, keep_session)?;
    tx.commit()?;
    Ok(())
}
```

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p nigel-core identity -- --test-threads=1 && cargo clippy --all-targets -- -D warnings`
Expected: 15 user tests pass (all identity tests green); clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/identity/
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "identity: users with a last-admin invariant, credentials and password change"
```

---

### Task 8: TOTP storage on users — enroll, clear, verify with a replay guard

**Files:**
- Modify: `crates/nigel-core/src/identity/users.rs`

**Interfaces:**
- Consumes: `ServerKey::{seal, open}` (Task 5), `totp::{matching_step, step_at, code_for_step}` (Task 4).
- Produces: `TotpCheck { Accepted, Rejected, NotEnrolled }`, `set_totp_secret(conn, id, &ServerKey, secret: &[u8], confirmed_step: i64, now) -> Result<()>`, `clear_totp(conn, id, now) -> Result<()>`, `verify_totp(conn, id, &ServerKey, code: &str, now) -> Result<TotpCheck>`. PR 2's login and account routes call these.

- [ ] **Step 1: Write the failing tests**

Append inside `users.rs`'s `mod tests`, after the last test:

```rust
    #[test]
    fn a_totp_code_is_accepted_once_and_never_replayed() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        let key = ServerKey::from_bytes([1; 32]);
        let secret = [3u8; 20];
        let now = at("2026-03-01T12:00:00Z");
        let step = totp::step_at(now);
        assert_eq!(
            verify_totp(&conn, pat, &key, "123456", now).unwrap(),
            TotpCheck::NotEnrolled
        );
        set_totp_secret(&conn, pat, &key, &secret, step - 1, now).unwrap();
        let stored: Vec<u8> = conn
            .query_row("SELECT totp_secret FROM users WHERE id = ?1", [pat], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(!stored.windows(20).any(|w| w == secret), "stored sealed");

        let confirmed = totp::code_for_step(&secret, step - 1);
        assert_eq!(
            verify_totp(&conn, pat, &key, &confirmed, now).unwrap(),
            TotpCheck::Rejected,
            "the confirming code cannot sign in"
        );
        let code = totp::code_for_step(&secret, step);
        assert_eq!(
            verify_totp(&conn, pat, &key, &code, now).unwrap(),
            TotpCheck::Accepted
        );
        assert_eq!(
            verify_totp(&conn, pat, &key, &code, now).unwrap(),
            TotpCheck::Rejected,
            "replay"
        );
        let next = totp::code_for_step(&secret, step + 1);
        assert_eq!(
            verify_totp(&conn, pat, &key, &next, now).unwrap(),
            TotpCheck::Accepted
        );
    }

    #[test]
    fn a_secret_sealed_under_another_key_is_an_error_not_a_rejection() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        set_totp_secret(
            &conn,
            pat,
            &ServerKey::from_bytes([1; 32]),
            &[3; 20],
            0,
            at(T0),
        )
        .unwrap();
        let err = verify_totp(
            &conn,
            pat,
            &ServerKey::from_bytes([2; 32]),
            "123456",
            at(T0),
        );
        assert!(err.is_err());
    }

    #[test]
    fn clearing_totp_turns_it_off() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        set_totp_secret(
            &conn,
            pat,
            &ServerKey::from_bytes([1; 32]),
            &[3; 20],
            0,
            at(T0),
        )
        .unwrap();
        clear_totp(&conn, pat, at(T0)).unwrap();
        assert!(!get_user(&conn, pat).unwrap().unwrap().totp_enabled);
        assert!(clear_totp(&conn, 999, at(T0)).is_err());
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p nigel-core identity::users -- --test-threads=1`
Expected: compile errors — `cannot find type ServerKey`, `cannot find function set_totp_secret`, `cannot find type TotpCheck`.

- [ ] **Step 3: Implement**

Change the imports at the top of `users.rs` to:

```rust
use super::seal::ServerKey;
use super::{sessions, stamp, totp, Role};
```

(replacing `use super::{sessions, stamp, Role};`), and append before `#[cfg(test)] mod tests`:

```rust
/// What a code proved against a user's second factor.
#[derive(Debug, PartialEq, Eq)]
pub enum TotpCheck {
    Accepted,
    Rejected,
    NotEnrolled,
}

/// Store a confirmed second factor, sealed for this user. `confirmed_step` is
/// the step of the code that confirmed it, so that code cannot sign in again.
pub fn set_totp_secret(
    conn: &Connection,
    id: i64,
    key: &ServerKey,
    secret: &[u8],
    confirmed_step: i64,
    now: DateTime<Utc>,
) -> Result<()> {
    let user = get_user(conn, id)?.ok_or_else(|| not_found(id))?;
    let sealed = key.seal(&user.uid, secret)?;
    conn.execute(
        "UPDATE users SET totp_secret = ?1, totp_last_step = ?2, updated_at = ?3 WHERE id = ?4",
        params![sealed, confirmed_step, stamp(now), id],
    )?;
    Ok(())
}

pub fn clear_totp(conn: &Connection, id: i64, now: DateTime<Utc>) -> Result<()> {
    let changed = conn.execute(
        "UPDATE users SET totp_secret = NULL, totp_last_step = NULL, updated_at = ?1 WHERE id = ?2",
        params![stamp(now), id],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// Check a code against a user's sealed secret. A code is accepted only when
/// its step is later than the last accepted one, which is then recorded, so an
/// observed code cannot be replayed within its window. A secret the server key
/// cannot open is an error, not a rejection: the caller logs which user.
pub fn verify_totp(
    conn: &Connection,
    id: i64,
    key: &ServerKey,
    code: &str,
    now: DateTime<Utc>,
) -> Result<TotpCheck> {
    let tx = immediate(conn)?;
    let row: Option<(String, Option<Vec<u8>>, Option<i64>)> = tx
        .query_row(
            "SELECT uid, totp_secret, totp_last_step FROM users WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (uid, sealed, last_step) = row.ok_or_else(|| not_found(id))?;
    let Some(sealed) = sealed else {
        return Ok(TotpCheck::NotEnrolled);
    };
    let secret = key.open(&uid, &sealed)?;
    match totp::matching_step(&secret, code, now) {
        Some(step) if last_step.is_none_or(|last| step > last) => {
            tx.execute(
                "UPDATE users SET totp_last_step = ?1 WHERE id = ?2",
                params![step, id],
            )?;
            tx.commit()?;
            Ok(TotpCheck::Accepted)
        }
        _ => Ok(TotpCheck::Rejected),
    }
}
```

- [ ] **Step 4: Run them to see them pass**

Run: `cargo test -p nigel-core identity -- --test-threads=1 && cargo clippy --all-targets -- -D warnings`
Expected: the 54 tests under `identity::` pass; clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/identity/users.rs
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "identity: store a sealed TOTP secret and refuse a replayed code"
```

---

### Task 9: `NIGEL_DB_PASSWORD_FILE`

**Files:**
- Modify: `crates/nigel-core/src/db.rs` (resolver and tests)
- Modify: `crates/nigel/src/cli/password.rs` (the never-prompt message)
- Test: `crates/nigel/tests/cli_dispatch.rs`

**Interfaces:**
- Consumes: `db::validate_password`, `env_password`, `PASSWORD_ENV_VAR`, the `encrypted_db()` test fixture in `db.rs`'s tests; `common::{TestEnv, TEST_TIMEOUT}` and `TestEnv::encrypt` in the end-to-end tests.
- Produces: `env_password_if_set(db_path)` reading `NIGEL_DB_PASSWORD_FILE` or `NIGEL_DB_PASSWORD` (unchanged signature, so `prompt_password_if_needed` and `unlock_without_prompting` pick it up and every command inherits it); `init_empty(&TestEnv)` in `cli_dispatch.rs`, which Task 10 reuses. PR 2's boot unlock in `serve` calls `env_password_if_set` too.

- [ ] **Step 1: Write the failing unit tests**

In `db.rs`'s `mod tests`, before `fn test_init_db_sets_schema_version`:

```rust
    fn password_file(dir: &Path, contents: &[u8]) -> OsString {
        let path = dir.join(format!("password-{}", hex::encode(contents)));
        std::fs::write(&path, contents).unwrap();
        path.into_os_string()
    }

    #[test]
    fn a_password_file_unlocks_with_one_trailing_newline_stripped() {
        let (dir, db_path) = encrypted_db();
        for contents in [&b"secret"[..], b"secret\n", b"secret\r\n"] {
            let file = password_file(dir.path(), contents);
            let resolved = env_password_from(&db_path, None, Some(file)).unwrap();
            assert_eq!(resolved.as_deref(), Some("secret"), "{contents:?}");
        }
    }

    #[test]
    fn only_one_newline_is_stripped() {
        let (dir, db_path) = encrypted_db();
        let file = password_file(dir.path(), b"secret\n\n");
        let err = env_password_from(&db_path, None, Some(file))
            .unwrap_err()
            .to_string();
        assert!(err.contains(PASSWORD_FILE_ENV_VAR), "{err}");
    }

    #[test]
    fn both_variables_set_is_an_error_naming_both() {
        let (dir, db_path) = encrypted_db();
        let file = password_file(dir.path(), b"secret");
        let err = env_password_from(&db_path, Some("secret".into()), Some(file))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(PASSWORD_ENV_VAR) && err.contains(PASSWORD_FILE_ENV_VAR),
            "{err}"
        );
    }

    #[test]
    fn a_missing_empty_or_wrong_password_file_is_fatal_and_quotes_nothing() {
        let (dir, db_path) = encrypted_db();
        let missing = dir.path().join("absent").into_os_string();
        let cases = [
            (OsString::new(), "empty"),
            (missing, "could not be read"),
            (password_file(dir.path(), b"\n"), "is empty"),
            (password_file(dir.path(), b"hunter2\n"), "may be damaged"),
        ];
        for (file, expected) in cases {
            let err = env_password_from(&db_path, None, Some(file))
                .unwrap_err()
                .to_string();
            assert!(err.contains(PASSWORD_FILE_ENV_VAR), "{err}");
            assert!(err.contains(expected), "{expected}: {err}");
            assert!(!err.contains("hunter2"), "{err}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_password_file_that_is_not_utf8_is_fatal() {
        let (dir, db_path) = encrypted_db();
        let file = password_file(dir.path(), b"secret\xff");
        let err = env_password_from(&db_path, None, Some(file))
            .unwrap_err()
            .to_string();
        assert!(err.contains("UTF-8"), "{err}");
    }

    #[test]
    fn neither_variable_set_defers_to_the_prompt() {
        let (_dir, db_path) = encrypted_db();
        assert_eq!(env_password_from(&db_path, None, None).unwrap(), None);
    }
```

- [ ] **Step 2: Write the failing end-to-end tests**

Append to `crates/nigel/tests/cli_dispatch.rs`:

```rust
/// `nigel init` alone: an empty set of books with no users.
fn init_empty(env: &TestEnv) {
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
}

#[test]
fn every_command_accepts_the_password_file() {
    let env = TestEnv::new();
    init_empty(&env);
    env.encrypt("hunter2");
    let file = env.home.path().join("nigel-db-password");
    std::fs::write(&file, "hunter2").unwrap();
    env.cmd()
        .arg("status")
        .env_remove("NIGEL_DB_PASSWORD")
        .env("NIGEL_DB_PASSWORD_FILE", &file)
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();
}

#[test]
fn a_wrong_or_doubled_password_source_fails_fast_naming_the_variable() {
    let env = TestEnv::new();
    init_empty(&env);
    env.encrypt("hunter2");
    let file = env.home.path().join("nigel-db-password");
    std::fs::write(&file, "notit\n").unwrap();

    env.cmd()
        .arg("status")
        .env_remove("NIGEL_DB_PASSWORD")
        .env("NIGEL_DB_PASSWORD_FILE", &file)
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("NIGEL_DB_PASSWORD_FILE did not unlock")
                .and(predicate::str::contains("notit").not()),
        );
    std::fs::write(&file, "hunter2\n").unwrap();
    env.cmd()
        .arg("status")
        .env("NIGEL_DB_PASSWORD_FILE", &file)
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("are both set"));
}
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test -p nigel-core db::tests -- --test-threads=1`
Expected: compile error — `cannot find function env_password_from`, `cannot find value PASSWORD_FILE_ENV_VAR`.

Run: `cargo test -p nigel --test cli_dispatch password -- --test-threads=1`
Expected: `every_command_accepts_the_password_file` and `a_wrong_or_doubled_password_source_fails_fast_naming_the_variable` FAIL (the variable is ignored, so `status` prompts on an empty stdin and errors without naming it).

- [ ] **Step 4: Implement**

In `db.rs`, replace this tail of `env_password` and the current `env_password_if_set`:

```rust
    // A wrong password and a damaged file are indistinguishable here: SQLCipher
    // reports both as "not a database", and `is_encrypted` reads anything without
    // the plaintext header as encrypted. Naming one cause would send an operator
    // rotating a password when the ledger is actually corrupt.
    Err(crate::error::NigelError::Other(format!(
        "{PASSWORD_ENV_VAR} did not unlock {}. The password may be wrong, or the database file may be damaged.",
        db_path.display()
    )))
}

/// Resolve a password for `db_path` from `NIGEL_DB_PASSWORD`, if set.
/// `Ok(None)` means the variable is unset and the caller should ask the user
/// some other way — this function never touches stdin.
pub fn env_password_if_set(db_path: &Path) -> Result<Option<String>> {
    env_password(db_path, std::env::var_os(PASSWORD_ENV_VAR))
}
```

with:

```rust
    Err(did_not_unlock(PASSWORD_ENV_VAR, db_path))
}

/// A wrong password and a damaged file are indistinguishable here: SQLCipher
/// reports both as "not a database", and `is_encrypted` reads anything without
/// the plaintext header as encrypted. Naming one cause would send an operator
/// rotating a password when the ledger is actually corrupt.
fn did_not_unlock(var: &str, db_path: &Path) -> crate::error::NigelError {
    crate::error::NigelError::Other(format!(
        "{var} did not unlock {}. The password may be wrong, or the database file may be damaged.",
        db_path.display()
    ))
}

/// Environment variable naming a file that holds the database password — a
/// systemd credential, say — so the password itself never sits in the
/// environment. The file's content is the password, one trailing newline
/// stripped.
const PASSWORD_FILE_ENV_VAR: &str = "NIGEL_DB_PASSWORD_FILE";

/// Validate a password read from the file `path` names. Every failure is fatal,
/// for the reason [`env_password`]'s are, and none of them quotes the file.
fn file_password(db_path: &Path, path: &std::ffi::OsStr) -> Result<Option<String>> {
    use crate::error::NigelError;
    if path.is_empty() {
        return Err(NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} is set but empty — it should name the file holding the password."
        )));
    }
    let path = Path::new(path);
    let bytes = Zeroizing::new(std::fs::read(path).map_err(|e| {
        NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} names {}, which could not be read: {e}.",
            path.display()
        ))
    })?);
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Err(NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} names {}, which is not valid UTF-8.",
            path.display()
        )));
    };
    let text = match text.strip_suffix('\n') {
        Some(rest) => rest.strip_suffix('\r').unwrap_or(rest),
        None => text,
    };
    if text.is_empty() {
        return Err(NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} names {}, which is empty.",
            path.display()
        )));
    }
    let mut pw = text.to_string();
    if validate_password(db_path, &pw)? {
        return Ok(Some(pw));
    }
    pw.zeroize();
    Err(did_not_unlock(PASSWORD_FILE_ENV_VAR, db_path))
}

/// The password from whichever of the two variables is set. Both set is an
/// error rather than a precedence rule: two sources for one secret is a
/// misconfiguration, and silently preferring one hides which is stale.
fn env_password_from(
    db_path: &Path,
    raw: Option<OsString>,
    file: Option<OsString>,
) -> Result<Option<String>> {
    match (raw, file) {
        (Some(_), Some(_)) => Err(crate::error::NigelError::Other(format!(
            "{PASSWORD_ENV_VAR} and {PASSWORD_FILE_ENV_VAR} are both set. Set one of them."
        ))),
        (None, Some(path)) => file_password(db_path, &path),
        (raw, None) => env_password(db_path, raw),
    }
}

/// Resolve a password for `db_path` from `NIGEL_DB_PASSWORD_FILE` or
/// `NIGEL_DB_PASSWORD`, if either is set. `Ok(None)` means neither is and the
/// caller should ask the user some other way — this function never touches
/// stdin.
pub fn env_password_if_set(db_path: &Path) -> Result<Option<String>> {
    env_password_from(
        db_path,
        std::env::var_os(PASSWORD_ENV_VAR),
        std::env::var_os(PASSWORD_FILE_ENV_VAR),
    )
}
```

In `crates/nigel/src/cli/password.rs`, `unlock_without_prompting`'s refusal names both variables:

```rust
             Set NIGEL_DB_PASSWORD or NIGEL_DB_PASSWORD_FILE — see \"Automated backups\" in the README.",
```

(replacing `Set NIGEL_DB_PASSWORD — see …`). Check nothing asserts the old sentence: `grep -rn "Set NIGEL_DB_PASSWORD —" crates/` must print nothing.

- [ ] **Step 5: Run them to see them pass**

Run: `cargo test -p nigel-core db::tests -- --test-threads=1 && cargo test -p nigel --test cli_dispatch password -- --test-threads=1`
Expected: all pass, including the existing `test_env_password_*`, `backup_fails_fast_on_wrong_env_password`, `env_password_is_not_echoed_on_failure` and `invoice_and_client_commands_work_on_encrypted_db_via_env_password`.

- [ ] **Step 6: Commit**

```bash
git add crates/nigel-core/src/db.rs crates/nigel/src/cli/password.rs crates/nigel/tests/cli_dispatch.rs
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "Read the books password from NIGEL_DB_PASSWORD_FILE

Setting both variables is an error, never a precedence rule."
```

---

### Task 10: `nigel user`

**Files:**
- Create: `crates/nigel/src/cli/user.rs`
- Modify: `crates/nigel/src/cli/mod.rs` (module, `Commands::User`, `UserCommands`, launch-sync skip, tests)
- Modify: `crates/nigel/src/main.rs` (import and dispatch)
- Test: `crates/nigel/tests/cli_dispatch.rs`

**Interfaces:**
- Consumes: `identity::users::{create_user, list_users, require_by_email, set_role, disable_user, enable_user, reset_password, NewUser, User, UserListing}`, `identity::Role`, `clock::now_utc`, `db::{get_connection, is_encrypted}`, `settings::get_data_dir`; `init_empty` from Task 9's end-to-end tests.
- Produces: `cli::user::{add, list, set_role, disable, enable, reset_password, RoleArg, NOT_ENCRYPTED_WARNING}`; `cli::UserCommands`. `nigel user` runs through `dispatch()`'s existing pre-flight — initialized-books check, password (prompt, `NIGEL_DB_PASSWORD`, `NIGEL_DB_PASSWORD_FILE`), migrations — and skips the launch syncs.

- [ ] **Step 1: Write the failing parse tests**

In `crates/nigel/src/cli/mod.rs`'s `mod tests`, after `previews_never_reach_the_network`:

```rust
    #[test]
    fn user_commands_never_reach_the_network() {
        let cli = Cli::try_parse_from(["nigel", "user", "list"]).unwrap();
        assert!(!launch_sync_allowed(&cli.command.unwrap()));
    }

    #[test]
    fn user_add_takes_exactly_one_of_admin_or_role() {
        let base = [
            "nigel",
            "user",
            "add",
            "pat@example.com",
            "--name",
            "Pat Example",
        ];
        assert!(Cli::try_parse_from(base).is_err(), "neither");
        let both = [&base[..], &["--admin", "--role", "bookkeeper"]].concat();
        assert!(Cli::try_parse_from(both).is_err(), "both");
        let admin = [&base[..], &["--admin"]].concat();
        assert!(Cli::try_parse_from(admin).is_ok());
        let role = [&base[..], &["--role", "read-only"]].concat();
        assert!(Cli::try_parse_from(role).is_ok());
        let bogus = [&base[..], &["--role", "owner"]].concat();
        assert!(Cli::try_parse_from(bogus).is_err());
    }
```

- [ ] **Step 2: Write the failing formatter tests and end-to-end tests**

Create `crates/nigel/src/cli/user.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: Role, must_change: bool, disabled: bool, totp: bool) -> User {
        User {
            id: 1,
            uid: "0".repeat(32),
            email: "pat@example.com".into(),
            display_name: "Pat Example".into(),
            role,
            totp_enabled: totp,
            must_change_password: must_change,
            password_changed_at: "2026-01-01T00:00:00Z".into(),
            disabled_at: disabled.then(|| "2026-01-02T00:00:00Z".into()),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn the_added_message_names_the_user_and_says_the_password_once() {
        let message = added_message(
            &user(Role::Admin, true, false, false),
            "K7QXM-2RTPA-W9HCN-4FJDE",
        );
        assert_eq!(
            message,
            "Added Pat Example <pat@example.com> (Admin).\n\
             Temporary password: K7QXM-2RTPA-W9HCN-4FJDE\n\
             Give this to Pat Example. They will choose their own password when they first sign in.\n"
        );
    }

    #[test]
    fn the_list_shows_status_two_step_and_last_seen() {
        let rows = vec![
            UserListing {
                user: user(Role::Admin, false, false, true),
                last_seen_at: Some("2026-01-05T10:00:00Z".into()),
                active_sessions: 1,
            },
            UserListing {
                user: User {
                    email: "sam@example.com".into(),
                    ..user(Role::ReadOnly, true, false, false)
                },
                last_seen_at: None,
                active_sessions: 0,
            },
            UserListing {
                user: User {
                    email: "lee@example.com".into(),
                    ..user(Role::Bookkeeper, false, true, false)
                },
                last_seen_at: None,
                active_sessions: 0,
            },
        ];
        let table = list_table(&rows);
        for expected in [
            "pat@example.com",
            "Admin",
            "active",
            "on",
            "2026-01-05T10:00:00Z",
            "Read-only",
            "must change password",
            "never",
            "disabled",
        ] {
            assert!(table.contains(expected), "{expected}\n{table}");
        }
    }

    #[test]
    fn an_empty_list_says_how_to_add_the_first_admin() {
        assert!(list_table(&[]).contains("--admin"));
    }

    #[test]
    fn both_spellings_of_read_only_parse() {
        use clap::ValueEnum;
        assert_eq!(RoleArg::from_str("read_only", false), Ok(RoleArg::ReadOnly));
        assert_eq!(RoleArg::from_str("read-only", false), Ok(RoleArg::ReadOnly));
        assert_eq!(Role::from(RoleArg::ReadOnly), Role::ReadOnly);
    }
}
```

Add `pub mod user;` after `pub mod update;` in `cli/mod.rs`, and append to `crates/nigel/tests/cli_dispatch.rs`:

```rust
/// The temporary password printed on the `Temporary password:` line.
fn temporary_password_from(stdout: &[u8]) -> String {
    let text = String::from_utf8_lossy(stdout);
    let line = text
        .lines()
        .find_map(|l| l.strip_prefix("Temporary password: "))
        .unwrap_or_else(|| panic!("no temporary password in:\n{text}"));
    line.trim().to_string()
}

/// `nigel user <args>` with neither password variable inherited and no stdin.
fn user_cmd(env: &TestEnv, args: &[&str]) -> assert_cmd::Command {
    let mut cmd = env.cmd();
    cmd.arg("user")
        .args(args)
        .env_remove("NIGEL_DB_PASSWORD")
        .env_remove("NIGEL_DB_PASSWORD_FILE")
        .write_stdin("")
        .timeout(TEST_TIMEOUT);
    cmd
}

#[test]
fn user_add_prints_a_temporary_password_once_and_warns_on_unencrypted_books() {
    let env = TestEnv::new();
    init_empty(&env);
    let out = user_cmd(&env, &["add", "Pat@Example.com", "--name", "Pat Example", "--admin"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Added Pat Example <pat@example.com> (Admin).").and(
                predicate::str::contains(
                    "Give this to Pat Example. They will choose their own password when they first sign in.",
                ),
            ),
        )
        .stderr(predicate::str::contains("These books are not encrypted."))
        .get_output()
        .stdout
        .clone();
    let temporary = temporary_password_from(&out);
    let shape = regex::Regex::new(r"^[0-9A-HJKMNP-TV-Z]{5}(-[0-9A-HJKMNP-TV-Z]{5}){3}$").unwrap();
    assert!(shape.is_match(&temporary), "{temporary}");

    let hash: String = env
        .db()
        .query_row("SELECT password_hash FROM users", [], |r| r.get(0))
        .unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(!hash.contains(&temporary));

    user_cmd(
        &env,
        &[
            "add",
            "sam@example.com",
            "--name",
            "Sam Example",
            "--role",
            "bookkeeper",
        ],
    )
    .assert()
    .success()
    .stderr(predicate::str::contains("not encrypted").not());
}

#[test]
fn user_add_refuses_a_first_user_who_is_not_an_admin() {
    let env = TestEnv::new();
    init_empty(&env);
    user_cmd(
        &env,
        &[
            "add",
            "sam@example.com",
            "--name",
            "Sam Example",
            "--role",
            "bookkeeper",
        ],
    )
    .assert()
    .failure()
    .stderr(predicate::str::contains("The first user must be an admin"));
    let users: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
        .unwrap();
    assert_eq!(users, 0);
}

#[test]
fn user_add_needs_exactly_one_of_admin_or_role() {
    let env = TestEnv::new();
    init_empty(&env);
    user_cmd(&env, &["add", "pat@example.com", "--name", "Pat Example"])
        .assert()
        .failure();
    user_cmd(
        &env,
        &[
            "add",
            "pat@example.com",
            "--name",
            "Pat Example",
            "--admin",
            "--role",
            "admin",
        ],
    )
    .assert()
    .failure();
}

#[test]
fn user_list_role_disable_enable_and_the_last_admin() {
    let env = TestEnv::new();
    init_empty(&env);
    user_cmd(
        &env,
        &["add", "pat@example.com", "--name", "Pat Example", "--admin"],
    )
    .assert()
    .success();
    user_cmd(
        &env,
        &[
            "add",
            "sam@example.com",
            "--name",
            "Sam Example",
            "--role",
            "bookkeeper",
        ],
    )
    .assert()
    .success();

    user_cmd(&env, &["list"]).assert().success().stdout(
        predicate::str::contains("pat@example.com")
            .and(predicate::str::contains("Bookkeeper"))
            .and(predicate::str::contains("must change password"))
            .and(predicate::str::contains("never"))
            .and(predicate::str::contains("argon2").not()),
    );

    user_cmd(&env, &["role", "pat@example.com", "read_only"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("only enabled admin"));
    user_cmd(&env, &["disable", "pat@example.com"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("only enabled admin"));

    user_cmd(&env, &["role", "sam@example.com", "read-only"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "sam@example.com is now Read-only.",
        ));
    user_cmd(&env, &["disable", "sam@example.com"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Disabled sam@example.com"));
    user_cmd(&env, &["disable", "sam@example.com"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already disabled"));
    user_cmd(&env, &["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("disabled"));
    user_cmd(&env, &["enable", "SAM@example.com"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Enabled sam@example.com."));
}

#[test]
fn user_commands_refuse_an_unknown_email_by_name() {
    let env = TestEnv::new();
    init_empty(&env);
    user_cmd(
        &env,
        &["add", "pat@example.com", "--name", "Pat Example", "--admin"],
    )
    .assert()
    .success();
    for args in [
        vec!["role", "nobody@example.com", "admin"],
        vec!["disable", "nobody@example.com"],
        vec!["enable", "nobody@example.com"],
        vec!["reset-password", "nobody@example.com"],
    ] {
        user_cmd(&env, &args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("nobody@example.com"));
    }
}

#[test]
fn user_reset_password_replaces_the_password_and_can_clear_two_step() {
    let env = TestEnv::new();
    init_empty(&env);
    let first = user_cmd(
        &env,
        &["add", "pat@example.com", "--name", "Pat Example", "--admin"],
    )
    .assert()
    .success()
    .get_output()
    .stdout
    .clone();
    let first = temporary_password_from(&first);
    env.db()
        .execute_batch(
            "UPDATE users SET totp_secret = x'0102', totp_last_step = 7, must_change_password = 0;",
        )
        .unwrap();

    let second = user_cmd(&env, &["reset-password", "pat@example.com"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Give this to Pat Example."))
        .get_output()
        .stdout
        .clone();
    assert_ne!(temporary_password_from(&second), first);
    let (must_change, secret): (bool, Option<Vec<u8>>) = env
        .db()
        .query_row(
            "SELECT must_change_password, totp_secret FROM users",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(must_change);
    assert!(secret.is_some(), "without --clear-totp two-step stays on");

    user_cmd(&env, &["reset-password", "pat@example.com", "--clear-totp"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Two-step verification is off"));
    let secret: Option<Vec<u8>> = env
        .db()
        .query_row("SELECT totp_secret FROM users", [], |r| r.get(0))
        .unwrap();
    assert!(secret.is_none());
}

#[test]
fn user_commands_unlock_encrypted_books_from_a_password_file() {
    let env = TestEnv::new();
    init_empty(&env);
    env.encrypt("hunter2");
    let file = env.home.path().join("nigel-db-password");
    std::fs::write(&file, "hunter2\n").unwrap();

    user_cmd(
        &env,
        &["add", "pat@example.com", "--name", "Pat Example", "--admin"],
    )
    .env("NIGEL_DB_PASSWORD_FILE", &file)
    .assert()
    .success()
    .stderr(predicate::str::contains("not encrypted").not());
    user_cmd(&env, &["list"])
        .env("NIGEL_DB_PASSWORD_FILE", &file)
        .assert()
        .success()
        .stdout(predicate::str::contains("pat@example.com"));
}
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test -p nigel --lib -- --test-threads=1 user`
Expected: compile errors — `no variant User in Commands`, `cannot find function added_message`, `cannot find type RoleArg`.

- [ ] **Step 4: Implement the subcommands**

In `cli/mod.rs`, add the variant to `Commands` before `/// Undo the last import`:

```rust
    /// Manage the people who sign in to a shared `nigel serve`.
    User {
        #[command(subcommand)]
        command: UserCommands,
    },
```

add the enum before `pub enum PasswordCommand`:

```rust
#[derive(Subcommand)]
pub enum UserCommands {
    /// Add a user with a temporary password. The first user must be --admin.
    #[command(group(clap::ArgGroup::new("grant").required(true).args(["admin", "role"])))]
    Add {
        /// Their email address, which they sign in with
        email: String,
        /// Their name, e.g. "Pat Example"
        #[arg(long)]
        name: String,
        /// Make them an admin (same as --role admin)
        #[arg(long)]
        admin: bool,
        /// Their role
        #[arg(long, value_enum)]
        role: Option<user::RoleArg>,
    },
    /// List users: email, name, role, status, two-step and last seen.
    List,
    /// Change a user's role. The last enabled admin cannot be demoted.
    Role {
        email: String,
        #[arg(value_enum)]
        role: user::RoleArg,
    },
    /// Disable a user and end their sessions. Users are never deleted.
    Disable { email: String },
    /// Enable a disabled user.
    Enable { email: String },
    /// Issue a new temporary password and end the user's sessions.
    ResetPassword {
        email: String,
        /// Also turn off their two-step verification (a lost device)
        #[arg(long)]
        clear_totp: bool,
    },
}
```

and add `| Commands::User { .. }` to `launch_sync_allowed`'s list, after `| Commands::Serve { .. }`.

Above the tests in `cli/user.rs`:

```rust
//! `nigel user` — a printing wrapper over `nigel_core::identity::users`.
//!
//! The CLI has full authority over the books it opens: it is how the first
//! admin is created and how a locked-out admin recovers.

use std::path::PathBuf;

use comfy_table::{Cell, Table};

use nigel_core::clock::now_utc;
use nigel_core::db::{get_connection, is_encrypted};
use nigel_core::error::Result;
use nigel_core::identity::users::{self, NewUser, User, UserListing};
use nigel_core::identity::Role;
use nigel_core::settings::get_data_dir;

pub const NOT_ENCRYPTED_WARNING: &str = "These books are not encrypted. Anyone who can read the \
     file can read the books. Encrypt them with `nigel password set` before you give anyone \
     else access.";

/// The role as typed on the command line. `read-only` is accepted beside the
/// stored spelling `read_only`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum RoleArg {
    Admin,
    Bookkeeper,
    #[value(name = "read_only", alias = "read-only")]
    ReadOnly,
}

impl From<RoleArg> for Role {
    fn from(arg: RoleArg) -> Self {
        match arg {
            RoleArg::Admin => Role::Admin,
            RoleArg::Bookkeeper => Role::Bookkeeper,
            RoleArg::ReadOnly => Role::ReadOnly,
        }
    }
}

fn db_path() -> PathBuf {
    get_data_dir().join("nigel.db")
}

pub(crate) fn temporary_password_lines(user: &User, temporary: &str) -> String {
    format!(
        "Temporary password: {temporary}\n\
         Give this to {}. They will choose their own password when they first sign in.\n",
        user.display_name
    )
}

pub(crate) fn added_message(user: &User, temporary: &str) -> String {
    format!(
        "Added {} <{}> ({}).\n{}",
        user.display_name,
        user.email,
        user.role.label(),
        temporary_password_lines(user, temporary)
    )
}

fn status(user: &User) -> &'static str {
    if !user.is_enabled() {
        "disabled"
    } else if user.must_change_password {
        "must change password"
    } else {
        "active"
    }
}

pub(crate) fn list_table(rows: &[UserListing]) -> String {
    if rows.is_empty() {
        return "No users. Add the first with `nigel user add <email> --name <name> --admin`."
            .to_string();
    }
    let mut table = Table::new();
    table.set_header(vec![
        "Email",
        "Name",
        "Role",
        "Status",
        "Two-step",
        "Last seen",
    ]);
    for row in rows {
        let user = &row.user;
        table.add_row(vec![
            Cell::new(&user.email),
            Cell::new(&user.display_name),
            Cell::new(user.role.label()),
            Cell::new(status(user)),
            Cell::new(if user.totp_enabled { "on" } else { "off" }),
            Cell::new(row.last_seen_at.as_deref().unwrap_or("never")),
        ]);
    }
    table.to_string()
}

pub fn add(email: &str, name: &str, role: Role) -> Result<()> {
    let path = db_path();
    let conn = get_connection(&path)?;
    let created = users::create_user(
        &conn,
        &NewUser {
            email,
            display_name: name,
            role,
        },
        now_utc(),
    )?;
    print!(
        "{}",
        added_message(&created.user, &created.temporary_password)
    );
    if created.first_user && !is_encrypted(&path)? {
        eprintln!("{NOT_ENCRYPTED_WARNING}");
    }
    Ok(())
}

pub fn list() -> Result<()> {
    let conn = get_connection(&db_path())?;
    println!("{}", list_table(&users::list_users(&conn, now_utc())?));
    Ok(())
}

pub fn set_role(email: &str, role: Role) -> Result<()> {
    let conn = get_connection(&db_path())?;
    let user = users::require_by_email(&conn, email)?;
    let user = users::set_role(&conn, user.id, role, now_utc())?;
    println!("{} is now {}.", user.email, user.role.label());
    Ok(())
}

pub fn disable(email: &str) -> Result<()> {
    let conn = get_connection(&db_path())?;
    let user = users::require_by_email(&conn, email)?;
    if !user.is_enabled() {
        println!("{} was already disabled.", user.email);
        return Ok(());
    }
    let user = users::disable_user(&conn, user.id, now_utc())?;
    println!("Disabled {} and ended their sessions.", user.email);
    Ok(())
}

pub fn enable(email: &str) -> Result<()> {
    let conn = get_connection(&db_path())?;
    let user = users::require_by_email(&conn, email)?;
    if user.is_enabled() {
        println!("{} is already enabled.", user.email);
        return Ok(());
    }
    let user = users::enable_user(&conn, user.id, now_utc())?;
    println!("Enabled {}.", user.email);
    Ok(())
}

pub fn reset_password(email: &str, clear_totp: bool) -> Result<()> {
    let conn = get_connection(&db_path())?;
    let user = users::require_by_email(&conn, email)?;
    let temporary = users::reset_password(&conn, user.id, clear_totp, now_utc())?;
    print!("{}", temporary_password_lines(&user, &temporary));
    if clear_totp {
        println!("Two-step verification is off for {}.", user.email);
    }
    println!("Every session {} had has ended.", user.email);
    Ok(())
}
```

In `crates/nigel/src/main.rs`, add `UserCommands` to the `use nigel::cli::{…}` list, and in `dispatch()`'s `match`, before `Commands::Undo`:

```rust
        Commands::User { command } => match command {
            UserCommands::Add {
                email,
                name,
                admin: _,
                role,
            } => {
                let role = role.map_or(nigel_core::identity::Role::Admin, Into::into);
                cli::user::add(&email, &name, role)
            }
            UserCommands::List => cli::user::list(),
            UserCommands::Role { email, role } => cli::user::set_role(&email, role.into()),
            UserCommands::Disable { email } => cli::user::disable(&email),
            UserCommands::Enable { email } => cli::user::enable(&email),
            UserCommands::ResetPassword { email, clear_totp } => {
                cli::user::reset_password(&email, clear_totp)
            }
        },
```

(`role` is `None` only when `--admin` was given: the `grant` group requires exactly one of the two.)

- [ ] **Step 5: Run them to see them pass**

Run:
```bash
cargo test -p nigel --lib -- --test-threads=1 user
cargo test -p nigel --test cli_dispatch user_ -- --test-threads=1
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```
Expected: 6 unit tests and 7 end-to-end tests pass; clippy and fmt clean.

- [ ] **Step 6: Try it by hand**

```bash
export HOME="$(mktemp -d)"
cargo run -q -- init --data-dir "$HOME/books"
cargo run -q -- user add pat@example.com --name "Pat Example" --admin
cargo run -q -- user add sam@example.com --name "Sam Example" --role read-only
cargo run -q -- user list
cargo run -q -- user disable pat@example.com      # refused: only enabled admin
cargo run -q -- user reset-password sam@example.com
cargo run -q -- serve --no-open --port 0          # still prints the /auth?token= link: local mode is unchanged; Ctrl-C
```
Expected: a temporary password in `XXXXX-XXXXX-XXXXX-XXXXX` form after each `add`/`reset-password`, the not-encrypted warning on stderr after the first `add` only, the table with `must change password`, the refusal naming `pat@example.com`, and `serve` behaving exactly as before.

- [ ] **Step 7: Commit**

```bash
git add crates/nigel/src/cli/user.rs crates/nigel/src/cli/mod.rs crates/nigel/src/main.rs crates/nigel/tests/cli_dispatch.rs
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "nigel user: add, list, role, disable, enable, reset-password"
```

---

### Task 11: Documentation and the whole-branch check

**Files:**
- Modify: `docs/commands.md`, `docs/architecture.md`, `docs/design-constraints.md`, `README.md`, `CHANGELOG.md`

**Interfaces:**
- Consumes: everything above.
- Produces: docs describing the current state; no code.

- [ ] **Step 1: `docs/commands.md`**

In the CLI block, after the `nigel password remove` line, add:

```bash
nigel user add pat@example.com --name "Pat Example" --admin  # Add a user with a temporary password; the first must be --admin
nigel user list                                   # Email, name, role, status, two-step, last seen
nigel user role sam@example.com read_only         # Change a role: admin, bookkeeper, read_only
nigel user disable sam@example.com                # Disable a user and end their sessions (users are never deleted)
nigel user enable sam@example.com                 # Enable a disabled user
nigel user reset-password sam@example.com         # New temporary password; ends their sessions (--clear-totp also turns off two-step)
```

Then, immediately before `### Web UI`, add these two sections:

````markdown
### Users

Users are the people who sign in to a shared `nigel serve`, identified by email. These
commands open the books in the configured data directory like every other command, so an
encrypted database is unlocked by the prompt, `NIGEL_DB_PASSWORD`, or
`NIGEL_DB_PASSWORD_FILE`. Whoever can open the file has full authority over its users;
this is also how a locked-out admin recovers. `nigel serve` does not read users yet: it
stays in local mode, with its token link, whatever this table holds.

```bash
nigel user add pat@example.com --name "Pat Example" --admin            # The first user must be --admin
nigel user add sam@example.com --name "Sam Example" --role bookkeeper  # --role admin|bookkeeper|read_only
nigel user list                                                        # Email, name, role, status, two-step, last seen
nigel user role sam@example.com read_only                              # Change a role
nigel user disable sam@example.com                                     # Disable and end their sessions
nigel user enable sam@example.com
nigel user reset-password sam@example.com                              # New temporary password; ends their sessions
nigel user reset-password pat@example.com --clear-totp                 # Also turn off two-step (lost device)
```

- Exactly one of `--admin` or `--role` is required; no role is granted by default.
- `add` and `reset-password` print a temporary password once, in the form
  `K7QXM-2RTPA-W9HCN-4FJDE`. It is stored only as an Argon2id hash and cannot be shown
  again. The user must choose their own password when they first sign in.
- `add` on unencrypted books warns that anyone who can read the file can read the books,
  and recommends `nigel password set` first.
- Emails are matched without regard to case or surrounding spaces. An unknown email is an
  error naming it.
- Users are never deleted. The last enabled admin cannot be disabled or demoted.
- `list` never prints a password hash or a two-step secret.
- Two-step secrets are sealed with `server.key`, which sits beside `settings.json` in
  `~/.config/nigel/`, not in the data directory, so it is not in any backup. If it is lost,
  each user with two-step on needs `nigel user reset-password <email> --clear-totp`.

### Unattended password

Any command that opens encrypted books reads the password from `NIGEL_DB_PASSWORD_FILE` (a
path to a file holding it, one trailing newline stripped) or from `NIGEL_DB_PASSWORD`
before it would prompt. Setting both is an error. A value that is empty, unreadable or
wrong stops the command rather than falling back to a prompt.
````

- [ ] **Step 2: `docs/architecture.md`**

Add this bullet to the `## Architecture` list, after the `**Documents:**` bullet:

````markdown
- **Identity:** `crates/nigel-core/src/identity/` — the people who sign in to a shared `nigel serve`, kept apart from `password.rs`, which is the books' encryption key. Tables: `users` (email unique without regard to case, a random 32-hex `uid`, an Argon2id hash, a role of `admin`/`bookkeeper`/`read_only`, an optional sealed TOTP secret with the last accepted step, `must_change_password`, `disabled_at`) and `sessions` (the SHA-256 of a 32-byte token, `cookie` or `bearer`, 30 days idle and 90 days absolute). `passwords.rs` holds the one rule check and the hashing, `totp.rs` RFC 6238, `seal.rs` the per-machine `server.key` in the config directory and XChaCha20-Poly1305 sealing keyed to each user's `uid`, `sessions.rs` the session rows, and `users.rs` every user write, each of which keeps at least one enabled admin. Every function that stamps or judges a time takes `now` from the caller (`clock::now_utc()`). `cli/user.rs` is the `nigel user` printing wrapper; `nigel serve` does not read these tables yet
````

In the Project Structure tree, add before `categorizer.rs`:

```
      identity/             # Who signs in to a shared server: users, sessions, passwords, second factor
        mod.rs                 # Role (read_only < bookkeeper < admin), the timestamp form, test helpers
        passwords.rs           # The password rules, Argon2id hash/verify, the dummy verify, temporary passwords
        common-passwords.txt   # The 10,000 most common passwords (SecLists, MIT), compiled in
        totp.rs                # RFC 6238 codes, the ±1 step window, base32 and the otpauth URI
        seal.rs                # ServerKey (<config_dir>/server.key) and sealing a secret to a user's uid
        sessions.rs            # Session rows: create, find, renew, end, sweep
        users.rs               # Users: create, list, role, disable, enable, reset, credentials, password change, TOTP storage
```

and after the `cli/` tree's `password.rs` line:

```
        user.rs                # nigel user add/list/role/disable/enable/reset-password — a printing wrapper over nigel_core::identity::users
```

- [ ] **Step 3: `docs/design-constraints.md`**

Add before the bullet that begins "The session middleware wraps the `/api` router with `.layer`":

````markdown
- **Users are sign-in identity, never the books' key, and their invariant lives in the data layer.** `identity::users` is the one writer of `users`, and the CLI and the API both call it: users are disabled, never deleted (`disabled_at`), and books that have users always keep one enabled admin — the first user must be an admin and the last enabled admin cannot be disabled or demoted, checked inside a `BEGIN IMMEDIATE` transaction and refused as `Conflict { code: "last_admin" }`. Email is trimmed and lower-cased before every write and lookup, and the column is `COLLATE NOCASE` besides. Nothing stored is a working credential: passwords are Argon2id (m = 19 MiB, t = 2, p = 1, pinned in `passwords.rs`, not taken from the crate's defaults), session tokens are stored as their SHA-256, temporary passwords exist in plaintext only in the one output that shows them, and TOTP secrets are sealed under `server.key`, which lives in the config directory so no backup or copied database carries it. A wrong email, a wrong password and a disabled user all cost one Argon2id verification (`verify_dummy` for an address with no user). No identity error message names a secret
- `NIGEL_DB_PASSWORD_FILE` names a file holding the books password, one trailing newline stripped, and is read wherever `NIGEL_DB_PASSWORD` is (`db::env_password_if_set`), so every command accepts it. Setting both is an error rather than a precedence rule: two sources for one secret is a misconfiguration, and silently preferring one would hide which is stale. An empty, unreadable or wrong file fails the command naming the variable and the path, never quoting the contents
````

- [ ] **Step 4: `README.md` and `CHANGELOG.md`**

In the README's "Automated backups", after "Read the password from a secret store, as above, rather than writing it into a script or a plist.", add a paragraph:

````markdown
Where the secret store hands over a file instead — a systemd credential, say — set `NIGEL_DB_PASSWORD_FILE` to its path. The file holds the password, and one trailing newline is ignored. Set one of the two variables, not both.
````

In `CHANGELOG.md`, at the top of `## [Unreleased]` → `### Added`:

````markdown
- **Users for a shared server** — `nigel user add/list/role/disable/enable/reset-password` keeps the people who will sign in to a shared `nigel serve`, by email, each with a role of admin, bookkeeper or read-only. `add` and `reset-password` print a temporary password once; the user chooses their own when they first sign in. Users are disabled, never deleted, and the last enabled admin cannot be disabled or demoted. Passwords are stored as Argon2id hashes, and two-step secrets are sealed with a per-machine `server.key` kept outside the data directory. `nigel serve` does not read users yet. See "Users" in `docs/commands.md`
- **`NIGEL_DB_PASSWORD_FILE`** — every command that opens encrypted books can read the password from a file, such as a systemd credential, instead of from `NIGEL_DB_PASSWORD`. Setting both is an error
````

- [ ] **Step 5: The whole-branch check**

Run each and read the result:
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test --no-default-features --features serve -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
(cd crates/nigel-ffi && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked -- --test-threads=1)
# the desktop shell needs WebKitGTK on Linux; run where CI's system libraries are installed:
(cd crates/nigel-desktop && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked -- --test-threads=1)
git diff --stat origin/main -- crates/nigel-core/src/server/   # must print nothing: PR 1 changes no HTTP behaviour
git diff --name-only origin/main -- backlog/                    # must print nothing
./scripts/check-no-real-data.sh; echo "exit=$?"                # must be exit=0
```
Expected: every suite green; both `git diff` commands print nothing; the check exits 0 (its pre-existing WARN lines in old specs are unrelated).

- [ ] **Step 6: Commit**

```bash
git add docs/commands.md docs/architecture.md docs/design-constraints.md README.md CHANGELOG.md
./scripts/check-no-real-data.sh --staged; echo "exit=$?"   # must be exit=0
git commit -m "Document users, the identity module and NIGEL_DB_PASSWORD_FILE"
```

