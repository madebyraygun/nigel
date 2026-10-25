# Multiuser phase 1: logins, roles and a public address (epic 32)

**Scope:** TASK-32.1 (users and sessions), 32.2 (auth), 32.3 (roles), 32.8 (SPA login and
user screens), 32.9 (exposure and deployment guide). Decision-11 puts this first in 2.0;
decision-12 makes a self-hosted `nigel serve` the 2.0 path for bookkeeper access.

**Goal:** a second person signs in to the operator's own `nigel serve` from anywhere, over
HTTPS, with their own credentials and a role that the server enforces. Nothing changes for
someone running Nigel alone on their own machine.

**Not in phase 1:** write attribution (32.4), the audit log (32.5), import history (32.6),
optimistic concurrency (32.7), email invitations (164), the Mac app's remote mode (144.16),
passkeys. Section 13 says how this design leaves room for each.

---

## 1. Modes

A server run is in one of two modes. The mode is **derived, never stored or configured**:

| | Local mode | Shared mode |
|---|---|---|
| When | The books have no users, **or** they are encrypted and still locked | The books are open and have at least one user |
| Who gets in | Whoever opens the one-time token link printed at startup | A signed-in user with a session |
| Hostnames accepted | Loopback only: `localhost`, `127.0.0.1`, `[::1]`, any port | Loopback, plus the host of `server_public_url` |
| Token link (`/auth?token=`) | Works | Refused; no token is minted or printed |
| Effective role | Admin (the local owner) | The signed-in user's role |

Rules that follow from it:

- **Local mode is the zero-setup default** and behaves exactly as `nigel serve` does now: loopback bind,
  Host/Origin check against the three loopback names, per-run token, `nigel_session` cookie,
  no accounts, no login screen.
- **Shared mode begins when the first user exists.** Creating the first user (section 6) is
  the only way in. No setting turns it on, so there is no state where non-loopback hosts are
  accepted and no per-user login exists.
- **Non-loopback hostnames are accepted only in shared mode.** The host guard reads the
  mode on every request. In local mode it uses `TrustedOrigins::loopback()` and nothing
  else, whatever `server_public_url` says. A set `server_public_url` in local mode is ignored, and
  `nigel serve` prints a notice saying why.
- **A locked database is local mode.** The users table is inside the books. While SQLCipher
  has no key, the server cannot tell whether users exist, so it takes the reading that
  accepts less. A server that is meant to be shared unlocks at boot (section 11.4), so it
  is never locked in practice. If it is, it fails closed: requests through the tunnel get
  `403`, and the operator can still reach it on loopback.
- **Re-evaluation.** The server works out the mode at startup and again after an unlock and
  after any user write through the API. It also re-checks when a request names the
  `server_public_url` host while the cached mode is local, so a first user added with the CLI to a running
  server takes effect without a restart. A stale reading can only be "local", which
  accepts less.
- **There is no way back to local mode in phase 1.** Users are disabled, never deleted,
  so books that have users stay shared. To stop others getting in, an admin disables
  them. A command that resets books to local mode can come later. The two
  operations that could swap the books underneath a running server, the data-folder switch
  and the books-password operations, are refused in shared mode (section 8.3).
- **Moving from local to shared mode ends the token session.** The token is dropped from memory. The next `/api`
  request with the old cookie gets `401` with `details.mode = "shared"`, and the SPA shows
  the login screen.

### 1.1 Which transports are affected

- **`build_router` (`nigel serve`)** gets everything in this spec.
- **`build_desktop_router` (Tauri, and the Swift shell's `InProcessTransport`)** is always
  local mode. It has no session layer and still has none. Its requests run as the local
  owner (admin), even when the books it opened have users. The trust boundary there is
  access to the file, the same as for the CLI. The role guard (section 8) is still layered
  on it, so a route with no role-table entry fails on the desktop too.
- **The CLI and TUI** are not gated by users or roles. Anyone who can open the file has
  full authority over it. That is also how a locked-out operator recovers (section 15).

### 1.2 The principal

Every `/api` request carries a `Principal` request extension, set by the session layer
(or by the desktop router):

```rust
pub enum Principal {
    /// CLI-equivalent authority: local-mode token session, or the desktop transport.
    LocalOwner,
    User(SessionUser), // id, uid, role, session_id, must_change_password
}
```

`Principal::satisfies(Role)` is true for `LocalOwner` and for a user whose role is at
least the one asked for. Handlers can take it as an extractor. In phase 1 only the auth,
account and users routes read it. 32.4 passes it to the data layer (section 13).

---

## 2. Data model (TASK-32.1)

One migration, **version 17**, the next free number on `main`. If another migration
lands first, this one takes the following number, and integer money (TASK-59) takes the
number after it, as decision-11 orders. It is appended to `MIGRATIONS` in
`crates/nigel-core/src/migrations.rs` and must be idempotent, like the ones before it.

```sql
CREATE TABLE IF NOT EXISTS users (
    id                    INTEGER PRIMARY KEY,
    uid                   TEXT NOT NULL UNIQUE,          -- 32 hex chars, random; stable identity
    email                 TEXT NOT NULL UNIQUE COLLATE NOCASE,
    display_name          TEXT NOT NULL,
    password_hash         TEXT NOT NULL,                 -- Argon2id PHC string
    role                  TEXT NOT NULL CHECK (role IN ('admin', 'bookkeeper', 'read_only')),
    totp_secret           BLOB,                          -- sealed (section 3); NULL = no two-step
    totp_last_step        INTEGER,                       -- last accepted TOTP time step (replay guard)
    must_change_password  INTEGER NOT NULL DEFAULT 0,
    password_changed_at   TEXT NOT NULL,
    disabled_at           TEXT,                          -- NULL = enabled
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id            INTEGER PRIMARY KEY,
    token_hash    BLOB NOT NULL UNIQUE,                  -- SHA-256 of the 32-byte token
    user_id       INTEGER NOT NULL REFERENCES users(id),
    kind          TEXT NOT NULL CHECK (kind IN ('cookie', 'bearer')),
    created_at    TEXT NOT NULL,
    last_seen_at  TEXT NOT NULL,
    expires_at    TEXT NOT NULL,
    user_agent    TEXT,                                  -- truncated to 256 chars
    ip            TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);
```

- **Email** is trimmed and lower-cased before it is stored and before every lookup. The
  `NOCASE` collation keeps the database enforcing uniqueness even if some path skips the
  normalization. The data layer checks the shape: one `@`, a non-empty local part and
  domain, no whitespace. Nothing is sent to the address in phase 1.
- **`uid`** is the identity that outlives renumbering and travels. Attribution and the
  audit log record it (section 13). The integer `id` is for joins and URLs.
- **Timestamps** are UTC RFC 3339, read through one clock seam so tests can pin it.
- **Users are never deleted.** `disabled_at` is the off switch. Attribution in 32.4 then
  never points at a missing row, and the "no way back to local mode" rule in section 1
  holds.
- **Invariant: shared mode always has at least one enabled admin.** The data layer refuses
  any write that would leave zero: the first user must be an admin, and the last enabled
  admin cannot be disabled or demoted. The CLI and the API go through the same function.

Data-layer module: `crates/nigel-core/src/identity/` (`users.rs`, `sessions.rs`,
`passwords.rs`, `totp.rs`, `seal.rs`). It is not under `server/`, because the CLI uses it too. The
existing `password.rs` is the books' encryption password and is left alone. The name
`identity` keeps the two apart.

---

## 3. Secrets at rest

SQLCipher may or may not be on, so nothing below relies on it. Each secret is stored so
that reading the database file does not give an attacker a working credential.

| Secret | Stored as | What reading the file gets an attacker |
|---|---|---|
| User password | Argon2id PHC string (`argon2` crate, v19, m = 19 MiB, t = 2, p = 1, 16-byte random salt per hash) | An offline guessing target, as slow as Argon2id makes it |
| Session token | SHA-256 of the 32-byte random token. No salt is needed, because the input is 256 bits of randomness | Nothing usable. A hash cannot be presented as a cookie |
| Temporary password | Hashed the same way as any password, shown once, never stored in plaintext | The same as a password |
| TOTP secret | Sealed with XChaCha20-Poly1305 under the **server key**, with the user's `uid` as associated data | Nothing usable without the server key, and a sealed secret cannot be moved to another user's row |

**The server key** is 32 random bytes in `<config_dir>/server.key`, created on first need
with mode `0600` in a `0700` directory, as `settings.json` is. It is per machine. It never
lives in the data directory, so it is never in a backup, in `nigel backup` output, or in
the books package that TASK-163 will define. A copied database therefore carries no usable
second factor. If the key file is lost, TOTP verification fails for every enrolled user,
`nigel serve` logs which users are affected, and recovery is `nigel user reset-password
<email> --clear-totp` for each (section 15).

**Shared mode does not require encrypted books, and warns clearly when they are not.**
The hashing and sealing above hold either way. Encryption adds SQLCipher underneath them
and protects the books themselves. When shared mode runs on unencrypted books:

- `nigel user add`, creating the first user, prints: "These books are not encrypted.
  Anyone who can read the file can read the books. Encrypt them with `nigel password set`
  before you give anyone else access."
- `nigel serve` prints the same notice at startup.
- Settings › Security shows admins the "Not encrypted" card with a sentence recommending
  encryption before the server is shared.
- The deployment guide's first step is encrypted books.

**Never logged:** passwords, temporary passwords, TOTP secrets and codes, session tokens.
Request bodies that carry them hold them in `server::secret::Secret` (redacted `Debug`,
zeroized on drop), as the unlock path already does. The `identity` module keeps secrets
out of its error messages.

---

## 4. Authentication (TASK-32.2)

### 4.1 Endpoints

All under `/api`, JSON in and out.

**`POST /api/auth/login`**. This is the only `/api` route that needs no session. It
is not gated by the locked guard either. Body:

```json
{ "email": "pat@example.com", "password": "…", "code": "123456", "client": "browser" }
```

- `code` is optional, and needed only when the user has TOTP on. `client` is `browser`
  (the default) or `native` (section 12).
- Success, `browser`: `200 { "user": {…} }` and `Set-Cookie: nigel_session=<token>;
  HttpOnly; SameSite=Strict; Path=/; Max-Age=2592000` (30 days), plus `; Secure` when the request's
  Host is the `server_public_url` host (always https, section 11.1). Loopback sessions
  are plain http, so they omit `Secure`.
- Success, `native`: `200 { "user": {…}, "token": "<hex>", "expiresAt": "…" }` and no
  cookie. A `bearer` session row is created.
- Password correct, TOTP on, `code` missing: `401 totp_required`. This does not count as a
  failure. The client asks for the code and sends all three fields again. There is no
  half-authenticated state on the server.
- A wrong email, a wrong password, a disabled user or a wrong code all get
  `401 invalid_credentials`, with the same message: "That email and password don't match."
  For a wrong code the message is "That code didn't match. Check the time on your device."
  That variant appears only after the password has been proven, because the
  `totp_required` step already revealed that. An unknown email still runs one Argon2id
  verification against a fixed dummy hash, so the timing reveals nothing.
- Rate limited: `429 too_many_attempts` with `details.retryAfterMs` (section 4.4).
- Local mode: `409 conflict`, `details.reason = "local_mode"`.

**`POST /api/auth/logout`**. Deletes the current session row and answers
`200 { "loggedOut": true }` with an expiring `Set-Cookie` for browser sessions. Calling it
in local mode is a no-op that answers the same.

**`GET /api/auth/me`**. Answers in both modes, and is not gated by the locked guard:

```json
{ "mode": "shared",
  "user": { "id": 2, "email": "pat@example.com", "displayName": "Pat Example",
            "role": "bookkeeper", "totpEnabled": true, "mustChangePassword": false },
  "role": "bookkeeper" }
```

In local mode, `user` is `null` and `role` is `"admin"`. The SPA keys everything off `role`.

**Account routes (the signed-in user only; any role):**

| Route | Body | Effect |
|---|---|---|
| `POST /api/account/password` | `currentPassword`, `newPassword` | Rules in section 4.5. Clears `must_change_password`, ends every other session of this user, keeps this one |
| `POST /api/account/totp/setup` | `currentPassword` | Returns `setupId`, `secret` (base32), `otpauthUri` and `qrSvg`. The pending secret is held in memory for 10 minutes, keyed by session. Nothing is written yet |
| `POST /api/account/totp/confirm` | `setupId`, `code` | Verifies the code against the pending secret, then seals and stores it |
| `POST /api/account/totp/remove` | `currentPassword`, `code` | Clears `totp_secret` and `totp_last_step` |

These routes exist only in shared mode. In local mode there is no account, and they answer
`409 local_mode`.

### 4.2 The session layer

`auth::session_guard` becomes mode-aware. It still wraps the whole `/api` router with
`.layer` (not `.route_layer`), so the 404 fallback stays covered.

1. **Local mode:** the current token check, unchanged. On success it inserts
   `Principal::LocalOwner`.
2. **Shared mode:** take the credential from `Authorization: Bearer <hex>` if present,
   otherwise from the `nigel_session` cookie. Hash it, then look up a session joined to
   its user where `expires_at > now` and `users.disabled_at IS NULL`. If nothing matches,
   answer `401 unauthorized` with `details.mode = "shared"`. Otherwise insert
   `Principal::User`. The role is read on every request, so a role change or a disable
   takes effect on the user's next request.
3. **Must change password:** while `must_change_password` is set, the only routes allowed
   are `GET /api/auth/me`, `GET /api/status`, `GET /api/ping`, `POST /api/account/password`
   and `POST /api/auth/logout`. Anything else gets `403 password_change_required`.
4. **Expiry:** a session ends after 30 days without use, and in any case 90 days after
   it was created. Cookie and bearer sessions are treated alike. At login,
   `expires_at = now + 30 days`. When `last_seen_at` is more than an hour old, the guard
   sets `last_seen_at = now` and `expires_at = min(now + 30 days, created_at + 90 days)`,
   and for cookie sessions it sends a fresh `Set-Cookie` with the same token and a
   `Max-Age` of the time remaining. Doing this once an hour keeps the write rate
   negligible. Renewal never moves `expires_at` past `created_at + 90 days`. After that
   the user signs in again.
5. **Sweep:** expired session rows are deleted at startup and on each successful login.

`session_token` in `AppState` becomes `Option<Arc<str>>`. It is `None` in shared mode,
and `/auth?token=` then answers `401` with `details.mode = "shared"` and sets no cookie.

### 4.3 TOTP

- RFC 6238: HMAC-SHA1, 6 digits, a 30-second step, and a 160-bit secret from the CSPRNG.
- **Clock skew:** the current step and one step either side are accepted (±30 s). The
  server's clock is expected to be NTP-synced, and the deployment guide checks for it.
- **Replay:** a code is accepted only if its step is greater than `totp_last_step`, which
  is then written. A code observed in transit cannot be used again within its window.
- **Enrollment** is self-service from the user's own Settings › Your account. It needs the
  current password, and it is confirmed by a code from the app before anything is stored.
  The QR code is rendered on the server as SVG (`qrcode` crate). The SPA shows it as an
  `<img>` with a data URI, never with `innerHTML`, and next to it shows the base32 secret
  for typing in by hand. `otpauth://totp/Nigel:<email>?secret=…&issuer=Nigel`.
- **Removal:** the user can remove it with their password and a current code. An admin
  can remove another user's TOTP (section 8.2), for a lost device.
- No recovery codes in phase 1. A lost device is an admin action, or a CLI action for the
  last admin (section 15).

### 4.4 Rate limiting and lockout

In memory and process-wide, like `UnlockGate`. A restart clears it, which is acceptable
because only someone with server access can restart.

| Bucket | Key | Limit | Lockout |
|---|---|---|---|
| Per account | Normalized email, **including emails that match no user**, so lockout behaviour does not reveal which accounts exist | 5 consecutive failures | 15 minutes; any success resets the count |
| Per client IP | Client IP (below) | 20 failures in a sliding 15 minutes | 15 minutes |

- A locked bucket gets `429 too_many_attempts` with `retryAfterMs` before any password is
  checked, so a locked-out guess costs the server no Argon2 time.
- Failures count wrong passwords and wrong codes. `totp_required` is not a failure.
- **Client IP:** the TCP peer address, unless the peer is loopback and `client_ip_header`
  is set (section 11.1). Then it is that header's value: `CF-Connecting-IP` as given, or
  the rightmost entry of `X-Forwarded-For`. The header is trusted only from loopback, so a
  remote client cannot pick its own bucket. If `client_ip_header` is unset behind a local
  proxy, every client shares the proxy's bucket. `nigel serve` warns when it is in shared
  mode with a `server_public_url` and no `client_ip_header`.
- Unlocking the books keeps its own `UnlockGate`, as it is now.

### 4.5 Password rules

Following NIST SP 800-63B: length and identity checks, no composition rules.

- At least 12 characters and at most 256. Unicode is allowed and is NFC-normalized
  before hashing.
- Not equal to the user's email, its local part, or their display name (case-insensitive).
- A new password must differ from the current one.
- **Temporary passwords** are generated by the server: 20 characters from an unambiguous
  base32 alphabet in four groups of five (about 100 bits), for example
  `K7QXM-2RTPA-W9HCN-4FJDE`. They set `must_change_password`. They are shown once, in the CLI's output or
  the admin's dialog, and are never retrievable again.
- One function enforces these rules, shared by the CLI and the API. A failure is
  `400 bad_request` with `details.reason` set to `too_short`, `too_long`,
  `matches_identity` or `unchanged`.

### 4.6 CSRF posture

The existing layers stay in place. Shared mode adds one rule:

1. **`SameSite=Strict`** on the session cookie. A cross-site request carries no session.
2. **Host check.** The `Host` must be a trusted host for the mode (section 1). This is
   what still blocks DNS rebinding.
3. **Origin check.** In shared mode, the `server_public_url` origin is compared whole
   (`https://books.example.com`, scheme and port included). Loopback keeps "any port", so
   the vite dev server still works.
4. **New:** in shared mode, a cookie-authenticated request whose method is not
   GET/HEAD/OPTIONS must carry a trusted `Origin`. With no `Origin` it gets `403 forbidden`.
   Browsers send `Origin` on every non-GET `fetch`, same-origin included, so the SPA is
   unaffected. A `POST /api/auth/login` with `client: "browser"` must carry a trusted
   `Origin` too, which blocks login CSRF. A `native` login sets no cookie, so it cannot be
   used to plant a session in a browser, and needs no `Origin`.
   Bearer-authenticated requests are exempt, because a bearer token is never sent
   automatically by a browser.
5. **`GET` never writes.** This is now a stated design constraint. The read-only role is
   "GET only" (section 8), and both the CSRF rule and the role table rely on it. A test
   asserts that every `GET` in the role table maps to read-only.
6. JSON routes need `Content-Type: application/json` (axum's `Json` already rejects
   anything else), so a cross-site HTML form cannot reach them. The multipart upload
   routes rely on rules 1, 3 and 4.

### 4.7 Security headers

These are added to `security_headers` as defaults (`or_insert`), alongside `nosniff` and
`X-Frame-Options: DENY`:

- `Referrer-Policy: same-origin` on everything.
- `Cache-Control: no-store` on every `/api` response, so books data never sits in a
  shared cache or the browser's disk cache.
- `Strict-Transport-Security: max-age=31536000` on responses to the `server_public_url` host only.
  It is never sent on loopback, where it would pin `localhost` to https.

**Content-Security-Policy is its own phase-1 task, TASK-32.10**, outside the five PRs in section 17.
It has to be checked against Web Awesome's and Lit's runtime styles and the invoice and
document preview iframes, so it is not part of PR 2. It must land before any server is
exposed publicly (section 17).

---

## 5. Login events and logging

Phase 1 has no audit table (32.5). `nigel serve` writes one line to stderr, which goes to
the journal under systemd, for each login success, login failure (with the reason class),
lockout, logout, password change, TOTP change and admin user action. Each line includes
the email, user id, client IP and session id. It never includes a secret. 32.5 replaces
these lines with audit rows and keeps the same fields.

---

## 6. CLI (`nigel user`)

These run against the books in the configured data directory, like every other command.
They go through the normal password pre-flight: an interactive prompt,
`NIGEL_DB_PASSWORD`, or `NIGEL_DB_PASSWORD_FILE` (section 11.4). The CLI has full
authority and is how the first admin is created.

```bash
nigel user add pat@example.com --name "Pat Example" --admin       # first user must be --admin
nigel user add sam@example.com --name "Sam Example" --role bookkeeper
nigel user list                                                    # email, name, role, status, two-step, last seen
nigel user role sam@example.com read_only
nigel user disable sam@example.com                                 # ends their sessions
nigel user enable sam@example.com
nigel user reset-password sam@example.com                          # new temporary password; ends their sessions
nigel user reset-password pat@example.com --clear-totp             # also removes two-step (lost device)
```

- `add` and `reset-password` print the temporary password once, with the sentence
  "Give this to <name>. They will choose their own password when they first sign in."
- Exactly one of `--admin` or `--role <admin|bookkeeper|read_only>` is required. No role
  is granted by default.
- `add` on books with no users prints: "Nigel will now require sign-in. Run
  `nigel serve` and sign in as pat@example.com." From there the server is in shared mode
  (section 1). On unencrypted books it also prints the encryption warning (section 3).
- Users are identified by email. `role`, `disable`, `enable` and `reset-password` refuse an unknown email
  with a clear error, and refuse to leave no enabled admin.
- `list` never prints hashes or secrets.

---

## 7. API additions for user management

All are admin-only (section 8). They work only on books that already have users, and
answer `409 local_mode` on books with none, because the first user is created with the
CLI (section 6), never over HTTP. On `nigel serve` that means shared mode. On the desktop
transport it means the local owner can manage the users of books that have them.

| Route | Body | Answer |
|---|---|---|
| `GET /api/users` | — | `[{ id, email, displayName, role, totpEnabled, mustChangePassword, disabled, createdAt, lastSeenAt, activeSessions }]` |
| `POST /api/users` | `email`, `displayName`, `role` | `201 { user, temporaryPassword }` |
| `PATCH /api/users/{id}` | `role?`, `displayName?` | `{ user }`. Refuses to demote the last enabled admin (`409`, reason `last_admin`) and refuses to change your own role (`409`, reason `self`) |
| `POST /api/users/{id}/disable` | — | Ends all their sessions. Refuses yourself and the last admin |
| `POST /api/users/{id}/enable` | — | — |
| `POST /api/users/{id}/reset-password` | — | `{ temporaryPassword }`. Sets `must_change_password` and ends all their sessions |
| `DELETE /api/users/{id}/sessions` | — | `{ ended: n }`. "Sign out everywhere" for that user. An admin may do this to themselves, which ends the current session too |
| `DELETE /api/users/{id}/totp` | — | Removes their two-step verification |

---

## 8. Roles and authorization (TASK-32.3)

### 8.1 Roles

| Role | Can |
|---|---|
| `admin` | Everything: users, security (the books password), service keys, the data folder, app settings, the letterhead (name, address, phone, logo, payment instructions), countersigning documents, and everything a bookkeeper can do |
| `bookkeeper` | All bookkeeping: import, categorize, review, reconcile, rules, accounts and categories, clients, invoices, reports and exports. Documents: prepare (file, edit, revise), send, track (sync, record responses, republish the signing record) and withdraw. Not users, security, service keys, the data folder, settings or the letterhead, and not countersigning, which signs for the business |
| `read_only` | `GET` only: the dashboard, register, reports, exports, lists, invoice and document detail and previews |

The order is `read_only < bookkeeper < admin`. `LocalOwner` passes every check.

### 8.2 One table, enforced on the server

`crates/nigel-core/src/server/authz.rs` holds `ROUTE_ROLES: &[(Method, &str, Role)]`,
keyed by method and axum's matched route template (`/api/invoices/{number}`). The
`role_guard` middleware is layered on the `/api` router **inside** the session layer, so
it runs after the principal is known. It looks up `(method, MatchedPath)`:

- If the entry exists and the principal satisfies it, the request goes through.
- If the entry exists and the principal does not satisfy it, the answer is
  `403 forbidden` with `details: { reason: "role", required: "<role>" }`.
- **If there is no entry, the answer is `403 forbidden` with `details.reason =
  "no_role_entry"`, in every mode and for the local owner too.** A route that someone
  forgets to list fails closed and fails the first test that calls it. It cannot ship
  open.
- An unmatched path still reaches the 404 fallback, which needs only a session.

The table becomes **the route inventory**. The hand-kept test tables in
`server/testutil.rs` (`DATA_ROUTES`, `PREVIEW_ROUTES`, `EXPORT_ROUTES`, `WRITE_ROUTES`)
are derived from it, with a request body for each write route. This closes the gaps
those tables have now. For example, `WRITE_ROUTES` lacks `POST /api/setup`,
`POST /api/documents`, `POST /api/documents/{id}/revise`,
`POST /api/documents/{id}/signing-record`, `DELETE /api/invoices/{number}`,
`POST /api/clients/{id}/archive` and `/unarchive`. `DATA_ROUTES` lacks
`GET /api/settings/company` and `GET /api/imports/{id}/rejects`. The locked-guard and
no-session tests then iterate every route.

### 8.3 Shared-mode refusals

Some operations are admin-only and are also refused outright in shared mode, because they
would swap or re-key the books under a running shared server:

| Route | In shared mode |
|---|---|
| `POST /api/settings/data-dir` | `409 conflict`, reason `shared_mode`. Change the folder on the server with `nigel load` and restart |
| `POST /api/settings/password/{set,change,remove}` | `409 conflict`, reason `shared_mode`. Use `nigel password` on the server, then update the credential (section 11.4) |
| `POST /api/unlock` | Cannot be reached: a shared server is unlocked by definition. Admin-only in the table for the desktop and local cases |

This is what makes "users never know the books password" hold by construction. No shared-mode
screen asks for it or changes it.

### 8.4 Route-by-route table

R = read-only, B = bookkeeper, A = admin. "Pre-auth" means no session is needed.

| Method | Route | Min |
|---|---|---|
| POST | `/api/auth/login` | pre-auth |
| POST | `/api/auth/logout` | R |
| GET | `/api/auth/me` | R |
| POST | `/api/account/password` | R |
| POST | `/api/account/totp/setup` | R |
| POST | `/api/account/totp/confirm` | R |
| POST | `/api/account/totp/remove` | R |
| GET | `/api/ping` | R |
| GET | `/api/status` | R (`dataDir`/`dataDirDisplay` are `null` for non-admins in shared mode) |
| POST | `/api/unlock` | A (local only, see 8.3) |
| POST | `/api/setup` | A |
| GET | `/api/users` | A |
| POST | `/api/users` | A |
| PATCH | `/api/users/{id}` | A |
| POST | `/api/users/{id}/disable` | A |
| POST | `/api/users/{id}/enable` | A |
| POST | `/api/users/{id}/reset-password` | A |
| DELETE | `/api/users/{id}/sessions` | A |
| DELETE | `/api/users/{id}/totp` | A |
| GET | `/api/settings/app` | R |
| PUT | `/api/settings/app` | A |
| GET | `/api/settings/company` | R |
| PUT | `/api/settings/company` | A (letterhead: name, address, phone, logo, payment instructions) |
| POST | `/api/settings/data-dir` | A (refused in shared mode) |
| POST | `/api/settings/password/set` | A (refused in shared mode) |
| POST | `/api/settings/password/change` | A (refused in shared mode) |
| POST | `/api/settings/password/remove` | A (refused in shared mode) |
| GET | `/api/reports/{pnl,expenses,tax,cashflow,balance,flagged,register,k1}` | R |
| GET | `/api/exports/{pnl,expenses,tax,cashflow,balance,flagged,register,k1}` | R |
| GET | `/api/accounts` | R |
| POST | `/api/accounts` | B |
| PATCH | `/api/accounts/{id}` | B |
| DELETE | `/api/accounts/{id}` | B |
| GET | `/api/categories` | R |
| POST | `/api/categories` | B |
| PATCH | `/api/categories/{id}` | B |
| DELETE | `/api/categories/{id}` | B |
| GET | `/api/rules` | R |
| POST | `/api/rules` | B |
| POST | `/api/rules/test` | B (a dry run, but read-only is GET only) |
| PATCH | `/api/rules/{id}` | B |
| DELETE | `/api/rules/{id}` | B |
| GET | `/api/imports` | R |
| GET | `/api/imports/formats` | R |
| GET | `/api/imports/{id}/rejects` | R |
| GET | `/api/csv-profiles` | R |
| POST | `/api/imports/upload` | B |
| POST | `/api/imports/preview` | B |
| POST | `/api/imports/confirm` | B |
| DELETE | `/api/imports/{id}` | B |
| PATCH | `/api/transactions/{id}` | B |
| POST | `/api/categorize` | B |
| GET | `/api/review/queue` | R |
| GET | `/api/review/{id}` | R |
| POST | `/api/review/{id}/apply` | B |
| POST | `/api/review/{id}/undo` | B |
| POST | `/api/reconcile` | B |
| GET | `/api/reconciliations` | R |
| GET | `/api/clients` | R |
| GET | `/api/clients/{id}` | R |
| POST | `/api/clients` | B |
| PATCH | `/api/clients/{id}` | B |
| DELETE | `/api/clients/{id}` | B |
| POST | `/api/clients/{id}/archive` | B |
| POST | `/api/clients/{id}/unarchive` | B |
| GET | `/api/invoices` | R |
| GET | `/api/invoices/aging` | R |
| GET | `/api/invoices/next-number` | R |
| GET | `/api/invoices/{number}` | R |
| GET | `/api/invoices/{number}/preview` | R |
| GET | `/api/invoices/{number}/preview.pdf` | R |
| POST | `/api/invoices` | B |
| PATCH | `/api/invoices/{number}` | B |
| DELETE | `/api/invoices/{number}` | B |
| POST | `/api/invoices/{number}/send` | B |
| POST | `/api/invoices/{number}/duplicate` | B |
| POST | `/api/invoices/{number}/void` | B |
| POST | `/api/invoices/{number}/pay` | B |
| POST | `/api/invoices/sync` | B |
| GET | `/api/documents` | R |
| GET | `/api/documents/{id}` | R |
| GET | `/api/documents/{id}/preview` | R |
| GET | `/api/documents/{id}/preview.pdf` | R |
| GET | `/api/documents/{id}/signing-record.pdf` | R |
| GET | `/api/document-kinds` | R |
| POST | `/api/documents` | B |
| PATCH | `/api/documents/{id}` | B |
| POST | `/api/documents/sync` | B |
| POST | `/api/documents/{id}/send` | B |
| POST | `/api/documents/{id}/accept` | B |
| POST | `/api/documents/{id}/request-changes` | B |
| POST | `/api/documents/{id}/decline` | B |
| POST | `/api/documents/{id}/countersign` | A (signs for the business) |
| POST | `/api/documents/{id}/withdraw` | B |
| POST | `/api/documents/{id}/revise` | B |
| POST | `/api/documents/{id}/signing-record` | B |
| POST | `/api/document-kinds/{id}/reactivate` | B |

Service keys (Stripe, Mailgun, R2) have no API route now. Any route added for them, or
for TASK-160, is admin-only. The table makes that a one-line decision when it lands.

### 8.5 Error envelope additions

| Code | Status | Meaning |
|---|---|---|
| `unauthorized` | 401 | Unchanged. In shared mode `details.mode = "shared"`, which tells the SPA to show the login screen rather than "open the link again" |
| `invalid_credentials` | 401 | Login refused: the email, password, user status or code didn't match |
| `totp_required` | 401 | The password was correct. Send it again with `code` |
| `forbidden` | 403 | Unchanged for Host/Origin. `details.reason = "role"` (with `required`) or `"no_role_entry"` for authorization |
| `password_change_required` | 403 | The session belongs to a user who must change a temporary password first |
| `conflict` | 409 | New reasons: `local_mode`, `shared_mode`, `last_admin`, `self`, `duplicate_email` |
| `too_many_attempts` | 429 | Login rate limit. `details.retryAfterMs` is set |

`GET /api/status` gains `"mode": "local" | "shared"`.

---

## 9. Web (TASK-32.8)

The component-first rules apply to everything here. Each visual piece is a `wc-*` in
`@nigel/ui` with a co-located preview covering its states, `describePreviewA11y` with zero
violations, tokens from `@nigel/theme`, and `controlsCss` adopted wherever a `wa-*`
primitive is rendered. `web/apps/app` only composes these components.

### 9.1 Session state and the login gate

- `web/apps/app/src/state/session.ts` holds a `principal` signal loaded from
  `GET /api/auth/me` at boot, next to `/api/status`.
- The app's gates run in this order: **login** (shared mode, not signed in) → **change
  password** (`mustChangePassword`) → **unlock** (local mode only) → **setup** → the app.
- A `401` with `details.mode = "shared"` from any call sets the login gate. It is
  distinct from `appUnauthorized`, the local-mode "session expired, open the link again"
  banner. `invalid_password` still never raises either.
- All calls go through `web/apps/app/src/api/`, as the existing guard test requires. The
  new client methods are `login`, `logout`, `me`, `changeOwnPassword`, `totpSetup`,
  `totpConfirm`, `totpRemove`, `listUsers`, `addUser`, `updateUser`, `disableUser`,
  `enableUser`, `resetUserPassword`, `endUserSessions` and `removeUserTotp`. Their types
  go in `api/types.ts`.

### 9.2 Components

| Component | What | Preview states |
|---|---|---|
| `wc-login-card` | A full-window card in the refresh design, sitting where `wc-unlock-card` sits: wordmark, Email, Password, Sign in. It shows **no book name or company name**, because nothing about the books is shown before sign-in. A second step shows a 6-digit code field (`autocomplete="one-time-code"`, `inputmode="numeric"`) with "Back". A change-password step has New password and Type it again | default, submitting, wrong credentials, code step, wrong code, locked out (countdown from `retryAfterMs`), must change password, password rule error |
| `wc-account-menu` | A monogram button (`wc-monogram` with the user's initials) at the right end of the top bar, after refresh. It opens a menu with the display name, email, a role badge, "Account settings…" (to Settings › Your account) and "Sign out". It is absent in local mode | admin, bookkeeper, read-only, long name, open |
| `wc-role-badge` | The role as a small pill: Admin / Bookkeeper / Read-only | each role |
| `wc-user-list` | Settings › Users rows: name, email, role badge, two-step on/off, status (active, disabled, must change password), last seen, and a per-row actions menu (Change role…, Reset password…, Sign out everywhere, Remove two-step, Disable or Enable). "Add user…" sits above the list | default, includes disabled, includes pending temporary password, only you, loading, error |
| `wc-user-dialog` | Add user (email, name, role as `wc-choice-card`s with one sentence per role) and Change role | add, change role, duplicate email error, saving |
| `wc-temporary-password` | Shown once after add or reset: the grouped password in a monospace field, a Copy button, the sentence "Give this to {name}. It won't be shown again. They'll choose their own password when they sign in.", and Done | default, copied |
| `wc-totp-setup` | Settings › Your account two-step setup: the QR image, the secret for typing in by hand, a code field, Confirm; and a "Turn off" row once enrolled | not enrolled, setting up, wrong code, enrolled, removing |

`wc-password-form` is reused for "Change your password" in Your account. It is one
password operation by design, and this is one.

Confirmations (disable, reset password, sign out everywhere, remove two-step) use
`wc-confirm` / `confirmDialog()`, never `window.confirm`.

### 9.3 Settings layout in shared mode

The existing groups stay, and one group is added. Sections a role cannot use are not
rendered.

- **This book:** Letterhead (admins edit the name, address, phone, logo and payment
  instructions; others see it read-only), **Users**
  (admins only), Security (admins only; in shared mode the status card plus the sentence
  "The password is managed on the server.", with no change/remove forms; on unencrypted
  books the "Not encrypted" card recommends encrypting on the server before sharing), Location (admins
  only; in shared mode the path and the sentence "Change the folder on the server.", with
  no picker).
- **Your account** (shared mode only): name, email and role, Change your password,
  Two-step verification.
- **This device:** Appearance (everyone, stored in the browser); Updates (admins only in
  shared mode, because it describes the server's binary).

In local mode, Settings is exactly what it is now.

### 9.4 Role-aware UI

- `web/apps/app/src/state/roles.ts` mirrors the role table for the client methods the SPA
  calls: `can(role, 'importConfirm')` and so on. Screens and actions read it. A read-only
  user sees no Import, Review or Reconcile nav items, no write buttons, and no
  drag-and-drop import target. A bookkeeper sees no Countersign action on a document and
  sees the letterhead read-only.
- **Parity test:** the Rust table is captured to
  `web/apps/app/src/__fixtures__/role-table.json` by the existing `fixture_capture`
  pattern (`cargo test --features serve capture_role_table -- --ignored`). A Rust test
  fails if the fixture is stale, and a vitest fails if `roles.ts` disagrees with it.
  Hiding is a convenience. The server enforces regardless.
- If a `403` with `reason: "role"` still reaches a screen (a role changed mid-session),
  it shows a toast, "Your role no longer allows this.", and refetches `/api/auth/me`.

---

## 10. Desktop shells

- Tauri and the Swift shell's `InProcessTransport` use `build_desktop_router`, which is
  always local mode with the local owner as principal (section 1.1). `/api/auth/me`
  answers `{ mode: "local", user: null, role: "admin" }`, so the SPA shows no login and
  no account menu. Settings › Users does appear when the open books have users, so a Mac
  can manage the users of books it holds.
- `scheme.rs`/`transport.rs` need no change beyond the role guard being part of the
  desktop router.

---

## 11. Exposure and deployment (TASK-32.9)

### 11.1 Settings

Both go in `settings.json`, with environment overrides in the same form as the
invoicing keys:

| Key | Env | Value | Meaning |
|---|---|---|---|
| `server_public_url` | `NIGEL_SERVER_PUBLIC_URL` | One `https://` origin: scheme and host, an optional port, no path. For example `"https://books.example.com"` | This server's own address (below) |
| `client_ip_header` | `NIGEL_CLIENT_IP_HEADER` | `"CF-Connecting-IP"` or `"X-Forwarded-For"`, or unset | Which header carries the client IP, trusted only from a loopback peer (section 4.4) |

**`server_public_url` is the one address this server answers to besides loopback.** In shared mode:

- its host is the only non-loopback `Host` the host guard accepts;
- its origin is the trusted `Origin` for browser requests and for the CSRF rule (section 4.6);
- requests to it get `Secure` cookies and `Strict-Transport-Security`;
- it is the base for links the server sends itself. Phase 1 sends none; invitations
  (TASK-164) build their one-time links from it.

In local mode it is ignored (section 1). A value that is not `https`, has a path, or does
not parse is refused at startup with an error naming the value.

**One value only.** A second name for the same server, such as a Tailscale name next to a
tunnel's hostname, is refused by the Host check like any other unlisted name. An operator
picks one address per server. Anyone who needs another way in uses loopback over SSH,
which is always accepted. A list can come later if a deployment needs one, without
changing anything else here.

`server_public_url` belongs to the same family as the publishing settings `invoice_public_url`
and `document_public_url`, and is distinct from both. Those two are where published
invoice and document pages live on R2. They have nothing to do with where `nigel serve`
answers. (Their old names, `public_base_url` and `documents_base_url`, and the matching
environment variables, remain accepted; TASK-166 does the rename.) `docs/invoicing.md` and the deployment guide each
say so in one line.

### 11.2 Binding

The server keeps binding `127.0.0.1` only. Every supported deployment puts something on the same
machine in front of it: `cloudflared`, a reverse proxy such as Caddy, or `tailscale serve`.
That process connects to loopback and forwards the original `Host`. Exposure is decided
by the Host check and the mode, not by the bind address. There is no `--bind` flag. A
proxy on another machine is not a supported setup: run the tunnel or proxy on the server.

### 11.3 What `nigel serve` prints

- Local mode: what it prints now, the URL and the token link.
- Shared mode: `Nigel is serving <company> to signed-in users at https://books.example.com
  (and http://127.0.0.1:5731).` No token is printed, so none ends up in the journal.
- Notices go to stderr: `server_public_url` set in local mode; shared mode on unencrypted books
  (section 3); shared mode with `server_public_url` but no `client_ip_header`.

### 11.4 Unlocking at boot

- `nigel serve` resolves the books password at startup. `NIGEL_DB_PASSWORD_FILE` (a path)
  wins, then `NIGEL_DB_PASSWORD`. If the database is encrypted and either is set, the
  server unlocks before it binds, runs pending migrations, and works out the mode. A wrong
  or unreadable password is a fatal startup error with the existing message ("did not
  unlock … password may be wrong, or the database file may be damaged"). Under systemd,
  `Restart=on-failure` with a start limit turns that into a visible failed unit rather
  than a loop.
- `NIGEL_DB_PASSWORD_FILE` is added to `db::env_password_if_set`, so every command
  accepts it, including `nigel user …` run on the server. Setting both variables is an
  error. The file's content is the password, with one trailing newline stripped.
- With neither variable set, `nigel serve` behaves as it does now: it starts locked, in
  local mode, and unlocks over loopback.
- The users never learn the books password. The deployment holds it as a systemd
  credential (section 11.6), and section 8.3 keeps it off every shared-mode screen.

### 11.5 The deployment guide

The guide is a new `docs/deployment.md`, linked from the README. All hostnames are
placeholders. Contents:

1. **Before you start:** a server you control, encrypted books, the `nigel` binary,
   time synced (`timedatectl` shows `System clock synchronized: yes`), a `nigel` system
   user, and a data directory under `/var/lib/nigel`.
2. **Create the first admin on the server:** `nigel user add you@example.com --name
   "Pat Example" --admin`, then sign in at loopback over an SSH tunnel
   (`ssh -L 5731:127.0.0.1:5731 server`) to set your password before anything is public.
3. **Worked example, Cloudflare Tunnel (`cloudflared`):**
   ```yaml
   # /etc/cloudflared/config.yml
   tunnel: <TUNNEL-ID>
   credentials-file: /etc/cloudflared/<TUNNEL-ID>.json
   ingress:
     - hostname: books.example.com
       service: http://127.0.0.1:5731
     - service: http_status:404
   ```
   `cloudflared tunnel create nigel`, `cloudflared tunnel route dns nigel
   books.example.com`, and run it as a service. Then set
   `server_public_url: "https://books.example.com"` and `client_ip_header: "CF-Connecting-IP"`.
   Do not set `httpHostHeader`, because Nigel must see the public name. **Say plainly:**
   Cloudflare terminates TLS, so Cloudflare's edge sees the traffic, books data included.
   That is the operator's choice of provider, outside decision-12's "Nigel never reads
   your books". Operators who don't want it use one of the next two options. Cloudflare
   Access can be layered in front as an extra gate. Note that it stops non-browser
   clients (the Mac app's remote mode) unless they are given a service token.
4. **Reverse proxy with TLS (Caddy):** `books.example.com { reverse_proxy 127.0.0.1:5731 }`,
   `client_ip_header: "X-Forwarded-For"`. Port 443 open, DNS pointed at the server.
5. **Tailscale:** `tailscale serve --bg --https=443 http://127.0.0.1:5731` for people on
   your tailnet. Funnel is for the public internet, and there TLS terminates on your own
   machine. Set `server_public_url` to the `https://<machine>.<tailnet>.ts.net` name, and verify
   that the forwarded `Host` is that name. A server has one `server_public_url`, so Tailscale is
   an alternative to a tunnel, not an addition to one (section 11.1).
6. **systemd unit:**
   ```ini
   # /etc/systemd/system/nigel.service
   [Unit]
   Description=Nigel books server
   After=network-online.target
   Wants=network-online.target
   StartLimitIntervalSec=300
   StartLimitBurst=5

   [Service]
   User=nigel
   Group=nigel
   Environment=HOME=/var/lib/nigel
   LoadCredentialEncrypted=nigel-db-password:/etc/nigel/nigel-db-password.cred
   Environment=NIGEL_DB_PASSWORD_FILE=%d/nigel-db-password
   ExecStart=/usr/local/bin/nigel serve --no-open --port 5731
   Restart=on-failure
   RestartSec=5
   NoNewPrivileges=yes
   ProtectSystem=strict
   ProtectHome=yes
   PrivateTmp=yes
   ReadWritePaths=/var/lib/nigel
   StateDirectory=nigel
   StateDirectoryMode=0700

   [Install]
   WantedBy=multi-user.target
   ```
   `%d` in `Environment=` needs systemd 250 or later. With `HOME=/var/lib/nigel`,
   `settings.json` (with `data_dir`, `server_public_url` and `client_ip_header`) lives at
   `/var/lib/nigel/.config/nigel/settings.json`, and `server.key` beside it.
   Create the credential with
   `systemd-creds encrypt --name=nigel-db-password - /etc/nigel/nigel-db-password.cred`,
   typing the password on stdin so it never appears in shell history. The credential is
   encrypted with the host's key (and the TPM where one is present). It is visible only
   to the service, and not through `/proc/<pid>/environ` as an environment variable
   would be.
7. **Backups:** a `nigel backup` systemd timer, and copying the result off the machine.
   `server.key` is not in the backup, and the guide says what that means (section 3).
8. **Updates:** replace the binary and `systemctl restart nigel`. Migrations run at boot
   after the unlock. Read the release notes for a migration before updating.
9. **Changing the books password or folder:** stop the service, run `nigel password
   change` or `nigel load` as the `nigel` user, re-create the credential, and start the
   service.
10. **Giving someone access:** Settings › Users › Add user, choose a role, and pass on the
    temporary password through a channel other than the one you send the address by.
11. **Checklist:** the Host check works (`curl -H 'Host: other.example.com'
    http://127.0.0.1:5731/api/ping` gets `403`); sign-in works through the public name;
    `journalctl -u nigel` shows the login lines.

---

## 12. The Mac app's remote mode (contract for TASK-144.16)

144.16 builds the client. Phase 1 fixes the server contract it uses:

- **Login:** `POST /api/auth/login` with `"client": "native"`. The answer carries a
  `token` and `expiresAt`, and sets no cookie. The session row is `kind = 'bearer'`, with
  the same 30-day idle expiry and 90-day absolute lifetime, the same revocation (disable, reset, "sign out
  everywhere") and the same rate limits as a browser session.
- **Holding it:** the shell stores the token in the Keychain and attaches
  `Authorization: Bearer <token>` natively to every request it proxies. The page under
  `nigel://localhost` never sees it, as the Swift spec requires.
- **Requests:** the shell issues its own `URLSession` requests to the server, so `Host` is
  the public name and no `Origin` is sent. Bearer requests are exempt from the Origin rule
  (section 4.6) and still subject to the Host check.
- **Renewal:** bearer sessions renew on use like cookie sessions, within the same 90-day
  absolute lifetime. There is no refresh token. After 90 days the shell shows its
  connection sheet and the user signs in again.
- **Ending:** a proxied `POST /api/auth/logout` deletes the session. When the shell sees it
  succeed, it deletes the Keychain item and shows its connection sheet. Any `401` with
  `mode: "shared"` does the same.
- **TOTP:** the shell's connection sheet handles `totp_required` by asking for the code,
  just as the web login card does.

---

## 13. Room for the later work

| Later | How this design leaves room |
|---|---|
| **Attribution (32.4)** | `Principal` is already on every request. 32.4 passes an `Actor` (the user's `uid`, or `local` for `LocalOwner`/CLI) into data-layer write functions as a parameter, and each write is recorded as a discrete operation: who, when, entity, change. Users are never deleted, so `created_by`/`updated_by` never dangle. Nothing in phase 1 adds a write path that bypasses the data layer |
| **Audit log (32.5)** | The login and admin events in section 5 already have fixed fields, and become audit rows with the same fields. Session rows carry IP and user agent for the audit to reference. Entries key on `uid` and stable entity ids, the shape a 3.0 sync log needs (decision-12) |
| **Import history (32.6)** | Import routes are already bookkeeper-gated. Per-import undo by id is a role-table entry like any other route |
| **Optimistic concurrency (32.7)** | No interaction: the role guard runs before the handler, and `409 conflict` with a reason is the established shape for a stale write |
| **Invitations (164)** | An `invitations` table (hashed one-time token, email, role, expiry, inviting admin's `uid`) that creates the user row when accepted. `users` needs no change, because an invite becomes a user with a password the invitee chose and `must_change_password = 0`. 164's "revoke access, ending sessions at once" is `disable` from section 7 |
| **Mac remote (144.16)** | Section 12 |
| **Passkeys** | Sessions do not care how they were created. Passkeys add a `webauthn_credentials` table and `/api/auth/passkey/*` routes that end in the same session creation as password login |
| **Service keys in the keychain (160)** | Any route for them is admin in the role table |
| **Books package (163)** | `server.key` is per machine and deliberately outside the package. Users and sessions live in the database, inside the package |

---

## 14. Threat model

**Assets:** the books, users' credentials, live sessions, and the books password.

| Threat | Mitigation |
|---|---|
| Internet password guessing / credential stuffing | Argon2id; per-account and per-IP lockout; optional TOTP; 12-character minimum |
| Account enumeration | Uniform `invalid_credentials` and timing (a dummy hash for unknown emails); lockout counted for unknown emails too |
| CSRF / login CSRF | `SameSite=Strict`; Origin required on cookie-authenticated writes and login; JSON content type; GET never writes |
| DNS rebinding | Exact Host/Origin match against the mode's trusted set |
| Exposure before per-user login exists | A non-loopback Host is refused unless the mode is shared, and shared mode needs a user. A locked server is local. Covered by an end-to-end test (section 16) |
| Stolen session cookie or bearer token | `HttpOnly`, `Secure` on the `server_public_url` host, server-side revocation (logout, sign out everywhere, disable), 30-day idle expiry and a 90-day absolute lifetime |
| XSS in the SPA | Lit escapes by default; the QR is an `<img>`, not markup; the cookie is `HttpOnly`; a Content-Security-Policy (TASK-32.10) lands before public exposure (section 4.7) |
| Copied database or stolen backup | Hashes only for passwords and sessions; TOTP sealed under a key outside the data directory; SQLCipher when on |
| Bookkeeper or read-only overreach | Server-side role table with deny-by-default; the letterhead and countersigning are admin-only; shared-mode refusals for books swaps and re-keys |
| A bookkeeper's changes are not attributed | Accepted for phase 1 (decision-11): the first deployment is a household with two trusted users. Bookkeeper access waits for 32.4/32.5 in phase 3 |
| Forged client-IP header | Trusted only from a loopback peer. The per-account limit applies regardless |
| Another local user on the server | In shared mode they need a login like anyone else. In local mode the token. The journal, which may hold a local-mode token, is readable only by root and `adm` |
| Tunnel provider reads traffic | Disclosed in the guide (section 11.5). Alternatives with TLS on the operator's machine are documented |
| Root on the server | Out of scope: root holds the credential and the file |

---

## 15. Failure modes

| Situation | What happens | Recovery |
|---|---|---|
| Admin forgot their password | Login fails | On the server: `nigel user reset-password <email>`, then sign in with the temporary password and choose a new one |
| Admin lost their TOTP device | The code step fails | `nigel user reset-password <email> --clear-totp`, or another admin uses "Remove two-step" |
| Every admin disabled or demoted | Prevented by the last-admin invariant (section 2) | — |
| A session reaches 90 days | The next request gets `401` (`mode: "shared"`) | Sign in again |
| Locked out after failed attempts | `429` with a countdown | Wait 15 minutes, or restart the service |
| TOTP always rejected | The server's or the phone's clock is off by more than ±30 s | Fix NTP (`timedatectl`). The guide's prerequisite step checks it |
| `server.key` lost or replaced | TOTP fails for every enrolled user; the log names them | `--clear-totp` per user; they enroll again |
| Credential missing at boot | The server starts locked, so local mode; the tunnel gets `403` | Fix the credential and restart, or unlock over an SSH tunnel with the token link |
| Credential wrong at boot | The unit fails with the unlock error | Re-create the credential |
| `server_public_url` is `http://` or has a path | Startup fails, naming the value | Use the `https://` origin |
| A second hostname (a Tailscale name next to the tunnel, say) | `403` on that name | One `server_public_url` per server; loopback over SSH for anything else (section 11.1) |
| Host header rewritten by the proxy | `403` through the tunnel | Remove the rewrite; the guide's checklist catches it |
| `client_ip_header` unset behind a proxy | All clients share one IP bucket, so one attacker can trip it for everyone (accounts still separate) | Set the header; startup warns |
| A role changed mid-session | The next request is judged on the new role | — |
| A user added with the CLI to a running local-mode server | The next request naming the `server_public_url` host re-evaluates the mode (section 1) | — |
| Two people edit the same row | Last write wins until 32.7 | 32.7 |

---

## 16. Testing

All Rust tests run serially (`--test-threads=1`), because the books password is a process global.

**Unit (`nigel-core`):**
- Argon2id hash and verify, PHC parameters pinned, dummy-hash timing path exercised.
- Password rules: each reason, the NFC normalization, and the temporary password format.
- TOTP against the RFC 6238 SHA-1 vectors; acceptance at −1, 0 and +1 steps and refusal at
  ±2; replay refused.
- Sealing: round trip; the wrong `uid` as associated data fails; the wrong key fails.
- The rate limiter on an injected clock: thresholds, sliding window, reset on success,
  `retryAfterMs`.
- Migration 17 on a fresh database and on a database at 16. Idempotent when rerun.
- Data layer: create/list/role/disable/enable/reset; email normalization and uniqueness;
  the last-admin invariant from every path; disable ends sessions; password change ends
  the other sessions.

**Server integration (tower `oneshot`, `server/testutil.rs`):**
- Mode derivation: no users → local; users → shared; encrypted and locked with users →
  local; unlock → shared, and the token cookie is then refused.
- Host guard: in local mode the `server_public_url` host gets `403`; in shared mode it passes and
  any other non-loopback host still gets `403`; the Origin is compared as a whole origin
  against `server_public_url`; an invalid `server_public_url` fails startup.
- Login, cookie attributes (`Secure` only on the `server_public_url` host), the `native` bearer token,
  logout, rolling renewal on an injected clock, idle expiry, the 90-day absolute lifetime
  (renewal never extends past it, and the cookie `Max-Age` shrinks toward it), the must-change-password
  allowlist, TOTP step-up, lockout, and the missing-Origin refusal on cookie writes
  (bearer exempt).
- **The role matrix:** for every entry in `ROUTE_ROLES`, × {anonymous, read_only,
  bookkeeper, admin} in shared mode, and × local owner, the request is refused with
  `401` or `403` exactly when it should be. Admitted requests are asserted not to be
  `401`/`403`. The matrix is generated from the table, so a new route is covered when it
  is listed.
- Deny-by-default: a test-only route with no table entry gets `403 no_role_entry` for the local owner.
- Every `GET` in the table maps to `read_only`. The locked-guard and no-session sweeps run
  over the full derived inventory.
- The shared-mode refusals in section 8.3.
- The desktop router: the local owner, role guard present, no session layer.

**End-to-end (`crates/nigel/tests/serve_exposure.rs`, behind the `serve` feature):** it
starts the real binary (`CARGO_BIN_EXE_nigel serve --no-open --port 0`) with a temporary
`HOME`/`XDG_CONFIG_HOME` and fixture books, reads the printed port, and talks raw HTTP
over TCP:
1. No users, `server_public_url = "https://books.example.com"` → `GET /api/ping` with
   `Host: books.example.com` gets **`403 forbidden`**, and with `Host: 127.0.0.1` gets
   `401`. This is the test that the server is not exposed before per-user login exists.
2. Add a user with `nigel user add … --admin` and restart → the same public-Host request
   gets `401` with `details.mode = "shared"`; `Host: other.example.com` gets `403`; the
   old token link gets `401`; login over the public Host returns a `Secure` cookie.
3. Encrypted books with users and no password variable → the public Host gets `403`
   (locked is local). With `NIGEL_DB_PASSWORD_FILE` set → `401` (shared).

**Web (vitest, from `web/`):**
- `describePreviewA11y` over every preview state of the seven new components.
- Screen tests: the gate order; the shared `401` shows login rather than the
  local-session banner; the must-change step; the account menu absent in local mode; the
  Settings sections present per role; write controls absent for read-only.
- `roles.ts` parity against the captured `role-table.json`.
- The `api/` guard test covers the new client methods.

---

## 17. Implementation sequence

Each step is one PR to `main`, reviewed before the next one that depends on it. Sizes: S
is about a day, M two to three days, L four to five.

| # | PR | Tasks | Size | Depends on |
|---|---|---|---|---|
| 1 | **Users data layer and CLI.** Migration 17; the `identity` module (users, sessions, Argon2id, password rules, TOTP, sealing and `server.key`); `nigel user add/list/role/disable/enable/reset-password`; `NIGEL_DB_PASSWORD_FILE`; `docs/commands.md` | 32.1 | M | — |
| 2 | **Auth and modes.** `Principal`; mode derivation and re-evaluation; mode-aware host guard with `server_public_url` and whole-origin matching; the session layer (cookie and bearer, rolling renewal, must-change allowlist); login/logout/me and the account routes; the rate limiter and `client_ip_header`; boot unlock in `serve`; the new security headers; the CSRF Origin rule; login event lines; the end-to-end exposure test. **Until PR 3, every non-auth route needs admin**, a temporary fail-closed table, so a non-admin user can do nothing while roles are unfinished. `docs/api.md`, `docs/design-constraints.md`, `docs/architecture.md` | 32.2 and the exposure half of 32.9 | L | 1 |
| 3 | **Roles.** `ROUTE_ROLES` and `role_guard` (deny by default); the users admin API; the shared-mode refusals; the test inventory derived from the table, closing today's gaps; the role matrix test; `role-table.json` capture; `docs/api.md` role table | 32.3 | M | 2 |
| 4 | **Web.** The seven components with previews and a11y; session state and gates; the account menu; Settings › Users, Your account, and the shared-mode variants of Security, Location and Updates; `roles.ts` and its parity test; README | 32.8 | L | 3 (components can start after 2 merges) |
| 5 | **Deployment guide.** `docs/deployment.md` (section 11.5); README Configuration and links; a one-line note in `docs/invoicing.md` on `server_public_url` versus `invoice_public_url` and `document_public_url`. Verified on the operator's server against the checklist, with placeholders only in the text | 32.9 (docs half) | S–M | 2 (drafting); merges after 4 |

Alongside these, the **Content-Security-Policy** (TASK-32.10) is its own phase-1 task: S–M, after
PR 2, independent of PRs 3–5 (section 4.7).

PRs 1–3 each carry a migration or the core auth path, so they run one at a time. Integer
money (TASK-59) waits for PR 1's migration number, as decision-11 orders. PR 5 can be
drafted alongside PRs 3 and 4. **The public address goes live only after both PR 3 and
the Content-Security-Policy task (TASK-32.10) have merged.** PR 2's admin-only stopgap is safe but
useless to a second person who is not an admin, and a CSP is a precondition for public
exposure.

---

## 18. Where the backlog text and this design differ

These are for the operator to reconcile in the tasks. This spec does not edit `backlog/`.

- **32.1** says "username" and "active flag". The design uses email and `disabled_at`,
  and adds `uid`, TOTP, must-change-password and session fields.
- **32.2** says "multiuser mode is an explicit setting". The design derives it from the
  existing users. It says "unlock becomes admin-only". The design has a shared server
  unlock at boot from a credential and never over HTTP.
- **32.3** lists admin-only "settings mutation". The design keeps that, makes the
  letterhead (logo and payment instructions included) and countersigning admin-only, and
  adds the shared-mode refusals.
- **32.8** says the admin users screen does "reset password". The design specifies a
  temporary password shown once, and adds "sign out everywhere" and "remove two-step".
- **32.9 AC #1** asks for a `--bind` flag with acknowledgment. The design has no
  `--bind`: the server always binds loopback behind a same-machine tunnel or proxy, and
  exposure is gated on the mode. The trusted-host setting is `server_public_url`, a single
  address. AC #3 adds Cloudflare Tunnel as the worked example.
- **Content-Security-Policy** is TASK-32.10, a phase-1 subtask of epic 32 separate from
  PR 2. It gates public exposure.
- **The publishing settings rename** (`invoice_public_url`, `document_public_url`, old
  names still read) is TASK-166.
