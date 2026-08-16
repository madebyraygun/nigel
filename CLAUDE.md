# CLAUDE.md

Working memory for agents in this repository: the rules, the commands, and where to look
for everything else. It is loaded into every context, so it stays short — see the size
budget at the end.

## Project Overview

Nigel — a Rust CLI bookkeeping tool to replace QuickBooks for small consultancies, usable for
personal finances as well. Cash-basis, single-entry accounting with bank CSV/XLSX imports,
rules-based categorization, and SQLite storage. A database keeps books under a **profile** —
`business` (the default, Schedule C / 1120-S chart of accounts) or `personal` (a household chart
with no tax mapping) — chosen at `nigel init --profile` or in onboarding and stored in the
`metadata` table. A web UI and JSON API ship in the same binary behind `nigel serve`.

## Where things are documented

| Read this | When |
|---|---|
| `docs/architecture.md` | What a module is, what it owns, how the pieces fit, the tree on disk |
| `docs/design-constraints.md` | The rules the code must keep, and why each exists |
| `docs/commands.md` | The full CLI reference |
| `docs/api.md` | HTTP endpoint inventory, error envelope, security model |
| `docs/invoicing.md` | Invoicing setup (Stripe, Mailgun, R2) and commands |
| `docs/importers.md` | Importer formats and how to add one |
| `docs/backlog-cli.md` | The `backlog` CLI manual |
| `backlog/decisions/` | Decisions with their reasoning, newest first |

Before changing behaviour in an area, read that area's entry in `docs/design-constraints.md`.
Most of what looks like a free choice there was decided already, and the reason is written down.

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
## Commands

Full reference in `docs/commands.md`; `nigel --help` is always current. These are the ones that
are not discoverable from `--help`:

```bash
cargo build --release                                  # Release build
cargo test -- --test-threads=1                         # All tests — SERIAL, the DB password is a process global
cargo test --no-default-features -- --test-threads=1   # Without gusto/pdf
cargo test -p nigel-core -- --test-threads=1            # nigel-core alone — a root-level run unifies its deps'
                                                         #   features with nigel's, masking what nigel-core ships
cargo fmt --check                                      # CI runs this first; a failure here fails the build
./scripts/check-no-real-data.sh --staged               # Judge by EXIT STATUS, never by grepping its output
nigel                                                  # Interactive dashboard
nigel serve                                            # Web UI + JSON API on 127.0.0.1:5731
```

`cargo build` works without node — `build.rs` seeds `web/dist` from `web/placeholder/index.html`
and the binary serves a "SPA not built" page. Run `npm run build` in `web/` before
`cargo build --release` to embed the real app.

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

Documentation lives where the detail lives, and this file stays small.

- A feature change updates the **relevant `docs/` file** — architecture in
  `docs/architecture.md`, a rule in `docs/design-constraints.md`, an endpoint in `docs/api.md`.
- A **user-facing** change updates `README.md` — Quick Start, Features, Configuration. That is
  the file someone reads before they have the repo, so it is the one that must not go stale.
- **CLAUDE.md changes only when a command, a rule, or a pointer changes.** It is not updated
  per PR, and a PR that only adds prose here is doing it in the wrong place.
- Design *rationale* belongs in the commit message, the decision record, or the module's own
  doc comment — the places a reader already goes to ask "why is this like this".
- Describe the current state. No "added in", "was formerly", "changed in version" — `git log`
  and `backlog/decisions/` carry history, and a doc that narrates its own edits rots fastest.

**Size budget: this file stays under 400 lines and 25 KB.** It is loaded into every context, so
every line is paid for on every turn of every session. Over budget means moving something to
`docs/`, not tightening the margins.

## Backlog.md tasks

The `backlog` CLI owns the task files; the full manual is in `docs/backlog-cli.md`.

- **Never edit a task file directly.** Every read and write goes through `backlog task …`,
  which keeps the metadata, the file naming and git in step.
- Use `--plain` on every read (`backlog task 42 --plain`, `backlog task list --plain`).
- **Task files are committed to `main`, never onto a feature branch** — a task is a
  project-wide record, independent of whether its branch lands. Closing or updating a task
  as part of the branch that does the work is the exception.
- Taking a task: `backlog task edit <id> -s "In Progress" -a @<you>`, then add a plan and
  share it before writing code.
- Only implement what the acceptance criteria say. More work means editing the AC first or
  filing a follow-up task.

The `backlog` tool writes its own manual into this file on some upgrades. If a few hundred
lines of generic CLI tutorial reappear here, move them back to `docs/backlog-cli.md`.
