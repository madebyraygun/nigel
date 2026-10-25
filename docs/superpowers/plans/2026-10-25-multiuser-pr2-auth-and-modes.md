# Multiuser PR 2 — Auth and Modes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `nigel serve` derives local or shared mode from the books, and in shared mode admits only signed-in users (cookie or bearer sessions with rolling expiry, optional TOTP, rate-limited login) over loopback or the one `server_public_url` host, while local mode and the desktop shells behave exactly as they do today.

**Architecture:** A mode cell in `AppState` (local with a token, or shared) is derived at startup from `identity::users::has_users` and re-derived after an unlock, a data-directory switch, and on a public-host request while local; it only ever moves local → shared within a run. The served router gains a mode-aware host guard, a session layer that inserts a `Principal` on every `/api` request, an interim admin-only guard (replaced by PR 3's role table), and auth/account routes built on PR 1's `identity` data layer. The desktop router stays pinned local and gets `Principal::LocalOwner` from a one-line layer.

**Tech Stack:** Rust 2021, axum 0.8.9, tokio, rusqlite/SQLCipher, argon2, chacha20poly1305, `qrcode` 0.14 (new, `svg` feature only), assert_cmd for the end-to-end test.

**Spec:** `docs/superpowers/specs/2026-10-23-multiuser-phase-1-design.md` (approved). This plan is PR 2 of its §17 sequence. Read §1, §3, §4, §5, §6, §8.5, §11.1–11.4, §14, §15 and §16 before starting any task. Backlog: TASK-32.2 (`backlog task 32.2 --plain`), whose notes carry PR 1's deferred review items; each is a step below, marked **(PR 1 review item N)**.

## Global Constraints

Copied from the spec. Every task's requirements include these.

- The mode is **derived, never stored or configured**: shared when the books are open and have at least one user; local when they have no users **or** are encrypted and still locked (§1).
- Local mode "behaves exactly as `nigel serve` does now: loopback bind, Host/Origin check against the three loopback names, per-run token, `nigel_session` cookie, no accounts, no login screen" (§1).
- "Non-loopback hostnames are accepted only in shared mode." In local mode the host guard uses `TrustedOrigins::loopback()` and nothing else, whatever `server_public_url` says (§1).
- "A stale reading can only be 'local', which accepts less." There is no way back to local mode in phase 1 (§1).
- Moving from local to shared ends the token session: the token is dropped from memory; the next `/api` request with the old cookie gets `401` with `details.mode = "shared"` (§1).
- `build_desktop_router` "is always local mode. It has no session layer and still has none. Its requests run as the local owner (admin), even when the books it opened have users" (§1.1).
- `server_public_url` / `NIGEL_SERVER_PUBLIC_URL`: "One `https://` origin: scheme and host, an optional port, no path." A value that is not `https`, has a path, or does not parse is refused at startup with an error naming the value (§11.1).
- `client_ip_header` / `NIGEL_CLIENT_IP_HEADER`: `"CF-Connecting-IP"` or `"X-Forwarded-For"`, or unset; trusted only from a loopback peer (§4.4, §11.1).
- The server keeps binding `127.0.0.1` only. There is no `--bind` flag (§11.2).
- Session cookie: `nigel_session=<token>; HttpOnly; SameSite=Strict; Path=/; Max-Age=2592000`, plus `; Secure` when the request's Host is the `server_public_url` host (§4.1).
- Sessions end 30 days after last use and in any case 90 days after creation; renewal at most once an hour; renewal never moves `expires_at` past `created_at + 90 days` (§4.2).
- Must-change-password allowlist: `GET /api/auth/me`, `GET /api/status`, `GET /api/ping`, `POST /api/account/password`, `POST /api/auth/logout`; anything else `403 password_change_required` (§4.2).
- Login: a wrong email, password, disabled user or code all get `401 invalid_credentials`, message "That email and password don't match."; a wrong code after the password was proven gets "That code didn't match. Check the time on your device."; unknown emails still cost one Argon2id verification (§4.1).
- Rate limits: per account (normalized email, including emails that match no user) 5 consecutive failures → 15-minute lockout, any success resets; per client IP 20 failures in a sliding 15 minutes → 15 minutes. A locked bucket gets `429 too_many_attempts` with `retryAfterMs` before any password is checked. `totp_required` is not a failure (§4.4).
- CSRF: in shared mode a cookie-authenticated request whose method is not GET/HEAD/OPTIONS must carry a trusted `Origin`, else `403 forbidden`; a `client: "browser"` login must carry a trusted `Origin`; bearer requests and `native` logins are exempt. "`GET` never writes" (§4.6).
- Security headers as defaults (`or_insert`): `Referrer-Policy: same-origin` everywhere; `Cache-Control: no-store` on every `/api` response; `Strict-Transport-Security: max-age=31536000` on responses to the `server_public_url` host only, never on loopback. **No Content-Security-Policy in this PR** — that is TASK-32.10 (§4.7).
- Never logged: passwords, temporary passwords, TOTP secrets and codes, session tokens. Bodies that carry them hold them in `server::secret::Secret` (§3).
- One stderr line per login success, login failure (with the reason class), lockout, logout, password change and TOTP change, each with email, user id, client IP and session id, never a secret (§5).
- `nigel user add` on books with no users prints: "Nigel will now require sign-in. Run `nigel serve` and sign in as pat@example.com." On unencrypted books it also prints the encryption warning (§6). `nigel serve` prints the same encryption warning at startup in shared mode (§3).
- `nigel serve` resolves the books password at startup from `NIGEL_DB_PASSWORD_FILE` or `NIGEL_DB_PASSWORD`; setting both is an error (PR 1's `db::env_password_if_set`); the file's content is the password with one trailing newline stripped; a wrong or unreadable password is a fatal startup error; with neither set, `nigel serve` starts locked, in local mode (§11.4).
- **Until PR 3 (roles), every non-auth route needs admin** — a temporary fail-closed table (§17).
- All Rust tests run serially: `cargo test -- --test-threads=1` (§16; CLAUDE.md).
- Public repository: no real names, hostnames, amounts or emails anywhere. Use `example.com`, `books.example.com`, `Pat Example`, `Sam Example`, documentation IPs (`203.0.113.0/24`, `198.51.100.0/24`). Judge `./scripts/check-no-real-data.sh --staged` by its **exit status** (CLAUDE.md).
- Every visual change ships through `@nigel/ui` — **this PR makes none**. The SPA gets no new screens (PR 4) and must keep working unchanged in local mode (spec §9.1, CLAUDE.md).
- Docs describe the current state; no "added in PR 2" prose. Comments obey the user's comment rules: nothing that justifies an edit, explains a config entry's history, or warns against regressions.

## Review Focus

The five inputs most likely to bite a person that the spec implies but does not spell out. Each has its test in the owning task.

1. **`server_public_url` naming loopback or written loosely** — `https://localhost`, `https://BOOKS.example.com/`, `https://books.example.com:443`. A loopback value would put `Strict-Transport-Security` on `localhost` and pin the operator's own browser to https for a year; a trailing slash or default port is how people paste a URL. Expected: loopback names are refused at startup by name; a trailing `/`, upper case and `:443` are normalized and match the browser's `Host`/`Origin`. *(Task 3, `loopback_and_loose_spellings`.)*
2. **A forged or odd client-IP header** — a remote peer sending `CF-Connecting-IP`, a proxy sending `X-Forwarded-For: 203.0.113.9, 198.51.100.7`, a header holding `not-an-ip`. Expected: the header is read only from a loopback peer, the rightmost `X-Forwarded-For` entry wins, anything that is not an IP falls back to the peer address. *(Task 8, `client_ip_trusts_the_header_only_from_loopback`.)*
3. **Concurrent guesses racing the limiter** — twenty parallel logins against one account, all admitted before any failure is recorded. Expected: each attempt is charged when admitted, so the sixth concurrent attempt on an account is refused before Argon2 runs. *(Task 8, `concurrent_attempts_are_charged_before_they_are_checked`.)*
4. **A log line built from what an attacker typed** — an "email" carrying `\n`, `"` or 10 KB of text in a failed login. Expected: one line per event, the email escaped and truncated, nothing that looks like a second event. *(Task 3, `an_email_cannot_forge_a_second_line`.)*
5. **A cookie session's token replayed as a bearer token** — the CSRF rule exempts bearer requests, so a cookie token in `Authorization: Bearer` would skip the Origin check. Expected: a session answers only on the transport it was issued for. *(Task 6, `a_session_answers_only_on_its_own_transport`.)*

## Rulings on what the spec leaves open

Each is decided here so the implementer does not have to. Several are worth the operator's eye; those are repeated in the PR description.

1. **The mode is monotonic within a run.** Re-derivation only ever moves local → shared. A data-directory switch or decrypt in shared mode does not return to local (there would be no token to print). The §8.3 shared-mode refusals stay in PR 3 as the spec sequences them; until then an admin who switches folders in shared mode lands on books served in shared mode and must restart the server if those books have no users. Fail-closed.
2. **Mode policy is chosen by constructor.** `AppState::new(db_path, token)` (the desktop shells, nigel-ffi, every existing test) is *pinned local* and never re-derives. `AppState::for_serve(db_path, mode, config)` is the only *derived* state. So `/api/auth/me` on the desktop answers `local` whatever the books hold (spec §10), and an unlock on the desktop cannot flip it to shared.
3. **Any error while deriving the mode reads as local.** `nigel serve` prints a notice at startup when that happens; runtime re-derivation is silent.
4. **Public-host re-derivation is throttled to once every 2 seconds** (`mode::REFRESH_INTERVAL`), shared across concurrent requests, so tunnel traffic against a local-mode server costs at most one users query per 2 s.
5. **`server_public_url` normalization.** Scheme must be `https` (any case). A single trailing `/` is accepted and dropped. Host is lower-cased; hostnames are `[a-z0-9.-]` labels, IPv6 literals are bracketed. `:443` is dropped as the default port. Paths, queries, fragments, userinfo, whitespace and control characters are refused. Loopback hosts (`localhost`, any `127.0.0.0/8` or `::1` literal) are refused — loopback is always accepted already, and `Secure`/HSTS on loopback would pin `localhost` to https. The `Host` header matches when it equals `host[:port]` (or `host:443` when no port was given); `Origin` matches when it equals `https://host[:port]` (or with `:443`), case-insensitively.
6. **`client_ip_header` values are case-insensitive**; any other value is a startup error naming it. A header value that does not parse as an IP address falls back to the TCP peer. A request with no peer (in-process tests) is `"unknown"`.
7. **`Secure` and HSTS key off the request's `Host` matching `server_public_url`, in either mode.** In local mode such a request is refused with `403`, and that refusal carries HSTS; harmless, and it keeps one rule.
8. **`Cache-Control: no-store` is added to paths under `/api` only**, with `or_insert`, so the static assets keep their own caching.
9. **`/api/auth/login`, `/api/auth/logout` and `/api/auth/me` are added to the locked guard's ungated list.** Account routes stay gated (a shared server is never locked).
10. **The interim admin table (PR 2 only)** keys on the nested path and method. Any signed-in role may call `GET /api/ping`, `GET /api/auth/me`, `POST /api/auth/logout` and the four `POST /api/account/*` routes. `POST /api/auth/login` is pre-auth. **Everything else, `GET /api/status` included, needs admin** and answers `403 forbidden` with `details: { reason: "role", required: "admin" }`. Unknown `/api` paths need admin too. It is layered on both routers; the local owner passes. PR 3 deletes `authz::interim_admin_guard` and replaces it with `role_guard`. (`dataDir` nulling for non-admins in status is therefore PR 3's, since no non-admin can reach status in PR 2.)
11. **The rate limiter charges an attempt when it is admitted** (`LoginLimiter::begin` returns an `Attempt`), and the caller settles it: `succeeded` resets the account and refunds the IP stamp; `not_a_failure` (`totp_required`, server-side errors, `totp_unavailable`) refunds both; `failed` (or dropping the attempt) keeps the charge. This closes the race where N parallel guesses all pass the check before any is counted.
12. **An account's failure count also resets after 15 minutes without a failure**, and a lockout that has expired starts the count again. Otherwise four typos spread over months would leave an account one guess from lockout.
13. **Limiter memory is capped at 10,000 keys per bucket.** Past the cap, idle entries (no failure in the window and not locked) are dropped first, then the entry whose last failure is oldest.
14. **Account routes that check the current password go through the same limiter**, keyed by the signed-in user's email and the client IP, so a stolen session cannot become a password oracle.
15. **Account-route answers:** wrong current password → `400 bad_request`, `details.reason = "wrong_password"`; wrong code at TOTP confirm/remove → `400`, `reason = "wrong_code"`; expired or unknown `setupId` → `409`, `reason = "setup_expired"`; setup while enrolled → `409`, `reason = "totp_enabled"`; remove while not enrolled → `409`, `reason = "totp_not_enabled"`; any account route on a local-owner principal → `409`, `reason = "local_mode"`. These are 400/409 rather than 401 so the SPA never mistakes them for "signed out".
16. **A user whose second factor this machine cannot check** (no `server.key`, or a key that does not open their secret) gets `500 internal` with `details.reason = "totp_unavailable"` at the code step, not counted as a failure, and a `totp_unavailable` event line. Login loads the key with `ServerKey::load` and never creates one; TOTP confirm uses `load_or_create`, so `server.key` appears on first enrollment.
17. **Event lines** are `auth event=<kind> email="<escaped>" user=<id|-> ip=<ip|-> session=<id|-> reason=<word|->` on stderr. The email is `{:?}`-escaped and cut to 254 characters; `reason` is a `&'static str`, so no variant can carry a secret.
18. **Body caps:** `POST /api/auth/login` and the four account routes accept at most 8 KiB (`413 payload_too_large`) **(PR 1 review item 2)**. Their bodies are parsed by `SecretJson`, whose rejections never quote the body.
19. **A malformed `Authorization` header is `401`, with no fallback to the cookie.** A session answers only on the transport it was issued for: a cookie session via the cookie, a bearer session via `Authorization: Bearer` (Review Focus 5).
20. **Renewal skips the `Set-Cookie` when the handler already set one** (logout's expiring cookie wins).
21. **The SPA in shared mode before PR 4:** no SPA source changes beyond the TypeScript mirror (`StatusResponse.mode?` and four new error codes in `API_ERROR_CODES`). Every request a browser makes without a session gets `401` with `details.mode = "shared"`, and the SPA shows its existing "Session expired. Reopen the link `nigel serve` printed" banner. That banner is wrong in shared mode, but shared mode exists only after someone deliberately runs `nigel user add`, and PR 4 replaces it with the login gate. In local mode nothing changes. See "Operator decisions" in the PR body.
22. **Shared-mode startup opens the browser at the loopback root** (no token) unless `--no-open`.
23. **The first-user wording lives in the CLI.** The data layer's refusal reads "The first user must be an admin."; `nigel user add` checks first and adds "Add them with --admin." **(PR 1 review item 8)**. `identity::users::require_users` (a `409 local_mode` refusal) is what PR 3's `POST /api/users` calls before `create_user`.
24. **Session issuance requires a proof type.** `sessions::create_session` takes a `SessionGrant`, which only `users::complete_sign_in` constructs (plus a `testutil`-gated `assume_verified` for tests) **(PR 1 review item 7)**.

---

## File Structure

New files:

| File | Responsibility |
|---|---|
| `crates/nigel-core/src/server/config.rs` | `PublicUrl`, `ClientIpHeader`, `ServerConfig::from_settings[_with]` |
| `crates/nigel-core/src/server/events.rs` | `AuthEvent`, `EventKind`, `emit` — the stderr event lines |
| `crates/nigel-core/src/server/mode.rs` | `Mode`, `ModeCell` (local+token or shared, monotonic), `derive` |
| `crates/nigel-core/src/server/principal.rs` | `Principal`, `SessionUser`, the extractor |
| `crates/nigel-core/src/server/authz.rs` | `interim_admin_guard`, `as_local_owner` (PR 3 replaces the first) |
| `crates/nigel-core/src/server/ratelimit.rs` | `LoginLimiter`, `Attempt`, `Bucket`, `client_ip`, `account_key` |
| `crates/nigel-core/src/server/pending_totp.rs` | `PendingTotpSetups` — in-memory TOTP enrollments, 10 minutes, keyed by session |
| `crates/nigel-core/src/server/boot.rs` | `unlock_with`/`unlock_from_env`, `BootFacts`, `startup_notices`, `serving_banner` |
| `crates/nigel-core/src/server/routes/sessions.rs` | `POST /api/auth/login`, `POST /api/auth/logout`, `GET /api/auth/me` |
| `crates/nigel-core/src/server/routes/account.rs` | `POST /api/account/password`, `/account/totp/{setup,confirm,remove}` |
| `crates/nigel-core/src/server/shared_tests.rs` | Shared-mode integration tests (tower `oneshot`) for Tasks 5–10 |
| `crates/nigel/tests/serve_exposure.rs` | End-to-end: the real binary, raw HTTP |

Modified files:

| File | Change |
|---|---|
| `crates/nigel-core/src/identity/{mod,users,sessions,passwords,seal}.rs` | PR 1 review items; `SessionGrant`; `complete_sign_in`; `Role` serde/Display |
| `crates/nigel-core/src/db.rs` | Cap the `NIGEL_DB_PASSWORD_FILE` read; `env_password_from` → `pub(crate)` |
| `crates/nigel-core/src/settings.rs` | `server_public_url`, `client_ip_header` fields |
| `crates/nigel-core/src/server/{mod,state,auth,error,extract,testutil}.rs` | Mode-aware guards, session layer, routers, headers, error codes, extractors, test helpers |
| `crates/nigel-core/src/server/routes/{mod,status,settings}.rs` | Mount new routes; ungated list; `mode` in status; refresh after unlock/switch |
| `crates/nigel-core/Cargo.toml` | `qrcode` |
| `crates/nigel-desktop/Cargo.lock`, `crates/nigel-ffi/Cargo.lock` | Lockfile refresh for `qrcode` (CI builds them `--locked`) |
| `crates/nigel/src/cli/{user,serve}.rs` | First-user message and wording; boot unlock in `preflight` |
| `crates/nigel/tests/common/mod.rs` | `binary_path()` for a long-lived child process |
| `web/apps/app/src/api/types.ts` | `StatusResponse.mode?`, four error codes |
| `docs/api.md`, `docs/design-constraints.md`, `docs/architecture.md`, `CHANGELOG.md` | Current-state docs |

## Shared interfaces (names every task relies on)

```rust
// identity (Tasks 1–2)
pub enum Role { ReadOnly, Bookkeeper, Admin }               // + Serialize/Deserialize snake_case, Display
pub const NOT_ENCRYPTED_WARNING: &str;                       // identity::NOT_ENCRYPTED_WARNING
pub fn passwords::warm_dummy_hash();
pub struct users::PasswordVerified { /* private */ }         // fn user(&self) -> &User
pub enum users::Credentials { Valid(PasswordVerified), Invalid }
pub enum users::SignIn { Granted(User, SessionGrant), CodeRequired(User), CodeRejected(User), TotpUnavailable(User) }
pub fn users::complete_sign_in(conn, proof: PasswordVerified, key: Option<&ServerKey>, code: Option<&str>, now) -> Result<SignIn>;
pub enum users::TotpCheck { Accepted, Rejected, NotEnrolled, Unopenable }
pub fn users::verify_user_password(conn, id: i64, password: &str) -> Result<bool>;
pub fn users::require_users(conn) -> Result<()>;             // Conflict { code: "local_mode" }
pub fn users::totp_unopenable(conn, key: Option<&ServerKey>) -> Result<Vec<String>>;
pub struct sessions::SessionGrant { /* private user_id */ }  // fn user_id(&self) -> i64; testutil: assume_verified(i64)
pub fn sessions::create_session(conn, grant: SessionGrant, kind, user_agent: Option<&str>, ip: Option<&str>, now) -> Result<IssuedSession>;
pub struct sessions::ActiveSession { /* … */ pub email: String /* new */ }
pub fn seal::permission_warning(path: &Path) -> Option<String>;

// server (Tasks 3–11)
pub struct config::PublicUrl;            // parse(&str) -> Result<Self, String>; matches_host(&str); matches_origin(&str); origin(); authority()
pub enum config::ClientIpHeader { CfConnectingIp, XForwardedFor }
pub struct config::ServerConfig { pub public_url: Option<PublicUrl>, pub client_ip_header: Option<ClientIpHeader> }
pub enum mode::Mode { Local, Shared }
pub fn mode::derive(db_path: &Path) -> crate::error::Result<Mode>;
impl AppState {
    pub fn new(db_path: PathBuf, session_token: String) -> Self;            // pinned local (unchanged signature)
    pub fn for_serve(db_path: PathBuf, mode: Mode, config: ServerConfig) -> Self;
    pub fn mode(&self) -> Mode;
    pub fn local_token(&self) -> Option<SessionToken>;
    pub async fn refresh_mode(&self) -> Mode;
    pub async fn refresh_mode_throttled(&self) -> Mode;
    pub fn now(&self) -> chrono::DateTime<chrono::Utc>;
    pub config: Arc<ServerConfig>, pub limiter: Arc<LoginLimiter>, pub totp_setups: Arc<PendingTotpSetups>,
}
pub enum principal::Principal { LocalOwner, User(SessionUser) }
pub async fn auth::served_host_guard(State<AppState>, Request, Next) -> Response;
pub async fn auth::session_guard(State<AppState>, Request, Next) -> Response;
pub(crate) fn auth::request_is_public(state: &AppState, headers: &HeaderMap) -> bool;
pub fn auth::session_cookie(token: &str, max_age_secs: i64, secure: bool) -> String;
pub fn auth::expired_cookie(secure: bool) -> String;
pub const auth::LOGIN_PATH: &str = "/auth/login";
pub struct extract::SecretJson<T>(pub T);  pub struct extract::PeerAddr(pub Option<SocketAddr>);
pub struct ratelimit::LoginLimiter;  // begin(&str, &str, Instant) -> Result<Attempt, Duration>; succeeded/not_a_failure(Attempt); failed(Attempt) -> Vec<Bucket>
pub fn ratelimit::client_ip(peer: Option<SocketAddr>, headers: &HeaderMap, header: Option<ClientIpHeader>) -> String;
pub fn ratelimit::account_key(email: &str) -> String;
```

## Security-critical tasks

Tasks 1, 2, 4, 5, 6, 7, 8, 9, 10 and 12 are security-critical: each needs a reviewer who reads the diff against the spec section named in the task, not just the tests. Task 3 is security-critical for `PublicUrl` only. Tasks 11 and 13 are ordinary.

---
### Task 1: `server.key` and password-file hardening (security-critical)

Closes **PR 1 review items 1 and 3**. Must land before any task that seals a TOTP secret over HTTP (Task 10).

**Files:**
- Modify: `crates/nigel-core/src/identity/seal.rs` (`load_or_create`, new `install_new_key`, `sync_dir`, `key_install_error`, `permission_warning`, tests)
- Modify: `crates/nigel-core/src/db.rs` (`file_password`, new `PASSWORD_FILE_MAX_BYTES`, tests)

**Interfaces:**
- Consumes: nothing new.
- Produces: `pub fn seal::permission_warning(path: &Path) -> Option<String>` (Task 11 prints it at boot).

- [ ] **Step 1: Write the failing seal tests**

Append inside `mod tests` in `crates/nigel-core/src/identity/seal.rs`:

```rust
    fn entries(dir: &Path) -> Vec<std::ffi::OsString> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn losing_the_race_keeps_the_winners_key_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEY_FILE);
        std::fs::write(&path, [9u8; 32]).unwrap();

        let err = install_new_key(dir.path(), &path, &[1u8; 32]).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&path).unwrap(), [9u8; 32]);
        assert_eq!(entries(dir.path()), [KEY_FILE]);
        let key = ServerKey::load_or_create(&path).unwrap();
        let sealed = ServerKey::from_bytes([9; 32]).seal(UID, b"x").unwrap();
        assert_eq!(key.open(UID, &sealed).unwrap().as_slice(), b"x");
    }

    #[test]
    fn a_failed_link_leaves_no_temp_file_and_the_error_names_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join(KEY_FILE);

        let err = install_new_key(dir.path(), &path, &[1u8; 32]).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(entries(dir.path()).is_empty(), "{:?}", entries(dir.path()));
        let message = key_install_error(&path, err).to_string();
        assert!(message.contains(&path.display().to_string()), "{message}");
    }

    #[cfg(unix)]
    #[test]
    fn loose_permissions_on_the_key_or_its_directory_are_reported() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let path = config.join(KEY_FILE);
        assert_eq!(permission_warning(&path), None, "no key yet");

        ServerKey::load_or_create(&path).unwrap();
        assert_eq!(permission_warning(&path), None);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let warning = permission_warning(&path).unwrap();
        assert!(warning.contains("chmod 600"), "{warning}");
        assert!(warning.contains(&path.display().to_string()), "{warning}");

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o755)).unwrap();
        let warning = permission_warning(&path).unwrap();
        assert!(warning.contains("chmod 700"), "{warning}");
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p nigel-core identity::seal -- --test-threads=1`
Expected: FAIL to compile — `install_new_key`, `key_install_error`, `permission_warning` not found.

- [ ] **Step 3: Implement**

In `seal.rs`, replace the body of `load_or_create` after `crate::settings::restrict_dir_permissions(dir)?;` and add the helpers:

```rust
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        rand::rngs::OsRng.fill_bytes(key.as_mut());
        match install_new_key(dir, path, key.as_ref()) {
            Ok(()) => Ok(Self(key)),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Self::load(path)?
                .ok_or_else(|| NigelError::Other(format!("{} vanished", path.display()))),
            Err(e) => Err(key_install_error(path, e)),
        }
    }
```

```rust
/// Write `key` to a private temporary file in `dir` and hard-link it to
/// `path`. A link never replaces an existing file, so a second process racing
/// to create the key gets `AlreadyExists` and reads the winner's. The
/// temporary file is removed on every path, and after a successful link the
/// directory is synced so the new name survives a crash.
fn install_new_key(dir: &Path, path: &Path, key: &[u8]) -> std::io::Result<()> {
    let mut suffix = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut suffix);
    let temp = dir.join(format!(".{KEY_FILE}.{}.tmp", hex::encode(suffix)));

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let linked = options
        .open(&temp)
        .and_then(|mut file| {
            file.write_all(key)?;
            file.sync_all()
        })
        .and_then(|()| std::fs::hard_link(&temp, path));
    let _ = std::fs::remove_file(&temp);
    linked?;
    sync_dir(dir)
}

#[cfg(unix)]
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}

fn key_install_error(path: &Path, e: std::io::Error) -> NigelError {
    NigelError::Other(format!(
        "Could not create the server key at {}: {e}",
        path.display()
    ))
}

/// A sentence for `nigel serve` to print when the key, or the directory it
/// lives in, is open to anyone but its owner. `None` when both are private or
/// there is no key yet.
pub fn permission_warning(path: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file_mode = std::fs::metadata(path).ok()?.permissions().mode() & 0o777;
        if file_mode & 0o077 != 0 {
            return Some(format!(
                "warning: {} can be read by other users (mode {file_mode:o}). Run `chmod 600 {}`.",
                path.display(),
                path.display()
            ));
        }
        let dir = path.parent()?;
        let dir_mode = std::fs::metadata(dir).ok()?.permissions().mode() & 0o777;
        if dir_mode & 0o077 != 0 {
            return Some(format!(
                "warning: {} is open to other users (mode {dir_mode:o}). Run `chmod 700 {}`.",
                dir.display(),
                dir.display()
            ));
        }
        None
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}
```

Delete the now-unused inline temp/link code from `load_or_create` (the `suffix`, `temp`, `options`, `linked` locals and the old `match linked`).

- [ ] **Step 4: Run the seal tests**

Run: `cargo test -p nigel-core identity::seal -- --test-threads=1`
Expected: PASS (all, including the existing `the_key_file_is_created_once_owner_only_and_reread`).

- [ ] **Step 5: Write the failing password-file test**

In `crates/nigel-core/src/db.rs` `mod tests`, after `a_password_file_that_is_not_utf8_is_fatal`:

```rust
    #[test]
    fn a_password_file_larger_than_the_cap_is_refused_without_quoting_it() {
        let (dir, db_path) = encrypted_db();
        let big = dir.path().join("big-password");
        std::fs::write(&big, vec![b'a'; PASSWORD_FILE_MAX_BYTES as usize + 1]).unwrap();
        let err = env_password_from(&db_path, None, Some(big.into_os_string()))
            .unwrap_err()
            .to_string();
        assert!(err.contains(PASSWORD_FILE_ENV_VAR), "{err}");
        assert!(err.contains("larger than 4096 bytes"), "{err}");
        assert!(!err.contains("aaaa"), "{err}");

        let at_cap = dir.path().join("at-cap-password");
        std::fs::write(&at_cap, vec![b'a'; PASSWORD_FILE_MAX_BYTES as usize]).unwrap();
        let err = env_password_from(&db_path, None, Some(at_cap.into_os_string()))
            .unwrap_err()
            .to_string();
        assert!(err.contains("may be damaged"), "a file at the cap is read: {err}");
    }
```

- [ ] **Step 6: Run it to verify it fails**

Run: `cargo test -p nigel-core db::tests::a_password_file_larger -- --test-threads=1`
Expected: FAIL to compile — `PASSWORD_FILE_MAX_BYTES` not found.

- [ ] **Step 7: Implement the cap**

In `db.rs`, after `const PASSWORD_FILE_ENV_VAR`:

```rust
/// The most a password file may hold. The file is one line of text; anything
/// larger is the wrong file, and reading it whole would pull an arbitrary
/// amount into memory.
const PASSWORD_FILE_MAX_BYTES: u64 = 4096;
```

In `file_password`, replace the `let bytes = Zeroizing::new(std::fs::read(path).map_err(...)?);` statement with:

```rust
    let could_not_read = |e: std::io::Error| {
        NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} names {}, which could not be read: {e}.",
            path.display()
        ))
    };
    let mut bytes = Zeroizing::new(Vec::new());
    {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(could_not_read)?;
        file.take(PASSWORD_FILE_MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(could_not_read)?;
    }
    if bytes.len() as u64 > PASSWORD_FILE_MAX_BYTES {
        return Err(NigelError::Other(format!(
            "{PASSWORD_FILE_ENV_VAR} names {}, which is larger than {PASSWORD_FILE_MAX_BYTES} bytes. It should hold only the password.",
            path.display()
        )));
    }
```

Also change `fn env_password_from` to `pub(crate) fn env_password_from` (Task 11's `boot::unlock_with` calls it).

- [ ] **Step 8: Run the db tests**

Run: `cargo test -p nigel-core db:: -- --test-threads=1`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
cargo fmt
git add crates/nigel-core/src/identity/seal.rs crates/nigel-core/src/db.rs
git commit -m "Harden server.key creation and cap the password file read

Sync the config directory after server.key is linked into place, name the
path when the link fails, and report a key or directory other users can
read. Cap NIGEL_DB_PASSWORD_FILE at 4 KiB. Carried from PR #79's review."
```

---

### Task 2: Identity sign-in surface (security-critical)

Closes **PR 1 review items 2 (the verify half), 4, 5, 6, 7 and 8**.

**Files:**
- Modify: `crates/nigel-core/src/identity/mod.rs` (`Role` serde/Display, `NOT_ENCRYPTED_WARNING`)
- Modify: `crates/nigel-core/src/identity/passwords.rs` (`dummy_phc` visibility, `warm_dummy_hash`, a test-only verify counter)
- Modify: `crates/nigel-core/src/identity/users.rs` (`PasswordVerified`, `Credentials`, `verify_credentials`, `SignIn`, `complete_sign_in`, `TotpCheck::Unopenable`, `set_totp_secret`, `verify_user_password`, `require_users`, `totp_unopenable`, first-user wording, tests)
- Modify: `crates/nigel-core/src/identity/sessions.rs` (`SessionGrant`, `create_session`, `ActiveSession.email`, constraint tests)
- Modify: `crates/nigel/src/cli/user.rs` (first-user check, the sign-in message, the warning constant's new home)
- Modify: `crates/nigel/tests/cli_dispatch.rs` (first-user message assertion)

**Interfaces:**
- Consumes: nothing new.
- Produces: everything under "identity" in "Shared interfaces" above.

- [ ] **Step 1: Write the failing `Role` test**

In `identity/mod.rs` `mod tests`:

```rust
    #[test]
    fn a_role_serializes_as_its_stored_word_and_displays_it() {
        assert_eq!(serde_json::to_string(&Role::ReadOnly).unwrap(), "\"read_only\"");
        assert_eq!(serde_json::to_string(&Role::Admin).unwrap(), "\"admin\"");
        let parsed: Role = serde_json::from_str("\"bookkeeper\"").unwrap();
        assert_eq!(parsed, Role::Bookkeeper);
        assert!(serde_json::from_str::<Role>("\"owner\"").is_err());
        assert_eq!(Role::ReadOnly.to_string(), "read_only");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p nigel-core identity::tests -- --test-threads=1`
Expected: FAIL to compile — `Role: Serialize` not satisfied.

- [ ] **Step 3: Implement `Role` serde and Display, and move the warning**

In `identity/mod.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Ordered `ReadOnly < Bookkeeper < Admin`. Serialized as the stored word.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    ReadOnly,
    Bookkeeper,
    Admin,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Printed by `nigel user add` for the first user and by `nigel serve` at
/// startup in shared mode, when the books are not encrypted.
pub const NOT_ENCRYPTED_WARNING: &str = "These books are not encrypted. Anyone who can read the \
     file can read the books. Encrypt them with `nigel password set` before you give anyone \
     else access.";
```

In `crates/nigel/src/cli/user.rs`, delete the local `NOT_ENCRYPTED_WARNING` constant and add `use nigel_core::identity::NOT_ENCRYPTED_WARNING;`.

- [ ] **Step 4: Run it**

Run: `cargo test -p nigel-core identity::tests -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Write the failing sessions tests**

In `identity/sessions.rs` `mod tests`, first update every existing `create_session(&conn, user, …)` / `create_session(&conn, sam, …)` call to `create_session(&conn, SessionGrant::assume_verified(user), …)` (and `sam` likewise) — a mechanical edit; `rg -n 'create_session\(' crates/nigel-core/src/identity` lists them. Then add:

```rust
    #[test]
    fn the_schema_refuses_a_bad_kind_a_repeated_hash_and_an_unknown_user() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let insert = |hash: &[u8], user_id: i64, kind: &str| {
            conn.execute(
                "INSERT INTO sessions (token_hash, user_id, kind, created_at, last_seen_at, expires_at)
                 VALUES (?1, ?2, ?3, 't', 't', 't')",
                params![hash, user_id, kind],
            )
        };
        assert!(insert(&[1; 32], user, "cookie").is_ok());
        assert!(insert(&[2; 32], user, "magic").is_err(), "kind CHECK");
        assert!(insert(&[1; 32], user, "bearer").is_err(), "token_hash UNIQUE");
        assert!(insert(&[3; 32], 9_999, "cookie").is_err(), "user_id FOREIGN KEY");
    }

    #[test]
    fn a_found_session_carries_its_users_email() {
        let (_dir, conn) = db();
        let user = admin(&conn);
        let now = at("2026-01-01T00:00:00Z");
        let issued = create_session(
            &conn,
            SessionGrant::assume_verified(user),
            SessionKind::Cookie,
            None,
            None,
            now,
        )
        .unwrap();
        let found = find_session(&conn, &issued.token, now).unwrap().unwrap();
        assert_eq!(found.email, "pat@example.com");
    }
```

- [ ] **Step 6: Run to verify failure**

Run: `cargo test -p nigel-core identity::sessions -- --test-threads=1`
Expected: FAIL to compile — `SessionGrant` not found, no field `email`.

- [ ] **Step 7: Implement `SessionGrant` and the email field**

In `sessions.rs`:

```rust
/// Proof that a sign-in passed every check it owed: the password, and the
/// second factor when the user has one. `users::complete_sign_in` is the only
/// production constructor and `create_session` consumes it, so a session
/// cannot be issued on a path that skipped verification.
#[derive(Debug)]
pub struct SessionGrant {
    user_id: i64,
}

impl SessionGrant {
    pub(super) fn for_verified(user_id: i64) -> Self {
        Self { user_id }
    }

    /// A grant without a sign-in, for tests that need a session.
    #[cfg(any(test, feature = "testutil"))]
    pub fn assume_verified(user_id: i64) -> Self {
        Self { user_id }
    }

    pub fn user_id(&self) -> i64 {
        self.user_id
    }
}
```

Change `create_session`'s signature and its first lines:

```rust
pub fn create_session(
    conn: &Connection,
    grant: SessionGrant,
    kind: SessionKind,
    user_agent: Option<&str>,
    ip: Option<&str>,
    now: DateTime<Utc>,
) -> Result<IssuedSession> {
    let user_id = grant.user_id;
```

and delete its old doc sentence "It does not check the user is enabled: callers create sessions only after …", replacing the doc with: `/// Issue a session for the user a grant names.`

Add `pub email: String,` to `ActiveSession` after `uid`, select `u.email` in `find_session`:

```rust
            "SELECT s.id, s.user_id, u.uid, u.email, u.role, u.must_change_password, s.kind,
                    s.created_at, s.last_seen_at, s.expires_at
               FROM sessions s JOIN users u ON u.id = s.user_id
              WHERE s.token_hash = ?1 AND s.expires_at > ?2 AND u.disabled_at IS NULL",
```

and shift the tuple: read ten columns (`r.get::<_, String>(3)?` is the email, role moves to 4, etc.) and set `email` in the returned `ActiveSession`.

- [ ] **Step 8: Run the sessions tests**

Run: `cargo test -p nigel-core identity::sessions -- --test-threads=1`
Expected: PASS. (users.rs tests will not compile yet; that is Step 9's work.)

- [ ] **Step 9: Write the failing users tests**

In `users.rs` `mod tests`, update the existing `create_session(&conn, pat.user.id, …)` calls to `create_session(&conn, SessionGrant::assume_verified(pat.user.id), …)` (import `crate::identity::sessions::SessionGrant`), and change `Credentials::Valid(u) if u.must_change_password` to `Credentials::Valid(p) if p.user().must_change_password`. Replace `a_secret_sealed_under_another_key_is_an_error_not_a_rejection` and `clearing_totp_turns_it_off` with the tightened versions, and add the new tests:

```rust
    fn last_step(conn: &Connection, id: i64) -> Option<i64> {
        conn.query_row(
            "SELECT totp_last_step FROM users WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn a_secret_another_key_sealed_is_unopenable_and_moves_nothing() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        let secret = [3u8; 20];
        let now = at("2026-03-01T12:00:00Z");
        let step = totp::step_at(now);
        set_totp_secret(&conn, pat, &ServerKey::from_bytes([1; 32]), &secret, step - 5, now)
            .unwrap();
        let code = totp::code_for_step(&secret, step);
        assert_eq!(
            verify_totp(&conn, pat, &ServerKey::from_bytes([2; 32]), &code, now).unwrap(),
            TotpCheck::Unopenable
        );
        assert_eq!(last_step(&conn, pat), Some(step - 5));
    }

    #[test]
    fn clearing_totp_removes_the_secret_and_the_replay_guard() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        set_totp_secret(&conn, pat, &ServerKey::from_bytes([1; 32]), &[3; 20], 7, at(T0))
            .unwrap();
        clear_totp(&conn, pat, at(T0)).unwrap();
        let (secret, step): (Option<Vec<u8>>, Option<i64>) = conn
            .query_row(
                "SELECT totp_secret, totp_last_step FROM users WHERE id = ?1",
                [pat],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((secret, step), (None, None));
        assert!(!get_user(&conn, pat).unwrap().unwrap().totp_enabled);
        assert!(matches!(
            clear_totp(&conn, 999, at(T0)),
            Err(NigelError::NotFound(_))
        ));
    }

    #[test]
    fn a_disabled_user_cannot_enroll_a_second_factor() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper).user.id;
        disable_user(&conn, sam, at(T0)).unwrap();
        let err = set_totp_secret(&conn, sam, &ServerKey::from_bytes([1; 32]), &[3; 20], 0, at(T0))
            .unwrap_err();
        assert!(
            matches!(err, NigelError::Conflict { code: "user_disabled", .. }),
            "{err}"
        );
        assert!(!get_user(&conn, sam).unwrap().unwrap().totp_enabled);
    }

    #[test]
    fn every_credential_path_runs_exactly_one_verification() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin);
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper);
        disable_user(&conn, sam.user.id, at(T0)).unwrap();
        for (email, password) in [
            ("nobody@example.com", "whatever password"),
            ("not an email", "whatever password"),
            ("pat@example.com", "wrong password here"),
            ("sam@example.com", sam.temporary_password.as_str()),
            ("pat@example.com", pat.temporary_password.as_str()),
        ] {
            let before = crate::identity::passwords::verifications_for_tests();
            verify_credentials(&conn, email, password).unwrap();
            assert_eq!(
                crate::identity::passwords::verifications_for_tests() - before,
                1,
                "{email}"
            );
        }
    }

    #[test]
    fn sign_in_without_two_step_is_granted_on_the_password_alone() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin);
        let Credentials::Valid(proof) =
            verify_credentials(&conn, "pat@example.com", &pat.temporary_password).unwrap()
        else {
            panic!("valid credentials");
        };
        let SignIn::Granted(user, grant) =
            complete_sign_in(&conn, proof, None, None, at(T0)).unwrap()
        else {
            panic!("granted");
        };
        assert_eq!(grant.user_id(), user.id);
    }

    #[test]
    fn sign_in_with_two_step_asks_for_a_code_and_checks_it() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin);
        let key = ServerKey::from_bytes([1; 32]);
        let secret = [3u8; 20];
        let now = at("2026-03-01T12:00:00Z");
        let step = totp::step_at(now);
        set_totp_secret(&conn, pat.user.id, &key, &secret, step - 5, now).unwrap();
        let proof = || match verify_credentials(&conn, "pat@example.com", &pat.temporary_password)
            .unwrap()
        {
            Credentials::Valid(p) => p,
            Credentials::Invalid => panic!("valid credentials"),
        };

        assert!(matches!(
            complete_sign_in(&conn, proof(), Some(&key), None, now).unwrap(),
            SignIn::CodeRequired(_)
        ));
        assert!(matches!(
            complete_sign_in(&conn, proof(), Some(&key), Some("  "), now).unwrap(),
            SignIn::CodeRequired(_)
        ));
        assert!(matches!(
            complete_sign_in(&conn, proof(), None, Some("123456"), now).unwrap(),
            SignIn::TotpUnavailable(_)
        ));
        assert!(matches!(
            complete_sign_in(&conn, proof(), Some(&ServerKey::from_bytes([2; 32])), Some("123456"), now)
                .unwrap(),
            SignIn::TotpUnavailable(_)
        ));
        let wrong = totp::code_for_step(&secret, step + 5);
        assert!(matches!(
            complete_sign_in(&conn, proof(), Some(&key), Some(&wrong), now).unwrap(),
            SignIn::CodeRejected(_)
        ));
        let right = totp::code_for_step(&secret, step);
        assert!(matches!(
            complete_sign_in(&conn, proof(), Some(&key), Some(&right), now).unwrap(),
            SignIn::Granted(..)
        ));
    }

    #[test]
    fn a_users_own_password_verifies_and_a_disabled_user_never_does() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin);
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper);
        assert!(verify_user_password(&conn, pat.user.id, &pat.temporary_password).unwrap());
        assert!(!verify_user_password(&conn, pat.user.id, "not the password").unwrap());
        disable_user(&conn, sam.user.id, at(T0)).unwrap();
        assert!(!verify_user_password(&conn, sam.user.id, &sam.temporary_password).unwrap());
        assert!(verify_user_password(&conn, 999, "x").is_err());
    }

    #[test]
    fn books_without_users_refuse_as_local_mode() {
        let (_dir, conn) = db();
        let err = require_users(&conn).unwrap_err();
        assert!(matches!(err, NigelError::Conflict { code: "local_mode", .. }), "{err}");
        add(&conn, "pat@example.com", Role::Admin);
        require_users(&conn).unwrap();
    }

    #[test]
    fn the_first_user_refusal_names_no_cli_flag() {
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
        .unwrap_err()
        .to_string();
        assert!(err.contains("The first user must be an admin."), "{err}");
        assert!(!err.contains("--admin"), "{err}");
    }

    #[test]
    fn users_whose_second_factor_this_key_cannot_open_are_named() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper).user.id;
        add(&conn, "lee@example.com", Role::ReadOnly);
        let key = ServerKey::from_bytes([1; 32]);
        set_totp_secret(&conn, pat, &key, &[3; 20], 0, at(T0)).unwrap();
        set_totp_secret(&conn, sam, &ServerKey::from_bytes([2; 32]), &[3; 20], 0, at(T0)).unwrap();
        assert_eq!(totp_unopenable(&conn, Some(&key)).unwrap(), ["sam@example.com"]);
        assert_eq!(
            totp_unopenable(&conn, None).unwrap(),
            ["pat@example.com", "sam@example.com"]
        );
    }
```

In `passwords.rs` `mod tests`, add:

```rust
    #[test]
    fn warming_the_dummy_hash_makes_it_ready() {
        warm_dummy_hash();
        assert!(dummy_phc().starts_with("$argon2id$"));
    }
```

- [ ] **Step 10: Run to verify failure**

Run: `cargo test -p nigel-core identity:: -- --test-threads=1`
Expected: FAIL to compile — `complete_sign_in`, `SignIn`, `TotpCheck::Unopenable`, `verify_user_password`, `require_users`, `totp_unopenable`, `verifications_for_tests`, `warm_dummy_hash` not found.

- [ ] **Step 11: Implement in `passwords.rs`**

```rust
#[cfg(test)]
thread_local! {
    static VERIFICATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many Argon2id verifications this thread has run, for the test that
/// every sign-in path runs exactly one.
#[cfg(test)]
pub(crate) fn verifications_for_tests() -> usize {
    VERIFICATIONS.with(|count| count.get())
}
```

At the top of `verify_password`'s body:

```rust
    #[cfg(test)]
    VERIFICATIONS.with(|count| count.set(count.get() + 1));
```

Make `dummy_phc` `pub(crate)` and add:

```rust
/// Compute the dummy hash now, so the first sign-in for an unknown address
/// does not pay for hashing it on top of verifying against it.
pub fn warm_dummy_hash() {
    let _ = dummy_phc();
}
```

- [ ] **Step 12: Implement in `users.rs`**

Replace the `Credentials` enum and `verify_credentials`:

```rust
/// A user whose password was just verified. Only `verify_credentials` makes
/// one, and `complete_sign_in` is the only thing that consumes it.
#[derive(Debug)]
pub struct PasswordVerified {
    user: User,
}

impl PasswordVerified {
    pub fn user(&self) -> &User {
        &self.user
    }
}

/// What a sign-in attempt proved. A wrong email, a wrong password and a
/// disabled user are all `Invalid`, and all cost one Argon2id verification.
#[derive(Debug)]
pub enum Credentials {
    Valid(PasswordVerified),
    Invalid,
}

/// Check an email and password with exactly one Argon2id verification on
/// every path: against the user's hash, or against the fixed dummy hash when
/// the address matches no user or is not an address at all.
pub fn verify_credentials(conn: &Connection, email: &str, password: &str) -> Result<Credentials> {
    let row = match normalize_email(email) {
        Ok(email) => conn
            .query_row(
                &format!("SELECT {USER_COLUMNS}, password_hash FROM users WHERE email = ?1"),
                [email],
                |r| Ok((user_from_row(r)?, r.get::<_, String>(11)?)),
            )
            .optional()?,
        Err(_) => None,
    };
    let (user, phc) = match row {
        Some((user, hash)) => (Some(user), hash),
        None => (None, dummy_phc().to_string()),
    };
    let matched = verify_password(password, &phc);
    match user {
        Some(user) if matched && user.is_enabled() => {
            Ok(Credentials::Valid(PasswordVerified { user }))
        }
        _ => Ok(Credentials::Invalid),
    }
}
```

Fix the import line: drop `verify_dummy`, add `dummy_phc`; add `use super::sessions::SessionGrant;`.

Add `Unopenable` to `TotpCheck` with a doc line (`/// The server key is missing or does not open this user's secret.`), and in `verify_totp` replace `let secret = key.open(&uid, &sealed)?;` with:

```rust
    let Ok(secret) = key.open(&uid, &sealed) else {
        return Ok(TotpCheck::Unopenable);
    };
```

and update its doc comment's last two lines to: `/// A secret the server key cannot open is \`Unopenable\`, not a rejection: the caller names the user.`

Add the sign-in completion:

```rust
/// Where a sign-in stands once the password is proven.
#[derive(Debug)]
pub enum SignIn {
    /// Every check passed. The grant issues exactly one session.
    Granted(User, SessionGrant),
    /// Two-step is on and no code was sent: ask for one.
    CodeRequired(User),
    /// Two-step is on and the code did not match.
    CodeRejected(User),
    /// Two-step is on and this machine cannot check it: no server key, or
    /// one that does not open this user's secret.
    TotpUnavailable(User),
}

/// Finish a sign-in whose password `verify_credentials` proved: grant it
/// outright when the user has no second factor, otherwise check `code`.
pub fn complete_sign_in(
    conn: &Connection,
    proof: PasswordVerified,
    key: Option<&ServerKey>,
    code: Option<&str>,
    now: DateTime<Utc>,
) -> Result<SignIn> {
    let user = proof.user;
    if !user.totp_enabled {
        let grant = SessionGrant::for_verified(user.id);
        return Ok(SignIn::Granted(user, grant));
    }
    let Some(code) = code.filter(|c| !c.trim().is_empty()) else {
        return Ok(SignIn::CodeRequired(user));
    };
    let Some(key) = key else {
        return Ok(SignIn::TotpUnavailable(user));
    };
    match verify_totp(conn, user.id, key, code, now)? {
        TotpCheck::Accepted | TotpCheck::NotEnrolled => {
            let grant = SessionGrant::for_verified(user.id);
            Ok(SignIn::Granted(user, grant))
        }
        TotpCheck::Rejected => Ok(SignIn::CodeRejected(user)),
        TotpCheck::Unopenable => Ok(SignIn::TotpUnavailable(user)),
    }
}
```

`NotEnrolled` here means the second factor was removed between the password check and this call; the password alone then suffices, as it would have a moment later.

In `set_totp_secret`, after `let user = get_user(...)`:

```rust
    if !user.is_enabled() {
        return Err(disabled(&user));
    }
```

guard the UPDATE with `WHERE id = ?4 AND disabled_at IS NULL`, and on `changed == 0` return `Err(disabled(&user))` (the row exists; only a concurrent disable empties the match). Add:

```rust
fn disabled(user: &User) -> NigelError {
    NigelError::Conflict {
        code: "user_disabled",
        message: format!(
            "{} is disabled. Enable them before turning on two-step verification.",
            user.email
        ),
    }
}

/// Whether `password` is this enabled user's current password: one Argon2id
/// verification, and `false` for a disabled user.
pub fn verify_user_password(conn: &Connection, id: i64, password: &str) -> Result<bool> {
    let (user, hash) = conn
        .query_row(
            &format!("SELECT {USER_COLUMNS}, password_hash FROM users WHERE id = ?1"),
            [id],
            |r| Ok((user_from_row(r)?, r.get::<_, String>(11)?)),
        )
        .optional()?
        .ok_or_else(|| not_found(id))?;
    Ok(verify_password(password, &hash) && user.is_enabled())
}

/// Refuse, as `local_mode`, an operation that only books with users support.
/// The first user is created with the CLI, never over HTTP, so an API route
/// that creates users calls this before `create_user`.
pub fn require_users(conn: &Connection) -> Result<()> {
    if has_users(conn)? {
        return Ok(());
    }
    Err(NigelError::Conflict {
        code: "local_mode",
        message: "These books have no users. The first one is added on the server with \
                  `nigel user add`."
            .into(),
    })
}

/// The emails of enabled users whose second factor this machine cannot
/// check: every enrolled user when there is no key, otherwise those whose
/// sealed secret the key does not open.
pub fn totp_unopenable(conn: &Connection, key: Option<&ServerKey>) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT uid, email, totp_secret FROM users
          WHERE totp_secret IS NOT NULL AND disabled_at IS NULL ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Vec<u8>>(2)?,
        ))
    })?;
    let mut unopenable = Vec::new();
    for row in rows {
        let (uid, email, sealed) = row?;
        if key.is_none_or(|key| key.open(&uid, &sealed).is_err()) {
            unopenable.push(email);
        }
    }
    Ok(unopenable)
}
```


In `create_user`, change the first-user message to `"The first user must be an admin."`.

- [ ] **Step 13: Write the failing CLI test**

In `crates/nigel/src/cli/user.rs` `mod tests`:

```rust
    #[test]
    fn the_first_user_message_says_sign_in_is_now_required() {
        assert_eq!(
            first_user_message("pat@example.com"),
            "Nigel will now require sign-in. Run `nigel serve` and sign in as pat@example.com."
        );
    }
```

In `crates/nigel/tests/cli_dispatch.rs`, find the test that runs `user add … --admin` on fresh books (search `"user", "add"`) and add to its assertions:

```rust
        .stdout(predicate::str::contains(
            "Nigel will now require sign-in. Run `nigel serve` and sign in as pat@example.com.",
        ))
```

Keep the existing `contains("The first user must be an admin")` assertion at line ~5185; add next to it `.stderr(predicate::str::contains("--admin"))` so the CLI hint is pinned.

- [ ] **Step 14: Implement the CLI side**

In `crates/nigel/src/cli/user.rs`:

```rust
pub(crate) fn first_user_message(email: &str) -> String {
    format!("Nigel will now require sign-in. Run `nigel serve` and sign in as {email}.")
}
```

and in `add`, before `users::create_user`:

```rust
    if role != Role::Admin && !users::has_users(&conn)? {
        return Err(nigel_core::error::NigelError::Invalid(
            "The first user must be an admin. Add them with --admin.".into(),
        ));
    }
```

and after printing `added_message`:

```rust
    if created.first_user {
        println!("{}", first_user_message(&created.user.email));
        if !is_encrypted(&path)? {
            eprintln!("{NOT_ENCRYPTED_WARNING}");
        }
    }
```

(replacing the old `if created.first_user && !is_encrypted(&path)?` block).

- [ ] **Step 15: Run everything touched**

Run: `cargo test -p nigel-core identity:: -- --test-threads=1 && cargo test -p nigel cli::user -- --test-threads=1 && cargo test -p nigel --test cli_dispatch user -- --test-threads=1`
Expected: PASS.

- [ ] **Step 16: Commit**

```bash
cargo fmt
git add crates/nigel-core/src/identity crates/nigel/src/cli/user.rs crates/nigel/tests/cli_dispatch.rs
git commit -m "Make a session require a completed sign-in

verify_credentials runs exactly one Argon2id verification on every path
and returns a PasswordVerified proof; complete_sign_in checks the second
factor and is the only producer of the SessionGrant that create_session
consumes. A secret the server key cannot open is Unopenable, a disabled
user cannot enroll two-step, Role serializes as its stored word, and the
first-user refusal names no CLI flag. nigel user add says sign-in is now
required. Carried from PR #79's review."
```

---
### Task 3: `ServerConfig` and the auth event lines (security-critical: `PublicUrl`)

**Files:**
- Create: `crates/nigel-core/src/server/config.rs`
- Create: `crates/nigel-core/src/server/events.rs`
- Modify: `crates/nigel-core/src/server/mod.rs` (`pub mod config; pub mod events;`)
- Modify: `crates/nigel-core/src/settings.rs` (two fields)

**Interfaces:**
- Consumes: `crate::settings::Settings`.
- Produces: `config::{PublicUrl, ClientIpHeader, ServerConfig, PUBLIC_URL_ENV, CLIENT_IP_HEADER_ENV}`; `events::{AuthEvent, EventKind, emit}`.

- [ ] **Step 1: Add the settings fields**

In `settings.rs`, after `document_response_url` in `Settings`:

```rust
    #[serde(default)]
    pub server_public_url: Option<String>,
    #[serde(default)]
    pub client_ip_header: Option<String>,
```

and `server_public_url: None, client_ip_header: None,` in `Default`. Any struct literal of `Settings` that does not end in `..Default::default()` fails to compile; add the two `None`s there.

- [ ] **Step 2: Write the failing config tests**

Create `crates/nigel-core/src/server/config.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn url(raw: &str) -> PublicUrl {
        PublicUrl::parse(raw).unwrap_or_else(|e| panic!("{raw}: {e}"))
    }

    #[test]
    fn an_https_origin_parses_to_its_host_and_origin() {
        let u = url("https://books.example.com");
        assert_eq!(u.authority(), "books.example.com");
        assert_eq!(u.origin(), "https://books.example.com");
        let u = url("https://books.example.com:8443");
        assert_eq!(u.authority(), "books.example.com:8443");
        assert_eq!(u.origin(), "https://books.example.com:8443");
        let u = url("https://[2001:db8::1]");
        assert_eq!(u.authority(), "[2001:db8::1]");
    }

    #[test]
    fn loopback_and_loose_spellings() {
        for loopback in [
            "https://localhost",
            "https://127.0.0.1",
            "https://127.1.2.3:8443",
            "https://[::1]",
            "https://LOCALHOST/",
        ] {
            let err = PublicUrl::parse(loopback).unwrap_err();
            assert!(err.contains("loopback"), "{loopback}: {err}");
        }
        for (raw, origin) in [
            ("https://BOOKS.example.com/", "https://books.example.com"),
            ("HTTPS://books.example.com:443", "https://books.example.com"),
            ("  https://books.example.com  ", "https://books.example.com"),
        ] {
            assert_eq!(url(raw).origin(), origin, "{raw}");
        }
    }

    #[test]
    fn anything_but_one_https_origin_is_refused_by_value() {
        for raw in [
            "http://books.example.com",
            "books.example.com",
            "https://",
            "https://books.example.com/nigel",
            "https://books.example.com?x=1",
            "https://books.example.com#top",
            "https://pat@books.example.com",
            "https://books.example.com:0",
            "https://books.example.com:99999",
            "https://books.example.com:abc",
            "https://bad_host.example.com",
            "https://books..example.com",
            "https://.example.com",
            "https://books example.com",
            "https://[not-v6]",
        ] {
            let err = PublicUrl::parse(raw).unwrap_err();
            assert!(err.contains("server_public_url"), "{raw}: {err}");
            assert!(err.contains(raw.trim()), "{raw}: {err}");
        }
    }

    #[test]
    fn the_host_header_and_origin_match_whole() {
        let u = url("https://books.example.com");
        assert!(u.matches_host("books.example.com"));
        assert!(u.matches_host("Books.Example.com"));
        assert!(u.matches_host("books.example.com:443"));
        assert!(!u.matches_host("books.example.com:8443"));
        assert!(!u.matches_host("evil.books.example.com"));
        assert!(!u.matches_host("books.example.com.evil.com"));
        assert!(u.matches_origin("https://books.example.com"));
        assert!(u.matches_origin("https://books.example.com:443"));
        assert!(!u.matches_origin("http://books.example.com"));
        assert!(!u.matches_origin("https://books.example.com:8443"));
        assert!(!u.matches_origin("https://books.example.com.evil.com"));
        assert!(!u.matches_origin("null"));

        let p = url("https://books.example.com:8443");
        assert!(p.matches_host("books.example.com:8443"));
        assert!(!p.matches_host("books.example.com"));
        assert!(p.matches_origin("https://books.example.com:8443"));
        assert!(!p.matches_origin("https://books.example.com"));
    }

    #[test]
    fn the_client_ip_header_is_one_of_two_names_in_any_case() {
        assert_eq!(
            ClientIpHeader::parse("CF-Connecting-IP").unwrap(),
            ClientIpHeader::CfConnectingIp
        );
        assert_eq!(
            ClientIpHeader::parse(" x-forwarded-for ").unwrap(),
            ClientIpHeader::XForwardedFor
        );
        let err = ClientIpHeader::parse("X-Real-IP").unwrap_err();
        assert!(err.contains("X-Real-IP") && err.contains("client_ip_header"), "{err}");
    }

    #[test]
    fn the_environment_wins_and_blank_values_are_unset() {
        let file = Settings {
            server_public_url: Some("https://file.example.com".into()),
            client_ip_header: Some("X-Forwarded-For".into()),
            ..Settings::default()
        };
        let none = ServerConfig::from_settings_with(&file, |_| None).unwrap();
        assert_eq!(none.public_url.unwrap().origin(), "https://file.example.com");
        assert_eq!(none.client_ip_header, Some(ClientIpHeader::XForwardedFor));

        let env = ServerConfig::from_settings_with(&file, |name| match name {
            PUBLIC_URL_ENV => Some("https://env.example.com".into()),
            CLIENT_IP_HEADER_ENV => Some("   ".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(env.public_url.unwrap().origin(), "https://env.example.com");
        assert_eq!(env.client_ip_header, Some(ClientIpHeader::XForwardedFor));

        let bad = ServerConfig::from_settings_with(&file, |name| {
            (name == PUBLIC_URL_ENV).then(|| "http://env.example.com".to_string())
        })
        .unwrap_err()
        .to_string();
        assert!(bad.contains("http://env.example.com"), "{bad}");

        assert_eq!(
            ServerConfig::from_settings_with(&Settings::default(), |_| None).unwrap(),
            ServerConfig::default()
        );
    }
}
```

Add `pub mod config;` to `server/mod.rs`.

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p nigel-core server::config -- --test-threads=1`
Expected: FAIL to compile — `PublicUrl` not found.

- [ ] **Step 4: Implement `config.rs`**

Put above the test module:

```rust
//! How `nigel serve` is reached, read from settings.json and the environment:
//! its one public address, and the header that carries a client's IP behind
//! a proxy on the same machine.

use std::net::{IpAddr, Ipv6Addr};

use crate::error::{NigelError, Result};
use crate::settings::Settings;

pub const PUBLIC_URL_ENV: &str = "NIGEL_SERVER_PUBLIC_URL";
pub const CLIENT_IP_HEADER_ENV: &str = "NIGEL_CLIENT_IP_HEADER";

/// The one non-loopback origin this server answers to in shared mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicUrl {
    /// `host` or `host:port`, lower-cased, an IPv6 literal bracketed. The
    /// default port 443 is never written.
    authority: String,
    explicit_port: bool,
    origin: String,
}

impl PublicUrl {
    /// One `https://` origin: scheme, host, an optional port, at most a
    /// trailing `/`. Loopback names are refused, because loopback is always
    /// accepted and `Secure` cookies and HSTS must never reach it.
    pub fn parse(raw: &str) -> std::result::Result<Self, String> {
        let value = raw.trim();
        let refuse = |why: &str| {
            format!(
                "server_public_url {value:?} {why}. Use one https:// origin with no path, \
                 such as \"https://books.example.com\"."
            )
        };
        let rest = value
            .get(..8)
            .filter(|scheme| scheme.eq_ignore_ascii_case("https://"))
            .map(|_| &value[8..])
            .ok_or_else(|| refuse("is not an https:// address"))?;
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        if rest.is_empty() {
            return Err(refuse("has no host"));
        }
        if rest.contains(['/', '?', '#', '@', '\\'])
            || rest.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(refuse("has a path, a query or credentials in it"));
        }

        let (host, port) = split_host_port(rest).ok_or_else(|| refuse("does not parse"))?;
        let host = host.to_ascii_lowercase();
        if is_loopback(&host) {
            return Err(refuse(
                "names the loopback interface, which Nigel always accepts already",
            ));
        }
        let port = match port {
            None => None,
            Some(p) => match p.parse::<u16>() {
                Ok(0) | Err(_) => return Err(refuse("has a port that is not a number from 1 to 65535")),
                Ok(443) => None,
                Ok(p) => Some(p),
            },
        };
        let host_part = if host.parse::<Ipv6Addr>().is_ok() {
            format!("[{host}]")
        } else {
            host
        };
        let authority = match port {
            Some(p) => format!("{host_part}:{p}"),
            None => host_part,
        };
        Ok(Self {
            origin: format!("https://{authority}"),
            explicit_port: port.is_some(),
            authority,
        })
    }

    /// `host[:port]` as a `Host` header names it.
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// `https://host[:port]`, as a browser sends it in `Origin`.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// True when a `Host` header names this address, whole.
    pub fn matches_host(&self, header: &str) -> bool {
        let header = header.trim().to_ascii_lowercase();
        header == self.authority
            || (!self.explicit_port && header == format!("{}:443", self.authority))
    }

    /// True when an `Origin` header is this origin, whole: scheme, host and port.
    pub fn matches_origin(&self, origin: &str) -> bool {
        let origin = origin.trim();
        origin.eq_ignore_ascii_case(&self.origin)
            || (!self.explicit_port && origin.eq_ignore_ascii_case(&format!("{}:443", self.origin)))
    }
}

/// `(host, port)` from an authority with no userinfo. A hostname is dot-
/// separated labels of letters, digits and hyphens; an IPv6 literal is
/// bracketed. `None` for anything else.
fn split_host_port(authority: &str) -> Option<(String, Option<&str>)> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (inner, after) = rest.split_once(']')?;
        inner.parse::<Ipv6Addr>().ok()?;
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':')?),
        };
        return Some((inner.to_string(), port));
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    let valid_label = |label: &str| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    if host.split('.').all(valid_label) {
        Some((host.to_string(), port))
    } else {
        None
    }
}

fn is_loopback(host: &str) -> bool {
    host == "localhost"
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Which header carries the client's address when a proxy on this machine
/// forwards to Nigel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIpHeader {
    CfConnectingIp,
    XForwardedFor,
}

impl ClientIpHeader {
    pub fn parse(raw: &str) -> std::result::Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cf-connecting-ip" => Ok(Self::CfConnectingIp),
            "x-forwarded-for" => Ok(Self::XForwardedFor),
            _ => Err(format!(
                "client_ip_header {:?} is not one Nigel reads. Use \"CF-Connecting-IP\" or \
                 \"X-Forwarded-For\".",
                raw.trim()
            )),
        }
    }

    /// The header's name as `http` spells it.
    pub fn header_name(self) -> &'static str {
        match self {
            Self::CfConnectingIp => "cf-connecting-ip",
            Self::XForwardedFor => "x-forwarded-for",
        }
    }
}

/// What `nigel serve` reads about how it is reached. `Default` is a server
/// with neither key set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerConfig {
    pub public_url: Option<PublicUrl>,
    pub client_ip_header: Option<ClientIpHeader>,
}

impl ServerConfig {
    pub fn from_settings(settings: &Settings) -> Result<Self> {
        Self::from_settings_with(settings, |name| std::env::var(name).ok())
    }

    /// The environment wins over settings.json, each trimmed, a blank value
    /// counting as unset. An invalid value is an error naming it.
    pub fn from_settings_with(
        settings: &Settings,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self> {
        let pick = |name: &str, file: &Option<String>| {
            let trimmed =
                |v: Option<String>| v.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
            trimmed(env(name)).or_else(|| trimmed(file.clone()))
        };
        let public_url = pick(PUBLIC_URL_ENV, &settings.server_public_url)
            .map(|v| PublicUrl::parse(&v))
            .transpose()
            .map_err(NigelError::Other)?;
        let client_ip_header = pick(CLIENT_IP_HEADER_ENV, &settings.client_ip_header)
            .map(|v| ClientIpHeader::parse(&v))
            .transpose()
            .map_err(NigelError::Other)?;
        Ok(Self {
            public_url,
            client_ip_header,
        })
    }
}
```

- [ ] **Step 5: Run the config tests**

Run: `cargo test -p nigel-core server::config -- --test-threads=1`
Expected: PASS. If `https://books example.com` is not refused, check that the whitespace test runs before `split_host_port`.

- [ ] **Step 6: Write the failing events tests**

Create `crates/nigel-core/src/server/events.rs` with the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_names_every_field_and_dashes_the_absent_ones() {
        let line = AuthEvent::new(EventKind::LoginSucceeded)
            .email("pat@example.com")
            .user(2)
            .ip("203.0.113.9")
            .session(14)
            .line();
        assert_eq!(
            line,
            "auth event=login_succeeded email=\"pat@example.com\" user=2 ip=203.0.113.9 session=14 reason=-"
        );
        let line = AuthEvent::new(EventKind::LoginFailed)
            .email("nobody@example.com")
            .ip("203.0.113.9")
            .reason("credentials")
            .line();
        assert_eq!(
            line,
            "auth event=login_failed email=\"nobody@example.com\" user=- ip=203.0.113.9 session=- reason=credentials"
        );
    }

    #[test]
    fn an_email_cannot_forge_a_second_line() {
        let typed = "pat@example.com\nauth event=login_succeeded \"x\"";
        let line = AuthEvent::new(EventKind::LoginFailed).email(typed).line();
        assert!(!line.contains('\n'), "{line}");
        assert!(line.contains("\\n"), "{line}");
        assert!(line.contains("\\\"x\\\""), "{line}");

        let long = "a".repeat(10_000);
        let line = AuthEvent::new(EventKind::LoginFailed).email(&long).line();
        assert!(line.len() < 400, "{}", line.len());
    }
}
```

Add `pub mod events;` to `server/mod.rs`.

- [ ] **Step 7: Run to verify failure**

Run: `cargo test -p nigel-core server::events -- --test-threads=1`
Expected: FAIL to compile.

- [ ] **Step 8: Implement `events.rs`**

```rust
//! One stderr line per sign-in event — the journal record under systemd
//! until the audit log replaces these lines with rows carrying the same
//! fields. `reason` is a `&'static str`, so no event can carry a secret.

/// The longest email a line quotes, in characters.
const EMAIL_MAX_CHARS: usize = 254;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    LoginSucceeded,
    LoginFailed,
    LoginRefused,
    Lockout,
    Logout,
    PasswordChanged,
    TotpEnabled,
    TotpRemoved,
    TotpUnavailable,
    SharedModeBegan,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoginSucceeded => "login_succeeded",
            Self::LoginFailed => "login_failed",
            Self::LoginRefused => "login_refused",
            Self::Lockout => "lockout",
            Self::Logout => "logout",
            Self::PasswordChanged => "password_changed",
            Self::TotpEnabled => "totp_enabled",
            Self::TotpRemoved => "totp_removed",
            Self::TotpUnavailable => "totp_unavailable",
            Self::SharedModeBegan => "shared_mode_began",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthEvent<'a> {
    kind: EventKind,
    email: Option<&'a str>,
    user_id: Option<i64>,
    ip: Option<&'a str>,
    session_id: Option<i64>,
    reason: Option<&'static str>,
}

impl<'a> AuthEvent<'a> {
    pub fn new(kind: EventKind) -> Self {
        Self {
            kind,
            email: None,
            user_id: None,
            ip: None,
            session_id: None,
            reason: None,
        }
    }

    pub fn email(mut self, email: &'a str) -> Self {
        self.email = Some(email);
        self
    }

    pub fn user(mut self, id: i64) -> Self {
        self.user_id = Some(id);
        self
    }

    pub fn ip(mut self, ip: &'a str) -> Self {
        self.ip = Some(ip);
        self
    }

    pub fn session(mut self, id: i64) -> Self {
        self.session_id = Some(id);
        self
    }

    pub fn reason(mut self, reason: &'static str) -> Self {
        self.reason = Some(reason);
        self
    }

    /// `auth event=… email="…" user=… ip=… session=… reason=…`. The email is
    /// whatever was typed at a login, so it is cut short and `{:?}`-escaped:
    /// a newline in it cannot start a second line.
    pub fn line(&self) -> String {
        let dash = |v: Option<String>| v.unwrap_or_else(|| "-".to_string());
        let email = self.email.map(|e| {
            let cut: String = e.chars().take(EMAIL_MAX_CHARS).collect();
            format!("{cut:?}")
        });
        let ip = self
            .ip
            .map(|ip| ip.chars().take(64).collect::<String>().escape_debug().to_string());
        format!(
            "auth event={} email={} user={} ip={} session={} reason={}",
            self.kind.as_str(),
            dash(email),
            dash(self.user_id.map(|id| id.to_string())),
            dash(ip),
            dash(self.session_id.map(|id| id.to_string())),
            self.reason.unwrap_or("-"),
        )
    }
}

/// Write the event's line to stderr.
pub fn emit(event: &AuthEvent<'_>) {
    eprintln!("{}", event.line());
}
```

`ratelimit::client_ip` only ever produces an IP address or `unknown`; escaping the IP anyway keeps the line one line whatever a later caller passes.

- [ ] **Step 9: Run the events tests**

Run: `cargo test -p nigel-core server::events -- --test-threads=1`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
cargo fmt
git add crates/nigel-core/src/server/config.rs crates/nigel-core/src/server/events.rs crates/nigel-core/src/server/mod.rs crates/nigel-core/src/settings.rs
git commit -m "Read server_public_url and client_ip_header; add auth event lines

server_public_url is one https origin, normalized and refused by value
when it is anything else or names loopback. client_ip_header is
CF-Connecting-IP or X-Forwarded-For. Both take an environment override.
Event lines escape the typed email so it cannot forge a line."
```

---

### Task 4: Error codes, the mode cell and `AppState` (security-critical)

**Files:**
- Modify: `crates/nigel-core/src/server/error.rs` (four codes, constructors, tests)
- Create: `crates/nigel-core/src/server/mode.rs`
- Modify: `crates/nigel-core/src/server/state.rs` (mode cell, config, clock, constructors)
- Modify: `crates/nigel-core/src/server/auth.rs` (`auth_redirect` and the local session check read `local_token()`)
- Modify: `crates/nigel-core/src/server/mod.rs` (`pub mod mode;`, `serve` builds `AppState::for_serve`)

**Interfaces:**
- Consumes: `config::ServerConfig` (Task 3), `events` (Task 3), `identity::users::has_users`.
- Produces: `mode::{Mode, derive, REFRESH_INTERVAL}`; `AppState::{for_serve, mode, local_token, refresh_mode, refresh_mode_throttled, now, config}`; `#[cfg(any(test, feature = "testutil"))] AppState::with_clock`; the `ApiError` constructors listed in Step 3.

- [ ] **Step 1: Write the failing error tests**

In `server/error.rs` `mod tests` (create the module if there is none at the bottom of the file):

```rust
    #[test]
    fn the_sign_in_codes_carry_their_statuses_and_details() {
        use axum::http::StatusCode;
        let cases = [
            (ApiError::invalid_credentials(), StatusCode::UNAUTHORIZED, "invalid_credentials"),
            (ApiError::login_code_mismatch(), StatusCode::UNAUTHORIZED, "invalid_credentials"),
            (ApiError::totp_required(), StatusCode::UNAUTHORIZED, "totp_required"),
            (ApiError::password_change_required(), StatusCode::FORBIDDEN, "password_change_required"),
            (ApiError::too_many_attempts(1500), StatusCode::TOO_MANY_REQUESTS, "too_many_attempts"),
            (ApiError::unauthorized_shared(), StatusCode::UNAUTHORIZED, "unauthorized"),
            (ApiError::origin_required(), StatusCode::FORBIDDEN, "forbidden"),
            (ApiError::forbidden_role(crate::identity::Role::Admin), StatusCode::FORBIDDEN, "forbidden"),
            (ApiError::local_mode(), StatusCode::CONFLICT, "conflict"),
        ];
        for (error, status, code) in cases {
            assert_eq!(error.code().status(), status, "{code}");
            assert_eq!(error.code().as_str(), code);
        }
        assert_eq!(
            ApiError::invalid_credentials().message(),
            "That email and password don't match."
        );
        assert_eq!(
            ApiError::login_code_mismatch().message(),
            "That code didn't match. Check the time on your device."
        );
        assert_eq!(
            ApiError::too_many_attempts(1500).details,
            Some(serde_json::json!({ "retryAfterMs": 1500 }))
        );
        assert_eq!(
            ApiError::unauthorized_shared().details,
            Some(serde_json::json!({ "mode": "shared" }))
        );
        assert_eq!(
            ApiError::forbidden_role(crate::identity::Role::Admin).details,
            Some(serde_json::json!({ "reason": "role", "required": "admin" }))
        );
        assert_eq!(
            ApiError::local_mode().details,
            Some(serde_json::json!({ "reason": "local_mode" }))
        );
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server::error -- --test-threads=1`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the codes and constructors**

Add to `ApiErrorCode`: `InvalidCredentials`, `TotpRequired`, `PasswordChangeRequired`, `TooManyAttempts`. In `status()`: `Self::InvalidCredentials | Self::TotpRequired => StatusCode::UNAUTHORIZED`, `Self::PasswordChangeRequired => StatusCode::FORBIDDEN`, `Self::TooManyAttempts => StatusCode::TOO_MANY_REQUESTS`. In `as_str()`: `"invalid_credentials"`, `"totp_required"`, `"password_change_required"`, `"too_many_attempts"`.

Add to `impl ApiError`:

```rust
    /// A shared-mode `401`: `details.mode` tells a client to show its sign-in
    /// screen rather than "open the link again".
    pub fn unauthorized_shared() -> Self {
        Self::new(ApiErrorCode::Unauthorized, "Not signed in.")
            .with_details(serde_json::json!({ "mode": "shared" }))
    }

    pub fn invalid_credentials() -> Self {
        Self::new(
            ApiErrorCode::InvalidCredentials,
            "That email and password don't match.",
        )
    }

    /// A wrong second-factor code at sign-in. Answered only after the
    /// password was proven, which `totp_required` had already revealed.
    pub fn login_code_mismatch() -> Self {
        Self::new(
            ApiErrorCode::InvalidCredentials,
            "That code didn't match. Check the time on your device.",
        )
    }

    pub fn totp_required() -> Self {
        Self::new(
            ApiErrorCode::TotpRequired,
            "Enter the six-digit code from your authenticator app.",
        )
    }

    pub fn totp_unavailable() -> Self {
        Self::new(
            ApiErrorCode::Internal,
            "Two-step verification can't be checked on this server. Ask an admin to turn it off for you.",
        )
        .with_details(serde_json::json!({ "reason": "totp_unavailable" }))
    }

    pub fn password_change_required() -> Self {
        Self::new(
            ApiErrorCode::PasswordChangeRequired,
            "Choose a new password before you continue.",
        )
    }

    pub fn too_many_attempts(retry_after_ms: u128) -> Self {
        Self::new(
            ApiErrorCode::TooManyAttempts,
            "Too many attempts. Try again later.",
        )
        .with_details(serde_json::json!({ "retryAfterMs": retry_after_ms }))
    }

    /// A cookie-authenticated change, or a browser sign-in, that carried no
    /// `Origin`.
    pub fn origin_required() -> Self {
        Self::new(
            ApiErrorCode::Forbidden,
            "Request rejected: it did not say which page sent it.",
        )
        .with_details(serde_json::json!({ "reason": "origin_required" }))
    }

    pub fn forbidden_role(required: crate::identity::Role) -> Self {
        Self::new(ApiErrorCode::Forbidden, "Your role doesn't allow this.")
            .with_details(serde_json::json!({ "reason": "role", "required": required.as_str() }))
    }

    /// An account or user operation on a server with no users.
    pub fn local_mode() -> Self {
        Self::conflict(
            "This server has no users, so there is no account to sign in to.",
            serde_json::json!({ "reason": "local_mode" }),
        )
    }

    /// The signed-in user's current password was wrong on an account route.
    pub fn wrong_password() -> Self {
        Self::bad_request("Your current password is not right.")
            .with_details(serde_json::json!({ "reason": "wrong_password" }))
    }

    /// A wrong code on an account route (confirming or removing two-step).
    pub fn code_mismatch() -> Self {
        Self::bad_request("That code didn't match. Check the time on your device.")
            .with_details(serde_json::json!({ "reason": "wrong_code" }))
    }
```

`details` is a private field; the test lives inside `error.rs`, so it can read it.

- [ ] **Step 4: Run the error tests**

Run: `cargo test -p nigel-core server::error -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Write the failing mode tests**

Create `crates/nigel-core/src/server/mode.rs` with the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::users::{create_user, NewUser};
    use crate::identity::Role;
    use crate::server::testutil::{encrypt, temp_db, PASSWORD};

    fn add_admin(db_path: &Path) {
        let conn = crate::db::get_connection(db_path).unwrap();
        create_user(
            &conn,
            &NewUser {
                email: "pat@example.com",
                display_name: "Pat Example",
                role: Role::Admin,
            },
            crate::clock::now_utc(),
        )
        .unwrap();
    }

    #[test]
    fn books_without_users_are_local_and_with_users_shared() {
        let (_dir, db_path) = temp_db();
        assert_eq!(derive(&db_path).unwrap(), Mode::Local);
        add_admin(&db_path);
        assert_eq!(derive(&db_path).unwrap(), Mode::Shared);
    }

    #[test]
    fn absent_or_empty_books_are_local() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nigel.db");
        assert_eq!(derive(&db_path).unwrap(), Mode::Local);
        std::fs::write(&db_path, b"").unwrap();
        assert_eq!(derive(&db_path).unwrap(), Mode::Local);
    }

    #[test]
    fn locked_books_with_users_are_local_until_unlocked() {
        let (_dir, db_path) = temp_db();
        add_admin(&db_path);
        encrypt(&db_path);
        crate::db::set_db_password_for_tests(None);
        assert_eq!(derive(&db_path).unwrap(), Mode::Local);
        crate::db::set_db_password_for_tests(Some(zeroize::Zeroizing::new(PASSWORD.to_string())));
        assert_eq!(derive(&db_path).unwrap(), Mode::Shared);
        crate::db::set_db_password_for_tests(None);
    }

    #[test]
    fn the_cell_moves_local_to_shared_once_and_drops_the_token() {
        let cell = ModeCell::local(ModePolicy::Derived, SessionToken::from("ab".repeat(32)));
        assert_eq!(cell.mode(), Mode::Local);
        assert!(cell.local_token().is_some());
        assert!(cell.enter_shared());
        assert_eq!(cell.mode(), Mode::Shared);
        assert!(cell.local_token().is_none());
        assert!(!cell.enter_shared(), "already shared");
    }

    #[test]
    fn a_pinned_cell_never_leaves_local() {
        let cell = ModeCell::local(ModePolicy::PinnedLocal, SessionToken::from("ab".repeat(32)));
        assert!(!cell.enter_shared());
        assert_eq!(cell.mode(), Mode::Local);
        assert!(cell.local_token().is_some());
    }

    #[test]
    fn refreshes_are_claimed_at_most_once_per_interval() {
        let cell = ModeCell::local(ModePolicy::Derived, SessionToken::from("ab".repeat(32)));
        let t0 = std::time::Instant::now();
        assert!(cell.claim_refresh(t0));
        assert!(!cell.claim_refresh(t0 + REFRESH_INTERVAL / 2));
        assert!(cell.claim_refresh(t0 + REFRESH_INTERVAL));
    }
}
```

`crate::db::set_db_password_for_tests` takes `Option<Zeroizing<String>>`. Add `pub mod mode;` to `server/mod.rs`.

- [ ] **Step 6: Run to verify failure**

Run: `cargo test -p nigel-core server::mode -- --test-threads=1`
Expected: FAIL to compile.

- [ ] **Step 7: Implement `mode.rs`**

```rust
//! Local or shared: derived from the books on every reading, never stored or
//! configured. Within one server run the mode only ever moves local → shared,
//! so a stale reading is always the one that accepts less.

use std::path::Path;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use super::secret::SessionToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Local,
    Shared,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Local => "local",
            Mode::Shared => "shared",
        }
    }
}

/// Whether a server may leave local mode. Every `AppState::new` caller — the
/// desktop shells and the tests — is pinned: its requests run as the local
/// owner whatever the books hold. Only `nigel serve` derives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModePolicy {
    PinnedLocal,
    Derived,
}

/// A public-host request against a local-mode server re-derives the mode, at
/// most this often, so traffic through a tunnel costs one users query per
/// interval rather than one per request.
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

enum Current {
    Local(SessionToken),
    Shared,
}

pub(crate) struct ModeCell {
    policy: ModePolicy,
    current: RwLock<Current>,
    last_refresh: Mutex<Option<Instant>>,
}

impl std::fmt::Debug for ModeCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModeCell")
            .field("policy", &self.policy)
            .field("mode", &self.mode())
            .finish()
    }
}

impl ModeCell {
    pub(crate) fn local(policy: ModePolicy, token: SessionToken) -> Self {
        Self {
            policy,
            current: RwLock::new(Current::Local(token)),
            last_refresh: Mutex::new(None),
        }
    }

    pub(crate) fn shared() -> Self {
        Self {
            policy: ModePolicy::Derived,
            current: RwLock::new(Current::Shared),
            last_refresh: Mutex::new(None),
        }
    }

    pub(crate) fn policy(&self) -> ModePolicy {
        self.policy
    }

    pub(crate) fn mode(&self) -> Mode {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        match *self.current.read().unwrap() {
            Current::Local(_) => Mode::Local,
            Current::Shared => Mode::Shared,
        }
    }

    pub(crate) fn local_token(&self) -> Option<SessionToken> {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        match &*self.current.read().unwrap() {
            Current::Local(token) => Some(token.clone()),
            Current::Shared => None,
        }
    }

    /// Move to shared mode and drop the token. True when this call changed
    /// the mode; a pinned cell never changes.
    pub(crate) fn enter_shared(&self) -> bool {
        if self.policy == ModePolicy::PinnedLocal {
            return false;
        }
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        let mut current = self.current.write().unwrap();
        if matches!(*current, Current::Shared) {
            return false;
        }
        *current = Current::Shared;
        true
    }

    /// Claim the right to re-derive at `now`: false when a refresh was
    /// claimed less than `REFRESH_INTERVAL` before it.
    pub(crate) fn claim_refresh(&self, now: Instant) -> bool {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut last = self.last_refresh.lock().unwrap();
        if last.is_some_and(|at| now.duration_since(at) < REFRESH_INTERVAL) {
            return false;
        }
        *last = Some(now);
        true
    }
}

/// The mode the books at `db_path` call for: shared when they exist, this
/// process can read them, and they have at least one user. Absent, empty and
/// locked books are local. An error is the caller's to read as local.
pub fn derive(db_path: &Path) -> crate::error::Result<Mode> {
    if !super::routes::status::initialized(db_path) {
        return Ok(Mode::Local);
    }
    if crate::db::get_db_password().is_none() && crate::db::is_encrypted(db_path)? {
        return Ok(Mode::Local);
    }
    let conn = crate::db::get_connection(db_path)?;
    Ok(if crate::identity::users::has_users(&conn)? {
        Mode::Shared
    } else {
        Mode::Local
    })
}
```

- [ ] **Step 8: Rework `AppState`**

In `server/state.rs`, replace the `session_token` field with:

```rust
    /// Local mode with its token, or shared mode; see `mode.rs`.
    mode: Arc<super::mode::ModeCell>,
    /// How this server is reached: its public address and client-IP header.
    pub config: Arc<super::config::ServerConfig>,
    /// The instant every session write stamps and every expiry is judged
    /// against. The wall clock in production; tests pin it.
    clock: StateClock,
```

Add:

```rust
type ClockFn = dyn Fn() -> chrono::DateTime<chrono::Utc> + Send + Sync;

#[derive(Clone)]
struct StateClock(Arc<ClockFn>);

impl std::fmt::Debug for StateClock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StateClock")
    }
}
```

Constructors and methods:

```rust
impl AppState {
    /// A state pinned to local mode, with `session_token` as its token: the
    /// desktop shells' state, and every test that does not need shared mode.
    pub fn new(db_path: PathBuf, session_token: String) -> Self {
        Self::with_mode(
            db_path,
            super::mode::ModeCell::local(
                super::mode::ModePolicy::PinnedLocal,
                SessionToken::from(session_token),
            ),
            super::config::ServerConfig::default(),
        )
    }

    /// The state `nigel serve` runs: its mode derived (and re-derived) from
    /// the books. A token is minted only for local mode.
    pub fn for_serve(
        db_path: PathBuf,
        mode: super::mode::Mode,
        config: super::config::ServerConfig,
    ) -> Self {
        let cell = match mode {
            super::mode::Mode::Local => super::mode::ModeCell::local(
                super::mode::ModePolicy::Derived,
                SessionToken::from(super::auth::generate_token()),
            ),
            super::mode::Mode::Shared => super::mode::ModeCell::shared(),
        };
        Self::with_mode(db_path, cell, config)
    }

    fn with_mode(
        db_path: PathBuf,
        mode: super::mode::ModeCell,
        config: super::config::ServerConfig,
    ) -> Self {
        Self {
            db_path: Arc::new(RwLock::new(db_path)),
            mode: Arc::new(mode),
            config: Arc::new(config),
            clock: StateClock(Arc::new(crate::clock::now_utc)),
            features: Features::detect(),
            unlock: Arc::new(UnlockGate::default()),
            db_gate: Arc::new(TokioRwLock::new(())),
            update_available: Arc::new(RwLock::new(None)),
        }
    }

    /// Pin the clock, for tests of expiry and renewal.
    #[cfg(any(test, feature = "testutil"))]
    pub fn with_clock(
        mut self,
        clock: impl Fn() -> chrono::DateTime<chrono::Utc> + Send + Sync + 'static,
    ) -> Self {
        self.clock = StateClock(Arc::new(clock));
        self
    }

    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        (self.clock.0)()
    }

    pub fn mode(&self) -> super::mode::Mode {
        self.mode.mode()
    }

    /// The local-mode token, or `None` once the server is shared.
    pub fn local_token(&self) -> Option<SessionToken> {
        self.mode.local_token()
    }

    /// Re-derive the mode from the books and move to shared if they call for
    /// it. Never moves back. Pinned states answer local without reading.
    pub async fn refresh_mode(&self) -> super::mode::Mode {
        if self.mode.policy() == super::mode::ModePolicy::PinnedLocal
            || self.mode() == super::mode::Mode::Shared
        {
            return self.mode();
        }
        let gate = self.db_gate.clone().read_owned().await;
        let db_path = self.db_path();
        let derived =
            super::routes::spawn_blocking_holding(gate, move || super::mode::derive(&db_path))
                .await;
        if let Ok((_gate, Ok(super::mode::Mode::Shared))) = derived {
            if self.mode.enter_shared() {
                super::events::emit(&super::events::AuthEvent::new(
                    super::events::EventKind::SharedModeBegan,
                ));
            }
        }
        self.mode()
    }

    /// [`Self::refresh_mode`], at most once per `mode::REFRESH_INTERVAL`.
    pub async fn refresh_mode_throttled(&self) -> super::mode::Mode {
        if self.mode() == super::mode::Mode::Shared
            || !self.mode.claim_refresh(std::time::Instant::now())
        {
            return self.mode();
        }
        self.refresh_mode().await
    }
}
```

Keep the existing `update_available`, `db_path`, `data_dir`, `set_db_path`, `is_locked` methods. Remove the `SessionToken` field's old doc. In the state test `a_derived_debug_of_the_state_carries_no_token`, change the last line to `assert_eq!(state.local_token().unwrap().expose(), token);`, and add:

```rust
    #[test]
    fn a_new_state_is_pinned_local_and_a_shared_serve_state_has_no_token() {
        let state = AppState::new(PathBuf::from("/books/acme/nigel.db"), "ab".repeat(32));
        assert_eq!(state.mode(), super::super::mode::Mode::Local);
        assert!(state.local_token().is_some());

        let shared = AppState::for_serve(
            PathBuf::from("/books/acme/nigel.db"),
            super::super::mode::Mode::Shared,
            Default::default(),
        );
        assert_eq!(shared.mode(), super::super::mode::Mode::Shared);
        assert!(shared.local_token().is_none());

        let local = AppState::for_serve(
            PathBuf::from("/books/acme/nigel.db"),
            super::super::mode::Mode::Local,
            Default::default(),
        );
        assert_eq!(local.local_token().unwrap().expose().len(), 64);
    }
```

- [ ] **Step 9: Point the token readers at `local_token()`**

In `server/auth.rs`:

```rust
pub async fn session_guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let Some(expected) = state.local_token() else {
        return ApiError::unauthorized_shared().into_response();
    };
    let authorized = req
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|cookies| cookie_value(cookies, SESSION_COOKIE))
        .any(|token| tokens_match(token, expected.expose()));
    if authorized {
        next.run(req).await
    } else {
        ApiError::unauthorized().into_response()
    }
}
```

(Task 6 replaces this function; this keeps the build green in between.) In `auth_redirect`:

```rust
    let Some(expected) = state.local_token() else {
        return ApiError::unauthorized_shared().into_response();
    };
    let token = query.token.unwrap_or_default();
    if !tokens_match(&token, expected.expose()) {
        return ApiError::unauthorized().into_response();
    }
```

In `server/mod.rs` `serve`, build the state as `nigel serve` will (Task 11 finishes this function):

```rust
    let mode = mode::derive(&db_path).unwrap_or(mode::Mode::Local);
    let state = AppState::for_serve(db_path, mode, config::ServerConfig::default());
```

and print the token from `state.local_token()` (`if let Some(token) = state.local_token() { … }`, printing the existing four lines; otherwise print `Nigel is serving on http://127.0.0.1:{port} to signed-in users.`).

- [ ] **Step 10: Add the `/auth` shared-mode test**

In `server/mod.rs` `mod tests`:

```rust
    #[tokio::test]
    async fn the_token_link_is_refused_once_the_server_is_shared() {
        let state = AppState::for_serve(
            PathBuf::from("/nonexistent/nigel.db"),
            mode::Mode::Shared,
            Default::default(),
        );
        let response = build_router(state)
            .oneshot(get_request("/auth?token=deadbeef").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert_eq!(json_body(response).await["error"]["details"]["mode"], "shared");
    }
```

- [ ] **Step 11: Run the server suite**

Run: `cargo test -p nigel-core server:: -- --test-threads=1`
Expected: PASS, including every existing test (they build `AppState::new`, pinned local).

- [ ] **Step 12: Commit**

```bash
cargo fmt
git add crates/nigel-core/src/server
git commit -m "Derive the server's mode from the books

AppState holds local mode with its token, or shared mode with none, and
only ever moves local to shared. AppState::new stays pinned local for the
desktop shells; nigel serve derives. The token link answers 401 with
mode shared once a server is shared. Adds the sign-in error codes."
```

---
### Task 5: The mode-aware host guard, security headers and router split (security-critical)

Spec §1 (hostnames), §4.6 rules 2–3, §4.7, §11.1–11.2.

**Files:**
- Modify: `crates/nigel-core/src/server/auth.rs` (`served_host_guard`, `request_is_public`)
- Modify: `crates/nigel-core/src/server/mod.rs` (`security_headers` with state, `base_router`, `build_router`, `build_desktop_router`, connect info in `serve_with_shutdown`)
- Modify: `crates/nigel-core/src/server/testutil.rs` (shared-mode helpers)
- Create: `crates/nigel-core/src/server/shared_tests.rs` (`#[cfg(test)] mod shared_tests;` in `server/mod.rs`)

**Interfaces:**
- Consumes: `AppState::{mode, refresh_mode_throttled, config}` (Task 4), `PublicUrl::{matches_host, matches_origin}` (Task 3).
- Produces: `auth::served_host_guard`, `pub(crate) fn auth::request_is_public(&AppState, &HeaderMap) -> bool`; testutil `PUBLIC_URL`, `PUBLIC_HOST`, `public_config()`, `served_app(&Path, ServerConfig) -> (Router, AppState)`, `add_ready_user(&Path, &str, Role) -> User`, `READY_PASSWORD`, `session_for(&Path, i64, SessionKind, DateTime<Utc>) -> String`.

- [ ] **Step 1: Add the testutil helpers**

Append to `crates/nigel-core/src/server/testutil.rs`:

```rust
use crate::identity::sessions::{self, SessionGrant, SessionKind};
use crate::identity::users::{self, NewUser, User};
use crate::identity::Role;
use super::config::{PublicUrl, ServerConfig};
use super::mode::Mode;

pub const PUBLIC_URL: &str = "https://books.example.com";
pub const PUBLIC_HOST: &str = "books.example.com";
/// The password `add_ready_user` gives every user it creates.
pub const READY_PASSWORD: &str = "a long passphrase for tests";

/// A config with `server_public_url` set to [`PUBLIC_URL`].
pub fn public_config() -> ServerConfig {
    ServerConfig {
        public_url: Some(PublicUrl::parse(PUBLIC_URL).expect("public url")),
        client_ip_header: None,
    }
}

/// The router `nigel serve` would build over `db_path`, its mode derived the
/// way `serve` derives it, and the state behind it.
pub fn served_app(db_path: &Path, config: ServerConfig) -> (Router, AppState) {
    let mode = super::mode::derive(db_path).unwrap_or(Mode::Local);
    let state = AppState::for_serve(db_path.to_path_buf(), mode, config);
    (super::build_router(state.clone()), state)
}

/// A user who has already chosen [`READY_PASSWORD`], so `must_change_password`
/// is clear.
pub fn add_ready_user(db_path: &Path, email: &str, role: Role) -> User {
    let conn = crate::db::get_connection(db_path).expect("connection");
    let now = crate::clock::now_utc();
    let created = users::create_user(
        &conn,
        &NewUser {
            email,
            display_name: "Pat Example",
            role,
        },
        now,
    )
    .expect("create user");
    users::change_password(
        &conn,
        created.user.id,
        &created.temporary_password,
        READY_PASSWORD,
        None,
        now,
    )
    .expect("choose a password");
    users::get_user(&conn, created.user.id)
        .expect("read user")
        .expect("user exists")
}

/// A session for `user_id` issued at `now`, as a sign-in would issue it.
pub fn session_for(
    db_path: &Path,
    user_id: i64,
    kind: SessionKind,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    let conn = crate::db::get_connection(db_path).expect("connection");
    let issued = sessions::create_session(
        &conn,
        SessionGrant::assume_verified(user_id),
        kind,
        None,
        None,
        now,
    )
    .expect("create session");
    issued.token.to_string()
}

/// A request addressed to [`PUBLIC_HOST`].
pub fn public_request(method: &str, uri: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, PUBLIC_HOST)
}
```

(`users::change_password` returns `Result<(), PasswordChangeError>`; `.expect` works because `PasswordChangeError: Debug`.)

- [ ] **Step 2: Write the failing host-guard and header tests**

Create `crates/nigel-core/src/server/shared_tests.rs`:

```rust
//! Shared-mode behaviour of the served router, end to end through tower.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use tower::ServiceExt;

use super::testutil::*;
use crate::identity::Role;

fn status_of(response: &axum::response::Response) -> StatusCode {
    response.status()
}

#[tokio::test]
async fn in_local_mode_the_public_host_is_refused_and_loopback_is_not() {
    let (_dir, db_path) = temp_db();
    let (app, state) = served_app(&db_path, public_config());
    let token = state.local_token().unwrap().expose().to_string();

    let public = app
        .clone()
        .oneshot(
            public_request("GET", "/api/ping")
                .header(header::COOKIE, format!("nigel_session={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_of(&public), StatusCode::FORBIDDEN);

    let loopback = app.oneshot(session_get("/api/ping", &token)).await.unwrap();
    assert_eq!(status_of(&loopback), StatusCode::OK);
}

#[tokio::test]
async fn in_shared_mode_the_public_host_passes_and_no_other_name_does() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());

    let public = app
        .clone()
        .oneshot(public_request("GET", "/api/ping").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(public.status(), StatusCode::UNAUTHORIZED, "past the host guard");

    for host in ["other.example.com", "books.example.com.evil.com", "books.example.com:8443"] {
        let refused = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/ping")
                    .header(header::HOST, host)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(refused.status(), StatusCode::FORBIDDEN, "{host}");
    }
}

#[tokio::test]
async fn the_public_origin_is_trusted_whole_and_only_in_shared_mode() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    for (origin, expected) in [
        ("https://books.example.com", StatusCode::UNAUTHORIZED),
        ("http://localhost:5173", StatusCode::UNAUTHORIZED),
        ("http://books.example.com", StatusCode::FORBIDDEN),
        ("https://books.example.com:8443", StatusCode::FORBIDDEN),
        ("https://evil.example.com", StatusCode::FORBIDDEN),
    ] {
        let response = app
            .clone()
            .oneshot(
                public_request("GET", "/api/ping")
                    .header(header::ORIGIN, origin)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{origin}");
    }

    let (_dir2, local_db) = temp_db();
    let (local_app, state) = served_app(&local_db, public_config());
    let token = state.local_token().unwrap().expose().to_string();
    let response = local_app
        .oneshot(
            get_request("/api/ping")
                .header(header::COOKIE, format!("nigel_session={token}"))
                .header(header::ORIGIN, PUBLIC_URL)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN, "local mode ignores the public origin");
}

#[tokio::test]
async fn a_first_user_added_to_a_running_local_server_takes_effect_on_the_next_public_request() {
    let (_dir, db_path) = temp_db();
    let (app, state) = served_app(&db_path, public_config());
    assert_eq!(state.mode(), super::mode::Mode::Local);

    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let response = app
        .oneshot(public_request("GET", "/api/ping").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(state.mode(), super::mode::Mode::Shared);
    assert!(state.local_token().is_none(), "the token is dropped");
}

#[tokio::test]
async fn hsts_and_no_store_go_where_they_belong() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());

    let public_api = app
        .clone()
        .oneshot(public_request("GET", "/api/ping").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(header_str(&public_api, header::STRICT_TRANSPORT_SECURITY), "max-age=31536000");
    assert_eq!(header_str(&public_api, header::CACHE_CONTROL), "no-store");
    assert_eq!(header_str(&public_api, header::REFERRER_POLICY), "same-origin");

    let loopback_shell = app
        .clone()
        .oneshot(get_request("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(header_str(&loopback_shell, header::STRICT_TRANSPORT_SECURITY), "");
    assert_eq!(header_str(&loopback_shell, header::REFERRER_POLICY), "same-origin");
    assert_ne!(
        header_str(&loopback_shell, header::CACHE_CONTROL),
        "",
        "the shell keeps its own caching header"
    );

    let loopback_api = app
        .oneshot(get_request("/api/ping").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(header_str(&loopback_api, header::STRICT_TRANSPORT_SECURITY), "");
    assert_eq!(header_str(&loopback_api, header::CACHE_CONTROL), "no-store");
}
```

Add `#[cfg(test)] mod shared_tests;` to `server/mod.rs`. (`loopback_shell`'s `Cache-Control` comes from `static_files.rs`; if the shell sets `no-store` itself, the `assert_ne!` still holds.)

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p nigel-core server::shared_tests -- --test-threads=1`
Expected: FAIL — the public host gets `403` in shared mode (the static loopback guard), and there is no HSTS/Referrer-Policy/Cache-Control.

- [ ] **Step 4: Implement the served host guard**

In `server/auth.rs`:

```rust
/// True when the request's `Host` is this server's `server_public_url`: the
/// request reached it over https through a proxy, so cookies are `Secure` and
/// the answer carries HSTS. Loopback is never public.
pub(crate) fn request_is_public(state: &AppState, headers: &axum::http::HeaderMap) -> bool {
    let Some(public) = state.config.public_url.as_ref() else {
        return false;
    };
    headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| public.matches_host(host))
}

/// Outermost guard of `nigel serve`. The `Host` must be loopback by any of
/// its names, or the `server_public_url` host while the server is shared; an
/// `Origin`, when sent, must be a loopback origin on any port or, while
/// shared, exactly the `server_public_url` origin. A public-host request to
/// a local-mode server re-derives the mode first, so a first user added with
/// the CLI takes effect without a restart.
pub async fn served_host_guard(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let loopback = TrustedOrigins::loopback();
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .map(|value| value.to_str().map(str::to_string));
    let Some(host) = host else {
        return ApiError::forbidden().into_response();
    };

    let public = state.config.public_url.clone();
    let via_public = public.as_ref().is_some_and(|p| p.matches_host(&host));
    if !via_public && !loopback.host_is_trusted(&host) {
        return ApiError::forbidden().into_response();
    }

    let mut mode = state.mode();
    if via_public && mode == super::mode::Mode::Local {
        mode = state.refresh_mode_throttled().await;
    }
    let shared = mode == super::mode::Mode::Shared;
    if via_public && !shared {
        return ApiError::forbidden().into_response();
    }

    if let Some(origin) = origin {
        let trusted = origin.is_ok_and(|origin| {
            loopback.origin_is_trusted(&origin)
                || (shared && public.as_ref().is_some_and(|p| p.matches_origin(&origin)))
        });
        if !trusted {
            return ApiError::forbidden().into_response();
        }
    }

    next.run(req).await
}
```

- [ ] **Step 5: Split the routers and extend the headers**

In `server/mod.rs`, replace `security_headers`, `finish_router`, `build_router` and `build_desktop_router`:

```rust
/// Headers on everything this server answers, refusals included, each a
/// **default** a handler may override (the invoice preview answers
/// `SAMEORIGIN` for its own `<iframe>`).
///
/// `nosniff` holds the browser to each response's stated type; `DENY` stops
/// framing; `Referrer-Policy: same-origin` keeps paths out of other sites'
/// logs. Every `/api` answer is `no-store`, so books data never sits in a
/// shared or disk cache. HSTS goes only on answers to the `server_public_url`
/// host, which a proxy serves over https; on loopback it would pin
/// `localhost` to https.
async fn security_headers(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let is_api = path == "/api" || path.starts_with("/api/");
    let public = auth::request_is_public(&state, req.headers());
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers
        .entry(header::X_CONTENT_TYPE_OPTIONS)
        .or_insert(HeaderValue::from_static("nosniff"));
    headers
        .entry(header::X_FRAME_OPTIONS)
        .or_insert(HeaderValue::from_static("DENY"));
    headers
        .entry(header::REFERRER_POLICY)
        .or_insert(HeaderValue::from_static("same-origin"));
    if is_api {
        headers
            .entry(header::CACHE_CONTROL)
            .or_insert(HeaderValue::from_static("no-store"));
    }
    if public {
        headers
            .entry(header::STRICT_TRANSPORT_SECURITY)
            .or_insert(HeaderValue::from_static("max-age=31536000"));
    }
    response
}

/// The `/api` nest, `/auth` and the static assets: what both routers serve.
/// `/auth` and the static assets carry no data and need no session.
fn base_router(api: Router<AppState>) -> Router<AppState> {
    Router::new()
        .nest("/api", api)
        .route("/auth", get(auth::auth_redirect))
        .fallback(static_files::static_handler)
}

/// The router `nigel serve` runs. Layers run outermost first: the blanket
/// headers, the mode-aware host guard, then — on `/api` only — the session
/// layer and the interim admin guard.
///
/// The session layer wraps the `/api` router itself rather than using
/// `route_layer`, so it also covers that router's 404 fallback.
pub fn build_router(state: AppState) -> Router {
    let api = routes::api_router(&state).layer(middleware::from_fn_with_state(
        state.clone(),
        auth::session_guard,
    ));
    base_router(api)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::served_host_guard,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), security_headers))
        .with_state(state)
}
```

and the desktop router (keep its existing doc comment):

```rust
#[cfg(feature = "desktop")]
pub fn build_desktop_router(state: AppState, trusted: auth::TrustedOrigins) -> Router {
    disable_ansi_output();

    let api = routes::api_router(&state);
    base_router(api)
        .layer(middleware::from_fn_with_state(trusted, auth::host_guard))
        .layer(middleware::from_fn_with_state(state.clone(), security_headers))
        .with_state(state)
}
```

Task 7 adds the interim admin guard and the desktop principal layer to these.

In `serve_with_shutdown`, serve with connect info so the session layer and limiter can see the TCP peer:

```rust
    axum::serve(
        listener,
        build_router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await?;
```

- [ ] **Step 6: Run the server suite**

Run: `cargo test -p nigel-core server:: -- --test-threads=1`
Expected: PASS — the new shared tests and every existing test (`every_response_carries_the_blanket_headers`, the desktop-router tests under `--features desktop`).

Also run: `cargo test -p nigel-core --features desktop server:: -- --test-threads=1`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
cargo fmt
git add crates/nigel-core/src/server
git commit -m "Accept the public host only in shared mode

nigel serve's host guard trusts loopback by its three names and, once the
books have users, the server_public_url host and origin, matched whole. A
public-host request to a local-mode server re-derives the mode first.
Adds Referrer-Policy, no-store on /api and HSTS on the public host."
```

---

### Task 6: `Principal` and the shared-mode session layer (security-critical)

Spec §1.2, §4.2, §4.6 rule 4, §12 (bearer).

**Files:**
- Create: `crates/nigel-core/src/server/principal.rs`
- Modify: `crates/nigel-core/src/server/auth.rs` (`session_guard`, cookie helpers, `LOGIN_PATH`, `MUST_CHANGE_ALLOWED`)
- Modify: `crates/nigel-core/src/server/mod.rs` (`pub mod principal;`)
- Modify: `crates/nigel-core/src/server/shared_tests.rs`

**Interfaces:**
- Consumes: `sessions::{find_session, renew_session, SessionKind, ActiveSession}`, `AppState::now`, `request_is_public` (Task 5).
- Produces: `principal::{Principal, SessionUser}` (with `FromRequestParts`), `auth::{session_cookie, expired_cookie, LOGIN_PATH}`.

- [ ] **Step 1: Write the failing session-layer tests**

Append to `shared_tests.rs`:

```rust
use chrono::{DateTime, Duration, Utc};
use std::sync::{Arc, Mutex};

use crate::identity::sessions::SessionKind;
use super::state::AppState;

fn t0() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&Utc)
}

/// A served router over `db_path` whose clock reads whatever `clock` holds.
fn served_at(
    db_path: &std::path::Path,
    clock: Arc<Mutex<DateTime<Utc>>>,
) -> (axum::Router, AppState) {
    let mode = super::mode::derive(db_path).unwrap();
    let state = AppState::for_serve(db_path.to_path_buf(), mode, public_config())
        .with_clock(move || *clock.lock().unwrap());
    (super::build_router(state.clone()), state)
}

fn cookie_get(uri: &str, token: &str) -> Request<Body> {
    public_request("GET", uri)
        .header(header::COOKIE, format!("nigel_session={token}"))
        .body(Body::empty())
        .unwrap()
}

fn bearer_get(uri: &str, token: &str) -> Request<Body> {
    public_request("GET", uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn a_cookie_or_bearer_session_signs_a_request_in() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let cookie = session_for(&db_path, pat.id, SessionKind::Cookie, crate::clock::now_utc());
    let bearer = session_for(&db_path, pat.id, SessionKind::Bearer, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());

    for request in [cookie_get("/api/ping", &cookie), bearer_get("/api/ping", &bearer)] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let none = app
        .clone()
        .oneshot(public_request("GET", "/api/ping").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(none.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(none).await["error"]["details"]["mode"], "shared");

    let unknown = app.oneshot(cookie_get("/api/ping", &"0".repeat(64))).await.unwrap();
    assert_eq!(unknown.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_session_answers_only_on_its_own_transport() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let cookie = session_for(&db_path, pat.id, SessionKind::Cookie, crate::clock::now_utc());
    let bearer = session_for(&db_path, pat.id, SessionKind::Bearer, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());

    for request in [bearer_get("/api/ping", &cookie), cookie_get("/api/ping", &bearer)] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    let malformed = app
        .oneshot(
            public_request("GET", "/api/ping")
                .header(header::AUTHORIZATION, "Basic cGF0OnBhc3M=")
                .header(header::COOKIE, format!("nigel_session={cookie}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::UNAUTHORIZED, "no fallback to the cookie");
}

#[tokio::test]
async fn the_old_token_cookie_is_refused_once_shared() {
    let (_dir, db_path) = temp_db();
    let (app, state) = served_app(&db_path, public_config());
    let token = state.local_token().unwrap().expose().to_string();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    assert_eq!(state.refresh_mode().await, super::mode::Mode::Shared);

    let response = app.oneshot(session_get("/api/ping", &token)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(response).await["error"]["details"]["mode"], "shared");
}

#[tokio::test]
async fn a_cookie_write_needs_an_origin_and_a_bearer_write_does_not() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let cookie = session_for(&db_path, pat.id, SessionKind::Cookie, crate::clock::now_utc());
    let bearer = session_for(&db_path, pat.id, SessionKind::Bearer, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());
    let body = r#"{"name":"Acme Checking","accountType":"checking"}"#;

    let no_origin = app
        .clone()
        .oneshot(
            public_request("POST", "/api/accounts")
                .header(header::COOKIE, format!("nigel_session={cookie}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_origin.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(no_origin).await["error"]["details"]["reason"], "origin_required");

    let with_origin = app
        .clone()
        .oneshot(
            public_request("POST", "/api/accounts")
                .header(header::COOKIE, format!("nigel_session={cookie}"))
                .header(header::ORIGIN, PUBLIC_URL)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(with_origin.status(), StatusCode::FORBIDDEN);
    assert_ne!(with_origin.status(), StatusCode::UNAUTHORIZED);

    let via_bearer = app
        .oneshot(
            public_request("POST", "/api/accounts")
                .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"name":"Acme Card","accountType":"checking"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(via_bearer.status(), StatusCode::FORBIDDEN);
    assert_ne!(via_bearer.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_user_who_must_change_their_password_reaches_only_the_allowlist() {
    let (_dir, db_path) = temp_db();
    let conn = crate::db::get_connection(&db_path).unwrap();
    let created = crate::identity::users::create_user(
        &conn,
        &crate::identity::users::NewUser {
            email: "pat@example.com",
            display_name: "Pat Example",
            role: Role::Admin,
        },
        crate::clock::now_utc(),
    )
    .unwrap();
    let token = session_for(&db_path, created.user.id, SessionKind::Bearer, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());

    for uri in ["/api/ping", "/api/status"] {
        let response = app.clone().oneshot(bearer_get(uri, &token)).await.unwrap();
        assert_ne!(response.status(), StatusCode::FORBIDDEN, "{uri}");
    }
    let refused = app.oneshot(bearer_get("/api/accounts", &token)).await.unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(refused).await["error"]["code"], "password_change_required");
}

#[tokio::test]
async fn renewal_slides_thirty_days_hourly_and_stops_at_ninety() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(&db_path, pat.id, SessionKind::Cookie, t0());
    let clock = Arc::new(Mutex::new(t0()));
    let (app, _state) = served_at(&db_path, clock.clone());
    let set_cookie_at = |days: i64| {
        *clock.lock().unwrap() = t0() + Duration::days(days);
        let app = app.clone();
        let token = token.clone();
        async move {
            let response = app.oneshot(cookie_get("/api/ping", &token)).await.unwrap();
            (response.status(), header_str(&response, header::SET_COOKIE))
        }
    };

    let (status, cookie) = set_cookie_at(0).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cookie, "", "used within the hour: no renewal");

    let (_, cookie) = set_cookie_at(20).await;
    assert!(cookie.contains("Max-Age=2592000"), "{cookie}");
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"), "{cookie}");
    assert!(cookie.contains("Secure"), "public host: {cookie}");

    set_cookie_at(45).await;
    let (_, cookie) = set_cookie_at(70).await;
    assert!(cookie.contains(&format!("Max-Age={}", 20 * 86_400)), "capped at 90 days: {cookie}");

    let (status, _) = set_cookie_at(90).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_idle_session_expires_after_thirty_days() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(&db_path, pat.id, SessionKind::Bearer, t0());
    let clock = Arc::new(Mutex::new(t0() + Duration::days(30)));
    let (app, _state) = served_at(&db_path, clock);
    let response = app.oneshot(bearer_get("/api/ping", &token)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn disabling_a_user_ends_their_next_request() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let sam = add_ready_user(&db_path, "sam@example.com", Role::Admin);
    let token = session_for(&db_path, sam.id, SessionKind::Bearer, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());
    assert_eq!(app.clone().oneshot(bearer_get("/api/ping", &token)).await.unwrap().status(), StatusCode::OK);

    let conn = crate::db::get_connection(&db_path).unwrap();
    crate::identity::users::disable_user(&conn, sam.id, crate::clock::now_utc()).unwrap();
    assert_eq!(app.oneshot(bearer_get("/api/ping", &token)).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}
```

The bodies are the shape `WRITE_ROUTES` in `testutil.rs` uses for `POST /api/accounts`. The assertions are only that a request gets past authentication, so the answer is checked as "not 401 and not 403".

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server::shared_tests -- --test-threads=1`
Expected: FAIL — shared mode answers every request `401` (Task 4's interim guard has no shared branch).

- [ ] **Step 3: Implement `principal.rs`**

```rust
//! Who a request runs as. The session layer (or the desktop router) inserts
//! one on every `/api` request; handlers take it as an extractor.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use super::error::ApiError;
use crate::identity::sessions::{ActiveSession, SessionKind};
use crate::identity::Role;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionUser {
    pub id: i64,
    pub uid: String,
    pub email: String,
    pub role: Role,
    pub session_id: i64,
    pub kind: SessionKind,
    pub must_change_password: bool,
}

impl From<&ActiveSession> for SessionUser {
    fn from(session: &ActiveSession) -> Self {
        Self {
            id: session.user_id,
            uid: session.uid.clone(),
            email: session.email.clone(),
            role: session.role,
            session_id: session.id,
            kind: session.kind,
            must_change_password: session.must_change_password,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Principal {
    /// CLI-equivalent authority: a local-mode token session, or the desktop
    /// transport.
    LocalOwner,
    User(SessionUser),
}

impl Principal {
    /// True for the local owner, and for a user whose role is at least `role`.
    pub fn satisfies(&self, role: Role) -> bool {
        match self {
            Principal::LocalOwner => true,
            Principal::User(user) => user.role >= role,
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Principal {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Principal>()
            .cloned()
            .ok_or_else(|| ApiError::internal("This request reached a handler with no principal."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: Role) -> Principal {
        Principal::User(SessionUser {
            id: 1,
            uid: "0".repeat(32),
            email: "pat@example.com".into(),
            role,
            session_id: 1,
            kind: SessionKind::Cookie,
            must_change_password: false,
        })
    }

    #[test]
    fn the_local_owner_satisfies_every_role_and_a_user_their_own_and_below() {
        for role in Role::ALL {
            assert!(Principal::LocalOwner.satisfies(role));
        }
        assert!(user(Role::Bookkeeper).satisfies(Role::ReadOnly));
        assert!(user(Role::Bookkeeper).satisfies(Role::Bookkeeper));
        assert!(!user(Role::Bookkeeper).satisfies(Role::Admin));
        assert!(!user(Role::ReadOnly).satisfies(Role::Bookkeeper));
    }
}
```

Add `pub mod principal;` to `server/mod.rs`.

- [ ] **Step 4: Implement the session layer**

In `server/auth.rs`, add imports (`axum::http::{HeaderMap, HeaderValue, Method}`, `zeroize::Zeroizing`, `crate::identity::sessions::{self, ActiveSession, SessionKind}`, `super::mode::Mode`, `super::principal::{Principal, SessionUser}`) and replace `session_guard`:

```rust
/// The one `/api` path that needs no session, as seen inside the nest.
pub const LOGIN_PATH: &str = "/auth/login";

/// What a user who must change a temporary password may still call.
const MUST_CHANGE_ALLOWED: [(Method, &str); 5] = [
    (Method::GET, "/auth/me"),
    (Method::GET, "/status"),
    (Method::GET, "/ping"),
    (Method::POST, "/account/password"),
    (Method::POST, "/auth/logout"),
];

/// `nigel_session=<token>; HttpOnly; SameSite=Strict; Path=/; Max-Age=<n>`,
/// with `; Secure` for a request to the public host.
pub fn session_cookie(token: &str, max_age_secs: i64, secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age_secs}{}",
        if secure { "; Secure" } else { "" }
    )
}

/// The cookie that ends a browser session.
pub fn expired_cookie(secure: bool) -> String {
    session_cookie("", 0, secure)
}

fn is_safe_method(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// The credential a request presents: `Authorization: Bearer` when that
/// header is present (a malformed one is `Err`, never a fall-back to the
/// cookie), otherwise the `nigel_session` cookie.
fn presented_credential(
    headers: &HeaderMap,
) -> Result<Option<(Zeroizing<String>, SessionKind)>, ()> {
    if let Some(value) = headers.get(header::AUTHORIZATION) {
        let value = value.to_str().map_err(|_| ())?;
        let scheme = value.get(..7).ok_or(())?;
        if !scheme.eq_ignore_ascii_case("bearer ") {
            return Err(());
        }
        let token = value[7..].trim();
        return Ok(Some((Zeroizing::new(token.to_string()), SessionKind::Bearer)));
    }
    Ok(headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|cookies| cookie_value(cookies, SESSION_COOKIE))
        .map(|token| (Zeroizing::new(token.to_string()), SessionKind::Cookie)))
}

fn seconds_until(expires_at: &str, now: chrono::DateTime<chrono::Utc>) -> Option<i64> {
    let at = chrono::DateTime::parse_from_rfc3339(expires_at)
        .ok()?
        .with_timezone(&chrono::Utc);
    Some((at - now).num_seconds().max(0))
}

/// Layered over `/api` only — `/auth` and the static assets carry no data.
///
/// Local mode: the per-run token cookie, as it has always been, and the
/// request runs as the local owner. Shared mode: a session looked up by the
/// hash of its token, joined to an enabled user, unexpired; the user's role
/// is read on every request. `POST /api/auth/login` passes without one.
pub async fn session_guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if req.uri().path() == LOGIN_PATH {
        return next.run(req).await;
    }
    match state.mode() {
        Mode::Local => local_session(state, req, next).await,
        Mode::Shared => shared_session(state, req, next).await,
    }
}

async fn local_session(state: AppState, mut req: Request, next: Next) -> Response {
    let Some(expected) = state.local_token() else {
        return ApiError::unauthorized_shared().into_response();
    };
    let authorized = req
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|cookies| cookie_value(cookies, SESSION_COOKIE))
        .any(|token| tokens_match(token, expected.expose()));
    if !authorized {
        return ApiError::unauthorized().into_response();
    }
    req.extensions_mut().insert(Principal::LocalOwner);
    next.run(req).await
}

async fn shared_session(state: AppState, mut req: Request, next: Next) -> Response {
    let Ok(Some((token, via))) = presented_credential(req.headers()) else {
        return ApiError::unauthorized_shared().into_response();
    };

    let now = state.now();
    let lookup = token.clone();
    let gate = state.db_gate.clone().read_owned().await;
    let db_path = state.db_path();
    let found = super::routes::spawn_blocking_holding(
        gate,
        move || -> crate::error::Result<Option<(ActiveSession, Option<String>)>> {
            let conn = crate::db::get_connection(&db_path)?;
            let Some(session) = sessions::find_session(&conn, &lookup, now)? else {
                return Ok(None);
            };
            let renewed = sessions::renew_session(&conn, &session, now)?;
            Ok(Some((session, renewed)))
        },
    )
    .await;
    let (session, renewed) = match found {
        Ok((_gate, Ok(Some(found)))) => found,
        Ok((_gate, Ok(None))) => return ApiError::unauthorized_shared().into_response(),
        Ok((_gate, Err(e))) => return ApiError::from(e).into_response(),
        Err(e) => return e.into_response(),
    };
    if session.kind != via {
        return ApiError::unauthorized_shared().into_response();
    }

    let method = req.method().clone();
    if via == SessionKind::Cookie
        && !is_safe_method(&method)
        && !req.headers().contains_key(header::ORIGIN)
    {
        return ApiError::origin_required().into_response();
    }
    if session.must_change_password {
        let path = req.uri().path();
        let allowed = MUST_CHANGE_ALLOWED
            .iter()
            .any(|(m, p)| *m == method && *p == path);
        if !allowed {
            return ApiError::password_change_required().into_response();
        }
    }

    let secure = request_is_public(&state, req.headers());
    req.extensions_mut()
        .insert(Principal::User(SessionUser::from(&session)));
    let mut response = next.run(req).await;

    if let (SessionKind::Cookie, Some(expires_at)) = (session.kind, renewed) {
        if !response.headers().contains_key(header::SET_COOKIE) {
            let cookie = seconds_until(&expires_at, now)
                .and_then(|max_age| HeaderValue::from_str(&session_cookie(&token, max_age, secure)).ok());
            if let Some(cookie) = cookie {
                response.headers_mut().append(header::SET_COOKIE, cookie);
            }
        }
    }
    response
}
```

The host guard has already refused any `Origin` that is present and untrusted, so this layer only needs to require that one is present.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p nigel-core server:: -- --test-threads=1`
Expected: PASS. Every pre-existing test still builds `AppState::new` (pinned local) and goes through `local_session`.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/nigel-core/src/server
git commit -m "Sign shared-mode requests in by session

The session layer reads a bearer token or the session cookie, looks up
its hashed session joined to an enabled user, renews it at most hourly
within the 90-day cap, and inserts the Principal. A cookie write needs an
Origin, a temporary password limits the user to the allowlist, and a
session answers only on the transport it was issued for."
```

---
### Task 7: The interim admin guard, the desktop principal, and status `mode` (security-critical)

Spec §1.1, §8.5 (`mode` in status), §17 ("until PR 3, every non-auth route needs admin"). Ruling 10.

**Files:**
- Create: `crates/nigel-core/src/server/authz.rs`
- Modify: `crates/nigel-core/src/server/mod.rs` (`pub mod authz;`, layer it on both routers)
- Modify: `crates/nigel-core/src/server/routes/status.rs` (`mode` field; refresh after unlock)
- Modify: `crates/nigel-core/src/server/routes/settings.rs` (refresh after a data-directory switch)
- Modify: `crates/nigel-core/src/server/shared_tests.rs`, `crates/nigel-core/src/server/mod.rs` tests

**Interfaces:**
- Consumes: `Principal` (Task 6), `auth::LOGIN_PATH`, `AppState::refresh_mode` (Task 4).
- Produces: `authz::interim_admin_guard`, `authz::as_local_owner`, `StatusResponse.mode`. **PR 3 deletes `interim_admin_guard` and `ANY_ROLE`, replacing them with `ROUTE_ROLES`/`role_guard`.**

- [ ] **Step 1: Write the failing tests**

Append to `shared_tests.rs`:

```rust
#[tokio::test]
async fn until_roles_land_a_non_admin_reaches_only_their_own_account() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let sam = add_ready_user(&db_path, "sam@example.com", Role::Bookkeeper);
    let token = session_for(&db_path, sam.id, SessionKind::Bearer, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());

    for uri in ["/api/status", "/api/accounts", "/api/reports/pnl", "/api/no-such-route"] {
        let response = app.clone().oneshot(bearer_get(uri, &token)).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{uri}");
        let json = json_body(response).await;
        assert_eq!(json["error"]["details"]["reason"], "role", "{uri}");
        assert_eq!(json["error"]["details"]["required"], "admin", "{uri}");
    }
    let ping = app.oneshot(bearer_get("/api/ping", &token)).await.unwrap();
    assert_eq!(ping.status(), StatusCode::OK);
}

#[tokio::test]
async fn an_admin_reaches_the_data_routes_and_status_reports_shared() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(&db_path, pat.id, SessionKind::Bearer, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());

    let accounts = app.clone().oneshot(bearer_get("/api/accounts", &token)).await.unwrap();
    assert_eq!(accounts.status(), StatusCode::OK);
    let status = app.oneshot(bearer_get("/api/status", &token)).await.unwrap();
    assert_eq!(json_body(status).await["mode"], "shared");
}

#[tokio::test]
async fn unlocking_books_that_have_users_ends_the_token_session() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    encrypt(&db_path);
    crate::db::set_db_password_for_tests(None);
    let (app, state) = served_app(&db_path, public_config());
    assert_eq!(state.mode(), super::mode::Mode::Local);
    let token = state.local_token().unwrap().expose().to_string();

    let unlocked = app
        .clone()
        .oneshot(session_post("/api/unlock", &token, &unlock_body(PASSWORD)))
        .await
        .unwrap();
    assert_eq!(unlocked.status(), StatusCode::OK);
    assert_eq!(state.mode(), super::mode::Mode::Shared);

    let next = app.oneshot(session_get("/api/ping", &token)).await.unwrap();
    assert_eq!(next.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(next).await["error"]["details"]["mode"], "shared");
    crate::db::set_db_password_for_tests(None);
}
```

In `server/mod.rs` `mod tests`, add:

```rust
    #[tokio::test]
    async fn status_reports_local_mode() {
        let (_dir, db_path) = temp_db();
        let (app, token) = app_for(&db_path);
        let response = app.oneshot(session_get("/api/status", &token)).await.unwrap();
        assert_eq!(json_body(response).await["mode"], "local");
    }

    #[cfg(feature = "desktop")]
    #[tokio::test]
    async fn the_desktop_router_runs_as_the_local_owner_even_over_books_with_users() {
        let (_dir, db_path) = temp_db();
        add_ready_user(&db_path, "pat@example.com", crate::identity::Role::Admin);
        let state = AppState::new(db_path.clone(), auth::generate_token());
        let router = build_desktop_router(
            state,
            auth::TrustedOrigins::exactly(vec!["localhost".to_string()]),
        );
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .header(header::HOST, "localhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["mode"], "local");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server:: -- --test-threads=1` and `cargo test -p nigel-core --features desktop server::tests::the_desktop_router_runs -- --test-threads=1`
Expected: FAIL — no `mode` in status; the bookkeeper reaches `/api/accounts`; unlock leaves the server local.

- [ ] **Step 3: Implement `authz.rs`**

```rust
//! Who may call what until the role table replaces this module's guard.
//!
//! Every `/api` route needs an admin except the few a signed-in user needs
//! for their own session and account, so a user who is not an admin can do
//! nothing with the books while roles are unfinished. The local owner — a
//! local-mode token session or the desktop transport — passes everything.

use axum::extract::Request;
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::auth::LOGIN_PATH;
use super::error::ApiError;
use super::principal::Principal;
use crate::identity::Role;

/// What any signed-in role may call, as seen inside the `/api` nest.
const ANY_ROLE: [(Method, &str); 7] = [
    (Method::GET, "/ping"),
    (Method::GET, "/auth/me"),
    (Method::POST, "/auth/logout"),
    (Method::POST, "/account/password"),
    (Method::POST, "/account/totp/setup"),
    (Method::POST, "/account/totp/confirm"),
    (Method::POST, "/account/totp/remove"),
];

pub async fn interim_admin_guard(req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path == LOGIN_PATH {
        return next.run(req).await;
    }
    let allowed = match req.extensions().get::<Principal>() {
        None => return ApiError::unauthorized().into_response(),
        Some(principal) => {
            principal.satisfies(Role::Admin)
                || ANY_ROLE.iter().any(|(m, p)| m == req.method() && *p == path)
        }
    };
    if allowed {
        next.run(req).await
    } else {
        ApiError::forbidden_role(Role::Admin).into_response()
    }
}

/// The desktop router's principal: it has no session layer, and its
/// requests run as the local owner whatever the books hold.
pub async fn as_local_owner(mut req: Request, next: Next) -> Response {
    req.extensions_mut().insert(Principal::LocalOwner);
    next.run(req).await
}
```

Add `pub mod authz;` to `server/mod.rs`. Layer it in `build_router`, **inside** the session layer:

```rust
    let api = routes::api_router(&state)
        .layer(middleware::from_fn(authz::interim_admin_guard))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::session_guard,
        ));
```

and in `build_desktop_router`:

```rust
    let api = routes::api_router(&state)
        .layer(middleware::from_fn(authz::interim_admin_guard))
        .layer(middleware::from_fn(authz::as_local_owner));
```

- [ ] **Step 4: Report the mode and refresh it**

In `routes/status.rs`, add to `StatusResponse` after `locked`:

```rust
    /// `local` or `shared`; a desktop shell is always `local`.
    mode: &'static str,
```

and set `mode: state.mode().as_str(),` in `current_status`. In `unlock`, inside `if unlocked {`, before the `return`:

```rust
        state.refresh_mode().await;
```

In `routes/settings.rs` `post_data_dir`, after the `switched?;` block closes and before `Ok(Json(current_status(&state).await?))`:

```rust
    state.refresh_mode().await;
```

- [ ] **Step 5: Run the suite (both feature sets)**

Run: `cargo test -p nigel-core server:: -- --test-threads=1 && cargo test -p nigel-core --features desktop server:: -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/nigel-core/src/server
git commit -m "Require an admin for every non-account route until roles land

A fail-closed interim guard lets any signed-in role reach only ping, its
own session and its own account; everything else needs an admin. The
desktop router runs as the local owner. Status reports the mode, and an
unlock or folder switch re-derives it."
```

---

### Task 8: The login rate limiter and client IP (security-critical)

Spec §4.4. Rulings 11–14. Review Focus 2 and 3.

**Files:**
- Create: `crates/nigel-core/src/server/ratelimit.rs`
- Modify: `crates/nigel-core/src/server/mod.rs` (`pub mod ratelimit;`)
- Modify: `crates/nigel-core/src/server/state.rs` (`pub limiter: Arc<LoginLimiter>`, initialized in `with_mode` as `Arc::new(LoginLimiter::default())`)

**Interfaces:**
- Consumes: `config::ClientIpHeader` (Task 3), `identity::users::normalize_email`.
- Produces: `LoginLimiter::{begin, succeeded, not_a_failure, failed}`, `Attempt`, `Bucket`, `client_ip`, `account_key`, constants `ACCOUNT_FAILURES`, `IP_FAILURES`, `WINDOW`, `LOCKOUT`, `MAX_KEYS`.

- [ ] **Step 1: Write the failing tests**

Create `ratelimit.rs` with the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    const PAT: &str = "pat@example.com";
    const IP: &str = "203.0.113.9";

    fn fail(limiter: &LoginLimiter, account: &str, ip: &str, at: Instant) -> Vec<Bucket> {
        limiter.failed(limiter.begin(account, ip, at).expect("admitted"))
    }

    #[test]
    fn five_failures_lock_an_account_for_fifteen_minutes() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..4 {
            assert!(fail(&limiter, PAT, IP, t0).is_empty(), "failure {i}");
        }
        assert_eq!(fail(&limiter, PAT, IP, t0), [Bucket::Account]);
        let wait = limiter.begin(PAT, "198.51.100.7", t0).unwrap_err();
        assert_eq!(wait, LOCKOUT);
        let later = t0 + Duration::from_secs(60);
        assert_eq!(limiter.begin(PAT, IP, later).unwrap_err(), LOCKOUT - Duration::from_secs(60));
        let after = t0 + LOCKOUT;
        let attempt = limiter.begin(PAT, IP, after).expect("the lockout has passed");
        limiter.failed(attempt);
        assert!(limiter.begin(PAT, IP, after).is_ok(), "the count started again");
    }

    #[test]
    fn a_success_resets_the_account_count() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..4 {
            fail(&limiter, PAT, IP, t0);
        }
        limiter.succeeded(limiter.begin(PAT, IP, t0).unwrap());
        for _ in 0..4 {
            assert!(fail(&limiter, PAT, IP, t0).is_empty());
        }
    }

    #[test]
    fn an_unknown_address_locks_like_a_known_one() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..5 {
            fail(&limiter, "nobody@example.com", IP, t0);
        }
        assert!(limiter.begin("nobody@example.com", "198.51.100.7", t0).is_err());
    }

    #[test]
    fn a_totp_required_answer_is_not_a_failure() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..4 {
            fail(&limiter, PAT, IP, t0);
        }
        for _ in 0..10 {
            limiter.not_a_failure(limiter.begin(PAT, IP, t0).unwrap());
        }
        assert_eq!(fail(&limiter, PAT, IP, t0), [Bucket::Account], "still the fifth failure");
    }

    #[test]
    fn an_account_count_lapses_after_fifteen_quiet_minutes() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..4 {
            fail(&limiter, PAT, IP, t0);
        }
        let later = t0 + WINDOW;
        for _ in 0..4 {
            assert!(fail(&limiter, PAT, IP, later).is_empty());
        }
    }

    #[test]
    fn twenty_failures_from_one_ip_in_the_window_lock_the_ip() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..19 {
            let account = format!("user{i}@example.com");
            assert!(fail(&limiter, &account, IP, t0 + Duration::from_secs(i)).is_empty());
        }
        let locks = fail(&limiter, "user19@example.com", IP, t0 + Duration::from_secs(19));
        assert_eq!(locks, [Bucket::Ip]);
        assert!(limiter.begin("fresh@example.com", IP, t0 + Duration::from_secs(20)).is_err());
        assert!(limiter.begin("fresh@example.com", "198.51.100.7", t0).is_ok());
    }

    #[test]
    fn the_ip_window_slides() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..19 {
            fail(&limiter, &format!("user{i}@example.com"), IP, t0);
        }
        let slid = t0 + WINDOW;
        assert!(fail(&limiter, "late@example.com", IP, slid).is_empty(), "the first 19 aged out");
    }

    #[test]
    fn concurrent_attempts_are_charged_before_they_are_checked() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        let held: Vec<Attempt> = (0..5).map(|_| limiter.begin(PAT, IP, t0).unwrap()).collect();
        assert!(limiter.begin(PAT, IP, t0).is_err(), "the sixth waits");
        drop(held);
    }

    #[test]
    fn memory_stays_bounded() {
        let limiter = LoginLimiter::default();
        let t0 = Instant::now();
        for i in 0..(MAX_KEYS + 50) {
            let ip = format!("10.0.{}.{}", i / 256, i % 256);
            fail(&limiter, &format!("user{i}@example.com"), &ip, t0);
        }
        let (accounts, clients) = limiter.sizes();
        assert!(accounts <= MAX_KEYS && clients <= MAX_KEYS, "{accounts} {clients}");
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn client_ip_trusts_the_header_only_from_loopback() {
        let loopback: SocketAddr = "127.0.0.1:50000".parse().unwrap();
        let remote: SocketAddr = "198.51.100.7:50000".parse().unwrap();
        let cf = Some(ClientIpHeader::CfConnectingIp);
        let xff = Some(ClientIpHeader::XForwardedFor);
        let forged = headers(&[("cf-connecting-ip", "203.0.113.9")]);

        assert_eq!(client_ip(Some(loopback), &forged, cf), "203.0.113.9");
        assert_eq!(client_ip(Some(remote), &forged, cf), "198.51.100.7");
        assert_eq!(client_ip(Some(loopback), &forged, None), "127.0.0.1");
        assert_eq!(
            client_ip(
                Some(loopback),
                &headers(&[("x-forwarded-for", "203.0.113.9, 198.51.100.20")]),
                xff
            ),
            "198.51.100.20"
        );
        assert_eq!(
            client_ip(
                Some(loopback),
                &headers(&[("x-forwarded-for", "203.0.113.9"), ("x-forwarded-for", "198.51.100.30")]),
                xff
            ),
            "198.51.100.30"
        );
        assert_eq!(
            client_ip(Some(loopback), &headers(&[("cf-connecting-ip", "not-an-ip")]), cf),
            "127.0.0.1"
        );
        assert_eq!(client_ip(None, &forged, cf), "unknown");
    }

    #[test]
    fn the_account_key_is_the_normalized_email_or_the_trimmed_input() {
        assert_eq!(account_key(" Pat@Example.com "), "pat@example.com");
        assert_eq!(account_key("  Not An Email "), "not an email");
        assert_eq!(account_key(&"a".repeat(1000)).chars().count(), 320);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server::ratelimit -- --test-threads=1`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `ratelimit.rs`**

```rust
//! Sign-in rate limits, per account and per client IP: in memory and
//! process-wide like `UnlockGate`, so a restart clears them.
//!
//! An attempt is charged as a failure when it is admitted, so parallel
//! guesses cannot all pass the check before any is counted; the caller then
//! settles it as a success, as not a failure, or leaves the charge standing.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::HeaderMap;

use super::config::ClientIpHeader;

pub const ACCOUNT_FAILURES: u32 = 5;
pub const IP_FAILURES: usize = 20;
pub const WINDOW: Duration = Duration::from_secs(15 * 60);
pub const LOCKOUT: Duration = Duration::from_secs(15 * 60);
/// Keys kept per bucket. Past it, idle entries go first, then the one whose
/// last failure is oldest.
pub const MAX_KEYS: usize = 10_000;
const ACCOUNT_KEY_MAX_CHARS: usize = 320;

/// Which limit an attempt tripped, for the lockout event line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Account,
    Ip,
}

impl Bucket {
    pub fn as_str(self) -> &'static str {
        match self {
            Bucket::Account => "account",
            Bucket::Ip => "ip",
        }
    }
}

#[derive(Debug, Default)]
struct Account {
    failures: u32,
    last_failure: Option<Instant>,
    locked_until: Option<Instant>,
}

impl Account {
    /// How long until this account admits an attempt; `None` when it does
    /// now. An expired lockout, or a count with no failure in the window,
    /// starts again from zero.
    fn wait(&mut self, now: Instant) -> Option<Duration> {
        if let Some(until) = self.locked_until {
            if now < until {
                return Some(until - now);
            }
            *self = Self::default();
        }
        if self.last_failure.is_some_and(|at| now.duration_since(at) >= WINDOW) {
            *self = Self::default();
        }
        None
    }

    fn idle(&self, now: Instant) -> bool {
        self.locked_until.is_none_or(|until| now >= until)
            && self.last_failure.is_none_or(|at| now.duration_since(at) >= WINDOW)
    }
}

#[derive(Debug, Default)]
struct Client {
    failures: VecDeque<Instant>,
    locked_until: Option<Instant>,
}

impl Client {
    fn prune(&mut self, now: Instant) {
        while self
            .failures
            .front()
            .is_some_and(|at| now.duration_since(*at) >= WINDOW)
        {
            self.failures.pop_front();
        }
    }

    fn wait(&mut self, now: Instant) -> Option<Duration> {
        if let Some(until) = self.locked_until {
            if now < until {
                return Some(until - now);
            }
            *self = Self::default();
        }
        None
    }

    fn idle(&mut self, now: Instant) -> bool {
        self.prune(now);
        self.failures.is_empty() && self.locked_until.is_none_or(|until| now >= until)
    }

    fn last_failure(&self) -> Option<Instant> {
        self.failures.back().copied()
    }
}

#[derive(Debug, Default)]
struct Buckets {
    accounts: HashMap<String, Account>,
    clients: HashMap<String, Client>,
}

impl Buckets {
    fn make_room(&mut self, now: Instant) {
        if self.accounts.len() >= MAX_KEYS {
            self.accounts.retain(|_, a| !a.idle(now));
        }
        while self.accounts.len() >= MAX_KEYS {
            let oldest = self
                .accounts
                .iter()
                .min_by_key(|(_, a)| a.last_failure)
                .map(|(k, _)| k.clone());
            match oldest {
                Some(key) => self.accounts.remove(&key),
                None => break,
            };
        }
        if self.clients.len() >= MAX_KEYS {
            self.clients.retain(|_, c| !c.idle(now));
        }
        while self.clients.len() >= MAX_KEYS {
            let oldest = self
                .clients
                .iter()
                .min_by_key(|(_, c)| c.last_failure())
                .map(|(k, _)| k.clone());
            match oldest {
                Some(key) => self.clients.remove(&key),
                None => break,
            };
        }
    }
}

/// One admitted attempt, already counted as a failure.
#[must_use = "settle the attempt with succeeded, not_a_failure or failed"]
#[derive(Debug)]
pub struct Attempt {
    account: String,
    ip: String,
    at: Instant,
    locked_account: bool,
    locked_ip: bool,
}

#[derive(Debug, Default)]
pub struct LoginLimiter {
    buckets: Mutex<Buckets>,
}

impl LoginLimiter {
    /// Admit an attempt and charge it, or answer how long until one would be
    /// admitted. Checked before any password is, so a locked-out guess costs
    /// no Argon2 time.
    pub fn begin(&self, account: &str, ip: &str, now: Instant) -> Result<Attempt, Duration> {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut buckets = self.buckets.lock().unwrap();
        let account_wait = buckets.accounts.get_mut(account).and_then(|a| a.wait(now));
        let ip_wait = buckets.clients.get_mut(ip).and_then(|c| c.wait(now));
        if let Some(wait) = account_wait.into_iter().chain(ip_wait).max() {
            return Err(wait);
        }
        buckets.make_room(now);

        let entry = buckets.accounts.entry(account.to_string()).or_default();
        entry.failures += 1;
        entry.last_failure = Some(now);
        let locked_account = entry.failures >= ACCOUNT_FAILURES;
        if locked_account {
            entry.locked_until = Some(now + LOCKOUT);
        }

        let client = buckets.clients.entry(ip.to_string()).or_default();
        client.prune(now);
        client.failures.push_back(now);
        let locked_ip = client.failures.len() >= IP_FAILURES;
        if locked_ip {
            client.locked_until = Some(now + LOCKOUT);
        }

        Ok(Attempt {
            account: account.to_string(),
            ip: ip.to_string(),
            at: now,
            locked_account,
            locked_ip,
        })
    }

    /// The attempt signed in: the account starts again and the attempt no
    /// longer counts against the IP.
    pub fn succeeded(&self, attempt: Attempt) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut buckets = self.buckets.lock().unwrap();
        buckets.accounts.remove(&attempt.account);
        refund_ip(&mut buckets, &attempt);
    }

    /// The attempt was not a failure (`totp_required`, or the server could
    /// not judge it): take back what `begin` charged.
    pub fn not_a_failure(&self, attempt: Attempt) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut buckets = self.buckets.lock().unwrap();
        if let Some(account) = buckets.accounts.get_mut(&attempt.account) {
            account.failures = account.failures.saturating_sub(1);
            if attempt.locked_account {
                account.locked_until = None;
            }
        }
        refund_ip(&mut buckets, &attempt);
    }

    /// The attempt failed and its charge stands. Answers the buckets it
    /// locked, for the event line.
    pub fn failed(&self, attempt: Attempt) -> Vec<Bucket> {
        let mut locked = Vec::new();
        if attempt.locked_account {
            locked.push(Bucket::Account);
        }
        if attempt.locked_ip {
            locked.push(Bucket::Ip);
        }
        locked
    }

    #[cfg(test)]
    fn sizes(&self) -> (usize, usize) {
        let buckets = self.buckets.lock().unwrap();
        (buckets.accounts.len(), buckets.clients.len())
    }
}

fn refund_ip(buckets: &mut Buckets, attempt: &Attempt) {
    if let Some(client) = buckets.clients.get_mut(&attempt.ip) {
        if let Some(position) = client.failures.iter().position(|at| *at == attempt.at) {
            client.failures.remove(position);
        }
        if attempt.locked_ip {
            client.locked_until = None;
        }
    }
}

/// The account bucket's key: the normalized email, or for anything that is
/// not an address the trimmed, lower-cased input — so unknown and malformed
/// addresses lock out exactly as real ones do. Cut to 320 characters.
pub fn account_key(email: &str) -> String {
    let key = crate::identity::users::normalize_email(email)
        .unwrap_or_else(|_| email.trim().to_lowercase());
    key.chars().take(ACCOUNT_KEY_MAX_CHARS).collect()
}

/// The client's address for the IP bucket: the TCP peer, unless the peer is
/// loopback and `header` is configured, when it is the address that header
/// carries (`CF-Connecting-IP` as given, the rightmost `X-Forwarded-For`
/// entry). A missing header, or one that does not hold an address, falls
/// back to the peer. No peer at all is `"unknown"`.
pub fn client_ip(
    peer: Option<SocketAddr>,
    headers: &HeaderMap,
    header: Option<ClientIpHeader>,
) -> String {
    let Some(peer) = peer else {
        return "unknown".to_string();
    };
    let forwarded = header
        .filter(|_| peer.ip().is_loopback())
        .and_then(|header| {
            let values = headers
                .get_all(header.header_name())
                .iter()
                .filter_map(|v| v.to_str().ok());
            match header {
                ClientIpHeader::CfConnectingIp => values.map(str::trim).next().map(str::to_string),
                ClientIpHeader::XForwardedFor => values
                    .flat_map(|v| v.split(','))
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .last()
                    .map(str::to_string),
            }
        })
        .and_then(|value| value.parse::<IpAddr>().ok());
    forwarded.unwrap_or(peer.ip()).to_string()
}
```

Add `pub mod ratelimit;` to `server/mod.rs`, and to `AppState`:

```rust
    /// Sign-in rate limits; see `ratelimit.rs`.
    pub limiter: Arc<super::ratelimit::LoginLimiter>,
```

initialized in `with_mode` as `limiter: Arc::new(super::ratelimit::LoginLimiter::default()),`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nigel-core server::ratelimit -- --test-threads=1`
Expected: PASS. `memory_stays_bounded` inserts 10,050 keys; it should finish in well under a second. If it is slow, the `min_by_key` scan in `make_room` is running on every insert past the cap — that is expected only after the cap, and only once per insert.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add crates/nigel-core/src/server
git commit -m "Add the sign-in rate limiter and client IP resolution

Five failures lock an account and twenty in fifteen minutes lock an IP,
each for fifteen minutes, with unknown addresses counted like known ones.
Attempts are charged when admitted so parallel guesses cannot slip past.
The client IP comes from the configured header only on a loopback peer."
```

---
### Task 9: Login, logout and me (security-critical)

Spec §4.1, §4.4, §4.6 rule 4 (login CSRF), §5, §12. Closes the body-cap half of **PR 1 review item 2** and verifies **item 5** (session constraints exercised over HTTP).

**Files:**
- Modify: `crates/nigel-core/src/server/extract.rs` (`SecretJson`, `PeerAddr`)
- Create: `crates/nigel-core/src/server/routes/sessions.rs`
- Modify: `crates/nigel-core/src/server/routes/mod.rs` (`pub mod sessions;`, merge its routes next to `status::routes()`)
- Modify: `crates/nigel-core/src/server/routes/status.rs` (`UNGATED_PATHS` gains the three auth paths)
- Modify: `crates/nigel-core/src/server/shared_tests.rs`, `crates/nigel-core/src/server/mod.rs` tests

**Interfaces:**
- Consumes: `users::{verify_credentials, complete_sign_in, Credentials, SignIn, get_user, User}`, `sessions::{create_session, sweep_expired, end_session, SessionKind, IDLE_DAYS}`, `seal::{ServerKey, key_path}`, `LoginLimiter`, `client_ip`, `account_key` (Task 8), `events` (Task 3), `session_cookie`, `expired_cookie`, `request_is_public` (Tasks 5–6), `Principal` (Task 6).
- Produces: `routes::sessions::{routes, UserBody, LOGIN_BODY_LIMIT}` — Task 10 reuses `UserBody`; `extract::{SecretJson, PeerAddr}`.

- [ ] **Step 1: Write the failing tests**

Append to `shared_tests.rs`:

```rust
use std::net::SocketAddr;
use axum::extract::ConnectInfo;

fn login_request(body: &str, origin: Option<&str>, peer: &str) -> Request<Body> {
    let mut builder = public_request("POST", "/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    let mut request = builder.body(Body::from(body.to_owned())).unwrap();
    let peer: SocketAddr = peer.parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    request
}

fn login_body(email: &str, password: &str) -> String {
    serde_json::json!({ "email": email, "password": password }).to_string()
}

const PEER: &str = "198.51.100.7:50000";

#[tokio::test]
async fn a_browser_login_sets_a_secure_cookie_on_the_public_host() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());

    let response = app
        .clone()
        .oneshot(login_request(&login_body("Pat@Example.com", READY_PASSWORD), Some(PUBLIC_URL), PEER))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = header_str(&response, header::SET_COOKIE);
    for part in ["nigel_session=", "HttpOnly", "SameSite=Strict", "Path=/", "Max-Age=2592000", "Secure"] {
        assert!(cookie.contains(part), "{part}: {cookie}");
    }
    let json = json_body(response).await;
    assert_eq!(json["user"]["email"], "pat@example.com");
    assert_eq!(json["user"]["role"], "admin");
    assert!(json.get("token").is_none());

    let token = cookie.split(';').next().unwrap().trim_start_matches("nigel_session=").to_string();
    let me = app.oneshot(cookie_get("/api/auth/me", &token)).await.unwrap();
    let me = json_body(me).await;
    assert_eq!(me["mode"], "shared");
    assert_eq!(me["role"], "admin");
    assert_eq!(me["user"]["displayName"], "Pat Example");
    assert_eq!(me["user"]["mustChangePassword"], false);
}

#[tokio::test]
async fn a_loopback_login_cookie_is_not_secure() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, "http://127.0.0.1:5731")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(login_body("pat@example.com", READY_PASSWORD)))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo("127.0.0.1:50000".parse::<SocketAddr>().unwrap()));
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!header_str(&response, header::SET_COOKIE).contains("Secure"));
}

#[tokio::test]
async fn a_native_login_answers_a_bearer_token_and_sets_no_cookie() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    let body = serde_json::json!({
        "email": "pat@example.com", "password": READY_PASSWORD, "client": "native"
    })
    .to_string();
    let response = app.clone().oneshot(login_request(&body, None, PEER)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_str(&response, header::SET_COOKIE), "");
    let json = json_body(response).await;
    let token = json["token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 64);
    assert!(json["expiresAt"].as_str().unwrap().ends_with('Z'));
    let ping = app.oneshot(bearer_get("/api/ping", &token)).await.unwrap();
    assert_eq!(ping.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_browser_login_without_an_origin_is_refused() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    let response = app
        .oneshot(login_request(&login_body("pat@example.com", READY_PASSWORD), None, PEER))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(header_str(&response, header::SET_COOKIE), "");
}

#[tokio::test]
async fn wrong_unknown_and_disabled_credentials_all_answer_alike() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let sam = add_ready_user(&db_path, "sam@example.com", Role::Admin);
    let conn = crate::db::get_connection(&db_path).unwrap();
    crate::identity::users::disable_user(&conn, sam.id, crate::clock::now_utc()).unwrap();
    let (app, _state) = served_app(&db_path, public_config());

    let mut answers = Vec::new();
    for (email, password, peer) in [
        ("pat@example.com", "not the password at all", "198.51.100.1:1"),
        ("nobody@example.com", READY_PASSWORD, "198.51.100.2:1"),
        ("sam@example.com", READY_PASSWORD, "198.51.100.3:1"),
        ("not an email", READY_PASSWORD, "198.51.100.4:1"),
    ] {
        let response = app
            .clone()
            .oneshot(login_request(&login_body(email, password), Some(PUBLIC_URL), peer))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{email}");
        let json = json_body(response).await;
        answers.push((json["error"]["code"].clone(), json["error"]["message"].clone()));
    }
    assert!(answers.windows(2).all(|w| w[0] == w[1]), "{answers:?}");
    assert_eq!(answers[0].0, "invalid_credentials");
    assert_eq!(answers[0].1, "That email and password don't match.");
}

#[tokio::test]
async fn five_wrong_passwords_lock_the_account_before_the_password_is_checked() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    for _ in 0..5 {
        app.clone()
            .oneshot(login_request(&login_body("pat@example.com", "wrong password here"), Some(PUBLIC_URL), PEER))
            .await
            .unwrap();
    }
    let locked = app
        .oneshot(login_request(&login_body("pat@example.com", READY_PASSWORD), Some(PUBLIC_URL), "198.51.100.99:1"))
        .await
        .unwrap();
    assert_eq!(locked.status(), StatusCode::TOO_MANY_REQUESTS);
    let json = json_body(locked).await;
    assert_eq!(json["error"]["code"], "too_many_attempts");
    assert!(json["error"]["details"]["retryAfterMs"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn a_second_factor_is_asked_for_then_checked() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let _config = TempConfig::new();
    let key = crate::identity::seal::ServerKey::load_or_create(&crate::identity::seal::key_path()).unwrap();
    let secret = [7u8; 20];
    let now = crate::clock::now_utc();
    let conn = crate::db::get_connection(&db_path).unwrap();
    crate::identity::users::set_totp_secret(&conn, pat.id, &key, &secret, crate::identity::totp::step_at(now) - 5, now).unwrap();
    let (app, _state) = served_app(&db_path, public_config());

    let needs_code = app
        .clone()
        .oneshot(login_request(&login_body("pat@example.com", READY_PASSWORD), Some(PUBLIC_URL), PEER))
        .await
        .unwrap();
    assert_eq!(needs_code.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(needs_code).await["error"]["code"], "totp_required");

    let wrong = serde_json::json!({ "email": "pat@example.com", "password": READY_PASSWORD, "code": "000000" }).to_string();
    let wrong = app.clone().oneshot(login_request(&wrong, Some(PUBLIC_URL), PEER)).await.unwrap();
    let wrong = json_body(wrong).await;
    assert_eq!(wrong["error"]["code"], "invalid_credentials");
    assert_eq!(wrong["error"]["message"], "That code didn't match. Check the time on your device.");

    let code = crate::identity::totp::code_for_step(&secret, crate::identity::totp::step_at(crate::clock::now_utc()));
    let right = serde_json::json!({ "email": "pat@example.com", "password": READY_PASSWORD, "code": code }).to_string();
    let right = app.oneshot(login_request(&right, Some(PUBLIC_URL), PEER)).await.unwrap();
    assert_eq!(right.status(), StatusCode::OK);
}

#[tokio::test]
async fn an_oversized_login_body_is_refused_before_any_check() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    let body = login_body("pat@example.com", &"x".repeat(9 * 1024));
    let response = app.oneshot(login_request(&body, Some(PUBLIC_URL), PEER)).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!body_string(response).await.contains("xxxx"));
}

#[tokio::test]
async fn logout_ends_the_session_and_expires_the_cookie() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(&db_path, pat.id, SessionKind::Cookie, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());
    let response = app
        .clone()
        .oneshot(
            public_request("POST", "/api/auth/logout")
                .header(header::COOKIE, format!("nigel_session={token}"))
                .header(header::ORIGIN, PUBLIC_URL)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(header_str(&response, header::SET_COOKIE).contains("Max-Age=0"));
    assert_eq!(json_body(response).await["loggedOut"], true);
    let after = app.oneshot(cookie_get("/api/ping", &token)).await.unwrap();
    assert_eq!(after.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sign_in_routes_answer_while_the_books_are_locked() {
    let (_dir, db_path) = temp_db();
    encrypt(&db_path);
    crate::db::set_db_password_for_tests(None);
    let (app, state) = served_app(&db_path, public_config());
    let token = state.local_token().unwrap().expose().to_string();
    let me = app.clone().oneshot(session_get("/api/auth/me", &token)).await.unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    let logout = app.oneshot(session_post("/api/auth/logout", &token, "")).await.unwrap();
    assert_eq!(logout.status(), StatusCode::OK);
}
```

In `server/mod.rs` `mod tests`:

```rust
    #[tokio::test]
    async fn in_local_mode_me_is_the_admin_owner_and_login_is_a_conflict() {
        let (_dir, db_path) = temp_db();
        let (app, token) = app_for(&db_path);
        let me = app.clone().oneshot(session_get("/api/auth/me", &token)).await.unwrap();
        let me = json_body(me).await;
        assert_eq!(me, serde_json::json!({ "mode": "local", "user": null, "role": "admin" }));

        let login = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header(header::HOST, HOST)
                    .header(header::ORIGIN, "http://127.0.0.1:5731")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"email":"pat@example.com","password":"x"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login.status(), StatusCode::CONFLICT);
        assert_eq!(json_body(login).await["error"]["details"]["reason"], "local_mode");
    }
```

`TempConfig::new()` redirects the config directory so `server.key` lands in a temp dir; keep the guard alive for the whole test.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server:: -- --test-threads=1`
Expected: FAIL — `/api/auth/*` answers `404`/`403`.

- [ ] **Step 3: Add the extractors**

In `server/extract.rs`:

```rust
use std::net::SocketAddr;

use axum::extract::ConnectInfo;
use axum::http::StatusCode;

/// `Json<T>` for bodies that carry a password or a code. Every rejection is
/// a fixed sentence that quotes nothing from the body, and a body over the
/// route's limit is a `413`.
pub struct SecretJson<T>(pub T);

impl<S, T> FromRequest<S> for SecretJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
                Err(ApiError::payload_too_large("That request body is too large."))
            }
            Err(JsonRejection::MissingJsonContentType(_)) => Err(ApiError::bad_request(
                "Expected a JSON body with Content-Type: application/json.",
            )),
            Err(_) => Err(ApiError::bad_request(
                "That request body is not the JSON this endpoint expects.",
            )),
        }
    }
}

/// The TCP peer, when the server was started with connect info. In-process
/// requests (tests, the desktop scheme) have none.
pub struct PeerAddr(pub Option<SocketAddr>);

impl<S: Send + Sync> FromRequestParts<S> for PeerAddr {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.0),
        ))
    }
}
```

- [ ] **Step 4: Implement `routes/sessions.rs`**

```rust
//! Signing in and out: `POST /api/auth/login`, `POST /api/auth/logout`,
//! `GET /api/auth/me`. Login is the one `/api` route that needs no session;
//! all three answer while the books are locked.

use std::time::Instant;

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::identity::seal::{self, ServerKey};
use crate::identity::sessions::{self, IssuedSession, SessionKind};
use crate::identity::users::{self, Credentials, SignIn, User};
use crate::identity::Role;

use super::super::auth::{expired_cookie, request_is_public, session_cookie};
use super::super::error::{ApiError, ApiResult};
use super::super::events::{emit, AuthEvent, EventKind};
use super::super::extract::{PeerAddr, SecretJson};
use super::super::mode::Mode;
use super::super::principal::Principal;
use super::super::ratelimit::{account_key, client_ip};
use super::super::secret::Secret;
use super::super::state::AppState;

/// The most a login body may be. A password is at most 256 characters.
pub const LOGIN_BODY_LIMIT: usize = 8 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/auth/login",
            post(login).layer(DefaultBodyLimit::max(LOGIN_BODY_LIMIT)),
        )
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LoginClient {
    #[default]
    Browser,
    Native,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest {
    email: String,
    password: Secret,
    #[serde(default)]
    code: Option<Secret>,
    #[serde(default)]
    client: LoginClient,
}

/// A user as the API shows one. No hash, no secret.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserBody {
    id: i64,
    email: String,
    display_name: String,
    role: Role,
    totp_enabled: bool,
    must_change_password: bool,
}

impl From<&User> for UserBody {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            role: user.role,
            totp_enabled: user.totp_enabled,
            must_change_password: user.must_change_password,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    user: UserBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
}

enum Outcome {
    BadPassword,
    CodeRequired,
    BadCode(User),
    TotpUnavailable(User),
    SignedIn(User, IssuedSession),
}

async fn login(
    State(state): State<AppState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Result<SecretJson<LoginRequest>, ApiError>,
) -> ApiResult<Response> {
    if state.mode() != Mode::Shared {
        return Err(ApiError::local_mode());
    }
    let SecretJson(request) = body?;
    let client = request.client;
    if client == LoginClient::Browser && !headers.contains_key(header::ORIGIN) {
        return Err(ApiError::origin_required());
    }

    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    let account = account_key(&request.email);
    let attempt = match state.limiter.begin(&account, &ip, Instant::now()) {
        Ok(attempt) => attempt,
        Err(wait) => {
            emit(&AuthEvent::new(EventKind::LoginRefused).email(&account).ip(&ip));
            return Err(ApiError::too_many_attempts(wait.as_millis()));
        }
    };

    let kind = match client {
        LoginClient::Browser => SessionKind::Cookie,
        LoginClient::Native => SessionKind::Bearer,
    };
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let now = state.now();
    let row_ip = ip.clone();
    let gate = state.db_gate.clone().read_owned().await;
    let db_path = state.db_path();
    let (_gate, outcome) = super::spawn_blocking_holding(
        gate,
        move || -> crate::error::Result<Outcome> {
            let conn = crate::db::get_connection(&db_path)?;
            let proof = match users::verify_credentials(
                &conn,
                &request.email,
                request.password.expose(),
            )? {
                Credentials::Valid(proof) => proof,
                Credentials::Invalid => return Ok(Outcome::BadPassword),
            };
            let key = if proof.user().totp_enabled {
                ServerKey::load(&seal::key_path()).ok().flatten()
            } else {
                None
            };
            let code = request.code.as_ref().map(Secret::expose);
            Ok(
                match users::complete_sign_in(&conn, proof, key.as_ref(), code, now)? {
                    SignIn::Granted(user, grant) => {
                        let issued = sessions::create_session(
                            &conn,
                            grant,
                            kind,
                            user_agent.as_deref(),
                            Some(&row_ip),
                            now,
                        )?;
                        sessions::sweep_expired(&conn, now)?;
                        Outcome::SignedIn(user, issued)
                    }
                    SignIn::CodeRequired(_) => Outcome::CodeRequired,
                    SignIn::CodeRejected(user) => Outcome::BadCode(user),
                    SignIn::TotpUnavailable(user) => Outcome::TotpUnavailable(user),
                },
            )
        },
    )
    .await?;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(e) => {
            state.limiter.not_a_failure(attempt);
            return Err(e.into());
        }
    };

    match outcome {
        Outcome::BadPassword => {
            let locked = state.limiter.failed(attempt);
            emit(
                &AuthEvent::new(EventKind::LoginFailed)
                    .email(&account)
                    .ip(&ip)
                    .reason("credentials"),
            );
            for bucket in locked {
                emit(
                    &AuthEvent::new(EventKind::Lockout)
                        .email(&account)
                        .ip(&ip)
                        .reason(bucket.as_str()),
                );
            }
            Err(ApiError::invalid_credentials())
        }
        Outcome::CodeRequired => {
            state.limiter.not_a_failure(attempt);
            Err(ApiError::totp_required())
        }
        Outcome::BadCode(user) => {
            let locked = state.limiter.failed(attempt);
            emit(
                &AuthEvent::new(EventKind::LoginFailed)
                    .email(&user.email)
                    .user(user.id)
                    .ip(&ip)
                    .reason("code"),
            );
            for bucket in locked {
                emit(
                    &AuthEvent::new(EventKind::Lockout)
                        .email(&user.email)
                        .user(user.id)
                        .ip(&ip)
                        .reason(bucket.as_str()),
                );
            }
            Err(ApiError::login_code_mismatch())
        }
        Outcome::TotpUnavailable(user) => {
            state.limiter.not_a_failure(attempt);
            emit(
                &AuthEvent::new(EventKind::TotpUnavailable)
                    .email(&user.email)
                    .user(user.id)
                    .ip(&ip),
            );
            Err(ApiError::totp_unavailable())
        }
        Outcome::SignedIn(user, issued) => {
            state.limiter.succeeded(attempt);
            emit(
                &AuthEvent::new(EventKind::LoginSucceeded)
                    .email(&user.email)
                    .user(user.id)
                    .ip(&ip)
                    .session(issued.id),
            );
            let native = client == LoginClient::Native;
            let body = LoginResponse {
                user: UserBody::from(&user),
                token: native.then(|| issued.token.to_string()),
                expires_at: native.then(|| issued.expires_at.clone()),
            };
            let mut response = Json(body).into_response();
            if !native {
                let cookie = session_cookie(
                    &issued.token,
                    sessions::IDLE_DAYS * 86_400,
                    request_is_public(&state, &headers),
                );
                response.headers_mut().append(
                    header::SET_COOKIE,
                    HeaderValue::from_str(&cookie).map_err(ApiError::internal)?,
                );
            }
            Ok(response)
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoggedOut {
    logged_out: bool,
}

/// Ends the current session. In local mode there is none, and the answer is
/// the same.
async fn logout(
    State(state): State<AppState>,
    principal: Principal,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let mut response = Json(LoggedOut { logged_out: true }).into_response();
    let Principal::User(user) = principal else {
        return Ok(response);
    };
    let session_id = user.session_id;
    super::with_conn(&state, move |conn| {
        sessions::end_session(conn, session_id).map(|_| ())
    })
    .await?;
    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    emit(
        &AuthEvent::new(EventKind::Logout)
            .email(&user.email)
            .user(user.id)
            .ip(&ip)
            .session(session_id),
    );
    if user.kind == SessionKind::Cookie {
        let cookie = expired_cookie(request_is_public(&state, &headers));
        response.headers_mut().append(
            header::SET_COOKIE,
            HeaderValue::from_str(&cookie).map_err(ApiError::internal)?,
        );
    }
    Ok(response)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeResponse {
    mode: &'static str,
    user: Option<UserBody>,
    role: Role,
}

async fn me(State(state): State<AppState>, principal: Principal) -> ApiResult<Json<MeResponse>> {
    match principal {
        Principal::LocalOwner => Ok(Json(MeResponse {
            mode: Mode::Local.as_str(),
            user: None,
            role: Role::Admin,
        })),
        Principal::User(signed_in) => {
            let id = signed_in.id;
            let user = super::with_conn(&state, move |conn| users::get_user(conn, id))
                .await?
                .ok_or_else(ApiError::unauthorized_shared)?;
            Ok(Json(MeResponse {
                mode: Mode::Shared.as_str(),
                role: user.role,
                user: Some(UserBody::from(&user)),
            }))
        }
    }
}
```

Mount it in `routes/mod.rs`:

```rust
pub mod sessions;
…
        .route("/ping", get(ping))
        .merge(status::routes())
        .merge(sessions::routes())
        .merge(data_router())
```

In `routes/status.rs`:

```rust
const UNGATED_PATHS: [&str; 6] = [
    "/ping",
    "/status",
    "/unlock",
    "/auth/login",
    "/auth/logout",
    "/auth/me",
];
```

and update the module doc's first paragraph to name them.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p nigel-core server:: -- --test-threads=1`
Expected: PASS. If `a_second_factor_is_asked_for_then_checked` fails on the last step with `invalid_credentials`, the code was generated in a different 30-second step from the one the server judged against — rerun; if it persists, check `state.now()` and `crate::clock::now_utc()` agree (no pinned clock in `served_app`).

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/nigel-core/src/server
git commit -m "Add login, logout and me

Login checks the rate limits first, verifies the password with one
Argon2id run, asks for and checks the second factor, and issues a cookie
or bearer session; every refusal reads the same. A browser login needs an
Origin, the body is capped at 8 KiB, and each outcome writes an event
line. Me answers in both modes; logout ends the session."
```

---

### Task 10: Account routes — password and TOTP (security-critical)

Spec §4.1 (account routes), §4.3, §4.5. Rulings 14–16. Uses `server.key` (Task 1 must be merged first).

**Files:**
- Modify: `crates/nigel-core/Cargo.toml` (`qrcode = { version = "0.14", default-features = false, features = ["svg"] }`)
- Create: `crates/nigel-core/src/server/pending_totp.rs`
- Modify: `crates/nigel-core/src/server/state.rs` (`pub totp_setups: Arc<PendingTotpSetups>`)
- Create: `crates/nigel-core/src/server/routes/account.rs`
- Modify: `crates/nigel-core/src/server/routes/mod.rs` (`pub mod account;`, `.merge(account::routes())` in `api_router`)
- Modify: `crates/nigel-core/src/server/shared_tests.rs`
- Modify: `crates/nigel-desktop/Cargo.lock`, `crates/nigel-ffi/Cargo.lock`

**Interfaces:**
- Consumes: `users::{change_password, PasswordChangeError, verify_user_password, set_totp_secret, verify_totp, clear_totp, get_user, TotpCheck}`, `totp::{generate_secret, matching_step, secret_base32, otpauth_uri, SECRET_LEN}`, `seal::{ServerKey, key_path}`, `LoginLimiter` (Task 8), `SecretJson`, `PeerAddr`, `Principal`, `UserBody` (Task 9).
- Produces: `PendingTotpSetups::{start, secret_for, finish}`, `SETUP_LIFETIME`.

- [ ] **Step 1: Write the failing tests**

Append to `shared_tests.rs`:

```rust
fn bearer_post(uri: &str, token: &str, body: serde_json::Value) -> Request<Body> {
    public_request("POST", uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn a_temporary_password_is_changed_and_other_sessions_end() {
    let (_dir, db_path) = temp_db();
    let conn = crate::db::get_connection(&db_path).unwrap();
    let created = crate::identity::users::create_user(
        &conn,
        &crate::identity::users::NewUser { email: "pat@example.com", display_name: "Pat Example", role: Role::Admin },
        crate::clock::now_utc(),
    )
    .unwrap();
    let this = session_for(&db_path, created.user.id, SessionKind::Bearer, crate::clock::now_utc());
    let other = session_for(&db_path, created.user.id, SessionKind::Bearer, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());

    for (new, reason) in [("short", "too_short"), ("pat@example.com", "matches_identity")] {
        let response = app
            .clone()
            .oneshot(bearer_post("/api/account/password", &this, serde_json::json!({
                "currentPassword": created.temporary_password.as_str(), "newPassword": new
            })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{new}");
        assert_eq!(json_body(response).await["error"]["details"]["reason"], reason);
    }
    let wrong = app
        .clone()
        .oneshot(bearer_post("/api/account/password", &this, serde_json::json!({
            "currentPassword": "not it at all", "newPassword": "a brand new passphrase"
        })))
        .await
        .unwrap();
    assert_eq!(json_body(wrong).await["error"]["details"]["reason"], "wrong_password");

    let changed = app
        .clone()
        .oneshot(bearer_post("/api/account/password", &this, serde_json::json!({
            "currentPassword": created.temporary_password.as_str(), "newPassword": "a brand new passphrase"
        })))
        .await
        .unwrap();
    assert_eq!(changed.status(), StatusCode::OK);

    let accounts = app.clone().oneshot(bearer_get("/api/accounts", &this)).await.unwrap();
    assert_eq!(accounts.status(), StatusCode::OK, "must-change cleared, this session kept");
    let ended = app.oneshot(bearer_get("/api/ping", &other)).await.unwrap();
    assert_eq!(ended.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn two_step_is_set_up_confirmed_and_removed() {
    let (_dir, db_path) = temp_db();
    let _config = TempConfig::new();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(&db_path, pat.id, SessionKind::Bearer, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());

    let wrong_password = app
        .clone()
        .oneshot(bearer_post("/api/account/totp/setup", &token, serde_json::json!({ "currentPassword": "nope nope nope" })))
        .await
        .unwrap();
    assert_eq!(json_body(wrong_password).await["error"]["details"]["reason"], "wrong_password");

    let setup = app
        .clone()
        .oneshot(bearer_post("/api/account/totp/setup", &token, serde_json::json!({ "currentPassword": READY_PASSWORD })))
        .await
        .unwrap();
    assert_eq!(setup.status(), StatusCode::OK);
    let setup = json_body(setup).await;
    let setup_id = setup["setupId"].as_str().unwrap().to_string();
    let secret_b32 = setup["secret"].as_str().unwrap();
    assert!(setup["otpauthUri"].as_str().unwrap().starts_with("otpauth://totp/Nigel:pat@example.com?secret="));
    assert!(setup["qrSvg"].as_str().unwrap().contains("<svg"));
    let secret = data_encoding::BASE32_NOPAD.decode(secret_b32.as_bytes()).unwrap();
    assert!(!crate::identity::users::get_user(&crate::db::get_connection(&db_path).unwrap(), pat.id).unwrap().unwrap().totp_enabled, "nothing stored yet");

    let bad = app
        .clone()
        .oneshot(bearer_post("/api/account/totp/confirm", &token, serde_json::json!({ "setupId": setup_id, "code": "000000" })))
        .await
        .unwrap();
    assert_eq!(json_body(bad).await["error"]["details"]["reason"], "wrong_code");

    let stale = app
        .clone()
        .oneshot(bearer_post("/api/account/totp/confirm", &token, serde_json::json!({ "setupId": "ff".repeat(16), "code": "000000" })))
        .await
        .unwrap();
    assert_eq!(json_body(stale).await["error"]["details"]["reason"], "setup_expired");

    let now = crate::clock::now_utc();
    let step = crate::identity::totp::step_at(now);
    let code = crate::identity::totp::code_for_step(&secret, step);
    let confirmed = app
        .clone()
        .oneshot(bearer_post("/api/account/totp/confirm", &token, serde_json::json!({ "setupId": setup_id, "code": code })))
        .await
        .unwrap();
    assert_eq!(confirmed.status(), StatusCode::OK);
    assert!(crate::identity::seal::key_path().exists(), "server.key created on first enrollment");

    let again = app
        .clone()
        .oneshot(bearer_post("/api/account/totp/setup", &token, serde_json::json!({ "currentPassword": READY_PASSWORD })))
        .await
        .unwrap();
    assert_eq!(json_body(again).await["error"]["details"]["reason"], "totp_enabled");

    let next = crate::identity::totp::code_for_step(&secret, step + 1);
    let removed = app
        .clone()
        .oneshot(bearer_post("/api/account/totp/remove", &token, serde_json::json!({ "currentPassword": READY_PASSWORD, "code": next })))
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);
    let conn = crate::db::get_connection(&db_path).unwrap();
    assert!(!crate::identity::users::get_user(&conn, pat.id).unwrap().unwrap().totp_enabled);
}

#[tokio::test]
async fn account_routes_are_a_conflict_for_the_local_owner() {
    let (_dir, db_path) = temp_db();
    let (app, token) = app_for(&db_path);
    let response = app
        .oneshot(session_post("/api/account/password", &token, r#"{"currentPassword":"x","newPassword":"y"}"#))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(json_body(response).await["error"]["details"]["reason"], "local_mode");
}

#[tokio::test]
async fn wrong_current_passwords_on_an_account_route_lock_like_logins() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(&db_path, pat.id, SessionKind::Bearer, crate::clock::now_utc());
    let (app, _state) = served_app(&db_path, public_config());
    for _ in 0..5 {
        app.clone()
            .oneshot(bearer_post("/api/account/totp/setup", &token, serde_json::json!({ "currentPassword": "wrong password here" })))
            .await
            .unwrap();
    }
    let locked = app
        .oneshot(bearer_post("/api/account/totp/setup", &token, serde_json::json!({ "currentPassword": READY_PASSWORD })))
        .await
        .unwrap();
    assert_eq!(locked.status(), StatusCode::TOO_MANY_REQUESTS);
}
```

Add `pending_totp.rs` unit tests (create the file with them):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn secret() -> Zeroizing<[u8; SECRET_LEN]> {
        Zeroizing::new([5u8; SECRET_LEN])
    }

    #[test]
    fn a_setup_is_held_per_session_for_ten_minutes() {
        let setups = PendingTotpSetups::default();
        let t0 = Instant::now();
        let id = setups.start(7, 2, secret(), t0);
        assert_eq!(id.len(), 32);
        let (user, held) = setups.secret_for(7, &id, t0).unwrap();
        assert_eq!((user, *held), (2, [5u8; SECRET_LEN]));
        assert!(setups.secret_for(8, &id, t0).is_none(), "another session");
        assert!(setups.secret_for(7, "00", t0).is_none(), "another id");
        assert!(setups.secret_for(7, &id, t0 + SETUP_LIFETIME).is_none(), "expired");
    }

    #[test]
    fn a_new_setup_replaces_the_last_and_finishing_forgets_it() {
        let setups = PendingTotpSetups::default();
        let t0 = Instant::now();
        let first = setups.start(7, 2, secret(), t0);
        let second = setups.start(7, 2, secret(), t0);
        assert!(setups.secret_for(7, &first, t0).is_none());
        assert!(setups.secret_for(7, &second, t0).is_some());
        setups.finish(7);
        assert!(setups.secret_for(7, &second, t0).is_none());
    }

    #[test]
    fn debug_shows_no_secret() {
        let setups = PendingTotpSetups::default();
        setups.start(7, 2, secret(), Instant::now());
        let printed = format!("{setups:?}");
        assert!(!printed.contains('5'), "{printed}");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server:: -- --test-threads=1`
Expected: FAIL to compile — no `pending_totp` items, no account routes.

- [ ] **Step 3: Add the dependency**

In `crates/nigel-core/Cargo.toml` `[dependencies]`:

```toml
qrcode = { version = "0.14", default-features = false, features = ["svg"] }
```

- [ ] **Step 4: Implement `pending_totp.rs`**

```rust
//! Two-step enrollments in progress: a fresh secret held in memory for ten
//! minutes, one per session, until a code from the user's app confirms it.
//! Nothing is written to the books until then.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rand::RngCore;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::identity::totp::SECRET_LEN;

pub const SETUP_LIFETIME: Duration = Duration::from_secs(10 * 60);

struct Pending {
    setup_id: String,
    user_id: i64,
    secret: Zeroizing<[u8; SECRET_LEN]>,
    expires: Instant,
}

#[derive(Default)]
pub struct PendingTotpSetups {
    by_session: Mutex<HashMap<i64, Pending>>,
}

impl std::fmt::Debug for PendingTotpSetups {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PendingTotpSetups(<redacted>)")
    }
}

impl PendingTotpSetups {
    /// Hold `secret` for `session_id`, replacing any setup that session had
    /// started. Answers the setup id the client sends back.
    pub fn start(
        &self,
        session_id: i64,
        user_id: i64,
        secret: Zeroizing<[u8; SECRET_LEN]>,
        now: Instant,
    ) -> String {
        let mut raw = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        let setup_id = hex::encode(raw);
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut pending = self.by_session.lock().unwrap();
        pending.retain(|_, p| p.expires > now);
        pending.insert(
            session_id,
            Pending {
                setup_id: setup_id.clone(),
                user_id,
                secret,
                expires: now + SETUP_LIFETIME,
            },
        );
        setup_id
    }

    /// The user and secret this session's unexpired setup `setup_id` holds.
    pub fn secret_for(
        &self,
        session_id: i64,
        setup_id: &str,
        now: Instant,
    ) -> Option<(i64, Zeroizing<[u8; SECRET_LEN]>)> {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let pending = self.by_session.lock().unwrap();
        let held = pending.get(&session_id)?;
        let same_id: bool = held.setup_id.as_bytes().ct_eq(setup_id.as_bytes()).into();
        (same_id && held.expires > now).then(|| (held.user_id, held.secret.clone()))
    }

    pub fn finish(&self, session_id: i64) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        self.by_session.lock().unwrap().remove(&session_id);
    }
}
```

Add `pub mod pending_totp;` to `server/mod.rs`, and to `AppState` `pub totp_setups: Arc<super::pending_totp::PendingTotpSetups>,` initialized in `with_mode` as `Arc::new(Default::default())`.

- [ ] **Step 5: Implement `routes/account.rs`**

```rust
//! The signed-in user's own account: `POST /api/account/password` and
//! two-step setup, confirmation and removal. Shared mode only — the local
//! owner has no account and gets `409 local_mode`.

use std::time::Instant;

use axum::extract::{DefaultBodyLimit, State};
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::identity::seal::{self, ServerKey};
use crate::identity::totp;
use crate::identity::users::{self, PasswordChangeError, TotpCheck};

use super::super::error::{ApiError, ApiResult};
use super::super::events::{emit, AuthEvent, EventKind};
use super::super::extract::{PeerAddr, SecretJson};
use super::super::principal::{Principal, SessionUser};
use super::super::ratelimit::client_ip;
use super::super::secret::Secret;
use super::super::state::AppState;

/// The most an account-route body may be.
pub const ACCOUNT_BODY_LIMIT: usize = 8 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/account/password", post(change_password))
        .route("/account/totp/setup", post(totp_setup))
        .route("/account/totp/confirm", post(totp_confirm))
        .route("/account/totp/remove", post(totp_remove))
        .layer(DefaultBodyLimit::max(ACCOUNT_BODY_LIMIT))
}

fn signed_in(principal: Principal) -> ApiResult<SessionUser> {
    match principal {
        Principal::User(user) => Ok(user),
        Principal::LocalOwner => Err(ApiError::local_mode()),
    }
}

fn event<'a>(kind: EventKind, user: &'a SessionUser, ip: &'a str) -> AuthEvent<'a> {
    AuthEvent::new(kind)
        .email(&user.email)
        .user(user.id)
        .ip(ip)
        .session(user.session_id)
}

fn emit_lockouts(user: &SessionUser, ip: &str, locked: Vec<super::super::ratelimit::Bucket>) {
    for bucket in locked {
        emit(&event(EventKind::Lockout, user, ip).reason(bucket.as_str()));
    }
}

/// Check the signed-in user's current password under the sign-in limits, so
/// a stolen session cannot become a way to guess the password.
async fn check_current_password(
    state: &AppState,
    user: &SessionUser,
    ip: &str,
    password: Secret,
) -> ApiResult<()> {
    let attempt = state
        .limiter
        .begin(&user.email, ip, Instant::now())
        .map_err(|wait| ApiError::too_many_attempts(wait.as_millis()))?;
    let id = user.id;
    let verified = super::with_conn(state, move |conn| {
        users::verify_user_password(conn, id, password.expose())
    })
    .await;
    match verified {
        Ok(true) => {
            state.limiter.succeeded(attempt);
            Ok(())
        }
        Ok(false) => {
            emit_lockouts(user, ip, state.limiter.failed(attempt));
            Err(ApiError::wrong_password())
        }
        Err(e) => {
            state.limiter.not_a_failure(attempt);
            Err(e)
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PasswordChange {
    current_password: Secret,
    new_password: Secret,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PasswordChanged {
    changed: bool,
}

/// Clears `must_change_password` and ends every other session of this user.
async fn change_password(
    State(state): State<AppState>,
    principal: Principal,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Result<SecretJson<PasswordChange>, ApiError>,
) -> ApiResult<Json<PasswordChanged>> {
    let user = signed_in(principal)?;
    let SecretJson(request) = body?;
    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    let attempt = state
        .limiter
        .begin(&user.email, &ip, Instant::now())
        .map_err(|wait| ApiError::too_many_attempts(wait.as_millis()))?;

    let (id, session_id, now) = (user.id, user.session_id, state.now());
    let gate = state.db_gate.clone().read_owned().await;
    let db_path = state.db_path();
    let (_gate, result) = super::spawn_blocking_holding(gate, move || {
        let conn = crate::db::get_connection(&db_path)?;
        users::change_password(
            &conn,
            id,
            request.current_password.expose(),
            request.new_password.expose(),
            Some(session_id),
            now,
        )
    })
    .await?;

    match result {
        Ok(()) => {
            state.limiter.succeeded(attempt);
            emit(&event(EventKind::PasswordChanged, &user, &ip));
            Ok(Json(PasswordChanged { changed: true }))
        }
        Err(PasswordChangeError::WrongCurrentPassword) => {
            emit_lockouts(&user, &ip, state.limiter.failed(attempt));
            Err(ApiError::wrong_password())
        }
        Err(PasswordChangeError::Rule(rule)) => {
            state.limiter.succeeded(attempt);
            Err(ApiError::bad_request(rule.to_string())
                .with_details(serde_json::json!({ "reason": rule.reason_code() })))
        }
        Err(PasswordChangeError::Failed(e)) => {
            state.limiter.not_a_failure(attempt);
            Err(e.into())
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TotpSetupRequest {
    current_password: Secret,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TotpSetupResponse {
    setup_id: String,
    secret: String,
    otpauth_uri: String,
    qr_svg: String,
}

fn qr_svg(uri: &str) -> ApiResult<String> {
    qrcode::QrCode::new(uri.as_bytes())
        .map(|code| {
            code.render::<qrcode::render::svg::Color>()
                .min_dimensions(200, 200)
                .build()
        })
        .map_err(|_| ApiError::internal("Could not draw the QR code."))
}

/// Starts enrollment: a new secret held in memory for ten minutes, returned
/// as base32, an `otpauth://` URI and a QR code. Nothing is stored yet.
async fn totp_setup(
    State(state): State<AppState>,
    principal: Principal,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Result<SecretJson<TotpSetupRequest>, ApiError>,
) -> ApiResult<Json<TotpSetupResponse>> {
    let user = signed_in(principal)?;
    let SecretJson(request) = body?;
    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    check_current_password(&state, &user, &ip, request.current_password).await?;

    let id = user.id;
    let current = super::with_conn(&state, move |conn| users::get_user(conn, id))
        .await?
        .ok_or_else(ApiError::unauthorized_shared)?;
    if current.totp_enabled {
        return Err(ApiError::conflict(
            "Two-step verification is already on. Turn it off before setting it up again.",
            serde_json::json!({ "reason": "totp_enabled" }),
        ));
    }

    let secret = totp::generate_secret();
    let uri = totp::otpauth_uri(&current.email, secret.as_ref());
    let response = TotpSetupResponse {
        secret: totp::secret_base32(secret.as_ref()).to_string(),
        otpauth_uri: uri.to_string(),
        qr_svg: qr_svg(&uri)?,
        setup_id: state
            .totp_setups
            .start(user.session_id, user.id, secret, Instant::now()),
    };
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TotpConfirmRequest {
    setup_id: String,
    code: Secret,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TotpChanged {
    totp_enabled: bool,
}

/// Verifies a code against the pending secret, then seals and stores it.
/// The first enrollment on a machine creates `server.key`.
async fn totp_confirm(
    State(state): State<AppState>,
    principal: Principal,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Result<SecretJson<TotpConfirmRequest>, ApiError>,
) -> ApiResult<Json<TotpChanged>> {
    let user = signed_in(principal)?;
    let SecretJson(request) = body?;
    let expired = || {
        ApiError::conflict(
            "That setup has expired. Start again.",
            serde_json::json!({ "reason": "setup_expired" }),
        )
    };
    let (owner, secret) = state
        .totp_setups
        .secret_for(user.session_id, &request.setup_id, Instant::now())
        .ok_or_else(expired)?;
    if owner != user.id {
        return Err(expired());
    }
    let now = state.now();
    let step = totp::matching_step(secret.as_ref(), request.code.expose(), now)
        .ok_or_else(ApiError::code_mismatch)?;

    let id = user.id;
    super::with_conn(&state, move |conn| {
        let key = ServerKey::load_or_create(&seal::key_path())?;
        users::set_totp_secret(conn, id, &key, secret.as_ref(), step, now)
    })
    .await?;
    state.totp_setups.finish(user.session_id);
    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    emit(&event(EventKind::TotpEnabled, &user, &ip));
    Ok(Json(TotpChanged { totp_enabled: true }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TotpRemoveRequest {
    current_password: Secret,
    code: Secret,
}

/// Clears the second factor, given the password and a current code.
async fn totp_remove(
    State(state): State<AppState>,
    principal: Principal,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Result<SecretJson<TotpRemoveRequest>, ApiError>,
) -> ApiResult<Json<TotpChanged>> {
    let user = signed_in(principal)?;
    let SecretJson(request) = body?;
    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    check_current_password(&state, &user, &ip, request.current_password).await?;

    let (id, now, code) = (user.id, state.now(), request.code);
    let check = super::with_conn(&state, move |conn| {
        let Some(key) = ServerKey::load(&seal::key_path())? else {
            return Ok(TotpCheck::Unopenable);
        };
        let check = users::verify_totp(conn, id, &key, code.expose(), now)?;
        if check == TotpCheck::Accepted {
            users::clear_totp(conn, id, now)?;
        }
        Ok(check)
    })
    .await?;

    match check {
        TotpCheck::Accepted => {
            emit(&event(EventKind::TotpRemoved, &user, &ip));
            Ok(Json(TotpChanged {
                totp_enabled: false,
            }))
        }
        TotpCheck::NotEnrolled => Err(ApiError::conflict(
            "Two-step verification is not on.",
            serde_json::json!({ "reason": "totp_not_enabled" }),
        )),
        TotpCheck::Rejected => Err(ApiError::code_mismatch()),
        TotpCheck::Unopenable => Err(ApiError::totp_unavailable()),
    }
}
```

Mount in `routes/mod.rs` `api_router`: `.merge(account::routes())` after `sessions::routes()`.

`users::change_password`'s closure returns `Result<(), PasswordChangeError>`; `get_connection(&db_path)?` converts through `From<NigelError> for PasswordChangeError`. `with_conn`'s closure returns `crate::error::Result<T>`; the `?` on `ServerKey::load_or_create` and `set_totp_secret` is already that type.

- [ ] **Step 6: Refresh the shells' lockfiles**

Run: `cargo update -p qrcode --manifest-path crates/nigel-desktop/Cargo.toml && cargo update -p qrcode --manifest-path crates/nigel-ffi/Cargo.toml`
(If `cargo update -p` complains the package is not in the lockfile yet, run `cargo metadata --format-version 1 --manifest-path <that Cargo.toml> > /dev/null` instead, which adds it.) Then `git diff --stat crates/nigel-desktop/Cargo.lock crates/nigel-ffi/Cargo.lock` should show only `qrcode` (and nothing it pulls in with `default-features = false`).

- [ ] **Step 7: Run the tests**

Run: `cargo test -p nigel-core server:: -- --test-threads=1`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
cargo fmt
git add crates/nigel-core crates/nigel-desktop/Cargo.lock crates/nigel-ffi/Cargo.lock
git commit -m "Add the account routes: password change and two-step

A signed-in user changes their password (ending their other sessions),
and sets up, confirms and removes two-step verification. Setup holds the
new secret in memory for ten minutes and draws its QR code as SVG; the
first confirmation on a machine creates server.key. Wrong current
passwords count against the sign-in limits."
```

---
### Task 11: Boot — unlock from the environment, startup banner and notices

Spec §3 (startup warning), §4.2 step 5 (sweep at startup), §11.3, §11.4. Closes the "warn at serve boot if `server.key`'s permissions are loose" part of **PR 1 review item 1** and the "dummy_phc warmed at startup" part of **item 2**.

**Files:**
- Create: `crates/nigel-core/src/server/boot.rs`
- Modify: `crates/nigel-core/src/server/mod.rs` (`pub mod boot;`, `run`, `serve`)
- Modify: `crates/nigel/src/cli/serve.rs` (`preflight` unlocks from the environment)

**Interfaces:**
- Consumes: `db::env_password_from` (Task 1, now `pub(crate)`), `mode::derive`, `ServerConfig::from_settings`, `seal::{permission_warning, key_path, ServerKey}`, `users::totp_unopenable`, `sessions::sweep_expired`, `passwords::warm_dummy_hash`, `identity::NOT_ENCRYPTED_WARNING`.
- Produces: `boot::{unlock_with, unlock_from_env, BootFacts, read_facts, startup_notices, serving_banner}`.

- [ ] **Step 1: Write the failing tests**

Create `boot.rs` with the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::config::{ClientIpHeader, PublicUrl, ServerConfig};
    use crate::server::testutil::{encrypt, temp_db, PASSWORD};

    fn public() -> ServerConfig {
        ServerConfig {
            public_url: Some(PublicUrl::parse("https://books.example.com").unwrap()),
            client_ip_header: None,
        }
    }

    #[test]
    fn a_password_in_the_environment_unlocks_encrypted_books_at_boot() {
        let (_dir, db_path) = temp_db();
        encrypt(&db_path);
        crate::db::set_db_password_for_tests(None);
        assert!(!unlock_with(&db_path, None, None).unwrap(), "nothing set: stays locked");
        assert!(crate::db::get_db_password().is_none());

        assert!(unlock_with(&db_path, Some(PASSWORD.into()), None).unwrap());
        assert!(crate::db::get_db_password().is_some());
        crate::db::set_db_password_for_tests(None);

        let err = unlock_with(&db_path, Some("wrong".into()), None).unwrap_err().to_string();
        assert!(err.contains("did not unlock"), "{err}");
        assert!(crate::db::get_db_password().is_none());
    }

    #[test]
    fn local_mode_with_a_public_url_says_why_it_is_ignored() {
        let notices = startup_notices(Mode::Local, &BootFacts::default(), &public(), None);
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(notices[0].contains("server_public_url"), "{notices:?}");
        assert!(notices[0].contains("nigel user add"), "{notices:?}");
        assert!(startup_notices(Mode::Local, &BootFacts::default(), &ServerConfig::default(), None).is_empty());
    }

    #[test]
    fn shared_mode_warns_about_plain_books_a_missing_ip_header_and_unopenable_two_step() {
        let facts = BootFacts {
            encrypted: false,
            company: Some("Acme".into()),
            totp_unopenable: vec!["sam@example.com".into()],
        };
        let notices = startup_notices(Mode::Shared, &facts, &public(), Some("warning: key".into()));
        let all = notices.join("\n");
        assert!(all.contains(crate::identity::NOT_ENCRYPTED_WARNING), "{all}");
        assert!(all.contains("client_ip_header"), "{all}");
        assert!(all.contains("sam@example.com") && all.contains("--clear-totp"), "{all}");
        assert!(all.contains("warning: key"), "{all}");

        let quiet = startup_notices(
            Mode::Shared,
            &BootFacts { encrypted: true, ..BootFacts::default() },
            &ServerConfig {
                client_ip_header: Some(ClientIpHeader::CfConnectingIp),
                ..public()
            },
            None,
        );
        assert!(quiet.is_empty(), "{quiet:?}");
    }

    #[test]
    fn the_banner_prints_a_token_only_in_local_mode() {
        let local = serving_banner(Mode::Local, None, &ServerConfig::default(), 5731, Some("ab12"));
        assert!(local.contains("http://127.0.0.1:5731/auth?token=ab12"), "{local}");

        let shared = serving_banner(Mode::Shared, Some("Acme"), &public(), 5731, None);
        assert_eq!(
            shared.lines().next().unwrap(),
            "Nigel is serving Acme to signed-in users at https://books.example.com (and http://127.0.0.1:5731)."
        );
        assert!(!shared.contains("token"), "{shared}");

        let loopback_only = serving_banner(Mode::Shared, None, &ServerConfig::default(), 5731, None);
        assert_eq!(
            loopback_only.lines().next().unwrap(),
            "Nigel is serving these books to signed-in users at http://127.0.0.1:5731."
        );
    }

    #[test]
    fn boot_facts_sweep_expired_sessions_and_name_the_company() {
        let (_dir, db_path) = temp_db();
        let conn = crate::db::get_connection(&db_path).unwrap();
        crate::db::set_metadata(&conn, "company_name", "Acme").unwrap();
        let user = crate::server::testutil::add_ready_user(&db_path, "pat@example.com", crate::identity::Role::Admin);
        let long_ago = chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z").unwrap().with_timezone(&chrono::Utc);
        crate::server::testutil::session_for(&db_path, user.id, crate::identity::sessions::SessionKind::Cookie, long_ago);

        let facts = read_facts(&db_path, Mode::Shared, crate::clock::now_utc()).unwrap();
        assert_eq!(facts.company.as_deref(), Some("Acme"));
        assert!(!facts.encrypted);
        let left: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
        assert_eq!(left, 0, "the expired session was swept");
    }
}
```

In `crates/nigel/src/cli/serve.rs` `mod tests`, add:

```rust
    #[cfg(feature = "serve")]
    #[test]
    fn preflight_leaves_encrypted_books_locked_without_a_password_variable() {
        nigel_core::db::set_db_password_for_tests(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("nigel.db");
        let conn = nigel_core::db::open_connection(&db_path, None).expect("open");
        nigel_core::db::init_db(&conn).expect("init");
        drop(conn);
        nigel_core::password::encrypt_database(&db_path, "correct horse battery staple").expect("encrypt");
        nigel_core::db::set_db_password_for_tests(None);

        preflight_with(&db_path, None, None).expect("preflight");
        assert!(nigel_core::db::get_db_password().is_none());

        preflight_with(&db_path, Some("correct horse battery staple".into()), None).expect("preflight");
        assert!(nigel_core::db::get_db_password().is_some());
        nigel_core::db::set_db_password_for_tests(None);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server::boot -- --test-threads=1`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `boot.rs`**

```rust
//! What `nigel serve` does before it binds: unlock the books from the
//! environment, read what the startup notices need, and say how it is
//! serving.

use std::ffi::OsString;
use std::path::Path;

use chrono::{DateTime, Utc};

use super::config::ServerConfig;
use super::mode::Mode;
use crate::error::Result;

/// Unlock encrypted books with a password from `NIGEL_DB_PASSWORD_FILE` or
/// `NIGEL_DB_PASSWORD` (`file` and `raw`), then leave it in the process-wide
/// slot. `Ok(false)` when the books are not encrypted or neither variable is
/// set; a wrong or unreadable password is an error, and fatal at startup.
pub fn unlock_with(db_path: &Path, raw: Option<OsString>, file: Option<OsString>) -> Result<bool> {
    if !db_path.exists() || !crate::db::is_encrypted(db_path)? {
        return Ok(false);
    }
    match crate::db::env_password_from(db_path, raw, file)? {
        Some(password) => {
            crate::db::set_db_password(Some(zeroize::Zeroizing::new(password)));
            Ok(true)
        }
        None => Ok(false),
    }
}

/// [`unlock_with`] from the process environment.
pub fn unlock_from_env(db_path: &Path) -> Result<bool> {
    unlock_with(
        db_path,
        std::env::var_os("NIGEL_DB_PASSWORD"),
        std::env::var_os("NIGEL_DB_PASSWORD_FILE"),
    )
}

/// What the banner and notices need from the books.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BootFacts {
    pub encrypted: bool,
    pub company: Option<String>,
    /// Users whose two-step this machine cannot check (shared mode only).
    pub totp_unopenable: Vec<String>,
}

/// Read the facts, and in shared mode sweep expired sessions.
pub fn read_facts(db_path: &Path, mode: Mode, now: DateTime<Utc>) -> Result<BootFacts> {
    if !super::routes::status::initialized(db_path) {
        return Ok(BootFacts::default());
    }
    let encrypted = crate::db::is_encrypted(db_path)?;
    if encrypted && crate::db::get_db_password().is_none() {
        return Ok(BootFacts {
            encrypted,
            ..BootFacts::default()
        });
    }
    let conn = crate::db::get_connection(db_path)?;
    let company = crate::db::get_metadata(&conn, "company_name").filter(|c| !c.trim().is_empty());
    let totp_unopenable = if mode == Mode::Shared {
        crate::identity::sessions::sweep_expired(&conn, now)?;
        let key = crate::identity::seal::ServerKey::load(&crate::identity::seal::key_path())
            .ok()
            .flatten();
        crate::identity::users::totp_unopenable(&conn, key.as_ref())?
    } else {
        Vec::new()
    };
    Ok(BootFacts {
        encrypted,
        company,
        totp_unopenable,
    })
}

/// The notices `nigel serve` prints to stderr before it binds.
pub fn startup_notices(
    mode: Mode,
    facts: &BootFacts,
    config: &ServerConfig,
    key_warning: Option<String>,
) -> Vec<String> {
    let mut notices = Vec::new();
    match mode {
        Mode::Local => {
            if config.public_url.is_some() {
                notices.push(
                    "notice: server_public_url is set, but these books have no users (or are \
                     still locked), so Nigel accepts loopback requests only. Add the first user \
                     with `nigel user add` to share them."
                        .to_string(),
                );
            }
        }
        Mode::Shared => {
            if !facts.encrypted {
                notices.push(crate::identity::NOT_ENCRYPTED_WARNING.to_string());
            }
            if config.public_url.is_some() && config.client_ip_header.is_none() {
                notices.push(
                    "notice: server_public_url is set and client_ip_header is not, so every \
                     client behind your proxy shares one sign-in rate limit. Set \
                     client_ip_header to \"CF-Connecting-IP\" or \"X-Forwarded-For\"."
                        .to_string(),
                );
            }
            if !facts.totp_unopenable.is_empty() {
                notices.push(format!(
                    "warning: two-step verification can't be checked for {} — this machine's \
                     server.key is missing or is not the one their secrets were sealed with. \
                     Recover each with `nigel user reset-password <email> --clear-totp`.",
                    facts.totp_unopenable.join(", ")
                ));
            }
        }
    }
    notices.extend(key_warning);
    notices
}

/// What `nigel serve` prints to stdout once it is listening.
pub fn serving_banner(
    mode: Mode,
    company: Option<&str>,
    config: &ServerConfig,
    port: u16,
    token: Option<&str>,
) -> String {
    let loopback = format!("http://127.0.0.1:{port}");
    match (mode, token) {
        (Mode::Local, Some(token)) => format!(
            "Nigel is serving on {loopback}\nOpen this URL to start a session:\n  {loopback}/auth?token={token}\nPress Ctrl-C to stop."
        ),
        _ => {
            let books = company.unwrap_or("these books");
            let first = match &config.public_url {
                Some(public) => format!(
                    "Nigel is serving {books} to signed-in users at {} (and {loopback}).",
                    public.origin()
                ),
                None => format!("Nigel is serving {books} to signed-in users at {loopback}."),
            };
            format!("{first}\nPress Ctrl-C to stop.")
        }
    }
}
```

Add `pub mod boot;` to `server/mod.rs`. Check `crate::db::set_metadata` exists with that signature (it does, `db.rs` line ~1138); `db::set_db_password` takes `Option<Zeroizing<String>>`.

- [ ] **Step 4: Wire `serve`**

In `server/mod.rs`:

```rust
/// Start the server and block until shutdown.
pub fn run(port: u16, no_open: bool) -> Result<()> {
    disable_ansi_output();

    let settings = crate::settings::try_load_settings()?;
    let config = config::ServerConfig::from_settings(&settings)?;
    let db_path = PathBuf::from(settings.data_dir).join("nigel.db");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| NigelError::Other(format!("Couldn't start the async runtime: {e}")))?;

    runtime.block_on(serve(db_path, port, no_open, config))
}

async fn serve(
    db_path: PathBuf,
    port: u16,
    no_open: bool,
    config: config::ServerConfig,
) -> Result<()> {
    let mode = mode::derive(&db_path).unwrap_or_else(|e| {
        eprintln!(
            "notice: could not read the users in {} ({e}). Serving in local mode.",
            db_path.display()
        );
        mode::Mode::Local
    });
    crate::identity::passwords::warm_dummy_hash();
    let facts = boot::read_facts(&db_path, mode, crate::clock::now_utc()).unwrap_or_else(|e| {
        eprintln!("notice: could not read these books before starting ({e}).");
        boot::BootFacts::default()
    });
    let key_warning = crate::identity::seal::permission_warning(&crate::identity::seal::key_path());
    for notice in boot::startup_notices(mode, &facts, &config, key_warning) {
        eprintln!("{notice}");
    }

    let state = AppState::for_serve(db_path, mode, config.clone());
    uploads::purge_stale(&uploads::uploads_dir(&state.db_path()), uploads::MAX_AGE);
    spawn_update_check(state.clone());

    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await?;
    let port = listener.local_addr()?.port();
    let token = state.local_token();
    println!(
        "{}",
        boot::serving_banner(
            mode,
            facts.company.as_deref(),
            &config,
            port,
            token.as_ref().map(|t| t.expose()),
        )
    );

    if !no_open {
        let url = match &token {
            Some(token) => format!("http://127.0.0.1:{port}/auth?token={}", token.expose()),
            None => format!("http://127.0.0.1:{port}/"),
        };
        if let Err(e) = open::that_detached(&url) {
            eprintln!("notice: couldn't open a browser ({e}) — open the URL above yourself.");
        }
    }

    serve_with_shutdown(listener, state, shutdown_signal()).await?;
    println!("Server stopped.");
    Ok(())
}
```

Remove the interim token printing Task 4 added.

- [ ] **Step 5: Unlock in the CLI pre-flight**

In `crates/nigel/src/cli/serve.rs`:

```rust
/// Unlock encrypted books from `NIGEL_DB_PASSWORD_FILE` or `NIGEL_DB_PASSWORD`
/// when either is set, then migrate whatever this process can open; leave an
/// absent database absent.
///
/// With neither variable, encrypted books stay locked — local mode, unlocked
/// over loopback — and the unlock endpoint runs their migrations. An absent
/// database is left for the setup gate.
pub(crate) fn preflight(db_path: &Path) -> Result<()> {
    preflight_with(
        db_path,
        std::env::var_os("NIGEL_DB_PASSWORD"),
        std::env::var_os("NIGEL_DB_PASSWORD_FILE"),
    )
}

fn preflight_with(
    db_path: &Path,
    raw: Option<std::ffi::OsString>,
    file: Option<std::ffi::OsString>,
) -> Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    if nigel_core::db::is_encrypted(db_path)? && !unlock(db_path, raw, file)? {
        return Ok(());
    }
    let conn = nigel_core::db::get_connection(db_path)?;
    nigel_core::db::init_db(&conn)
}

#[cfg(feature = "serve")]
fn unlock(
    db_path: &Path,
    raw: Option<std::ffi::OsString>,
    file: Option<std::ffi::OsString>,
) -> Result<bool> {
    nigel_core::server::boot::unlock_with(db_path, raw, file)
}

#[cfg(not(feature = "serve"))]
fn unlock(
    _db_path: &Path,
    _raw: Option<std::ffi::OsString>,
    _file: Option<std::ffi::OsString>,
) -> Result<bool> {
    Ok(false)
}
```

`nigel_core::server` exists only with `serve`, hence the two `unlock`s; `cargo test --no-default-features -p nigel` checks the second one compiles.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p nigel-core server:: -- --test-threads=1 && cargo test -p nigel cli::serve -- --test-threads=1 && cargo test --no-default-features -- --test-threads=1`
Expected: PASS.

- [ ] **Step 7: Run it by hand**

```bash
cargo build -q
NIGEL="$PWD/target/debug/nigel"
SCRATCH=$(mktemp -d)
HOME="$SCRATCH" "$NIGEL" init --data-dir "$SCRATCH/books"
HOME="$SCRATCH" timeout 3 "$NIGEL" serve --no-open --port 0     # the token banner, then exits
HOME="$SCRATCH" "$NIGEL" user add pat@example.com --name "Pat Example" --admin
HOME="$SCRATCH" NIGEL_SERVER_PUBLIC_URL=https://books.example.com timeout 3 "$NIGEL" serve --no-open --port 0
rm -rf "$SCRATCH"
```

`HOME` is set per command, never exported, so cargo and rustup keep the real one. Expected: the last run prints the not-encrypted warning and the `client_ip_header` notice on stderr, and `Nigel is serving these books to signed-in users at https://books.example.com (and http://127.0.0.1:<port>).` with no token.

- [ ] **Step 8: Commit**

```bash
cargo fmt
git add crates/nigel-core/src/server crates/nigel/src/cli/serve.rs
git commit -m "Unlock and describe the server at startup

nigel serve unlocks encrypted books from NIGEL_DB_PASSWORD_FILE or
NIGEL_DB_PASSWORD before it binds, derives the mode, sweeps expired
sessions, and warms the dummy hash. In shared mode it prints no token and
warns about plain books, a missing client_ip_header, a loose server.key
and users whose two-step this machine cannot check."
```

---

### Task 12: The end-to-end exposure test (security-critical)

Spec §16 "End-to-end", §14 "Exposure before per-user login exists".

**Files:**
- Modify: `crates/nigel/tests/common/mod.rs` (`pub fn binary_path()`)
- Create: `crates/nigel/tests/serve_exposure.rs`

**Interfaces:**
- Consumes: the built `nigel` binary; `nigel_core::password::encrypt_database`.
- Produces: nothing.

- [ ] **Step 1: Expose the harness's binary**

In `crates/nigel/tests/common/mod.rs`:

```rust
/// The binary path [`TestEnv::cmd`] runs, for a test that needs a long-lived
/// child process rather than an `assert_cmd` run.
pub fn binary_path() -> &'static std::path::Path {
    private_binary()
}
```

- [ ] **Step 2: Write the test**

Create `crates/nigel/tests/serve_exposure.rs`:

```rust
//! `nigel serve` is not reachable through a public hostname until the books
//! have a user: the real binary, raw HTTP over TCP.

#![cfg(feature = "serve")]

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use common::TestEnv;

const PUBLIC_URL: &str = "https://books.example.com";
const PUBLIC_HOST: &str = "books.example.com";
const BOOKS_PASSWORD: &str = "correct horse battery staple";

struct Server {
    child: Child,
    port: u16,
    token: Option<String>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn port_in(line: &str) -> Option<u16> {
    let rest = line.split("http://127.0.0.1:").nth(1)?;
    rest.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

fn serve(env: &TestEnv, vars: &[(&str, &str)]) -> Server {
    let mut cmd = Command::new(common::binary_path());
    cmd.env("HOME", env.home.path());
    for var in common::INVOICING_ENV_VARS.iter().chain(&[
        "NIGEL_DB_PASSWORD",
        "NIGEL_DB_PASSWORD_FILE",
        "NIGEL_SERVER_PUBLIC_URL",
        "NIGEL_CLIENT_IP_HEADER",
    ]) {
        cmd.env_remove(var);
    }
    for (key, value) in vars {
        cmd.env(key, value);
    }
    cmd.args(["serve", "--no-open", "--port", "0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().expect("spawn nigel serve");

    let stdout = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let (mut port, mut token) = (None, None);
    loop {
        let line = rx
            .recv_timeout(Duration::from_secs(60))
            .expect("nigel serve printed its banner");
        port = port.or_else(|| port_in(&line));
        if let Some(t) = line.split("/auth?token=").nth(1) {
            token = Some(t.trim().to_string());
        }
        if line.starts_with("Press Ctrl-C") {
            break;
        }
    }
    Server {
        child,
        port: port.expect("a port in the banner"),
        token,
    }
}

/// `(status, lower-cased head, body)` of one HTTP/1.1 exchange.
fn exchange(port: u16, request: &str) -> (u16, String, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("a status line");
    (status, head.to_ascii_lowercase(), body.to_string())
}

fn get(port: u16, host: &str, path: &str) -> (u16, String, String) {
    exchange(
        port,
        &format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
    )
}

fn login(port: u16, host: &str, origin: &str, email: &str, password: &str) -> (u16, String, String) {
    let body = format!(r#"{{"email":"{email}","password":"{password}"}}"#);
    exchange(
        port,
        &format!(
            "POST /api/auth/login HTTP/1.1\r\nHost: {host}\r\nOrigin: {origin}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    )
}

fn init_books(env: &TestEnv) {
    env.cmd()
        .args(["init", "--data-dir"])
        .arg(env.data_dir())
        .assert()
        .success();
}

/// Add the first admin and answer their temporary password.
fn add_admin(env: &TestEnv) -> String {
    let output = env
        .cmd()
        .args(["user", "add", "pat@example.com", "--name", "Pat Example", "--admin"])
        .output()
        .expect("nigel user add");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("Temporary password: "))
        .expect("a temporary password")
        .trim()
        .to_string()
}

#[test]
fn a_public_host_is_refused_while_the_books_have_no_users() {
    let env = TestEnv::new();
    init_books(&env);
    let server = serve(&env, &[("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL)]);
    assert!(server.token.is_some(), "local mode prints the token link");

    let (status, _, body) = get(server.port, PUBLIC_HOST, "/api/ping");
    assert_eq!(status, 403, "{body}");
    assert!(body.contains("forbidden"), "{body}");

    let (status, _, _) = get(server.port, &format!("127.0.0.1:{}", server.port), "/api/ping");
    assert_eq!(status, 401);
}

#[test]
fn the_first_user_turns_on_shared_mode_and_the_public_host() {
    let env = TestEnv::new();
    init_books(&env);
    let old_token = serve(&env, &[]).token.expect("a local token");
    let temporary = add_admin(&env);

    let server = serve(&env, &[("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL)]);
    assert!(server.token.is_none(), "shared mode prints no token");
    let loopback = format!("127.0.0.1:{}", server.port);

    let (status, _, body) = get(server.port, PUBLIC_HOST, "/api/ping");
    assert_eq!(status, 401, "{body}");
    assert!(body.contains(r#""mode":"shared""#), "{body}");

    let (status, _, _) = get(server.port, "other.example.com", "/api/ping");
    assert_eq!(status, 403);

    let (status, head, _) = get(server.port, &loopback, &format!("/auth?token={old_token}"));
    assert_eq!(status, 401);
    assert!(!head.contains("set-cookie"), "{head}");

    let (status, head, body) = login(server.port, PUBLIC_HOST, PUBLIC_URL, "pat@example.com", &temporary);
    assert_eq!(status, 200, "{body}");
    let cookie = head
        .lines()
        .find(|l| l.starts_with("set-cookie:"))
        .expect("a session cookie");
    for part in ["nigel_session=", "httponly", "samesite=strict", "secure"] {
        assert!(cookie.contains(part), "{part}: {cookie}");
    }
    assert!(head.contains("strict-transport-security: max-age=31536000"), "{head}");
    assert!(body.contains(r#""mustChangePassword":true"#), "{body}");

    let (status, head, _) = login(
        server.port,
        &loopback,
        &format!("http://{loopback}"),
        "pat@example.com",
        &temporary,
    );
    assert_eq!(status, 200);
    let cookie = head.lines().find(|l| l.starts_with("set-cookie:")).unwrap();
    assert!(!cookie.contains("secure"), "loopback cookies are plain http: {cookie}");
    assert!(!head.contains("strict-transport-security"), "{head}");
}

#[test]
fn locked_books_are_local_until_the_server_unlocks_them_at_boot() {
    let env = TestEnv::new();
    init_books(&env);
    add_admin(&env);
    nigel_core::password::encrypt_database(&env.data_dir().join("nigel.db"), BOOKS_PASSWORD)
        .expect("encrypt the books");

    {
        let server = serve(&env, &[("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL)]);
        assert!(server.token.is_some(), "locked books are local");
        let (status, _, _) = get(server.port, PUBLIC_HOST, "/api/ping");
        assert_eq!(status, 403);
    }

    let password_file = env.home.path().join("books-password");
    std::fs::write(&password_file, format!("{BOOKS_PASSWORD}\n")).unwrap();
    let server = serve(
        &env,
        &[
            ("NIGEL_SERVER_PUBLIC_URL", PUBLIC_URL),
            ("NIGEL_DB_PASSWORD_FILE", password_file.to_str().unwrap()),
        ],
    );
    assert!(server.token.is_none(), "unlocked at boot: shared");
    let (status, _, body) = get(server.port, PUBLIC_HOST, "/api/ping");
    assert_eq!(status, 401, "{body}");
    assert!(body.contains(r#""mode":"shared""#), "{body}");
}
```

If `nigel init --data-dir` in this harness is spelled differently, copy the exact form from an existing `cli_dispatch.rs` test that inits fresh books. If `nigel_core::password` is not `pub` from the `nigel` test crate's point of view, use the helper the existing `NIGEL_DB_PASSWORD_FILE` tests in `cli_dispatch.rs` (~line 5050) use to encrypt.

- [ ] **Step 3: Run it**

Run: `cargo test -p nigel --test serve_exposure -- --test-threads=1`
Expected: PASS (three tests). Then `cargo test --no-default-features -p nigel --test serve_exposure` should compile to zero tests.

If a request hangs, the response is likely chunked and the connection is kept open: check that `Connection: close` is in every request.

- [ ] **Step 4: Commit**

```bash
git add crates/nigel/tests/common/mod.rs crates/nigel/tests/serve_exposure.rs
git commit -m "Test that serve is not exposed before a user exists

Starts the real binary and speaks raw HTTP: a public Host is refused with
no users and with locked books; after the first user it reaches the
session layer, the old token link is refused, and login over the public
name returns a Secure cookie and HSTS while loopback gets neither."
```

---

### Task 13: Docs, the TypeScript mirror, and the full verification

**Files:**
- Modify: `docs/api.md`, `docs/design-constraints.md`, `docs/architecture.md`, `CHANGELOG.md`
- Modify: `web/apps/app/src/api/types.ts`

- [ ] **Step 1: Mirror the API in TypeScript**

In `web/apps/app/src/api/types.ts`, add to `StatusResponse` after `locked`:

```ts
  /**
   * `local` (no users, or still locked) or `shared` (signed-in users only).
   * A desktop shell is always `local`.
   */
  mode?: 'local' | 'shared';
```

and add to `API_ERROR_CODES`: `'invalid_credentials'`, `'totp_required'`, `'password_change_required'`, `'too_many_attempts'`. No other SPA change.

Run (from `web/`): `npm ci && npm run typecheck && npm test && npm run lint`
Expected: PASS.

- [ ] **Step 2: `docs/api.md`**

Replace the "Running the server" paragraph after the code block and the whole "Security model" section with current-state text covering, in this order:

1. **Modes.** Local (no users, or encrypted and locked): what it always was — token link, `nigel_session` cookie, loopback only. Shared (books open with at least one user): signed-in users only; the token link answers `401` with `details.mode = "shared"`; non-loopback `Host` accepted only for `server_public_url`. The mode is derived, re-derived after an unlock, a data-directory switch, and on a public-host request while local (at most every 2 s), and never returns to local within a run.
2. **Bind** (unchanged: `127.0.0.1` only).
3. **Host/Origin.** Loopback names on any port in both modes; in shared mode also the `server_public_url` host, and its origin compared whole (scheme, host, port).
4. **Sessions.** Local: the per-run token. Shared: `Authorization: Bearer <hex>` or the `nigel_session` cookie, looked up by SHA-256, joined to an enabled user; the cookie attributes; 30-day idle / 90-day absolute; hourly renewal with a fresh `Set-Cookie` whose `Max-Age` is the time remaining; a session answers only on the transport it was issued for; the must-change allowlist.
5. **CSRF.** `SameSite=Strict`; Host; Origin; in shared mode a cookie write without `Origin` is `403` with `details.reason = "origin_required"`; a browser login needs `Origin`; bearer and native logins are exempt; GET never writes.
6. **Headers.** `nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: same-origin`, `Cache-Control: no-store` on `/api`, HSTS on the public host only.
7. **Who may call what.** Until the role table lands, a signed-in user who is not an admin may call only `GET /api/ping`, `GET /api/auth/me`, `POST /api/auth/logout` and `POST /api/account/*`; everything else answers `403` with `details: { "reason": "role", "required": "admin" }`. The local owner (token session, desktop) may call everything.
8. **Rate limits** (the table from spec §4.4, plus "charged when admitted", the 15-minute idle reset, and that account routes checking a current password share them) and **`client_ip_header`**.
9. **Boot unlock** (`NIGEL_DB_PASSWORD_FILE` / `NIGEL_DB_PASSWORD`) and what `nigel serve` prints in each mode, including the notices.
10. **Event lines** — the format from Ruling 17, one per event kind listed in `EventKind`.

Keep the paragraph about the two routers, adding: the desktop router has no session layer and runs every request as the local owner, so `/api/auth/me` answers `local` there whatever the books hold.

In "Locked state", list `POST /api/auth/login`, `POST /api/auth/logout` and `GET /api/auth/me` among the routes that answer while locked.

Add rows to the error table:

```markdown
| `invalid_credentials` | 401 | Login refused: the email, password, user status or code didn't match |
| `totp_required` | 401 | The password was correct; send it again with `code` |
| `password_change_required` | 403 | This session's user must change a temporary password first |
| `too_many_attempts` | 429 | Login rate limit; `details.retryAfterMs` says how long |
```

and change the `unauthorized` row's meaning to "Missing or invalid session; in shared mode `details.mode = "shared"`", the `forbidden` row's to "Host/Origin check failed, a cookie write without `Origin` (`details.reason = "origin_required"`), or the role check (`details.reason = "role"`, `details.required`)", and the `conflict` row's to include `local_mode`.

Under `GET /api/status`, document `mode`. Add a new "### Signing in" subsection under Endpoints with `POST /api/auth/login` (body, both `client` values, every answer from spec §4.1, the 8 KiB cap), `POST /api/auth/logout`, `GET /api/auth/me` (both shapes), and an "### Your account" subsection with the four account routes, their bodies, answers and `details.reason` values from Ruling 15, the 8 KiB cap, and that setup holds the secret for 10 minutes per session and the first confirmation creates `server.key`.

- [ ] **Step 3: `docs/design-constraints.md`**

Add these bullets next to the existing identity bullet (line ~102), describing the current state:

```markdown
- **The server's mode is derived from the books and only ever widens within a run.** `server::mode::derive` reads shared when the books exist, this process holds their key, and `identity::users::has_users` is true; absent, empty, locked and unreadable books are local. `AppState::for_serve` is the only state that re-derives (after an unlock, a data-directory switch, and on a public-host request while local, at most every `REFRESH_INTERVAL`), and `ModeCell::enter_shared` is one-way, dropping the token, so a stale reading is always the one that accepts less. `AppState::new` is pinned local: a desktop shell runs every request as the local owner whatever the books hold. Nothing turns shared mode on but a user existing, so there is no state where a non-loopback host is accepted and no per-user login exists.
- **The public host is one whole origin, and only shared mode accepts it.** `served_host_guard` trusts loopback by its three names and, in shared mode, `server_public_url`'s host and origin compared whole; `PublicUrl::parse` refuses anything but one https origin and refuses loopback names, because `Secure` cookies and HSTS key off the request's Host matching it and must never reach `localhost`. The client IP comes from `client_ip_header` only when the TCP peer is loopback.
- **A session is issued only from a completed sign-in, and answers only on its own transport.** `sessions::create_session` consumes a `SessionGrant`, which only `users::complete_sign_in` constructs; `verify_credentials` runs exactly one Argon2id verification on every path (the warmed dummy hash for an unknown address). The session layer refuses a cookie session presented as a bearer token and the reverse, because the CSRF rule exempts bearer requests. A cookie write in shared mode needs an `Origin`; a browser login needs one too.
- **Sign-in attempts are charged when admitted.** `LoginLimiter::begin` counts the attempt before any password is checked and the handler settles it, so parallel guesses cannot all pass the check, and a locked bucket costs no Argon2 time. Account routes that check a current password go through the same limiter, so a stolen session is not a password oracle.
- **Until the role table lands, every non-account route needs an admin.** `authz::interim_admin_guard` lets any signed-in role reach only `GET /api/ping`, its own session and its own account; it fails closed for unknown paths too. The local owner passes it.
- **Nothing secret reaches a log line.** `server::events::AuthEvent` has no field that can hold a password, code or token (`reason` is `&'static str`), and the email is cut and `{:?}`-escaped because at a failed login it is whatever was typed.
```

- [ ] **Step 4: `docs/architecture.md`**

In the **Web server** bullet (line ~38), update `auth.rs` to "the per-run token, the mode-aware host guard (`served_host_guard`) and the desktop's static one, the session layer (`session_guard`: local token or shared cookie/bearer session, renewal, the must-change allowlist and the Origin rule) and cookie helpers", `state.rs` to add "the mode cell, server config, clock, sign-in limiter and pending TOTP setups", and append: "`mode.rs` derives local or shared from the books; `config.rs` reads `server_public_url` and `client_ip_header`; `principal.rs` is the `Principal` every `/api` request carries; `authz.rs` holds the interim admin guard and the desktop's local-owner layer; `ratelimit.rs` the sign-in limits and client IP; `events.rs` the stderr event lines; `pending_totp.rs` two-step enrollments in progress; `boot.rs` the startup unlock, notices and banner; `routes/sessions.rs` login/logout/me; `routes/account.rs` the signed-in user's password and two-step."

In the project tree (line ~173), add one line each for `config.rs`, `events.rs`, `mode.rs`, `principal.rs`, `authz.rs`, `ratelimit.rs`, `pending_totp.rs`, `boot.rs` under `server/`, and `sessions.rs`, `account.rs` under `routes/`, each with a short `#` comment in the existing style; update `auth.rs` and `state.rs`'s comments to match the bullet.

- [ ] **Step 5: `CHANGELOG.md`**

Under `## [Unreleased]` → `### Added`, add first:

```markdown
- **Sign-in for a shared server** — once the books have a user (`nigel user add`), `nigel serve` admits only signed-in users: `POST /api/auth/login` with email and password (and a six-digit code when two-step is on) returns an `HttpOnly`, `SameSite=Strict` session cookie, or a bearer token for a native client. Sessions end after 30 days unused and 90 days after sign-in, and renew as they are used. Five wrong passwords lock an account for 15 minutes, and twenty from one address lock that address. A user with a temporary password must choose their own before anything else, and can turn two-step verification on and off for themselves. Until roles arrive, only admins can reach the books. Books with no users work exactly as before. The web app's sign-in screen is not built yet; use the API (see "Signing in" in `docs/api.md`)
- **`server_public_url` and `client_ip_header`** — the one https address a shared server answers to besides loopback, set in settings.json or `NIGEL_SERVER_PUBLIC_URL`, and the header a proxy on the same machine passes the client's address in (`CF-Connecting-IP` or `X-Forwarded-For`, or `NIGEL_CLIENT_IP_HEADER`). The address is accepted only once the books have a user; before that `nigel serve` refuses it and says why. A value that is not one https origin stops the server at startup
- **`nigel serve` unlocks at startup** from `NIGEL_DB_PASSWORD_FILE` or `NIGEL_DB_PASSWORD`, so a server under systemd starts ready. Without either it starts locked as before
```

Under `### Changed`:

```markdown
- **`nigel serve` sends `Referrer-Policy: same-origin` on everything and `Cache-Control: no-store` on every API answer**, so books data is never kept in a browser's disk cache
- **`nigel user add` says sign-in is now required** when it adds the first user
```

Under `### Fixed` (PR 1 review items that change behaviour):

```markdown
- **`server.key` survives a crash right after it is created** — the config directory is synced after the key is linked into place, a failure names the path, and `nigel serve` warns when the key or its directory can be read by other users
- **`NIGEL_DB_PASSWORD_FILE` refuses a file larger than 4 KiB** instead of reading it whole
```

- [ ] **Step 6: Sweep for real data and stale prose**

Run: `./scripts/check-no-real-data.sh` (whole tree) and `git diff origin/main --stat`.
Expected: exit status 0. Every warning must be fixture or statutory data. Read every changed doc paragraph once for "added in", "now", "previously" narration and remove it.

- [ ] **Step 7: Full verification**

Run each, in order, and confirm the output before moving on:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test --no-default-features --features serve -- --test-threads=1
cargo test -p nigel-core -- --test-threads=1
cargo test -p nigel-core --features desktop -- --test-threads=1
(cd crates/nigel-ffi && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked -- --test-threads=1)
(cd crates/nigel-desktop && cargo test --locked -- --test-threads=1)   # needs the WebKitGTK dev libraries; CI runs it if they are absent here
(cd web && npm run lint && npm run typecheck && npm test && npm run build)
```

Expected: all PASS. Then confirm local mode is unchanged by hand: `cargo run -- serve` on books with no users prints the token link, the browser opens the SPA, and the dashboard loads exactly as on `main`.

- [ ] **Step 8: Commit**

```bash
git add docs/api.md docs/design-constraints.md docs/architecture.md CHANGELOG.md web/apps/app/src/api/types.ts
./scripts/check-no-real-data.sh --staged; echo "exit $?"
git commit -m "Document sign-in, modes and the shared server's security model"
```

(Commit only if the script's exit status is 0.)

---

## Hand-off to PR 3

- Delete `authz::interim_admin_guard` and `ANY_ROLE`; layer `role_guard` (deny by default) in the same place on both routers, inside the session layer.
- `GET /api/status`'s `dataDir`/`dataDirDisplay` become `null` for non-admins (spec §8.4) — PR 2 never lets a non-admin reach status.
- The §8.3 shared-mode refusals (data-dir switch, password set/change/remove) — PR 2's mode cell is monotonic, so until then a switch in shared mode stays shared.
- `POST /api/users` calls `identity::users::require_users` before `create_user`, so the first-user rule surfaces as `409 local_mode` over HTTP and never as CLI wording.
- After any user write through the API, call `state.refresh_mode().await` (spec §1 re-evaluation).
- The admin user routes emit `server::events` lines with new `EventKind`s for admin actions (spec §5).
