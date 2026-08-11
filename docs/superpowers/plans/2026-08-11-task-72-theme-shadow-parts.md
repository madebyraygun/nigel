# Task 72 — Theme part overrides reach shadow roots — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Spec: `docs/superpowers/specs/2026-08-11-task-72-theme-shadow-parts-design.md`.
Read it first — every "why" below lives there.

**Goal:** the `wa-*` treatment `@nigel/theme` has always shipped actually
applies. Dialogs paint an opaque panel, form fields carry field chrome, buttons
get the brand gradient, and a new component cannot silently opt out.

**One PR (PR-4a), six commits.** Every commit leaves `web/` green.

## Architecture

`@nigel/theme` composes Lit `css` modules into `nigelTheme`, which
`scripts/build-css.js` emits as `dist/css/nigel.css` — the **document-level**
sheet `apps/app/src/main.ts` and `packages/ui/preview/main.ts` import. Every
`wc-*` and `nigel-*` element is a `LitElement` with an open shadow root, so the
only thing the document sheet delivers into a component is inherited custom
properties. `::part()` rules must therefore live in the shadow root that *hosts*
the primitive.

This plan splits the sheet by audience: tokens and print recolouring stay at
document level; the `wa-*` part overrides become `controlsCss`, adopted by the
twenty-three files that render a Web Awesome primitive, with a source-scan guard
test in each package.

## Tech stack

lit 3, Web Awesome 3 (cherry-picked imports, never the autoloader, never the WA
stylesheet), vite 6, vitest 2, TypeScript 5.7, axe-core 4.10, Node 20.19+ (22 in
CI). npm workspaces, no turbo.

## Global constraints

- **TDD, always.** Write the failing test, watch it fail *for the right reason*,
  implement, watch it pass. A step that skips the failure is not done.
- **All web commands run from `web/`.** The suites, exactly as CI runs them and
  in CI's order:

  ```bash
  npm ci                 # committed lockfile
  npm run lint           # eslint across all three packages
  npm run typecheck      # tsc --noEmit across all three packages
  npm test               # vitest across all three packages
  npm run build          # theme -> ui -> app, output to web/dist
  ```

- Per-package while iterating: `npm test -w @nigel/theme`,
  `npm test -w @nigel/ui`, `npm test -w @nigel/app`. A single file:
  `npm test -w @nigel/theme -- __tests__/print.test.ts`.
- **The a11y suite is not separate.** `describePreviewA11y(preview)` runs
  `axe.run()` over every state a `.preview.ts` declares and is part of
  `npm test -w @nigel/ui`.
- **The contrast suite is not separate either.**
  `packages/theme/__tests__/contrast.test.ts` runs inside
  `npm test -w @nigel/theme`. This task changes no colour value; if contrast
  fails, something unintended moved.
- **Component-First UI Workflow (MANDATORY).** Component in
  `packages/ui/src/components/`, co-located `.preview.ts` covering every visible
  state, `.test.ts` ending in `describePreviewA11y(preview)`, and only then
  consumed by `apps/app`. No bespoke components in the app.
- **Order matters in `@nigel/theme`.** `nigelTheme` composes light tokens →
  dark overrides → print. Do not reorder, and do not add a second copy of the
  dark token block: `contrast.test.ts` selects tokens by the *n*th `#rrggbb`
  occurrence and a third copy shifts every index.
- **`packages/theme` builds before `packages/ui` builds before `apps/app`.**
  `build-css.js` consumes `dist/themes/nigel.js`, so a theme change is not
  visible to the app until `npm run build -w @nigel/theme` has run.
- **axe under jsdom does not evaluate colour contrast.** A green
  `describePreviewA11y` does not prove a panel is opaque. The proof is the
  rule-presence assertions plus eyes on the harness (`npm run preview`, :9090).
- Conventional Commits (`fix:`, `feat:`, `refactor:`, `test:`, `docs:`).

## Prerequisite check (do this first)

- [ ] **Step 0: confirm the diagnosis in a real browser.** The spec predicts
      that *all* of `global.ts` and the chrome half of `print.ts` are dead, not
      just the dialog and field rules. Before deleting or moving anything:

  ```bash
  cd web && npm run build && npm run dev      # :5173
  cargo run -- serve --no-open                # other terminal, for the token URL
  ```

  In devtools, on the clients screen with the Edit dialog open:
  - [ ] select the `wa-dialog` inside `wc-manager-dialog` and confirm no
        `::part(body)` rule from `nigel.css` appears in its computed styles;
  - [ ] confirm a primary `wa-button` shows no `--nc-grad-brand` background;
  - [ ] open print preview and confirm the sidebar, header and export buttons
        are **still on the page** (the spec's print prediction);
  - [ ] on a machine set to a dark OS theme, confirm print preview comes out
        dark (that is TASK-75's AC #5 and is fixed there, not here — record it).

  **If any prediction is wrong, stop and re-diagnose** before writing code. The
  fix is shaped by which rules are actually dead.

---

## Task 1: Prove the mechanism with a failing test

**Files:** modify `web/packages/ui/src/components/wc-manager-dialog.test.ts` and
`web/packages/ui/src/components/wc-client-form.test.ts` — co-located, as every
component test in this library is.

**Interfaces:** none yet. This task only pins the property.

- [ ] **Step 1: Write the failing tests.** In `wc-manager-dialog.test.ts`:

```ts
import { controlsCss } from '@nigel/theme';   // does not exist yet — fails to import

function styleText(ctor: typeof WcManagerDialog): string {
  return [ctor.styles].flat(Infinity).map((s) => String((s as CSSResult).cssText)).join('\n');
}

it('adopts the wa-* part overrides into its own shadow root', () => {
  const text = styleText(WcManagerDialog);
  expect(text).toContain(controlsCss.cssText);
  // The rule that was never matching: the dialog panel's own surface.
  expect(text).toMatch(/wa-dialog::part\(body\)/);
});
```

  And in `wc-client-form.test.ts`, the same shape asserting
  `wa-input::part(base)`.

- [ ] **Step 2: Watch them fail** — `npm test -w @nigel/ui`. The failure should
  be a missing export from `@nigel/theme`, not a missing rule. That is the right
  reason.
- [ ] **Step 3:** no implementation in this task. Leave the tests red and move
  to Task 2; they go green in Task 3.

> Note for the executor: this is the one place the plan deliberately leaves a
> red suite between tasks. If that breaks your workflow, fold Tasks 1–3 into one
> commit — but write the assertions before the export exists either way.

---

## Task 2: Split `global.ts` into `controlsCss`

**Files:** rename `web/packages/theme/src/global.ts` →
`web/packages/theme/src/controls.ts`; modify `src/index.ts`,
`src/themes/nigel.ts`; create `__tests__/controls.test.ts`; modify
`__tests__/nigel-theme.test.ts`.

**Interfaces:** produces `export const controlsCss: CSSResult` from
`@nigel/theme`; removes `globalCss`; `nigelTheme` no longer carries any
`::part(` rule.

- [ ] **Step 1: Write the failing tests** in `packages/theme/__tests__/controls.test.ts`:

```ts
describe('controlsCss', () => {
  it.each([
    'wa-dialog::part(header)', 'wa-dialog::part(body)', 'wa-dialog::part(footer)',
    'wa-input::part(base)', 'wa-select::part(base)', 'wa-textarea::part(base)',
    'wa-button[variant=\'brand\']::part(base)',
    '::part(form-control-label)', ':focus-visible',
  ])('carries %s', (rule) => expect(controlsCss.cssText).toContain(rule));

  it('reads only tokens, never a literal brand value', () => {
    expect(controlsCss.cssText).not.toMatch(/#[0-9a-f]{6}/i);
  });
});

describe('nigelTheme', () => {
  it('ships no ::part() rule — a document sheet cannot reach one', () => {
    expect(nigelTheme.cssText).not.toContain('::part(');
  });
});
```

  The last assertion is the one that encodes AC #5.

  **Caveat for the executor:** `controlsCss` currently contains
  `color: #2b2b33` on the brand button (`global.ts:15`). Either keep the literal
  and drop that assertion, or replace it with `var(--wa-color-text)` — which is
  the same value in light mode and the *correct* value in dark, where a
  hardcoded near-black on a pastel gradient is what the token exists to avoid.
  Recommended: replace it, and say so in the commit message.

- [ ] **Step 2: Implement.** `git mv src/global.ts src/controls.ts`, rename the
  export, rewrite the doc comment (the current one states the mechanism
  backwards — say that `::part()` reaches one boundary down *from the tree the
  rule is in*, which is why this sheet is adopted by components rather than
  loaded by the document). Remove `${globalCss}` from `nigelTheme`. Export
  `controlsCss` from `src/index.ts` and delete the `globalCss` export.
- [ ] **Step 3: Move the three orphaned assertions** out of
  `nigel-theme.test.ts` ("carries the global wa-* shadow-part overrides", and
  the `parts` arm of the ordering test) into `controls.test.ts`; narrow the
  ordering test to light-before-dark-before-print.
- [ ] **Step 4: Verify.** `npm test -w @nigel/theme`, then
  `npm run lint && npm run typecheck` from `web/`.
- [ ] **Step 5: Commit** `refactor(theme): move the wa-* part overrides into an adoptable controlsCss`.

---

## Task 3: Adopt `controlsCss` in all twenty-three primitive hosts

**Files:** modify the twenty `packages/ui/src/components/wc-*.ts` and three
`apps/app/src/screens/*.ts` listed in spec §1.

**Interfaces:** no public API change. Each file gains an import and one array
element.

- [ ] **Step 1: Tasks 1's tests are already failing.** Extend them: add the same
  two-line assertion to `wc-confirm.test.ts`, `wc-send-dialog.test.ts`,
  `wc-account-form.test.ts`, `wc-category-form.test.ts`, `wc-rule-form.test.ts`
  — the four manager dialogs AC #4 names, plus the two other dialog hosts.
- [ ] **Step 2: Implement** in every file:

```ts
import { controlsCss } from '@nigel/theme';

static styles = [controlsCss, css`  …unchanged…  `];
```

  Order is load bearing: `controlsCss` first, so a component's own rules can
  still override the shared treatment.

  For `apps/app/src/screens/{dashboard,register,settings}.ts` the same edit
  applies. This is not "styling logic in the app" — it is adopting the shared
  sheet, which is what the guard test in Task 4 requires.

- [ ] **Step 3: Verify.** `npm test -w @nigel/ui && npm test -w @nigel/app`,
  then `npm run typecheck` from `web/`. Every previously-red assertion is green.
- [ ] **Step 4: Look at it.** `npm run build -w @nigel/theme && npm run preview`,
  then walk :9090: the manager dialog, all five forms, the send dialog, the
  confirm dialog, the unlock card, the register toolbar. Buttons should now
  carry the brand gradient for the first time — confirm that is wanted before
  going further (spec §7 question 4).
- [ ] **Step 5: Commit** `fix(ui): adopt the theme's wa-* part overrides into every primitive host`.

---

## Task 4: The guard test, in both packages

**Files:** create `web/packages/ui/src/__tests__/controls-adoption.test.ts` and
`web/apps/app/src/__tests__/controls-adoption.test.ts`.

**Interfaces:** a build failure for any file that renders a `wa-*` primitive
without adopting the sheet.

- [ ] **Step 1: Write the test** — modelled on
  `apps/app/src/__tests__/api-seam.test.ts` and
  `apps/app/src/__tests__/dependency-manifest.test.ts`, both of which walk the
  source tree in a node environment and assert on file text.

```ts
// For every .ts under src that is not *.test.ts / *.preview.ts:
//   if it contains '@awesome.me/webawesome/dist/components/'
//   then it must contain "from '@nigel/theme'" and 'controlsCss'
const offenders = files.filter(rendersPrimitive).filter((f) => !adoptsControls(f));
expect(offenders, `these render a wa-* primitive without adopting controlsCss:
${offenders.join('\n')}`).toEqual([]);
```

  In the app's `vite.config.ts` the `test.environmentMatchGlobs` list already
  sends `**/__tests__/screen-freshness.test.ts` to jsdom and leaves the rest of
  `__tests__` on node — this file wants node, so no config change.

  In `packages/ui`, `vitest.config.ts` sets `environment: 'jsdom'` globally;
  that is fine for a file-reading test, just slower. Do not add a per-file
  environment override for it.

- [ ] **Step 2: Prove the guard bites.** Temporarily delete the `controlsCss`
  entry from `wc-client-form.ts`, run the suite, confirm the failure names that
  file, then restore it. A guard test that has never failed is not a guard.
- [ ] **Step 3: Verify.** `npm test` from `web/`.
- [ ] **Step 4: Commit** `test(web): fail the build when a wa-* host does not adopt controlsCss`.

---

## Task 5: Move the print chrome into the components that own it

**Scope gate:** spec §7 question 1. If Sam has said this belongs in a
separate PR, skip this task and note it — nothing later depends on it.

**Files:** modify `web/packages/theme/src/print.ts`,
`web/packages/theme/src/controls.ts`, `web/packages/theme/__tests__/print.test.ts`;
modify `wc-app-shell.ts`, `wc-nav-sidebar.ts`, `wc-toast.ts`,
`wc-export-links.ts`, `wc-period-nav.ts`, `wc-register-toolbar.ts`.

**Interfaces:** none public. Print stops depending on document reach.

- [ ] **Step 1: Write the failing tests.**

  In `packages/theme/__tests__/print.test.ts`, replace the six assertions that
  merely check for the presence of `wc-app-shell::part(sidebar)` and the tag
  list — they assert text that cannot match anything — with:

```ts
it('leaves component chrome to the components', () => {
  expect(printCss.cssText).not.toContain('wc-app-shell::part(');
  expect(printCss.cssText).not.toContain('wc-nav-sidebar');
});

it('keeps the token repaint, which is the part that reaches shadow roots', () => {
  expect(printCss.cssText).toMatch(/--wa-color-bg:\s*#ffffff/);
});
```

  In `packages/ui/src/components/wc-app-shell.test.ts` and each of the five
  self-hiding components, assert the component's own flattened style text
  carries `@media print` and a `display: none` for the right selector.

- [ ] **Step 2: Implement.**
  - `wc-app-shell` gains `@media print { header, .banner { display: none }
    ::slotted([slot='sidebar']) { display: none } .content { display: block;
    padding: 0; overflow: visible } }`. Keep the exposed parts — they are cheap
    and documented.
  - `wc-nav-sidebar`, `wc-toast`, `wc-export-links`, `wc-period-nav`,
    `wc-register-toolbar` each gain `@media print { :host { display: none } }`.
  - `controls.ts` gains its own `@media print` block: `wa-button, wa-select {
    display: none }`, `[data-print='hide'] { display: none }`, `thead { display:
    table-header-group }`, `tr { break-inside: avoid }`, `a { text-decoration:
    none }` — so content inside a shadow root gets the same treatment the
    document sheet gives content outside one.
  - `print.ts` keeps the `@page` margin, the `:root` token repaint, the
    `html, body` rule, and the document-level `thead`/`tr`/`a`/`[data-print]`
    rules; it loses the `wc-*` selectors and the `wa-button, wa-select` line.
- [ ] **Step 3: Verify.** `npm test` from `web/`, then a **browser print
  preview**: `npm run build && cargo run -- serve --no-open`, open a report, and
  walk `web/README.md`'s print checklist. No sidebar, no header, no banner, no
  period control, no export buttons, headings repeating across a page break.
- [ ] **Step 4: Commit** `fix(ui): hide print chrome from inside the components that own it`.

---

## Task 6: The preview state AC #6 asks for

**Files:** modify `web/packages/ui/src/components/wc-manager-dialog.preview.ts`
and `wc-client-form.preview.ts`.

- [ ] **Step 1: Add the state.** `wc-manager-dialog.preview.ts` gains
  `over-a-populated-list`: a `wc-manager-table` with six rows of realistic data,
  with the dialog `open` above it and a form slotted in. That is the bug as
  reported — the clients table showing straight through the panel — and the
  state that makes the fix visible in the harness.
- [ ] **Step 2: Add the field state** if `wc-client-form.preview.ts` does not
  already have one with all four fields populated and none focused. The reported
  symptom was that only the focused textarea looked painted, so the unfocused
  state is the one worth declaring.
- [ ] **Step 3: The a11y test comes for free.** `describePreviewA11y(preview)`
  is already the last line of both test files and picks the new states up
  automatically — **do not restate the states inside the test**.
- [ ] **Step 4: Verify.** `npm test -w @nigel/ui` (zero axe violations across
  every declared state), then `npm run preview` and look at the new state at
  :9090 in a browser. The table must not be legible through the panel.
- [ ] **Step 5: Commit** `test(ui): cover a dialog open over a populated list`.

---

## Task 7: Documentation and the manual pass

- [ ] `CLAUDE.md`:
  - the `@nigel/theme` sentence in "Web UI (SPA)" — the token sheet is the
    document sheet; the `wa-*` treatment ships as `controlsCss` adopted by the
    components that render primitives, because a document-level `::part()` rule
    cannot reach them;
  - "Component selection" — a `wc-*` wrapper that renders a `wa-*` primitive
    adopts `controlsCss`, and a guard test enforces it;
  - "SPA exports and printing" — correct the claim that shell chrome is hidden
    "through the parts `wc-app-shell` exposes"; it is hidden by the components,
    and only the token repaint rides the document sheet (if Task 5 shipped).
- [ ] `web/README.md`: the same two corrections, in the component-first workflow
  section and the Printing section.
- [ ] **Manual pass** against demo data:

  ```bash
  cd web && npm run build
  cargo run -- serve --no-open        # open the printed token URL
  ```

  - [ ] Clients → Edit: opaque panel, all four fields with visible chrome
  - [ ] Accounts, Categories, Rules: the same dialog, the same result (AC #4)
  - [ ] A destructive Delete (`confirmDialog`) — `wc-confirm` mounts into
        `document.body`, so it is the one dialog on a different path; check it
  - [ ] Send dialog on an invoice detail
  - [ ] Unlock screen on an encrypted database (`wc-unlock-card`)
  - [ ] Settings screen (`wa-input`, `wa-switch` in an app screen, not a `wc-*`)
  - [ ] Focus ring visible on every control, in both light and dark OS settings
  - [ ] Print preview per Task 5

- [ ] **Commit** `docs: correct how the theme reaches wa-* primitives`.

---

## Definition of done

- [ ] `npm run lint && npm run typecheck && npm test && npm run build` green
      from `web/`, in that order (CI's order)
- [ ] `cargo build --release` after the web build, and the app served from the
      binary looks the same as it did under `npm run dev`
- [ ] The guard test fails when the adoption line is removed (proved in Task 4)
- [ ] `nigelTheme.cssText` contains no `::part(`
- [ ] Every AC in TASK-72 has a check next to it in spec §4, and the manual pass
      above has been walked rather than assumed
