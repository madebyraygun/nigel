# Multiuser PR 3 — Roles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every `/api` route on both routers is admitted by one server-side table of minimum roles (deny by default), admins manage users over the API, a shared server refuses to swap or re-key its books, and the test inventory, the role matrix and the SPA's `role-table.json` are all derived from that table.

**Architecture:** `server/authz.rs` replaces the interim admin guard with `ROUTE_ROLES` (method + axum route template → `Access`) and `role_guard`, a `route_layer` applied inside `routes::api_router`, so no router can be assembled without it and the 404 fallback still needs only a session. A new `routes/users.rs` puts the admin user routes on PR 1's `identity::users` data layer. The hand-kept route lists in `testutil.rs` become `testutil::inventory()`, one sample request per table entry, which drives the locked and no-session sweeps and a generated role matrix. PR 2's deferred review items land first, so the matrix runs against the final session layer.

**Tech Stack:** Rust 2021, axum 0.8.9 (`MatchedPath`, `Router::route_layer`), tokio, rusqlite/SQLCipher, assert_cmd for the end-to-end test; TypeScript (one type in `web/apps/app/src/api/types.ts`).

**Spec:** `docs/superpowers/specs/2026-10-23-multiuser-phase-1-design.md` (approved). This plan is PR 3 of its §17 sequence. Read §1, §1.1, §1.2, §4.2, §4.6, §5, §7, §8 (all of it), §9.4 (the parity fixture only), §14, §15 and §16 before starting any task. Backlog: TASK-32.3 (`backlog task 32.3 --plain`), whose notes carry PR 2's deferred review items; each is a step below, marked **(PR 2 review item N)**:

| N | Item | Task |
|---|---|---|
| 1 | Refuse a second `Origin` header | 1 |
| 2 | One table-driven test that no route on the served router skips the guard | 8 |
| 3 | Renew only accepted requests (transport, Origin and must-change checks before `renew_session`) | 1 |
| 4 | Test: must-change allows `POST /api/account/password` and refuses `POST /api/status` | 1 |
| 5 | Test: `Path=/` on a renewed cookie | 1 |
| 6 | Test: an empty `Bearer` | 1 |
| 7 | Test: denial on POST for non-admins | 5 |
| 8 | Test: logout keeps the user's other sessions | 1 |
| 9 | Test: bearer logout | 1 |
| 10 | A dropped login request leaves its attempt charged (drop guard) | 2 |
| 11 | Sweep before `create_session` | 2 |
| 12 | cfg-gate `preflight`/`preflight_with`/`unlock` under `serve` and drop the stub | 3 |
| 13 | Log the user id on a wrong-password `login_failed` and on `login_refused` | 2 |
| 14 | `request_is_public` via `sole_host` | 1 |
| 15 | A shared-mode data-folder switch to user-less or encrypted books is refused (§8.3) | 7 |

## Global Constraints

Copied from the spec. Every task's requirements include these.

- Roles are ordered `read_only < bookkeeper < admin`. "`LocalOwner` passes every check" (§8.1).
- `admin`: "Everything: users, security (the books password), service keys, the data folder, app settings, the letterhead (name, address, phone, logo, payment instructions), countersigning documents, and everything a bookkeeper can do" (§8.1).
- `bookkeeper`: "All bookkeeping: import, categorize, review, reconcile, rules, accounts and categories, clients, invoices, reports and exports. Documents: prepare (file, edit, revise), send, track (sync, record responses, republish the signing record) and withdraw. Not users, security, service keys, the data folder, settings or the letterhead, and not countersigning" (§8.1).
- `read_only`: "`GET` only: the dashboard, register, reports, exports, lists, invoice and document detail and previews" (§8.1).
- `ROUTE_ROLES` is keyed by method and axum's matched route template (`/api/invoices/{number}`). Entry satisfied → through. Entry not satisfied → `403 forbidden`, `details: { reason: "role", required: "<role>" }`. **No entry → `403 forbidden`, `details.reason = "no_role_entry"`, in every mode and for the local owner too.** An unmatched path still reaches the 404 fallback, which needs only a session (§8.2).
- The table is the route inventory; the hand-kept test tables are derived from it, with a request body for each write route; the locked-guard and no-session tests iterate every route (§8.2).
- Shared-mode refusals: `POST /api/settings/data-dir` and `POST /api/settings/password/{set,change,remove}` answer `409 conflict`, reason `shared_mode`. `POST /api/unlock` stays admin-only for the desktop and local cases (§8.3).
- `GET /api/status`: `dataDir`/`dataDirDisplay` are `null` for non-admins in shared mode (§8.4).
- Users API (§7): all admin-only; on books with no users every route answers `409 local_mode`. `PATCH /api/users/{id}` refuses demoting the last enabled admin (`409 last_admin`) and changing your own role (`409 self`); disable refuses yourself and the last admin; reset-password sets `must_change_password` and ends all their sessions; `DELETE …/sessions` answers `{ ended: n }` and may end the caller's own sessions; `DELETE …/totp` removes their two-step.
- `GET /api/users` answers `[{ id, email, displayName, role, totpEnabled, mustChangePassword, disabled, createdAt, lastSeenAt, activeSessions }]`; `POST /api/users` takes `email`, `displayName`, `role` and answers `201 { user, temporaryPassword }` (§7).
- The server re-derives the mode "after any user write through the API" (§1).
- New `409` reasons: `local_mode`, `shared_mode`, `last_admin`, `self`, `duplicate_email` (§8.5).
- One stderr line per admin user action with the email, user id, client IP and session id, never a secret (§5).
- "`GET` never writes" (§4.6 rule 5). The read-only role is GET only.
- The desktop router (`build_desktop_router`) is always local, has no session layer, runs as the local owner, and **has the role guard**, "so a route with no role-table entry fails on the desktop too" (§1.1).
- The CLI and TUI are not gated by roles (§1.1).
- The parity fixture is `web/apps/app/src/__fixtures__/role-table.json`, captured by `cargo test --features serve capture_role_table -- --ignored`; a Rust test fails when it is stale (§9.4). `roles.ts` and its vitest are PR 4.
- Out of scope here: Content-Security-Policy (TASK-32.10), the settings rename (TASK-166 — use `server_public_url`, `public_base_url` and the other current names), `--bind` (none in phase 1), any way back to local mode, SPA screens (PR 4).
- PR 2's numbers stand: 2 s re-derive throttle, 10,000 limiter keys per bucket, 8 KiB auth/account body caps, 4 KiB password-file cap.
- All Rust tests run serially: `cargo test -- --test-threads=1` (§16; CLAUDE.md).
- Public repository: fixture cast only — Acme, Cedar Systems, Juniper Labs, Harbor & Vale, Globex, Initech, Pat Example, Sam Example, `example.com`, `books.example.com`, documentation IPs. Judge `./scripts/check-no-real-data.sh --staged` by its **exit status** (CLAUDE.md).
- Docs describe the current state; no "added in PR 3" prose. Comments obey the user's rules: nothing that justifies an edit, explains a config entry's history, or warns against regressions.

## Review Focus

The five inputs most likely to bite a person that the spec implies but no table entry spells out. Each has its test in the owning task.

1. **A method the table does not list on a path it does** — `HEAD /api/accounts` from a read-only user, `PUT /api/accounts` from an admin. Expected: `HEAD` is judged as the `GET` it is (axum serves it from the `GET` handler); any other unlisted method is `403 no_role_entry`, never a route that a later `.put()` could open silently. *(Task 5, `head_is_judged_as_get_and_an_unlisted_method_is_refused`.)*
2. **Paths that almost match** — `/api/accounts/`, `/api/%61ccounts`, `/api/users/abc`, `/api/users/999999`. Expected: the near-misses reach the 404 fallback (no data, no role bypass); a non-numeric or unknown user id is judged for role **first**, so a read-only user gets `403` and learns nothing about which ids exist. *(Task 5, `near_miss_paths_reach_the_fallback`; Task 6, `a_non_admin_learns_nothing_about_user_ids`.)*
3. **An admin acting on themselves** — demoting themselves, disabling themselves, resetting their own password or removing their own two-step through the admin routes, ending their own sessions. Expected: role change, disable, reset and two-step removal on yourself are `409 self`; ending your own sessions works and the very next request with that session is `401`. *(Task 6, `an_admin_cannot_act_on_themselves_except_to_sign_out`.)*
4. **A role changed mid-session** — an admin demotes a bookkeeper who still holds a live session. Expected: the bookkeeper's next write is `403 role` with no re-login, because the session layer reads the role on every request. *(Task 6, `a_demotion_takes_effect_on_the_next_request`.)*
5. **A shared server's admin trying to move or re-key the books** — a data-folder switch to books with no users or to encrypted books, a password set on unencrypted books. Expected: `409 shared_mode` before any file is touched; `state.db_path()` and the books' encryption are unchanged. *(Task 7, `a_shared_server_refuses_to_move_or_rekey_its_books`.)*

## Rulings on what the spec leaves open

Each is decided here so the implementer does not have to. The ones worth the operator's eye are repeated in the PR description.

1. **The table's value type is `Access`, not `Role`.** `pub enum Access { PreAuth, Min(Role) }`, `pub const ROUTE_ROLES: &[(Method, &str, Access)]`. `POST /api/auth/login` is the one `PreAuth` entry, so the table is the whole `/api` inventory (spec §8.2's `(Method, &str, Role)` cannot say "no session needed"). `role_guard` lets a `PreAuth` entry through with or without a principal.
2. **`role_guard` is a `route_layer` applied at the end of `routes::api_router`**, after every merge and before the fallback and the locked guard are added. A `route_layer` sees the full nested `MatchedPath` (`/api/invoices/{number}`) and never wraps the fallback, so unmatched paths reach the 404 with only a session. Putting it in `api_router` rather than in `build_router`/`build_desktop_router` means no router can be built without it. A request inside the guard with no `MatchedPath` is `403 no_role_entry` (fail closed); with no principal it is `401 unauthorized` (as the interim guard did).
3. **`HEAD` is looked up as `GET`.** Any other method absent from the table — `OPTIONS`, or a `PUT` on a path that only has `GET` — is `403 no_role_entry`, not axum's `405`. A `405` would require letting unlisted methods through to the method router, which is exactly how an unlisted `.put()` would ship open.
4. **The GET rule, as a test:** every `Min(ReadOnly)` entry is a `GET`, and every non-`GET` entry is at least `Min(Bookkeeper)` (except `PreAuth` login and the signed-in-user routes, which are `Min(ReadOnly)` `POST`s by spec §8.4: logout and the four `/account/*`). A `GET` above read-only is allowed only for `GET /api/users`, named in an exception list in the test. **Spec conflict:** §4.6 rule 5 and §16 say "every `GET` in the role table maps to read-only", but §8.4 makes `GET /api/users` admin-only, and it must be — it lists every user's email. The test enforces the direction that matters for the CSRF argument (read-only never reaches a non-GET that writes the books).
5. **Test-only routes have test-only entries.** `routes/mod.rs` keeps `/_guarded_probe` (GET, listed in a `#[cfg(test)] TEST_ROUTE_ROLES` as `Min(ReadOnly)`) and gains `/_unlisted_probe` (GET, deliberately absent). `authz::lookup` consults `ROUTE_ROLES` and, under `cfg(test)`, `TEST_ROUTE_ROLES`.
6. **Users routes on books with no users answer `409 local_mode` for every route, `GET` included**, via `users::require_users`. On `nigel serve` an admin principal only exists in shared mode or as the local owner of user-less books, so in practice this is the local owner; on the desktop, books with users are manageable (§10).
7. **"Self" is the signed-in user's id.** The local owner has no id, so `self` refusals never apply to it; the last-admin invariant (data layer) still does. Refused as `409 self`: `PATCH` with a `role` that differs from the caller's current role (the same role is a no-op), disable, **reset-password and `DELETE …/totp` on yourself** — the last two would let a session alone replace its own password or second factor without the current password or code that `/api/account/*` asks for. `DELETE /api/users/{id}/sessions` on yourself is allowed (spec §7) and answers with an expiring cookie when the caller's session is a cookie session.
8. **Answers of the users routes.** `GET` → the list (oldest first). `POST` → `201 { user, temporaryPassword }`. `PATCH`, `disable`, `enable`, `DELETE …/totp` → `{ user }`. `reset-password` → `{ temporaryPassword }`. `DELETE …/sessions` → `{ ended }`. An unknown id → `404 not_found`, `details.reason = "user"`; a non-numeric id → `400` (the existing `ApiPath` rejection). Disable of a disabled user and enable of an enabled one succeed unchanged (the data layer's behaviour). `POST` and `PATCH` bodies are capped at the account routes' 8 KiB and parsed by `ApiJson`; `role` is `"admin" | "bookkeeper" | "read_only"`, anything else `400`.
9. **`PATCH /api/users/{id}` is one transaction.** A new `users::update_user(conn, id, role: Option<Role>, display_name: Option<&str>, now)` applies both under one `BEGIN IMMEDIATE`, keeping the last-admin check; an empty body is `400 bad_request`, `details.reason = "empty_patch"`.
10. **Admin action lines.** New `EventKind`s: `UserAdded`, `UserRoleChanged`, `UserRenamed`, `UserDisabled`, `UserEnabled`, `UserPasswordReset`, `UserSessionsEnded`, `UserTotpRemoved`. Each names the **target's** email and id, the client IP, the actor's session id, and a new trailing field `by=<actor user id>|local`. Every line gains `by=` (`-` for the PR 2 kinds); the two existing format tests change to match.
11. **Shared-mode refusals are checked first in the handler**, before the body is parsed or any gate is taken, using `state.mode()`. The desktop (pinned local) and a local-mode `nigel serve` keep today's behaviour exactly. `POST /api/unlock` gets no shared-mode check: a shared server is unlocked, so the handler already answers "not locked".
12. **`dataDir`/`dataDirDisplay` become `null` when the principal does not satisfy `Admin` and the mode is shared** — decided in the `status` handler from the `Principal` extractor; `current_status` (shared with `unlock`) stays as it is. The TypeScript mirror types both as `string | null`, and the three call sites (`palette.ts`, `shell-preferences.ts`, `nigel-app.ts`) handle `null` by omitting the "Reveal data folder" palette entry and the location preference.
13. **The test inventory is `testutil::ROUTE_SAMPLES`**: one `(Method, template, uri, body)` per table entry, with a test that the sample set and the table match exactly. `DATA_ROUTES`, `PREVIEW_ROUTES`, `EXPORT_ROUTES` and `WRITE_ROUTES` become functions over it (`data_routes()`, `preview_routes()`, `export_routes()`, `write_routes()`), sorted by kind: a `GET` whose template contains `/preview` or `signing-record.pdf` is a preview, one under `/api/exports/` is an export, every other `GET` is data. The locked sweep skips `locked_guard`'s ungated paths; the no-session sweep skips `PreAuth`.
14. **Bodies in the inventory are chosen so an admitted request cannot reach outside the test's temp directory.** The matrix runs under `TempConfig` (no service keys, so `send`/`sync` refuse at configuration); `settings/data-dir` names `/nonexistent/nigel-matrix`; `password/set` sends an empty `newPassword` (refused by the handler); users routes target a dedicated victim user (`/api/users/4`), never the caller. Task 9 asserts these refusals for the local owner so a later change to a handler cannot turn the matrix into a real encrypt or a real send.
15. **The matrix asserts on the error envelope, not the status.** A request is "refused by the guards" when its `error.code` is `unauthorized`, or `forbidden` with `details.reason` of `role` or `no_role_entry`; anything else, including `401 invalid_credentials` from login and `409 local_mode`, is "admitted". Each request carries a fresh bearer session, so logout and "end sessions" in the inventory cannot sign the matrix out.
16. **Route-inventory completeness is checked against the source.** A test reads `server/routes/*.rs` with `include_str!`, collects every `.route("…"` path literal, and asserts the set equals the table's path set (with `/api` stripped). A new path missing from the table fails it; a new method on a listed path is still denied by default and fails the first test that calls it.
17. **`role-table.json` shape:** a JSON array sorted by path then method, `{ "method": "GET", "path": "/api/accounts", "min": "read_only" }`, with `"min": "pre_auth"` for login. `authz::role_table_json() -> String` renders it (pretty, trailing newline). The capture test lives beside the other fixture captures in `crates/nigel/src/cli/fixture_capture.rs`; the stale check is a normal test there that reads the committed file at run time (so `nigel-core` never `include_str!`s across crates).
18. **`docs/api.md` carries the full route-by-route table**, and a test in `authz.rs` (`include_str!("../../../../docs/api.md")`; `nigel-core` is `publish = false`) parses its rows and asserts they equal `ROUTE_ROLES`, so the documented matrix cannot drift.
19. **Renewal is decided in memory, written only for accepted requests.** `shared_session` looks the session up, runs the transport, Origin and must-change checks, and only then calls a new `sessions::renewal_due(&ActiveSession, now) -> Result<bool>`; when due, a second blocking call runs `renew_session`. A refused request never moves `last_seen_at` (PR 2 review item 3).
20. **Duplicate `Origin` headers are refused** by both host guards with `403 forbidden`, like duplicate `Host` headers; a non-UTF-8 `Origin` is refused as it is today (PR 2 review item 1).
21. **The attempt drop guard is a type.** `ratelimit::ChargedAttempt` owns an `Arc<LoginLimiter>` and the `Attempt`; `succeeded`, `not_a_failure` and `failed` consume it; its `Drop` settles an unsettled attempt with `failed`, so a request dropped mid-check keeps its charge by construction, not by the absence of code. Login and every account route that checks a current password use it (PR 2 review item 10).
22. **`nigel user add`'s first-user line reads correctly whether or not a server is running:** "Nigel will now require sign-in. Sign in to `nigel serve` as pat@example.com — a server that is already running switches on its next request." This departs from spec §6's literal wording, which told the operator to run a server that may already be running (a running server re-derives the mode on its next request, §1).

---

## File Structure

New files:

| File | Responsibility |
|---|---|
| `crates/nigel-core/src/server/routes/users.rs` | The eight admin user routes (§7) |
| `web/apps/app/src/__fixtures__/role-table.json` | The captured role table, for PR 4's `roles.ts` parity test |

Modified files:

| File | Change |
|---|---|
| `crates/nigel-core/src/server/authz.rs` | `Access`, `ROUTE_ROLES`, `lookup`, `role_guard`, `role_table_json`; delete `interim_admin_guard` and `ANY_ROLE`; keep `as_local_owner` |
| `crates/nigel-core/src/server/routes/mod.rs` | Mount `users`; apply `role_guard` as a `route_layer`; `/_unlisted_probe` |
| `crates/nigel-core/src/server/mod.rs` | Drop the interim layer from both routers; tests move to the inventory |
| `crates/nigel-core/src/server/auth.rs` | `sole_origin`; `request_is_public` via `sole_host`; renew only accepted requests |
| `crates/nigel-core/src/server/ratelimit.rs` | `ChargedAttempt`, `LoginLimiter::begin_charged` |
| `crates/nigel-core/src/server/routes/{sessions,account}.rs` | `ChargedAttempt`; sweep before create; user id in event lines |
| `crates/nigel-core/src/server/routes/{settings,status}.rs` | Shared-mode refusals; `dataDir` nulling |
| `crates/nigel-core/src/server/events.rs` | Admin `EventKind`s and the `by=` field |
| `crates/nigel-core/src/server/error.rs` | `ApiError::no_role_entry`, `ApiError::shared_mode`, `ApiError::self_action` |
| `crates/nigel-core/src/identity/{users,sessions}.rs` | `users::update_user`; `sessions::renewal_due` |
| `crates/nigel-core/src/server/testutil.rs` | `ROUTE_SAMPLES`, `inventory()`, the four derived lists, matrix helpers |
| `crates/nigel-core/src/server/shared_tests.rs` | Session/login carry-over tests, the role matrix, users and refusal tests |
| `crates/nigel-core/src/server/routes/{exports,invoices}.rs` | Tests call `export_routes()` / `preview_routes()` |
| `crates/nigel/src/cli/{serve,user}.rs` | cfg-gated preflight; first-user wording |
| `crates/nigel/src/cli/fixture_capture.rs` | `capture_role_table` and its stale check |
| `crates/nigel/tests/serve_exposure.rs`, `crates/nigel/tests/common/serve.rs` | The roles end-to-end scenario |
| `web/apps/app/src/api/types.ts`, `web/apps/app/src/{palette,shell-preferences}.ts`, `web/apps/app/src/components/nigel-app.ts` | `dataDir: string \| null` |
| `docs/api.md`, `docs/design-constraints.md`, `docs/architecture.md`, `CHANGELOG.md` | Current-state docs |

## Shared interfaces (names every task relies on)

```rust
// identity (Tasks 1, 6)
pub fn sessions::renewal_due(session: &ActiveSession, now: DateTime<Utc>) -> Result<bool>;
pub fn users::update_user(conn: &Connection, id: i64, role: Option<Role>, display_name: Option<&str>, now: DateTime<Utc>) -> Result<User>;

// server::ratelimit (Task 2)
pub struct ChargedAttempt;              // succeeded(self); not_a_failure(self); failed(self) -> Vec<Bucket>; newly_full(&self) -> &[Bucket]; Drop → failed
impl LoginLimiter { pub fn begin_charged(self: &Arc<Self>, account: &str, known_user: bool, ip: &str, now: Instant) -> Result<ChargedAttempt, Duration>; }

// server::authz (Tasks 4–5)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access { PreAuth, Min(Role) }
pub const ROUTE_ROLES: &[(Method, &str, Access)];
pub fn lookup(method: &Method, template: &str) -> Option<Access>;   // HEAD → GET; cfg(test) also TEST_ROUTE_ROLES
pub async fn role_guard(req: Request, next: Next) -> Response;
pub fn role_table_json() -> String;
pub async fn as_local_owner(req: Request, next: Next) -> Response;  // unchanged

// server::error (Tasks 4, 6, 7)
impl ApiError {
    pub fn no_role_entry() -> Self;   // 403 forbidden, details { reason: "no_role_entry" }
    pub fn shared_mode() -> Self;     // 409 conflict, details { reason: "shared_mode" }
    pub fn self_action(message: impl Into<String>) -> Self; // 409 conflict, details { reason: "self" }
}
// ApiError::forbidden_role(Role) and ApiError::local_mode() exist already.

// server::events (Task 6)
pub enum EventKind { /* PR 2 kinds */, UserAdded, UserRoleChanged, UserRenamed, UserDisabled, UserEnabled, UserPasswordReset, UserSessionsEnded, UserTotpRemoved }
impl AuthEvent<'_> { pub fn by(self, actor: &Principal) -> Self; }   // by=<id>|local; absent → by=-

// server::testutil (Tasks 8–9)
pub struct RouteCase { pub method: Method, pub template: &'static str, pub uri: &'static str, pub body: Option<&'static str>, pub access: Access }
pub const ROUTE_SAMPLES: &[(Method, &str, &str, &str)];   // (method, template, uri, body or "")
pub fn inventory() -> Vec<RouteCase>;
pub fn data_routes() -> Vec<&'static str>;
pub fn preview_routes() -> Vec<&'static str>;
pub fn export_routes() -> Vec<&'static str>;
pub fn write_routes() -> Vec<(&'static str, &'static str, &'static str)>;
pub fn case_request(case: &RouteCase, credential: Credential) -> Request<Body>;
pub enum Credential<'a> { None, Bearer(&'a str), LocalCookie(&'a str) }
pub async fn guard_refusal(response: Response) -> Option<&'static str>;   // Some("unauthorized" | "role" | "no_role_entry") or None
```

## Security-critical tasks

Tasks 1, 2, 4, 5, 6, 7, 8, 9 and 11 are security-critical: each needs a reviewer who reads the diff against the spec section named in the task, not just the tests. Tasks 3, 10 and 12 are ordinary.

---

### Task 1: Session-layer carry-overs (security-critical)

Spec §4.2, §4.6. Rulings 19, 20. **PR 2 review items 1, 3, 4, 5, 6, 8, 9, 14.**

**Files:**
- Modify: `crates/nigel-core/src/server/auth.rs`
- Modify: `crates/nigel-core/src/identity/sessions.rs`
- Test: `crates/nigel-core/src/server/shared_tests.rs`, `crates/nigel-core/src/server/mod.rs` (`mod tests`), `crates/nigel-core/src/identity/sessions.rs` (`mod tests`)

**Interfaces:**
- Consumes: `served_at`, `cookie_get`, `bearer_get`, `t0` in `shared_tests.rs`; `add_ready_user`, `session_for`, `public_request` in `testutil.rs`.
- Produces: `sessions::renewal_due`; `auth::sole_origin` (private).

- [ ] **Step 1: Write the failing tests**

In `identity/sessions.rs` tests:

```rust
#[test]
fn renewal_is_due_only_after_an_hour() {
    let (_dir, conn) = db();
    let user = admin(&conn);
    let start = at("2026-01-01T00:00:00Z");
    let issued = create_session(&conn, SessionGrant::assume_verified(user), SessionKind::Cookie, None, None, start).unwrap();
    let found = find_session(&conn, &issued.token, start).unwrap().unwrap();
    assert!(!renewal_due(&found, at("2026-01-01T01:00:00Z")).unwrap());
    assert!(renewal_due(&found, at("2026-01-01T01:00:01Z")).unwrap());
}
```

Append to `shared_tests.rs` (helpers: a local `fn last_seen(db_path, token) -> String` that reads `last_seen_at` by `sessions::token_hash`):

```rust
#[tokio::test]
async fn a_refused_request_does_not_renew_its_session() {
    // (a) cookie POST with no Origin → 403 origin_required
    // (b) a must-change user's GET /api/accounts → 403 password_change_required
    // (c) a cookie session presented as Bearer → 401
    // each at t0 + 2h against a session issued at t0; afterwards last_seen(..) == stamp(t0) for all three
}

#[tokio::test]
async fn an_accepted_request_renews_and_the_cookie_keeps_path_root() {
    // cookie session at t0, GET /api/ping at t0 + 2h → 200
    // Set-Cookie contains "; Path=/;" and "Max-Age=" ; last_seen == stamp(t0 + 2h)
}

#[tokio::test]
async fn must_change_allows_its_own_password_post_and_refuses_a_post_to_status() {
    // bearer session for a user with must_change_password set
    // POST /api/account/password {"currentPassword":"wrong","newPassword":"…"} → error.code != "password_change_required"
    // POST /api/status → 403, error.code == "password_change_required"
}

#[tokio::test]
async fn an_empty_bearer_is_unauthorized_and_never_falls_back_to_the_cookie() {
    // "Bearer", "Bearer ", "Bearer    " each with a valid cookie also present → 401, details.mode == "shared"
}

#[tokio::test]
async fn logout_ends_this_session_and_keeps_the_others() {
    // two bearer sessions A and B for pat; POST /api/auth/logout with A → 200
    // GET /api/ping with A → 401; with B → 200
}

#[tokio::test]
async fn a_bearer_logout_ends_the_bearer_session_and_sets_no_cookie() {
    // POST /api/auth/logout with bearer → 200 {"loggedOut":true}, no Set-Cookie; the token is then 401
}

#[tokio::test]
async fn two_origin_headers_are_refused() {
    // public_request("GET", "/api/ping") with two Origin headers, both PUBLIC_URL → 403 forbidden
    // same on loopback with two loopback origins → 403
}

#[tokio::test]
async fn a_request_with_two_hosts_is_never_public() {
    // Host: books.example.com twice → 403 and no Strict-Transport-Security header
}
```

In `server/mod.rs` tests, under `#[cfg(feature = "desktop")]`: `the_desktop_host_guard_refuses_two_origin_headers` (two `nigel://localhost` origins → 403).

Write each body in full in the style of the neighbouring tests; the comments above are the assertions.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server:: identity::sessions -- --test-threads=1`
Expected: FAIL — `renewal_due` undefined; refused requests move `last_seen_at`; two Origins pass; the two-Host refusal carries HSTS.

- [ ] **Step 3: Implement**

- `sessions::renewal_due`: `now - last_seen_at > RENEW_AFTER_SECONDS`, the same comparison `renew_session` makes (have `renew_session` call it).
- `auth::sole_origin(headers) -> Result<Option<&str>, ()>`: none → `Ok(None)`; exactly one UTF-8 value → `Ok(Some)`; two or more, or non-UTF-8 → `Err(())`. Both `served_host_guard` and `host_guard` refuse `Err` with `ApiError::forbidden()`.
- `request_is_public` reads the host through `sole_host`.
- `shared_session`: first blocking call returns only `find_session`; then the transport, Origin and must-change checks; then, if `renewal_due`, a second blocking call (same `db_gate` read + `spawn_blocking_holding`) runs `renew_session`. A renewal error is logged with `eprintln!` and the request proceeds unrenewed — the request was already authorized.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nigel-core server:: identity::sessions -- --test-threads=1` and `cargo test -p nigel-core --features desktop server::tests -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/server/auth.rs crates/nigel-core/src/identity/sessions.rs crates/nigel-core/src/server/shared_tests.rs crates/nigel-core/src/server/mod.rs
git commit -m "Renew only accepted sessions; refuse a second Origin; public host via sole_host"
```

---

### Task 2: Login carry-overs (security-critical)

Spec §4.1, §4.4, §5. Ruling 21. **PR 2 review items 10, 11, 13.**

**Files:**
- Modify: `crates/nigel-core/src/server/ratelimit.rs`
- Modify: `crates/nigel-core/src/server/routes/sessions.rs`, `crates/nigel-core/src/server/routes/account.rs`
- Test: `ratelimit.rs` (`mod tests`), `routes/sessions.rs` (`mod tests`), `shared_tests.rs`

**Interfaces:**
- Consumes: `LoginLimiter::{begin, succeeded, not_a_failure, failed}`, `AppState::password_check`.
- Produces: `ChargedAttempt`, `LoginLimiter::begin_charged`; in `sessions.rs` the pure builders `fn refused_event(account: &str, user_id: Option<i64>, ip: &ClientIp) -> AuthEvent` and `fn failure_events(ip, email, user_id: Option<i64>, reason, locked) -> Vec<AuthEvent>` (replacing `emit_failure`'s inline construction; `emit_failure` emits what `failure_events` returns).

- [ ] **Step 1: Write the failing tests**

In `ratelimit.rs` tests:

```rust
#[test]
fn a_dropped_charged_attempt_keeps_its_charge() {
    let limiter = Arc::new(LoginLimiter::default());
    let now = Instant::now();
    for _ in 0..ACCOUNT_FAILURES {
        drop(limiter.begin_charged("pat@example.com", true, "203.0.113.9", now).unwrap());
    }
    assert!(limiter.begin_charged("pat@example.com", true, "203.0.113.9", now).is_err());
}

#[test]
fn a_settled_charged_attempt_is_settled_once() {
    // begin_charged then not_a_failure → a fresh begin_charged on the account succeeds ACCOUNT_FAILURES more times
}
```

In `shared_tests.rs`:

```rust
#[tokio::test]
async fn a_login_dropped_while_waiting_keeps_its_charge() {
    // served app over books with pat; hold every password_check permit
    // (acquire password_check_slots() permits via state.password_check()).
    // Five times: tokio::time::timeout(50ms, app.clone().oneshot(login_request(wrong password))) → Err(elapsed)
    // release the permits; the sixth login → 429 too_many_attempts
}

#[tokio::test]
async fn a_login_sweeps_expired_sessions_before_it_issues_one() {
    // an expired session row for pat (created at t0 - 100 days via session_for)
    // login at t0 → 200; SELECT COUNT(*) FROM sessions == 1 and that row is the new one
}
```

In `routes/sessions.rs` tests:

```rust
#[test]
fn a_wrong_password_line_names_the_user_when_the_email_is_one() {
    let ip = ClientIp { bucket: "203.0.113.9".into(), log: "203.0.113.9".into() };
    let lines: Vec<String> = failure_events(&ip, "pat@example.com", Some(2), "credentials", vec![])
        .iter().map(AuthEvent::line).collect();
    assert!(lines[0].contains("event=login_failed") && lines[0].contains(" user=2 "), "{lines:?}");
    let refused = refused_event("pat@example.com", Some(2), &ip).line();
    assert!(refused.contains("event=login_refused") && refused.contains(" user=2 "), "{refused}");
    let unknown = refused_event("nobody@example.com", None, &ip).line();
    assert!(unknown.contains(" user=- "), "{unknown}");
}
```

(Match `ClientIp`'s real field names.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core ratelimit routes::sessions shared_tests -- --test-threads=1`
Expected: FAIL — `begin_charged`, `failure_events`, `refused_event` undefined.

- [ ] **Step 3: Implement**

- `ChargedAttempt { limiter: Arc<LoginLimiter>, attempt: Option<Attempt> }`; the three settle methods `take()` the attempt; `Drop` calls `limiter.failed(attempt)` on one still present. `begin_charged` wraps `begin`.
- `AppState.limiter` is already an `Arc<LoginLimiter>`; login and the account routes that check a current password switch to `begin_charged`.
- Login: `is_known_user` becomes `known_user_id(conn, email) -> Result<Option<i64>>`; pass `known.is_some()` to the limiter and `known` to `refused_event` and to `failure_events` on `Outcome::BadPassword`.
- In the blocking closure, `sessions::sweep_expired(&conn, now)?` runs **before** `create_session`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nigel-core server:: -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/server/ratelimit.rs crates/nigel-core/src/server/routes/sessions.rs crates/nigel-core/src/server/routes/account.rs crates/nigel-core/src/server/shared_tests.rs
git commit -m "Keep a dropped login's charge by construction; sweep before issuing; name the user in refusal lines"
```

---

### Task 3: CLI carry-overs

Ruling 22. **PR 2 review item 12.**

**Files:**
- Modify: `crates/nigel/src/cli/serve.rs`, `crates/nigel/src/cli/user.rs`

- [ ] **Step 1: Update the wording test first**

In `cli/user.rs` tests, change `the_first_user_message_says_sign_in_is_now_required` to expect:

```rust
"Nigel will now require sign-in. Sign in to `nigel serve` as pat@example.com — a server that is already running switches on its next request."
```

Run: `cargo test -p nigel cli::user -- --test-threads=1` → FAIL. Change `first_user_message` to match. Run again → PASS.

- [ ] **Step 2: cfg-gate the preflight**

In `cli/serve.rs`: put `preflight`, `preflight_with` and `unlock` under `#[cfg(feature = "serve")]`, delete the `#[cfg(not(feature = "serve"))] fn unlock` stub, and gate the two ungated preflight tests with `#[cfg(feature = "serve")]` too. `run` under `not(serve)` is unchanged.

Run: `cargo build --no-default-features 2>&1 | grep -c warning` → `0`; `cargo test -p nigel cli::serve -- --test-threads=1` → PASS; `cargo test --no-default-features -p nigel cli:: -- --test-threads=1` → PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/nigel/src/cli/serve.rs crates/nigel/src/cli/user.rs
git commit -m "Build serve's preflight only with serve; first-user line holds with a server running"
```

---

### Task 4: The role table and its guard (security-critical)

Spec §8.1, §8.2, §8.4, §8.5. Rulings 1–5.

**Files:**
- Modify: `crates/nigel-core/src/server/authz.rs`
- Modify: `crates/nigel-core/src/server/error.rs` (`no_role_entry`)
- Test: `authz.rs` (`mod tests`)

**Interfaces:**
- Consumes: `Principal::satisfies`, `ApiError::forbidden_role`.
- Produces: `Access`, `ROUTE_ROLES`, `lookup`, `role_guard`, `ApiError::no_role_entry`. Not yet layered anywhere (Task 5).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_table_has_no_duplicates_and_every_path_is_under_api() { /* (method, path) unique; path starts with "/api/" */ }

#[test]
fn read_only_is_get_only_and_writes_need_a_bookkeeper() {
    const ANY_ROLE_POSTS: [&str; 5] = ["/api/auth/logout", "/api/account/password",
        "/api/account/totp/setup", "/api/account/totp/confirm", "/api/account/totp/remove"];
    const GETS_ABOVE_READ_ONLY: [&str; 1] = ["/api/users"];
    for (method, path, access) in ROUTE_ROLES {
        match (method, access) {
            (&Method::GET, Access::Min(Role::ReadOnly)) => {}
            (&Method::GET, _) => assert!(GETS_ABOVE_READ_ONLY.contains(path), "GET {path}"),
            (_, Access::PreAuth) => assert_eq!(*path, "/api/auth/login"),
            (_, Access::Min(Role::ReadOnly)) => assert!(ANY_ROLE_POSTS.contains(path), "{method} {path}"),
            (_, Access::Min(_)) => {}
        }
    }
}

#[test]
fn the_letterhead_countersigning_users_and_settings_writes_are_admin_only() {
    for (method, path) in [
        (Method::PUT, "/api/settings/company"), (Method::PUT, "/api/settings/app"),
        (Method::POST, "/api/documents/{id}/countersign"), (Method::POST, "/api/settings/data-dir"),
        (Method::POST, "/api/settings/password/set"), (Method::POST, "/api/unlock"),
        (Method::POST, "/api/setup"), (Method::GET, "/api/users"), (Method::POST, "/api/users"),
    ] {
        assert_eq!(lookup(&method, path), Some(Access::Min(Role::Admin)), "{method} {path}");
    }
}

#[test]
fn head_is_looked_up_as_get_and_nothing_else_is_inferred() {
    assert_eq!(lookup(&Method::HEAD, "/api/accounts"), Some(Access::Min(Role::ReadOnly)));
    assert_eq!(lookup(&Method::PUT, "/api/accounts"), None);
    assert_eq!(lookup(&Method::OPTIONS, "/api/accounts"), None);
}

#[tokio::test]
async fn the_guard_admits_refuses_and_denies_by_default() {
    // Router with real templates: "/api/accounts" (POST → Min(Bookkeeper)), "/api/auth/login" (PreAuth),
    // "/api/_never_listed" (GET, absent), all `ok` handlers, `.route_layer(from_fn(role_guard))`,
    // and an outer test layer that inserts the Principal named by an `x-test-principal` header.
    // read_only POST /api/accounts → 403 reason "role", required "bookkeeper"
    // bookkeeper POST /api/accounts → 200
    // LocalOwner GET /api/_never_listed → 403 reason "no_role_entry"
    // no principal POST /api/auth/login → 200; no principal POST /api/accounts → 401
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server::authz -- --test-threads=1`
Expected: FAIL — `Access`, `ROUTE_ROLES`, `lookup`, `role_guard` undefined.

- [ ] **Step 3: Write `ROUTE_ROLES` exactly as spec §8.4**

Ordered as the spec's table, `R` = `Access::Min(Role::ReadOnly)`, `B` = `Min(Bookkeeper)`, `A` = `Min(Admin)`; the `{pnl,…}` rows expand to eight entries each; `/api/auth/login` is `PreAuth`. Use `const R: Access = …` shorthands inside the module. The full set (111 entries):

```text
POST /api/auth/login PreAuth | POST /api/auth/logout R | GET /api/auth/me R
POST /api/account/password R | POST /api/account/totp/{setup,confirm,remove} R
GET /api/ping R | GET /api/status R | POST /api/unlock A | POST /api/setup A
GET /api/users A | POST /api/users A | PATCH /api/users/{id} A
POST /api/users/{id}/{disable,enable,reset-password} A | DELETE /api/users/{id}/sessions A | DELETE /api/users/{id}/totp A
GET /api/settings/app R | PUT /api/settings/app A | GET /api/settings/company R | PUT /api/settings/company A
POST /api/settings/data-dir A | POST /api/settings/password/{set,change,remove} A
GET /api/reports/{pnl,expenses,tax,cashflow,balance,flagged,register,k1} R
GET /api/exports/{pnl,expenses,tax,cashflow,balance,flagged,register,k1} R
GET /api/accounts R | POST /api/accounts B | PATCH,DELETE /api/accounts/{id} B
GET /api/categories R | POST /api/categories B | PATCH,DELETE /api/categories/{id} B
GET /api/rules R | POST /api/rules B | POST /api/rules/test B | PATCH,DELETE /api/rules/{id} B
GET /api/imports R | GET /api/imports/formats R | GET /api/imports/{id}/rejects R | GET /api/csv-profiles R
POST /api/imports/{upload,preview,confirm} B | DELETE /api/imports/{id} B
PATCH /api/transactions/{id} B | POST /api/categorize B
GET /api/review/queue R | GET /api/review/{id} R | POST /api/review/{id}/{apply,undo} B
POST /api/reconcile B | GET /api/reconciliations R
GET /api/clients R | GET /api/clients/{id} R | POST /api/clients B | PATCH,DELETE /api/clients/{id} B
POST /api/clients/{id}/{archive,unarchive} B
GET /api/invoices R | GET /api/invoices/aging R | GET /api/invoices/next-number R | GET /api/invoices/{number} R
GET /api/invoices/{number}/preview R | GET /api/invoices/{number}/preview.pdf R
POST /api/invoices B | PATCH,DELETE /api/invoices/{number} B
POST /api/invoices/{number}/{send,duplicate,void,pay} B | POST /api/invoices/sync B
GET /api/documents R | GET /api/documents/{id} R | GET /api/documents/{id}/preview R
GET /api/documents/{id}/preview.pdf R | GET /api/documents/{id}/signing-record.pdf R | GET /api/document-kinds R
POST /api/documents B | PATCH /api/documents/{id} B | POST /api/documents/sync B
POST /api/documents/{id}/{send,accept,request-changes,decline,withdraw,revise,signing-record} B
POST /api/documents/{id}/countersign A | POST /api/document-kinds/{id}/reactivate B
```

Every route in the current router (enumerated from `routes/*.rs`) is in spec §8.4, and every §8.4 route is either in the router or added by Task 6 (the eight `/api/users` routes) — there is no code route the spec omits.

- [ ] **Step 4: Implement `lookup` and `role_guard`**

`lookup`: `HEAD` → `GET`, then a linear search of `ROUTE_ROLES` (and `TEST_ROUTE_ROLES` under `cfg(test)`). `role_guard`: read `MatchedPath` from extensions (absent → `ApiError::no_role_entry()`); `lookup` (absent → `no_role_entry`); `PreAuth` → through; `Min(role)` → no `Principal` → `ApiError::unauthorized()`, `principal.satisfies(role)` → through, else `ApiError::forbidden_role(role)`. Rewrite the module doc comment to describe the table as the current state.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p nigel-core server::authz -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/nigel-core/src/server/authz.rs crates/nigel-core/src/server/error.rs
git commit -m "Add ROUTE_ROLES and the deny-by-default role guard"
```

---

### Task 5: Layer the role guard on every router (security-critical)

Spec §1.1, §8.2. Rulings 2, 3, 5. **PR 2 review item 7.**

**Files:**
- Modify: `crates/nigel-core/src/server/routes/mod.rs`, `crates/nigel-core/src/server/mod.rs`, `crates/nigel-core/src/server/authz.rs` (delete `interim_admin_guard`, `ANY_ROLE` and their test)
- Test: `shared_tests.rs`, `server/mod.rs` (`mod tests`)

**Interfaces:**
- Consumes: `role_guard`, `TEST_ROUTE_ROLES` (Task 4).
- Produces: `api_router` with the guard applied; `/api/_unlisted_probe` under `cfg(test)`.

- [ ] **Step 1: Write the failing tests**

Replace `until_roles_land_a_non_admin_reaches_only_their_own_account` in `shared_tests.rs` with:

```rust
#[tokio::test]
async fn a_non_admin_post_is_judged_by_its_route_not_refused_wholesale() {
    // users: pat admin, sam bookkeeper, rae read_only (fresh bearer sessions)
    // rae  POST /api/accounts {"name":"Acme Checking","accountType":"checking"} → 403 reason "role", required "bookkeeper"
    // sam  same POST → 201 (admitted)
    // sam  PUT /api/settings/company → 403 reason "role", required "admin"
    // rae  GET /api/accounts → 200 ; rae GET /api/status → 200
}

#[tokio::test]
async fn head_is_judged_as_get_and_an_unlisted_method_is_refused() {
    // rae HEAD /api/accounts → 200 ; pat PUT /api/accounts → 403 reason "no_role_entry"
}

#[tokio::test]
async fn near_miss_paths_reach_the_fallback() {
    // rae GET /api/accounts/ , /api/%61ccounts , /api/no-such-route → 404 not_found
}
```

In `server/mod.rs` tests:

```rust
#[tokio::test]
async fn an_unlisted_route_is_refused_even_for_the_local_owner() {
    let (_dir, db_path) = temp_db();
    let (app, token) = app_for(&db_path);
    let response = app.oneshot(session_get("/api/_unlisted_probe", &token)).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(response).await["error"]["details"]["reason"], "no_role_entry");
}

#[cfg(feature = "desktop")]
#[tokio::test]
async fn the_desktop_router_has_the_role_guard() {
    // build_desktop_router over temp_db, GET /api/_unlisted_probe with Host: localhost → 403 no_role_entry;
    // GET /api/_guarded_probe → 200
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server:: -- --test-threads=1` and `cargo test -p nigel-core --features desktop server::tests::the_desktop_router_has -- --test-threads=1`
Expected: FAIL — the interim guard refuses the bookkeeper's POST; `/_unlisted_probe` is not mounted.

- [ ] **Step 3: Implement**

- `routes::api_router`: after the merges, `.route_layer(middleware::from_fn(authz::role_guard))`, then `.fallback(api_not_found)`, then the locked-guard `.layer`. Mount `/_unlisted_probe` next to `/_guarded_probe` under `cfg(test)`.
- `build_router` and `build_desktop_router`: remove `.layer(middleware::from_fn(authz::interim_admin_guard))`. Update both doc comments (layers outermost first: headers, host guard, session layer or local-owner layer, the locked guard, then the role guard per route).
- Delete `interim_admin_guard`, `ANY_ROLE` and `a_request_with_no_principal_is_refused_except_the_login`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nigel-core -- --test-threads=1` and `cargo test -p nigel-core --features desktop -- --test-threads=1`
Expected: PASS. Any existing test that now hits `no_role_entry` names a route missing from the table — add the entry from spec §8.4, never a wildcard.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/server/routes/mod.rs crates/nigel-core/src/server/mod.rs crates/nigel-core/src/server/authz.rs crates/nigel-core/src/server/shared_tests.rs
git commit -m "Replace the interim admin guard with the role table on both routers"
```

---

### Task 6: The users admin API (security-critical)

Spec §5, §7, §8.4, §8.5, §15. Rulings 6–10.

**Files:**
- Create: `crates/nigel-core/src/server/routes/users.rs`
- Modify: `crates/nigel-core/src/server/routes/mod.rs` (`pub mod users;`, merge `users::routes()` into `data_router`)
- Modify: `crates/nigel-core/src/identity/users.rs` (`update_user`)
- Modify: `crates/nigel-core/src/server/events.rs`, `crates/nigel-core/src/server/error.rs` (`self_action`)
- Test: `identity/users.rs` (`mod tests`), `events.rs` (`mod tests`), `shared_tests.rs`

**Interfaces:**
- Consumes: `users::{require_users, list_users, create_user, update_user, disable_user, enable_user, reset_password, clear_totp, get_user}`, `sessions::end_user_sessions`, `Principal`, `PeerAddr`, `client_ip`, `AppState::refresh_mode`, `auth::expired_cookie`, `request_is_public`.
- Produces: `routes::users::routes() -> Router<AppState>`; `UserBody` (the §7 row shape) for PR 4's types.

- [ ] **Step 1: Write the failing data-layer and event tests**

```rust
// identity/users.rs
#[test]
fn update_user_changes_role_and_name_together_and_keeps_the_last_admin() {
    // pat admin only: update_user(pat, Some(Bookkeeper), Some("Pat E."), now) → Conflict { code: "last_admin" }, name unchanged
    // add sam admin; update_user(pat, Some(Bookkeeper), Some("Pat E."), now) → role Bookkeeper, display_name "Pat E."
    // update_user(sam, None, Some("   "), now) → the display-name refusal clean_display_name gives
}

// events.rs
#[test]
fn an_admin_action_line_names_the_target_and_the_actor() {
    // AuthEvent::new(EventKind::UserDisabled).email("sam@example.com").user(3).ip("203.0.113.9").session(14).by(&admin_principal(2)).line()
    // == "auth event=user_disabled email=\"sam@example.com\" user=3 ip=203.0.113.9 session=14 reason=- by=2"
    // with Principal::LocalOwner → ends " by=local"; with no .by → ends " by=-"
}
```

Update the two existing line assertions in `events.rs` to end in ` by=-`.

- [ ] **Step 2: Write the failing route tests** (`shared_tests.rs`; pat admin id 1, sam bookkeeper id 2, all bearer)

```rust
#[tokio::test]
async fn an_admin_adds_lists_changes_and_resets_users() {
    // POST /api/users {"email":"rae@example.com","displayName":"Sam Example","role":"read_only"} → 201;
    //   body.user.role == "read_only", body.user.mustChangePassword == true, body.temporaryPassword matches ^[A-Z2-9]{5}(-[A-Z2-9]{5}){3}$
    // same POST again → 409 reason "duplicate_email"
    // GET /api/users → 3 rows; keys exactly {id,email,displayName,role,totpEnabled,mustChangePassword,disabled,createdAt,lastSeenAt,activeSessions}
    // PATCH /api/users/{rae} {"role":"bookkeeper"} → 200 user.role "bookkeeper"
    // PATCH /api/users/{rae} {} → 400 reason "empty_patch" ; {"role":"owner"} → 400
    // POST /api/users/{sam}/reset-password → 200 temporaryPassword present; sam's old session → 401
    // DELETE /api/users/{sam}/totp → 200 ; POST /api/users/{sam}/disable → 200 user.disabled true ; enable → false
    // POST /api/users/999999/disable → 404 reason "user"
}

#[tokio::test]
async fn an_admin_cannot_act_on_themselves_except_to_sign_out() {
    // pat (only admin): PATCH /api/users/1 {"role":"bookkeeper"} → 409 "self"; {"role":"admin"} → 200 (no-op)
    // POST /api/users/1/disable → 409 "self"; reset-password → 409 "self"; DELETE /api/users/1/totp → 409 "self"
    // with a second admin sam2: sam2 PATCH /api/users/1 {"role":"bookkeeper"} → 200; then sam2 demoting itself → 409 "self"
    // pat cookie session (with Origin): DELETE /api/users/1/sessions → 200 {"ended": n>=1}, Set-Cookie Max-Age=0; same cookie next → 401
}

#[tokio::test]
async fn the_last_admin_cannot_be_demoted_or_disabled_by_another_route() {
    // desktop/local owner over books with one admin: PATCH /api/users/1 {"role":"read_only"} → 409 "last_admin"; disable → 409 "last_admin"
}

#[tokio::test]
async fn a_demotion_takes_effect_on_the_next_request() {
    // sam bookkeeper session S; sam POST /api/accounts → 201; pat PATCH /api/users/2 {"role":"read_only"}; sam (same S) POST /api/accounts → 403 role
}

#[tokio::test]
async fn a_non_admin_learns_nothing_about_user_ids() {
    // sam: GET /api/users, POST /api/users/999999/disable, POST /api/users/abc/disable → all 403 reason "role"
}

#[tokio::test]
async fn users_routes_on_books_without_users_are_local_mode() {
    // app_for(temp_db) local owner: GET /api/users → 409 "local_mode"; POST /api/users → 409 "local_mode" (no user created)
}

#[tokio::test]
async fn adding_a_user_through_the_api_rederives_the_mode() {
    // temp_db (no users); state = AppState::for_serve(db_path, Mode::Local, public_config());
    // state.refresh_mode_throttled().await (spends the 2 s throttle); then add pat with the data layer, as the CLI would;
    // the local owner's POST /api/users (sam, read_only) → 201 (the throttled re-check still reads local);
    // then state.mode() == Shared without waiting out the throttle, and the old token cookie → 401 details.mode "shared"
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p nigel-core identity::users server:: -- --test-threads=1`
Expected: FAIL — `update_user` undefined; `/api/users` answers `404` (listed in the table, no handler mounted).

- [ ] **Step 4: Implement**

- `users::update_user`: one `immediate` transaction; the same last-admin check as `set_role` when demoting an enabled admin; `clean_display_name` for a name; `updated_at` stamped once. `set_role` stays for the CLI.
- `routes/users.rs`: eight handlers, each `Principal` + `State` + `PeerAddr` + `HeaderMap`; each runs `require_users` inside its `with_conn`, then the self check (Ruling 7) against `Principal::User(u).id`, then the data-layer call; `state.refresh_mode().await` after every successful write; one `events::emit` per success with Ruling 10's fields. `.layer(DefaultBodyLimit::max(ACCOUNT_BODY_LIMIT))` on the router (make the constant `pub(crate)` in `account.rs`). The temporary password is serialized from the `Zeroizing<String>` and never logged. `DELETE …/sessions` on the caller's own id with a cookie session appends `expired_cookie(request_is_public(..))`.
- `NotFound` from the data layer maps through `not_found_because(err, "user")`.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p nigel-core -- --test-threads=1` and `cargo test -p nigel-core --features desktop -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/nigel-core/src/server/routes/users.rs crates/nigel-core/src/server/routes/mod.rs crates/nigel-core/src/server/routes/account.rs crates/nigel-core/src/identity/users.rs crates/nigel-core/src/server/events.rs crates/nigel-core/src/server/error.rs crates/nigel-core/src/server/shared_tests.rs
git commit -m "Add the admin users API with self and last-admin refusals and event lines"
```

---

### Task 7: Shared-mode refusals and status redaction (security-critical)

Spec §8.3, §8.4 (`/api/status`). Rulings 11, 12. **PR 2 review item 15.**

**Files:**
- Modify: `crates/nigel-core/src/server/routes/settings.rs`, `crates/nigel-core/src/server/routes/status.rs`, `crates/nigel-core/src/server/error.rs` (`shared_mode`)
- Modify: `web/apps/app/src/api/types.ts`, `web/apps/app/src/palette.ts`, `web/apps/app/src/shell-preferences.ts`, `web/apps/app/src/components/nigel-app.ts`
- Test: `shared_tests.rs`, the existing tests beside `palette.ts` / `shell-preferences.ts`

**Interfaces:**
- Consumes: `AppState::mode`, `Principal`.
- Produces: `ApiError::shared_mode()`; `StatusResponse.data_dir: Option<String>`, `data_dir_display: Option<String>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn a_shared_server_refuses_to_move_or_rekey_its_books() {
    let _config = TempConfig::new();
    // shared app over books with pat admin (bearer)
    // a second temp dir holding (a) user-less books, (b) encrypted books
    // POST /api/settings/data-dir {"path": <a>} → 409 reason "shared_mode"; same for <b>
    // POST /api/settings/password/set {"newPassword":"a long passphrase for tests"} → 409 "shared_mode"
    // change / remove → 409 "shared_mode"
    // afterwards: state.db_path() unchanged; db::is_encrypted(&db_path) == false; state.mode() == Shared
}

#[tokio::test]
async fn the_desktop_and_local_mode_keep_the_folder_and_password_operations() {
    // app_for(temp_db) local owner: POST /api/settings/data-dir {"path":"/nonexistent/nigel-x"} → error.details.reason != "shared_mode"
}

#[tokio::test]
async fn status_hides_the_data_folder_from_non_admins_in_shared_mode() {
    // sam bookkeeper GET /api/status → dataDir null, dataDirDisplay null, mode "shared"
    // pat admin → both strings; local owner (app_for) → both strings
}
```

Web (beside the existing tests for each file): a `StatusResponse` with `dataDir: null` yields no "reveal data folder" palette entry and no location preference, and `nigel-app` renders with it.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core shared_tests -- --test-threads=1`; from `web/`: `npm run typecheck && npm test`
Expected: Rust FAIL (the switch happens; `dataDir` is a string); web FAIL on the new tests.

- [ ] **Step 3: Implement**

- `post_data_dir` and the three `post_password_*` handlers: `if state.mode() == Mode::Shared { return Err(ApiError::shared_mode()) }` as their first line. The message for data-dir: "Change the folder on the server with `nigel load`, then restart `nigel serve`." For passwords: "Use `nigel password` on the server, then update the credential `nigel serve` unlocks with."
- `status` handler: take `Principal`; after `current_status`, null both fields when `state.mode() == Shared && !principal.satisfies(Role::Admin)`.
- `types.ts`: `dataDir: string | null; dataDirDisplay: string | null;`, doc comment saying they are `null` for a non-admin on a shared server. Handle `null` at the three call sites.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nigel-core -- --test-threads=1`; from `web/`: `npm run typecheck && npm test && npm run lint`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/server/routes/settings.rs crates/nigel-core/src/server/routes/status.rs crates/nigel-core/src/server/error.rs crates/nigel-core/src/server/shared_tests.rs web/apps/app/src
git commit -m "Refuse folder switches and re-keys on a shared server; hide the folder from non-admins"
```

---

### Task 8: The test inventory, derived from the table (security-critical)

Spec §8.2, §16. Rulings 13, 16. **PR 2 review item 2.**

**Files:**
- Modify: `crates/nigel-core/src/server/testutil.rs`
- Modify: `crates/nigel-core/src/server/mod.rs` (`mod tests`), `crates/nigel-core/src/server/routes/exports.rs`, `crates/nigel-core/src/server/routes/invoices.rs` (tests only)
- Test: `testutil.rs` (`mod tests`), `authz.rs` (`mod tests`), `server/mod.rs` (`mod tests`)

**Interfaces:**
- Consumes: `ROUTE_ROLES`, `Access`, `locked_guard`'s `UNGATED_PATHS` (make it `pub(crate)`).
- Produces: `ROUTE_SAMPLES`, `RouteCase`, `inventory()`, `data_routes()`, `preview_routes()`, `export_routes()`, `write_routes()`.

- [ ] **Step 1: Write the failing tests**

```rust
// testutil.rs
#[test]
fn every_table_entry_has_exactly_one_sample() {
    // set of (method, template) in ROUTE_SAMPLES == set in ROUTE_ROLES; no duplicate samples;
    // every sample's uri matches its template (each {param} replaced by a non-empty segment)
}

// authz.rs
#[test]
fn every_route_in_the_source_is_in_the_table() {
    // include_str! each of routes/{account,accounts,categories,clients,documents,exports,imports,
    //   invoices,mod,reconcile,reports,review,rules,sessions,settings,setup,status,transactions,users}.rs;
    // collect `.route(` followed by optional whitespace/newline and a "…" literal; drop "/_guarded_probe" and "/_unlisted_probe";
    // prefix "/api"; assert equal to the set of ROUTE_ROLES paths.
}

// server/mod.rs — replaces every_route_without_a_session_is_unauthorized and the two locked sweeps
#[tokio::test]
async fn no_route_on_the_served_router_skips_the_session_layer() {
    // shared mode (served_app over seeded books with pat) AND local mode (app_for):
    // for case in inventory() where case.access != PreAuth: case_request(case, Credential::None) → 401 unauthorized
}

#[tokio::test]
async fn a_locked_database_refuses_every_gated_route() {
    // seeded_db + encrypt + app_for: for case in inventory() where the nested path is not in UNGATED_PATHS:
    // case_request(case, LocalCookie(token)) → 423 locked
}
```

Gaps this closes, which the new sweeps must visibly include (assert each is in `inventory()`): `POST /api/setup`, `POST /api/documents`, `POST /api/documents/{id}/revise`, `POST /api/documents/{id}/signing-record`, `DELETE /api/invoices/{number}`, `POST /api/clients/{id}/archive`, `POST /api/clients/{id}/unarchive`, `GET /api/settings/company`, `GET /api/imports/{id}/rejects`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nigel-core server:: -- --test-threads=1`
Expected: FAIL — `ROUTE_SAMPLES`/`inventory` undefined.

- [ ] **Step 3: Implement**

- `ROUTE_SAMPLES`: keep today's URIs and bodies from `DATA_ROUTES`/`WRITE_ROUTES`/`PREVIEW_ROUTES`/`EXPORT_ROUTES` (`1248`, `1252`, id `1`), add the gaps and the users routes (`/api/users/4`, bodies per Ruling 14), and the Ruling 14 bodies for `settings/data-dir` (`{"path":"/nonexistent/nigel-matrix"}`) and `password/set` (`{"newPassword":""}`). Login: `{"email":"nobody@example.com","password":"x","client":"native"}`. Multipart routes (`imports/upload`, `documents`, `documents/{id}/revise`) send `{}` — the guards run before extraction.
- `inventory()` joins samples to `ROUTE_ROLES` on `(method, template)`, panicking with the pair on a miss.
- The four derived functions per Ruling 13; delete the four consts and update the callers in `mod.rs`, `exports.rs`, `invoices.rs`.
- `case_request(case, credential)`: `Host: 127.0.0.1:5731`; `Content-Type: application/json` and `Origin: http://127.0.0.1:5731` on bodies; the credential as `Authorization: Bearer` or `Cookie: nigel_session=`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nigel-core -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/server/testutil.rs crates/nigel-core/src/server/mod.rs crates/nigel-core/src/server/authz.rs crates/nigel-core/src/server/routes/status.rs crates/nigel-core/src/server/routes/exports.rs crates/nigel-core/src/server/routes/invoices.rs
git commit -m "Derive the route inventory from the role table; sweep every route"
```

---

### Task 9: The role matrix (security-critical)

Spec §16 ("The role matrix", "Deny-by-default", "The desktop router"). Rulings 14, 15. AC #2–#4.

**Files:**
- Modify: `crates/nigel-core/src/server/testutil.rs` (`guard_refusal`, `matrix_books`)
- Test: `crates/nigel-core/src/server/shared_tests.rs`, `crates/nigel-core/src/server/mod.rs` (`mod tests`, desktop)

**Interfaces:**
- Consumes: `inventory()`, `case_request`, `add_ready_user`, `session_for`, `served_app`, `public_config`.
- Produces: `pub fn matrix_books() -> (TempDir, PathBuf, [i64; 4])` — seeded books with admin, bookkeeper, read-only and the victim (id 4, read-only); `guard_refusal`.

- [ ] **Step 1: Write the matrix tests**

```rust
fn expected(access: Access, caller: Option<Role>) -> Option<&'static str> {
    match (access, caller) {
        (Access::PreAuth, _) => None,
        (Access::Min(_), None) => Some("unauthorized"),
        (Access::Min(min), Some(role)) if role >= min => None,
        (Access::Min(_), Some(_)) => Some("role"),
    }
}

#[tokio::test]
async fn the_role_matrix_holds_for_every_route_in_shared_mode() {
    let _config = TempConfig::new();
    let (_dir, db_path, ids) = matrix_books();
    let (app, state) = served_app(&db_path, Default::default());
    assert_eq!(state.mode(), Mode::Shared);
    let callers = [(None, None), (Some(Role::ReadOnly), Some(ids[2])),
                   (Some(Role::Bookkeeper), Some(ids[1])), (Some(Role::Admin), Some(ids[0]))];
    for case in inventory() {
        for (role, id) in callers {
            let token = id.map(|id| session_for(&db_path, id, SessionKind::Bearer, crate::clock::now_utc()));
            let credential = token.as_deref().map_or(Credential::None, Credential::Bearer);
            let response = app.clone().oneshot(case_request(&case, credential)).await.unwrap();
            assert_eq!(guard_refusal(response).await, expected(case.access, role),
                "{} {} as {role:?}", case.method, case.template);
        }
    }
    crate::db::set_db_password_for_tests(None);
}

#[tokio::test]
async fn the_local_owner_passes_every_listed_route() {
    // TempConfig; seeded_db (no users) + app_for; every case with LocalCookie(token) → guard_refusal == None
    // and: settings/data-dir, password/set, invoices/{n}/send, documents/{id}/send each answer a 4xx/409
    // other than 401/403 (Ruling 14 — the handler refused; nothing left the temp dir)
    // afterwards db::is_encrypted(&db_path) == false and get_db_password().is_none()
}

#[cfg(feature = "desktop")]
#[tokio::test]
async fn the_desktop_router_runs_the_matrix_as_the_local_owner() {
    // matrix_books (books WITH users) + build_desktop_router: every case, no credential, Host localhost → guard_refusal == None
}
```

- [ ] **Step 2: Run and fix the table or the samples, never the expectation**

Run: `cargo test -p nigel-core the_role_matrix the_local_owner_passes -- --test-threads=1` and `cargo test -p nigel-core --features desktop the_desktop_router_runs_the_matrix -- --test-threads=1`
Expected: PASS. A failure names the route and caller: compare the entry with spec §8.4; a guard that is right with a sample that reaches outside the temp dir is a sample to change (Ruling 14).

- [ ] **Step 3: Commit**

```bash
git add crates/nigel-core/src/server/testutil.rs crates/nigel-core/src/server/shared_tests.rs crates/nigel-core/src/server/mod.rs
git commit -m "Generate the role matrix from the table for every role, the local owner and the desktop"
```

---

### Task 10: Capture `role-table.json`

Spec §9.4. Ruling 17.

**Files:**
- Modify: `crates/nigel-core/src/server/authz.rs` (`role_table_json`)
- Modify: `crates/nigel/src/cli/fixture_capture.rs`
- Create: `web/apps/app/src/__fixtures__/role-table.json`

- [ ] **Step 1: Write the failing tests**

```rust
// authz.rs
#[test]
fn the_role_table_renders_sorted_with_pre_auth_named() {
    let json: serde_json::Value = serde_json::from_str(&role_table_json()).unwrap();
    let rows = json.as_array().unwrap();
    assert_eq!(rows.len(), ROUTE_ROLES.len());
    assert!(rows.contains(&serde_json::json!({"method":"POST","path":"/api/auth/login","min":"pre_auth"})));
    assert!(rows.contains(&serde_json::json!({"method":"POST","path":"/api/documents/{id}/countersign","min":"admin"})));
    // sorted by (path, method)
}

// fixture_capture.rs
#[test]
fn the_committed_role_table_is_current() {
    let path = fixtures_root().join("role-table.json");
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(committed, nigel_core::server::authz::role_table_json(),
        "{} is stale: cargo test --features serve capture_role_table -- --ignored", path.display());
}

#[test]
#[ignore = "writes fixtures into web/; run deliberately"]
fn capture_role_table() { /* write role_table_json() to that path; println! it */ }
```

(`fixtures_root()` = `CARGO_MANIFEST_DIR/../../web/apps/app/src/__fixtures__`; reuse it in the two existing dir helpers.)

- [ ] **Step 2: Run to verify failure**, then implement `role_table_json` (pretty JSON + `\n`).

Run: `cargo test -p nigel --features serve fixture_capture -- --test-threads=1` → FAIL (no fixture).

- [ ] **Step 3: Capture and verify**

Run: `cargo test -p nigel --features serve capture_role_table -- --ignored --test-threads=1`, then `cargo test -p nigel --features serve fixture_capture -- --test-threads=1`
Expected: the file is written; the stale check PASSES.

- [ ] **Step 4: Commit**

```bash
git add crates/nigel-core/src/server/authz.rs crates/nigel/src/cli/fixture_capture.rs web/apps/app/src/__fixtures__/role-table.json
git commit -m "Capture the role table for the web app's parity test"
```

---

### Task 11: Roles end to end (security-critical)

Spec §16 end-to-end, §7, §8.3.

**Files:**
- Modify: `crates/nigel/tests/serve_exposure.rs`, `crates/nigel/tests/common/serve.rs`

**Interfaces:**
- Consumes: `init`, `add_admin`, `Server::{start, send, get, login}`, `Reply`.
- Produces: `Server::request(&self, method, path, host, bearer: Option<&str>, body: Option<&str>) -> Reply` (raw HTTP with `Content-Type: application/json` when there is a body).

- [ ] **Step 1: Write the test**

```rust
#[test]
fn an_admin_adds_a_read_only_user_who_can_read_and_not_write() {
    // init; add_admin (temporary password T); start with NIGEL_SERVER_PUBLIC_URL=https://books.example.com
    // native login as pat with T → token; POST /api/account/password → new password; native login again → admin token A
    // A: POST /api/users {"email":"sam@example.com","displayName":"Sam Example","role":"read_only"} → 201, temporaryPassword S
    // sam native login with S → token; POST /api/account/password; login → read-only token R
    // R: GET /api/accounts (Host: books.example.com) → 200
    // R: POST /api/accounts {"name":"Acme Checking","accountType":"checking"} → 403, "\"reason\":\"role\""
    // R: GET /api/status → "\"dataDir\":null"
    // A: POST /api/settings/data-dir {"path":"/tmp"} → 409, "\"reason\":\"shared_mode\""
    // A: POST /api/users/2/disable → 200; R: GET /api/accounts → 401
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p nigel --features serve --test serve_exposure -- --test-threads=1`
Expected: PASS (all five scenarios).

- [ ] **Step 3: Commit**

```bash
git add crates/nigel/tests/serve_exposure.rs crates/nigel/tests/common/serve.rs
git commit -m "Prove roles and the shared-mode refusals through the real binary"
```

---

### Task 12: Docs and full verification

Ruling 18. AC #1.

**Files:**
- Modify: `docs/api.md`, `docs/design-constraints.md`, `docs/architecture.md`, `CHANGELOG.md`
- Test: `crates/nigel-core/src/server/authz.rs` (`mod tests`)

- [ ] **Step 1: Write the failing doc-parity test**

```rust
#[test]
fn api_md_documents_every_route_with_its_role() {
    // include_str!("../../../../docs/api.md"); between "### Roles" and the next "### " heading,
    // parse rows `| METHOD | `/api/…` | pre-auth|read-only|bookkeeper|admin |`;
    // assert the parsed set == ROUTE_ROLES rendered the same way
}
```

Run: `cargo test -p nigel-core server::authz::tests::api_md -- --test-threads=1` → FAIL.

- [ ] **Step 2: `docs/api.md`**

- Under "Security model", replace the interim "Who may call what" text with a `### Roles` subsection: the three roles and what each can do (spec §8.1 wording), deny by default (`403` with `no_role_entry`), `HEAD` as `GET`, an unlisted method is `403`, the local owner passes everything, and the full route-by-route table (one row per `ROUTE_ROLES` entry, `| Method | Route | Minimum role |`). This makes the doc-parity test pass.
- Add `### Shared-mode refusals` (§8.3 table) and note in `GET /api/status` that `dataDir`/`dataDirDisplay` are `null` for a non-admin on a shared server.
- Add a `### Users` endpoint section with the eight routes, bodies, answers and reasons (spec §7 plus Rulings 6–9), and that every one is admin-only and `409 local_mode` on books with no users.
- Error table: `forbidden` gains `details.reason = "no_role_entry"`; `conflict` lists `local_mode`, `shared_mode`, `last_admin`, `self`, `duplicate_email`. Event lines: the admin kinds and the `by=` field.

- [ ] **Step 3: `docs/design-constraints.md`**

Replace the "Until the role table lands…" bullet with current-state bullets: the role table is the route inventory and denies by default on both routers (and why it is a `route_layer` in `api_router`); `GET` never writes and read-only is `GET` only, with `GET /api/users` the one admin-only `GET`; a shared server refuses the folder switch and the books-password operations, so no shared-mode screen ever asks for the books password; an admin cannot change their own role, disable, reset or remove the second factor of themselves through the admin routes; a session is renewed only after the request is accepted.

- [ ] **Step 4: `docs/architecture.md`**

`authz.rs` → "the role table (`ROUTE_ROLES`), the deny-by-default `role_guard` and the desktop's local-owner layer"; add `routes/users.rs` (the admin user routes) to the Web server bullet and the tree; mention `role-table.json` beside the other `__fixtures__`.

- [ ] **Step 5: `CHANGELOG.md`** under `## [Unreleased]`

```markdown
### Added
- **Roles on a shared server** — every API route has a minimum role: read-only users can view the dashboard, register, reports, exports, lists and previews; bookkeepers can also import, categorize, review, reconcile and manage rules, accounts, categories, clients, invoices and documents; admins can also manage users, settings, the letterhead, the data folder and the books password, and countersign documents. A route without a role is refused for everyone
- **User management over the API** — admins list, add, change, disable, enable and reset users, end their sessions and remove their two-step (`/api/users`, see "Users" in `docs/api.md`)

### Changed
- **A shared server refuses to switch its data folder or change the books password**; do both on the server with `nigel load` and `nigel password`
- **`GET /api/status` hides the data folder from non-admins on a shared server**
- **`nigel user add`'s first-user message** holds whether or not `nigel serve` is already running
```

- [ ] **Step 6: Sweep for real data and stale prose**

Run: `./scripts/check-no-real-data.sh; echo "exit $?"` and `git diff origin/main --stat`
Expected: `exit 0`. Every warning is fixture or statutory data. Read each changed doc paragraph once for "added in", "now", "previously", "interim" narration and remove it.

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

Expected: all PASS. Then by hand: `cargo run -- serve` on books with no users prints the token link and the SPA loads and edits exactly as on `main` (the local owner passes every route).

- [ ] **Step 8: Commit**

```bash
git add docs/api.md docs/design-constraints.md docs/architecture.md CHANGELOG.md crates/nigel-core/src/server/authz.rs
./scripts/check-no-real-data.sh --staged; echo "exit $?"
git commit -m "Document roles, the users API and the shared-mode refusals"
```

(Commit only if the script's exit status is 0.)

---

## Hand-off to PR 4

- `roles.ts` mirrors `role-table.json` by client method name; its vitest fails when they disagree (spec §9.4). The fixture's `path` is the axum template; map client methods to `(method, path)` there.
- The users client methods (`listUsers`, `addUser`, `updateUser`, `disableUser`, `enableUser`, `resetUserPassword`, `endUserSessions`, `removeUserTotp`) and their types follow Task 6's answers and Ruling 8.
- A `403` with `reason: "role"` reaching a screen shows "Your role no longer allows this." and refetches `/api/auth/me` (spec §9.4).
- Settings › Location and Security in shared mode show the §9.3 sentences; the server already refuses the operations (`409 shared_mode`), and `dataDir` is `null` for non-admins.
