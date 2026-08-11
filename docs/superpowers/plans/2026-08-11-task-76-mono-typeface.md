# Task 76 — A bundled mono primary typeface — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Spec: `docs/superpowers/specs/2026-08-11-task-76-mono-typeface-design.md`.
Read it first — every "why" below lives there.

**Goal:** the browser reads as the same product as the CLI. A self-hosted,
subset, bundled mono as the primary face, no network request for type, the
register and report tables still legible, and the added binary weight measured.

**One PR (PR-4c), seven commits.** Last of Stream 4, after PR-4a (TASK-72) and
PR-4b (TASK-75).

**Two decisions need Sam's sign-off before Task 1**: the family (spec §2
recommends IBM Plex Mono) and the scope (spec §3 recommends mono everywhere in
the app, and the published invoice keeping its text face). Do not start without
them — the whole task is those two decisions plus their consequences.

## Architecture

`--wa-font-family-sans` in `web/packages/theme/src/tokens/typography.ts` is the
single owner of the primary face; no component names a family. Fonts are added
as a new token module (`src/tokens/font-faces.ts`) composed into `nigelTheme`,
with the woff2 files committed under `packages/theme/src/fonts/` and copied to
`dist/fonts/` by `scripts/build-css.js`. `@font-face` URLs are **relative**, so
Vite resolves them from `dist/css/nigel.css` and emits hashed copies into
`web/dist/assets/`, which rust-embed bakes into the binary and
`src/server/static_files.rs` serves as `font/woff2` with an immutable cache
header. The preview harness gets the same faces from the same declaration,
because it aliases the same built CSS file.

## Tech stack

lit 3, vite 6, vitest 2, TypeScript 5.7, rust-embed. Font tooling
(`fonttools`/`pyftsubset`) is used **once, locally**, to produce the committed
subsets — it is never a build or CI dependency.

## Global constraints

- **TDD, always** for the code; **measure, never estimate** for the sizes.
- **All web commands run from `web/`**, in CI's order:

  ```bash
  npm ci
  npm run lint           # eslint across all three packages
  npm run typecheck      # tsc --noEmit across all three packages
  npm test               # vitest across all three packages
  npm run build          # theme -> ui -> app, output to web/dist
  ```

- Per-package: `npm test -w @nigel/theme`, `npm test -w @nigel/ui`,
  `npm test -w @nigel/app`. Single file:
  `npm test -w @nigel/theme -- __tests__/font-faces.test.ts`.
- **The contrast suite** (`packages/theme/__tests__/contrast.test.ts`) and the
  **a11y suite** (`describePreviewA11y` in every `@nigel/ui` component test)
  both run inside `npm test`. Neither should be affected by this task; if either
  moves, something unintended changed.
- **`packages/theme/__tests__/build-css.test.ts` runs the real pipeline**
  (`npx tsc` then `node scripts/build-css.js`) — that is where the "the fonts
  actually reach `dist/`" assertion belongs, for the same reason it already
  says: a stylesheet that only exists because a test wrote it proves nothing.
- **`cargo build --release` must follow `npm run build`.** rust-embed bakes
  `web/dist`; `build.rs` emits `rerun-if-changed` for it. A binary built without
  the web build ships the placeholder.
- **Nothing in the SPA may fetch a font at runtime.** No `<link>`, no
  `@import url(http`, no absolute font URL. Tests enforce it (Task 3).
- **Component-First UI Workflow** still applies: the one new preview state in
  Task 6 carries its axe run automatically.
- Conventional Commits.

## Prerequisite check (do this first)

- [ ] **Step 0: confirm PR-4a and PR-4b have merged.**

  ```bash
  rg -n "controlsCss" web/packages/theme/src/index.ts
  rg -n "COLOR_MODE_STORAGE_KEY" web/packages/theme/src/index.ts
  ```

  Both must hit. Judging type on an unpainted UI, or in only one of two modes,
  wastes the harness walk that is most of this task's value.

- [ ] **Step 1: record the two decisions** (family, scope) in the task notes
      before writing code, with whatever Sam said. Everything below assumes
      IBM Plex Mono, weights 400/500/600, mono everywhere in the app, and the
      invoice template unchanged.

- [ ] **Step 2: baseline the measurements** — these are AC #7's "before":

  ```bash
  cd web && npm run build && du -sb dist
  cargo build --release && ls -l target/release/nigel
  ```

  Write both numbers down. Same machine, same toolchain, for the "after".

---

## Task 1: Acquire, subset and commit the faces

**Files:** create `web/packages/theme/src/fonts/*.woff2` and
`web/packages/theme/src/fonts/LICENSE`.

**Interfaces:** none yet — bytes only.

- [ ] **Step 1: Check whether the pinned release ships a variable woff2.** If it
  does, one file spanning 400–600 replaces three, and it is usually smaller than
  two statics. If it does not, take Regular 400, Medium 500, SemiBold 600. This
  is a fact to look up, not a decision to make.
- [ ] **Step 2: Subset.** Latin ranges plus the symbols the UI draws (spec §4.5):

  ```bash
  pyftsubset IBMPlexMono-Regular.ttf \
    --output-file=ibm-plex-mono-400.woff2 --flavor=woff2 --layout-features='' \
    --unicodes="U+0000-00FF,U+0100-017F,U+2000-206F,U+20A0-20BF,U+2122,\
U+2190-2193,U+25A1,U+25B2,U+25C6,U+25CF,U+25D1,U+2298,U+2713,U+2717,U+27F3"
  ```

  Record the exact command used — it goes verbatim into `web/README.md` in
  Task 7, and it is the only way anyone regenerates these files later.

- [ ] **Step 3: Sanity-check coverage** before committing: open each woff2 and
  confirm `— … “ ” ’ · • ✓ ✗ × ⟳ ◻ ◑ ● ◆ ▲ ⊘ ← ↑ → ↓` and `é ñ ü` all render.
  A missing glyph falls back mid-word, which is the failure this subset is sized
  to avoid.
- [ ] **Step 4: Measure** `ls -l` each file and the total. Write it down.
  **If the total is over ~150 KB, stop and look** — the subset is wrong, not the
  plan.
- [ ] **Step 5: Commit** the files plus the OFL text:
  `feat(theme): vendor the IBM Plex Mono latin subsets`. Say the measured sizes
  in the commit body.

---

## Task 2: `font-faces.ts` and the build copy

**Files:** create `web/packages/theme/src/tokens/font-faces.ts` and
`web/packages/theme/__tests__/font-faces.test.ts`; modify
`web/packages/theme/src/themes/nigel.ts`, `src/index.ts`,
`scripts/build-css.js`, `__tests__/build-css.test.ts`.

**Interfaces:** produces `export const fontFacesCss: CSSResult`, composed into
`nigelTheme` **first**, before the token modules.

- [ ] **Step 1: Write the failing tests** in `font-faces.test.ts` (modelled on
  boxcraft's `packages/theme/__tests__/font-faces.test.ts`, which is the working
  precedent for this shape):

```ts
it('declares one face per bundled weight');
it('names the family the typography token asks for', () => {
  expect(typographyCss.cssText).toContain("'IBM Plex Mono'");
  expect(fontFacesCss.cssText).toContain("font-family: 'IBM Plex Mono'");
});
it('sets font-display: swap on every face');
it('sources every face from a relative woff2 — no CDN, no absolute path', () => {
  const srcs = [...fontFacesCss.cssText.matchAll(/src:\s*url\((['"])(.*?)\1\)/g)].map((m) => m[2]);
  expect(srcs.length).toBeGreaterThan(0);
  for (const src of srcs) {
    expect(src).toMatch(/^\.\.\/fonts\/.*\.woff2$/);
  }
});
it('leaves no http reference anywhere in the composed sheet', () => {
  expect(nigelTheme.cssText).not.toMatch(/url\(\s*['"]?(https?:)?\/\//);
});
it('composes into nigelTheme so both apps pick it up');
```

  And in `build-css.test.ts`, the assertion that catches the failure mode that
  would otherwise ship silently:

```ts
it('copies every declared face into dist/fonts', () => {
  const declared = [...nigelTheme.cssText.matchAll(/url\(['"]\.\.\/fonts\/(.*?)['"]\)/g)]
    .map((m) => m[1]);
  expect(declared.length).toBeGreaterThan(0);
  for (const file of declared) {
    expect(existsSync(resolve(pkgRoot, 'dist/fonts', file)), file).toBe(true);
  }
});
```

- [ ] **Step 2: Implement.** The `@font-face` module, `${fontFacesCss}` first in
  `nigelTheme`, the export in `index.ts`, and a copy step in `build-css.js`
  (`fs.cpSync(src/fonts → dist/fonts, { recursive: true })`) beside the existing
  `mkdirSync`/`writeFileSync`.
- [ ] **Step 3: Verify.** `npm test -w @nigel/theme` — the build test runs the
  real `tsc` + script pipeline, so this proves the copy, not just the intent.
- [ ] **Step 4: Commit** `feat(theme): declare the self-hosted mono faces`.

---

## Task 3: Prove nothing is fetched

**Files:** create `web/apps/app/src/__tests__/no-remote-fonts.test.ts`.

- [ ] **Step 1: Write the test** (node environment, a source scan in the shape
  of `api-seam.test.ts`): `apps/app/index.html`,
  `packages/ui/preview/index.html` and `web/placeholder/index.html` contain no
  `<link rel="preconnect">`, no `fonts.googleapis`, no `fonts.gstatic`, and no
  `@import url(http`.
- [ ] **Step 2: Verify** it passes now (it should) and that it fails if you add
  a Google Fonts link temporarily. A guard that has never failed is not a guard.
- [ ] **Step 3: Commit** `test(web): fail the build on a remote font reference`.

---

## Task 4: Flip the token

**Files:** modify `web/packages/theme/src/tokens/typography.ts`,
`web/apps/app/index.html`; modify
`web/packages/theme/__tests__/nigel-theme.test.ts`.

**Interfaces:** `--wa-font-family-sans` and `--wa-font-family-mono` both become
the Plex Mono stack.

- [ ] **Step 1: Write the failing test:**

```ts
it('makes the bundled mono the primary face', () => {
  expect(nigelTheme.cssText).toMatch(/--wa-font-family-sans:\s*'IBM Plex Mono'/);
});
it('keeps a system mono behind it, so a missing face still aligns columns', () => {
  expect(nigelTheme.cssText).toMatch(/--wa-font-family-sans:[^;]*ui-monospace/);
});
```

- [ ] **Step 2: Implement.** Both family tokens point at
  `'IBM Plex Mono', ui-monospace, SFMono-Regular, Menlo, Consolas, monospace`.
  Keep `--nc-font-money` as an alias of the mono token — it names an intent, and
  the comment should now say that the intent and the default coincide.

  **Rewrite the module's doc comment.** It currently says "nigel bundles no
  webfonts, so nothing is added to the embedded binary and nothing is fetched at
  runtime" — half of that is now false and the other half is still true and
  still load bearing. Say what is bundled, why it is bundled rather than
  fetched, and where the files live.

- [ ] **Step 3: Update `apps/app/index.html`'s literal fallback** so the pre-token
  first paint is already mono: `var(--wa-font-family-sans, ui-monospace,
  SFMono-Regular, Menlo, monospace)`. Leave `web/placeholder/index.html` alone —
  it is served when no `web/dist` exists, so there is no font to reference.
- [ ] **Step 4: Verify.** `npm test && npm run typecheck && npm run build` from
  `web/`. Confirm `web/dist/assets/` now contains the hashed woff2 files.
- [ ] **Step 5: Commit** `feat(theme): adopt IBM Plex Mono as the primary face`.

---

## Task 5: Walk the harness at the new face, and only then touch the metrics

This is the task the whole PR exists to get right. Do not skip to the metrics.

- [ ] **Step 1: Walk every state at today's `14px / 1.5`.**
      `npm run build -w @nigel/theme && npm run preview`, then :9090, in both
      light and dark (PR-4b's switcher is in the harness):

  - [ ] `wc-register-table` — the densest thing in the app
  - [ ] `wc-report-table` — section, subtotal and total rows
  - [ ] `wc-manager-table` — the rules columns are the widest
  - [ ] `wc-invoice-table`, `wc-invoice-summary`, `wc-payment-list`
  - [ ] `wc-money` — sign column alignment across a column of figures
  - [ ] `wc-aging-bars` — five bucket labels across the strip
  - [ ] `wc-manager-dialog` + all five forms — `wc-client-form` sets
        `min-width: 20rem`, and buttons grow with their labels
  - [ ] `wc-nav-sidebar` against `--nc-sidebar-width`
  - [ ] `wc-app-shell` header against `--nc-header-height: 48px`
  - [ ] `wc-stat-card`, `wc-count-grid`, `wc-notice-bar`, `wc-empty-state`

- [ ] **Step 2: Walk the real app** at 1280px and at 1024px:

  ```bash
  cd web && npm run build && cargo run -- serve --no-open
  ```

  Register, all nine reports, the three managers, invoices, clients, import,
  reconcile, settings. Look for: horizontal overflow, wrapped table headers,
  clipped buttons, a sidebar label that no longer fits, a dialog wider than the
  viewport.

- [ ] **Step 3: Decide the metrics, in a separate commit.** If step 1 or 2 shows
  crowding, the candidate is `--wa-font-size-base: 13px` and
  `--wa-line-height: 1.45` (boxcraft runs Chivo Mono at 13px/1.35). Change those
  two tokens, re-walk, and keep it as its own commit so it can be reverted
  without reverting the face.
- [ ] **Step 4: The zero.** Check whether the digit zero is distinguishable in
  the money columns. If the family offers a slashed-zero stylistic set, decide
  whether to enable it via `font-feature-settings` on the money surfaces —
  record the decision either way.
- [ ] **Step 5: Verify.** `npm test` from `web/` — contrast and axe must both be
  untouched by any of this.
- [ ] **Step 6: Commit** `fix(theme): tune the base metrics for the mono face`
  (only if step 3 changed anything).

---

## Task 6: The canary preview state

**Files:** modify `web/packages/ui/src/components/wc-register-table.preview.ts`.

- [ ] **Step 1: Add the state** `longest-realistic-row`: a 60-character bank
  description, a long vendor, the longest stock category name
  (`Cost of Goods Sold`, `Taxes & Licenses`, `Repairs & Maintenance` — take the
  real longest from `src/db.rs`'s `BUSINESS_CATEGORIES`), and a six-figure
  amount. This is where a mono first overflows, so it is the state worth
  declaring permanently.
- [ ] **Step 2: The a11y run comes for free** — `describePreviewA11y(preview)`
  is already the last line of `wc-register-table.test.ts` and picks the state
  up. Do not restate it in the test.
- [ ] **Step 3: Verify.** `npm test -w @nigel/ui`, then look at the state at
  :9090 at a narrow window width.
- [ ] **Step 4: Commit** `test(ui): declare the widest realistic register row`.

---

## Task 7: Measure, document, and the offline pass

- [ ] **Step 1: Measure the "after"** — the three numbers AC #7 wants:

  ```bash
  cd web && npm run build && du -sb dist
  cargo build --release && ls -l target/release/nigel
  ls -l web/packages/theme/src/fonts/
  ```

  Record all three deltas against the Step 0 baseline. The binary delta is the
  number the AC is about, and it is not the sum of the font files — measure it,
  do not derive it.

- [ ] **Step 2: The offline pass** (AC #2, proved rather than argued):

  ```bash
  cargo run -- serve --no-open
  ```

  With devtools' network panel open and the browser set to offline after the
  first load, hard-reload and confirm: the app renders in Plex Mono, and every
  request in the panel is same-origin. No entry to any font host, in any state,
  including the unlock gate.

- [ ] **Step 3: Documentation.**
  - `web/README.md` — a "Typefaces" section: family, weights, the verbatim
    `pyftsubset` command from Task 1, the measured file sizes and binary delta,
    where the files live, and the rule that a glyph outside the subset falls
    back per-glyph rather than failing.
  - `CLAUDE.md` — the `@nigel/theme` architecture entry (the theme now ships
    font files, self-hosted and bundled, never fetched), and a Key Design
    Constraints line carrying spec §3.2's decision: **the client-facing invoice
    template keeps a text face**, because the published page has nowhere to get
    the font, a CDN link would be a third-party request on someone else's bill,
    and `pdf.rs` renders through printpdf's built-in faces so the HTML and PDF
    halves of one document would diverge. Without that line, the next person
    will "fix" the inconsistency.
  - Task notes — the two decisions from Step 1 of the prerequisites, and the
    measurements.
- [ ] **Step 4: Commit** `docs: record the bundled typeface and its weight`.

---

## Definition of done

- [ ] `npm run lint && npm run typecheck && npm test && npm run build` green
      from `web/`, in CI's order
- [ ] `cargo build --release` green, and the served binary renders the mono
- [ ] Offline reload makes no font request (Task 7 Step 2, observed)
- [ ] `contrast.test.ts` and every `describePreviewA11y` run unchanged and green
- [ ] The harness and the real app walked at 1280 and 1024, both modes, with no
      overflow (Task 5)
- [ ] Three measurements recorded (fonts, `web/dist`, release binary)
- [ ] The invoice-template decision is written down in `CLAUDE.md`, not only in
      the spec
- [ ] Every AC in TASK-76 has a check next to it in spec §6
