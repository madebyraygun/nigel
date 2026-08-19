# First-Run Onboarding in the Desktop App (TASK-33.17) — Design

**Goal:** A machine that has never run Nigel gets a first-run experience in the
desktop app — and from a browser against `nigel serve` — that is delightful,
on-brand, and functionally equivalent to the CLI's onboarding: profile, identity,
optional password, then demo / start fresh / load existing books. Today that
machine gets a broken dashboard over a zero-byte database.

## Facts the design rests on

- The CLI's onboarding TUI (`crates/nigel/src/cli/onboarding.rs`) runs when
  `settings.json` does not exist and collects, in order: profile (business /
  personal), name + business-or-household name + optional password (with a
  confirm step), then one of three exits — view the demo, start from scratch,
  load an existing data directory. `cli/dashboard.rs` consumes the result:
  saves `user_name` into `settings.json`, sets the process-global database
  password *before* the file exists (so SQLCipher's `PRAGMA key` encrypts it
  from the first write), creates the data dir plus `exports/`, `snapshots/`,
  `backups/` with restricted permissions, runs `init_db_with_profile`, and
  writes `company_name` into database metadata. That consumption logic lives
  inline in the CLI and nowhere else.
- The demo (`cli/demo.rs`) is CLI-only: `setup_demo()` builds a separate
  `<data_dir>/demo/` directory with its own seeded database (company
  "Acme Consulting LLC") and repoints `settings.data_dir` at it. Nothing in
  `nigel-core` can reach it, so no HTTP route can offer it.
- `GET /api/status` already answers `initialized`, but it means only
  `db_path.exists()` (`routes/status.rs`) — a zero-byte file left by a stray
  connection counts as initialized. The SPA's boot phases
  (`web/apps/app/src/state/app-store.ts`) are `starting | locked | failed |
  ready`; nothing consumes `initialized`. The `locked` gate replaces the shell
  entirely (`nigel-app.ts` `.gate`), which is the layout precedent a setup gate
  wants.
- `nigel serve` (`cli/serve.rs`) runs `init_db` at startup whenever the
  database is unencrypted — **including when the file does not exist yet** — so
  a web-first user silently gets default books and no setup ever. The desktop
  shell (`nigel-desktop/src/main.rs`) runs no pre-flight at all.
- Guard ordering: in web mode `session_guard` wraps all of `/api`
  (`server/mod.rs::build_router`), and `locked_guard` inside it ungates only
  `/ping`, `/status`, `/unlock`. The desktop router has no session guard by
  construction. `locked` requires an encrypted file, so a nonexistent database
  is never `locked`.
- The web brand infrastructure already exists and is parity-pinned:
  `@nigel/theme`'s `gradient.ts` holds `NIGEL_PALETTE` (the TUI's `GRADIENT`
  stops, enforced by `palette-parity.test.ts`), `gradientColor(t)` (a port of
  `effects::gradient_color`), gradient CSS custom properties, and
  `brandCycleKeyframes`. The TUI particle system is ported in
  `web/packages/ui/src/components/snake-engine.ts` (`MAX_PARTICLES`,
  `PARTICLE_CHARS`). The website renders the wordmark as per-character
  `<span class="char">` elements sharing one animated gradient with staggered
  `animation-delay` (`site/main.js`, `site/styles.css`). No web wordmark or
  splash component exists yet.
- The TUI onboarding's intro is particles for 500 ms, a 500 ms shuffled
  per-character logo reveal, then the form — skippable by any key. Its voice is
  the same register as the dashboard's `GREETINGS`.

## Design

### 1. A shared setup engine in `nigel-core`

The CLI's inline consumption logic becomes `nigel_core::setup`, the single
implementation both the TUI and the HTTP route call:

- `pub struct SetupPlan { pub user_name: String, pub company_name: String,
  pub profile: db::Profile, pub password: Option<String> }`
- `pub fn run(plan: &SetupPlan) -> Result<()>` — exactly the dashboard's
  sequence today: save `user_name` to settings, set the password global,
  create the data dir and its three subdirectories with restricted
  permissions, `get_connection` + `init_db_with_profile`, write
  `company_name` metadata. `cli/dashboard.rs` is refactored to call it; the
  TUI keeps only what is TUI-shaped (the screens, the profile-mismatch
  notice).
- Demo seeding moves to `nigel_core::demo`: `seed_demo(conn)` and
  `setup_demo_dir() -> Result<PathBuf>` (build `<data_dir>/demo/`, init, seed,
  repoint `settings.data_dir`), with `cli/demo.rs` reduced to the CLI wrapper
  and its stdin/stdout concerns. Same fixture cast, same idempotence.

### 2. The server tells the truth about an empty machine

- `initialized` tightens to "the file exists **and is non-empty**", so a
  zero-byte remnant reads as uninitialized.
- `nigel serve`'s pre-flight stops creating the database: it migrates an
  existing unencrypted file and otherwise leaves absence alone, so a web-first
  user reaches setup instead of silently getting default books.
- New route `POST /api/setup`, body
  `{userName, companyName, profile, password?, action: 'fresh' | 'demo'}`
  (camelCase, `deny_unknown_fields`):
  - Refuses with 409 when the database is already initialized — setup is not
    re-runnable, and the guard is the route's, not the client's.
  - Builds a `SetupPlan` and calls `nigel_core::setup::run`. For `demo` it then
    calls `setup_demo_dir` and rebinds `AppState` to the demo database the way
    the data-directory switch does (`state.set_db_path`, under the write gate).
  - Answers the fresh `StatusResponse`, so the client's next render needs no
    second round trip.
  - Sits behind the normal guards. In web mode the session guard applies (the
    user arrived via the token URL, so they have a session); `locked_guard`
    never blocks it because an uninitialized database cannot be locked. No
    ungated-path changes.
- "Load an existing data directory" is not a setup-route concern: the identity
  step has already run `setup`-adjacent persistence for `user_name` only via
  the flow below, and the existing data-directory switch
  (`routes/settings.rs`) already validates, migrates and rebinds. The SPA's
  load path calls that. If the target is encrypted, the boot phase flips to
  `locked` and the existing unlock gate takes over — the flows compose instead
  of duplicating.

### 3. The SPA gains a `needs-setup` boot phase

In `app-store.ts`: `locked` wins first (an encrypted file exists and needs its
key); then `needs-setup` when `status.initialized === false`; then `failed`,
`starting`, `ready` as today. `nigel-app.ts` renders the setup screen in the
same shell-replacing `.gate` treatment the unlock screen uses.

The setup screen walks the TUI's steps, one visible at a time:

1. **Arrival** — the gradient wordmark reveals over drifting particles, then
   the first question slides in. Any click/keypress skips straight to the
   form, and `prefers-reduced-motion` gets a static gradient wordmark with no
   particles and no reveal — delight is additive, never a gate.
2. **Profile** — "Right then — what are we keeping books for?" Two cards:
   business (Schedule C / 1120-S) and personal.
3. **Identity** — name, business-or-household name (label follows the profile,
   as the TUI's does), optional password with an inline confirm field that
   appears only when the password is non-empty. Mismatch is an inline error,
   never a dead end.
4. **First move** — "How shall we start?" Three cards: view the demo, start
   from scratch, load an existing data directory. Demo and fresh submit
   `POST /api/setup`; load asks for the directory (a text field in the
   browser; the desktop shell may add a native folder dialog later without
   changing this flow) and calls the data-directory switch.

On success the store refreshes status and boot lands on `ready` (or `locked`
for an encrypted loaded directory); the dashboard greets a named user in a
named company. Copy throughout is in Nigel's voice — the register of
`GREETINGS`, concrete and dry, never corporate. TASK-116 (the shared greeting
source) is adjacent, not a dependency: setup copy is its own, but it must read
like the same person.

### 4. Brand components (Component-First)

Both ship in `@nigel/ui`, read only `@nigel/theme` tokens, and carry previews
plus `describePreviewA11y` states:

- **`wc-wordmark`** — the ASCII wordmark as per-character spans sharing one
  animated gradient with staggered delays (the website's mechanism, driven by
  `NIGEL_PALETTE` tokens instead of the site's inline hexes). Properties for
  `animated` and `reveal`; reduced-motion renders the static gradient. The
  ASCII art is the TUI's `LOGO`, and a parity test pins the two against each
  other the way `palette-parity.test.ts` pins the palette.
- **`wc-particle-field`** — the ambient drift, extracted from the engine
  already ported in `snake-engine.ts` rather than ported a second time.
  Density-capped, `aria-hidden`, inert under reduced motion.

The setup flow itself is a screen composition in `apps/app` using existing
`wc-*` form primitives; any new visual element it needs beyond these two goes
through `@nigel/ui` like everything else.

### 5. Desktop shell touches

- `main.rs` gains `.min_inner_size(900.0, 700.0)` so neither setup nor the
  shell ever renders crushed.
- No desktop-specific setup code otherwise: the shell reaches the same
  `/api/setup` over the custom scheme, with no session dance because the
  desktop router never had one. Keychain-backed unlock stays TASK-33.4;
  packaging stays 33.6.

## Testing

- **Core:** `nigel_core::setup::run` unit tests — fresh dir tree with
  restricted permissions; encrypted-from-first-write when a password is in the
  plan (reopen without the key fails); profile honored; `company_name` and
  `user_name` land where they belong. Demo move: seeding stays idempotent,
  `setup_demo_dir` repoints and seeds, CLI wrapper tests keep passing.
- **Server:** route tests — fresh setup answers ready status; demo setup
  rebinds to the demo database and its company name; password setup leaves an
  encrypted, unlocked process; second call answers 409; `initialized` is false
  for a zero-byte file; serve pre-flight leaves an absent database absent.
- **SPA:** app-store boot derivation (locked beats needs-setup; needs-setup
  beats ready); setup screen step tests with the fake client (profile → label
  switch, password confirm mismatch, demo/fresh submit shapes, load path
  delegating to the data-dir call, skip-intro, error surfacing); component
  previews with zero axe violations, including reduced-motion states.
- **CLI:** existing onboarding/dashboard tests stay green through the
  refactor; the TUI's behavior is unchanged.
- **Manual (operator, macOS):** desktop first run under a clean `HOME` —
  arrival animation, skip, each exit; a password setup that unlocks on
  relaunch; demo books landing with fixture data. State the steps, never real
  figures.

## Out of scope

- OS keychain and unlock persistence (TASK-33.4).
- A native folder picker for the load path (folds into later desktop polish).
- The shared greeting source (TASK-116).
- Any change to what the TUI onboarding collects or offers.
