# Task 75 — Light / dark / system mode switcher — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Spec: `docs/superpowers/specs/2026-08-11-task-75-mode-switcher-design.md`.
Read it first — every "why" below lives there.

**Goal:** an explicit three-state choice — light, dark, follow the system — that
survives a reload, tracks the OS live while following it, is reachable by
keyboard, and cannot leak onto paper.

**One PR (PR-4b), six commits.** Depends on **PR-4a (TASK-72) being merged**:
the switcher is a `wa-radio-group` inside a `wc-*` shadow root and renders
unstyled without `controlsCss`, and the print AC cannot be verified while the
print sheet cannot reach the chrome it hides.

## Architecture

The CSS contract already exists in
`web/packages/theme/src/tokens/color.ts`: no class on `<html>` means follow the
system (`@media (prefers-color-scheme: dark) { :root:not(.light-mode) }`),
`.light-mode` opts out, `.dark-mode` forces dark. This task adds the writer — a
dependency-free module in `@nigel/theme` — a `wc-mode-switcher` control in
`@nigel/ui`, one call in `apps/app/src/main.ts`, one slotted element in
`nigel-app`, and a specificity fix so `printCss` outranks both mode selectors.

## Tech stack

lit 3, Web Awesome 3 (`wa-radio` / `wa-radio-group`, already proven under jsdom
by `wc-category-form`), vite 6, vitest 2, TypeScript 5.7, axe-core 4.10.

## Global constraints

- **TDD, always.** Failing test first, watch it fail for the right reason, then
  implement.
- **All web commands run from `web/`**, and CI runs them in this order:

  ```bash
  npm ci
  npm run lint           # eslint across all three packages
  npm run typecheck      # tsc --noEmit across all three packages
  npm test               # vitest across all three packages
  npm run build          # theme -> ui -> app, output to web/dist
  ```

- Per-package while iterating: `npm test -w @nigel/theme`,
  `npm test -w @nigel/ui`, `npm test -w @nigel/app`. Single file:
  `npm test -w @nigel/theme -- __tests__/print.test.ts`.
- **The contrast suite is `packages/theme/__tests__/contrast.test.ts`**, run by
  `npm test -w @nigel/theme`. It selects a token by the *n*th `#rrggbb`
  occurrence in `nigelTheme.cssText`: occurrence 0 is light, occurrence 1 is
  dark. **Do not add a third copy of the dark token block, and do not reorder
  the composition** — either shifts every index and the failure message will
  point nowhere near the cause. Declarations with no hex value (`color-scheme`)
  are safe.
- **The a11y suite is `describePreviewA11y(preview)`** in each component's
  `.test.ts`, run by `npm test -w @nigel/ui`. Adding a preview state adds its
  axe run automatically — never restate the states in the test.
- **axe under jsdom does not evaluate colour contrast.** "Passes axe in both
  modes" is a structural claim; the colour guarantee is `contrast.test.ts`.
- **jsdom's `matchMedia` always answers `matches: false` and never fires
  `change`.** Anything that reads it takes an injectable seam, following
  `initializeAppStore(client, { reload })`.
- **Component-First UI Workflow (MANDATORY)**: component in
  `packages/ui/src/components/`, co-located `.preview.ts` with every visible
  state, `.test.ts` ending in `describePreviewA11y(preview)`, then consumed by
  `apps/app`.
- **`packages/theme` builds before `ui` builds before `app`.** A theme change is
  not visible to the app until `npm run build -w @nigel/theme` has run.
- Conventional Commits.

## Prerequisite check (do this first)

- [ ] **Step 0: confirm PR-4a has merged.**

  ```bash
  rg -n "controlsCss" web/packages/theme/src/index.ts web/packages/ui/src/components/wc-category-form.ts
  rg -n "::part\(" web/packages/theme/src/themes/nigel.ts
  ```

  The first must hit; the second must not. If `controlsCss` does not exist,
  **stop** — building the switcher first means reviewing an unpainted control
  and then re-reviewing it.

- [ ] **Step 1: reproduce the print bug**, because it is AC #5 and it is a
      current failure rather than a hypothetical. On a machine (or an OS setting)
      with dark mode on:

  ```bash
  cd web && npm run build && npm run dev
  ```

  Open a report, print preview. It should come out **dark**, because
  `:root:not(.light-mode)` is (0,2,0) and print's `:root` is (0,1,0).
  Record what you see; if it comes out light, the browser is forcing
  `prefers-color-scheme: light` for print and §2.3's fix is a precaution rather
  than a repair — implement it anyway, and say which it turned out to be.

---

## Task 1: Print wins by specificity

Do this first. It is the smallest change, it is independent of everything else,
and it is the one thing in this task that fixes a bug rather than adding a
feature.

**Files:** modify `web/packages/theme/src/print.ts`,
`web/packages/theme/__tests__/print.test.ts`.

**Interfaces:** none. `printCss` keeps its shape.

- [ ] **Step 1: Write the failing test** in `print.test.ts`:

```ts
it('outranks both mode selectors rather than relying on source order', () => {
  // :root:not(.light-mode) and :root.dark-mode are both (0,2,0). A bare :root
  // is (0,1,0) and loses regardless of where it appears in the sheet.
  expect(printCss.cssText).toMatch(/@media print\s*{\s*:root:root\s*{/);
});
```

  Replace the existing `it('ships inside the composed theme, after the dark
  overrides')` assertion about index order with one that keeps the *order* check
  (still true and still wanted) but stops claiming order is what decides.

- [ ] **Step 2: Implement.** Change the print token block's selector from
  `:root` to `:root:root`, with a short comment saying what the doubling is for
  — the trick is not self-evident and a reader who skips the comment will
  "simplify" it back.
- [ ] **Step 3: Verify.** `npm test -w @nigel/theme`. Then
  `npm run build -w @nigel/theme && npm run dev`, force dark by hand
  (`document.documentElement.classList.add('dark-mode')` in the console) and
  print preview: black on white.
- [ ] **Step 4: Commit** `fix(theme): make the print palette outrank the mode selectors`.

---

## Task 2: `color-mode.ts` — the pure module

**Files:** create `web/packages/theme/src/color-mode.ts` and
`web/packages/theme/__tests__/color-mode.test.ts`; modify
`web/packages/theme/src/index.ts`.

**Interfaces:** produces

```ts
export const COLOR_MODES = ['light', 'dark', 'system'] as const;
export type ColorMode = (typeof COLOR_MODES)[number];
export const COLOR_MODE_STORAGE_KEY = 'nigel.color-mode';
export const LIGHT_CLASS = 'light-mode';
export const DARK_CLASS = 'dark-mode';

export function readMode(storage?: Pick<Storage, 'getItem'>): ColorMode;
export function writeMode(mode: ColorMode, storage?: Pick<Storage, 'setItem'>): void;
export function applyMode(mode: ColorMode, root?: Element): void;
export function resolveMode(mode: ColorMode, mql?: Pick<MediaQueryList, 'matches'>): 'light' | 'dark';
export function initColorMode(options?: { storage?: Storage; root?: Element }): ColorMode;
```

  No Lit import. `@nigel/theme` has shipped CSS only until now; this is the one
  behaviour module, and it belongs here because the class names, the media query
  and the print interaction are all defined in this package.

- [ ] **Step 1: Write the failing tests** (node environment — the theme package's
  `vitest.config.ts` is already `environment: 'node'`, so pass a fake element
  rather than reaching for a DOM):

```ts
describe('readMode', () => {
  it('defaults to system when nothing is stored');
  it('defaults to system for an unrecognised value');
  it('returns a stored light or dark');
  it('returns system when storage throws', () => {
    // Safari in private mode throws on getItem rather than returning null.
    expect(readMode({ getItem() { throw new DOMException('denied'); } })).toBe('system');
  });
});

describe('writeMode', () => {
  it('stores the mode');
  it('does not throw when storage refuses');
});

describe('applyMode', () => {
  it('adds light-mode and removes dark-mode for light');
  it('adds dark-mode and removes light-mode for dark');
  it('removes both for system — the media query does the tracking, not us', () => {
    const root = fakeRoot(['dark-mode']);
    applyMode('system', root);
    expect([...root.classList]).toEqual([]);
  });
  it('is idempotent');
});

describe('resolveMode', () => {
  it('answers the mode itself for light and dark');
  it('answers dark for system when the query matches');
  it('answers light for system with no query available');   // jsdom's default
});
```

- [ ] **Step 2: Implement.** Every storage access inside `try/catch`. `readMode`
  rewrites an unrecognised value to nothing rather than persisting garbage.
  `applyMode` uses `classList.add`/`remove`, never `className =`, because
  nothing else on `<html>` may be clobbered.
- [ ] **Step 3: Export** from `src/index.ts`.
- [ ] **Step 4: Verify.** `npm test -w @nigel/theme && npm run typecheck` from
  `web/`.
- [ ] **Step 5: Commit** `feat(theme): add the color-mode contract and applier`.

---

## Task 3: `color-scheme` follows the class

**Files:** modify `web/packages/theme/src/tokens/color.ts`; modify
`web/packages/theme/__tests__/nigel-theme.test.ts`.

- [ ] **Step 1: Write the failing test:**

```ts
it('pins color-scheme when a mode is forced, so native widgets follow', () => {
  expect(nigelTheme.cssText).toMatch(/:root\.light-mode\s*{[^}]*color-scheme:\s*light/);
  expect(nigelTheme.cssText).toMatch(/:root\.dark-mode\s*{[^}]*color-scheme:\s*dark/);
});
```

- [ ] **Step 2: Implement** by adding `color-scheme: dark;` inside the existing
  `:root.dark-mode` block and a new two-line `:root.light-mode { color-scheme:
  light; }` rule. **Do not** duplicate `darkTokens` — see Global constraints.
- [ ] **Step 3: Verify.** `npm test -w @nigel/theme`, and specifically confirm
  `contrast.test.ts` still passes (it must; the additions carry no hex values).
- [ ] **Step 4: Commit** `feat(theme): pin color-scheme to the chosen mode`.

---

## Task 4: `wc-mode-switcher`

**Files:** create `web/packages/ui/src/components/wc-mode-switcher.ts`,
`wc-mode-switcher.preview.ts`, `wc-mode-switcher.test.ts`; modify
`web/packages/ui/src/components/index.ts`.

**Interfaces:** produces

```ts
@customElement('wc-mode-switcher')
class WcModeSwitcher extends LitElement {
  mode: ColorMode = 'system';          // property, fully controlled
  resolved: 'light' | 'dark' = 'light';// what "system" currently means
}
// event: 'nc-color-mode-change', detail { mode: ColorMode }, bubbles + composed
```

- [ ] **Step 1: Write the preview first** — the workflow's step 2, and the thing
  that makes the axe run exist. States: `system`, `system-dark`, `light`,
  `dark`, `in-a-header` (slotted into a `wc-app-shell`).
- [ ] **Step 2: Write the failing tests:**

```ts
it('renders three radios labelled Light, Dark and System');
it('marks the current mode as checked');
it('emits nc-color-mode-change with the chosen mode');
it('never writes storage itself — the app owns persistence', () => {
  // the component is controlled; a component that persisted would make the
  // preview harness change the real app's mode.
});
it('says what System currently resolves to');
it('is hidden on paper', () => {
  expect(styleText(WcModeSwitcher)).toMatch(/@media print[^}]*:host[^}]*display:\s*none/s);
});

describePreviewA11y(preview);          // last line, as always
```

- [ ] **Step 3: Implement.** `wa-radio-group` (label "Appearance",
  `orientation="horizontal"`) with three `wa-radio`s. Adopt `controlsCss` —
  PR-4a's guard test will fail the build otherwise, which is the intended
  behaviour. Add the `@media print { :host { display: none } }` block per
  PR-4a's rule that a component hides its own chrome. Labels are words, not
  icons alone.
- [ ] **Step 4: Verify.** `npm test -w @nigel/ui` — zero axe violations across
  all five states — then `npm run preview` and drive it with the keyboard only:
  Tab to the group, arrows between options, no mouse.
- [ ] **Step 5: Commit** `feat(ui): add the appearance mode switcher`.

---

## Task 5: Wire it into the app

**Files:** modify `web/apps/app/src/main.ts`,
`web/apps/app/src/components/nigel-app.ts`,
`web/apps/app/src/components/nigel-app.test.ts`,
`web/packages/ui/preview/main.ts`.

- [ ] **Step 1: Write the failing tests** in `nigel-app.test.ts`:

```ts
it('renders the switcher in the shell header');
it('applies the stored mode at boot', () => {
  // seed localStorage with 'dark', mount, expect documentElement to carry dark-mode
});
it('persists a change and applies it', () => {
  // dispatch nc-color-mode-change from the switcher, expect storage + class
});
it('removes both classes when system is chosen');
```

- [ ] **Step 2: Implement.**
  - `main.ts` gains one line: `initColorMode()` between the CSS import and the
    component imports. It stays a bootstrap, not logic.
  - `nigel-app` renders `<wc-mode-switcher slot="header-actions" …>` inside
    `wc-app-shell`, holds the mode in a `@state`, and handles
    `nc-color-mode-change` by calling `writeMode` then `applyMode`. The
    component stays controlled; the container owns persistence.
  - The switcher is **not** rendered on the unlock gate — the gate replaces the
    shell, so there is no header. The stored mode is still applied, because
    `initColorMode()` ran before anything rendered.
  - `preview/main.ts` also calls `initColorMode()` so the harness can be
    reviewed in both modes.
- [ ] **Step 3: Resolved-mode hint.** Wire `resolveMode` with a real
  `matchMedia` in the app, injected in tests. If the listener is more trouble
  than the hint is worth, spec §6 question 5 is the escape hatch — take it
  explicitly rather than shipping a hint that lies.
- [ ] **Step 4: Verify.** `npm test -w @nigel/app`, then `npm run lint &&
  npm run typecheck` from `web/`.
- [ ] **Step 5: Commit** `feat(web): persist and apply the appearance mode`.

---

## Task 6: First paint

**Scope gate:** spec §6 question 1. If Sam prefers to accept the flash, skip
this task and record the decision.

**Files:** modify `web/apps/app/index.html`; create
`web/apps/app/src/__tests__/color-mode-bootstrap.test.ts`.

- [ ] **Step 1: Write the failing test** (node environment — it reads files):

```ts
it('the inline bootstrap uses the same key and classes as the module', () => {
  const html = readFileSync(resolve(appRoot, 'index.html'), 'utf8');
  expect(html).toContain(COLOR_MODE_STORAGE_KEY);
  expect(html).toContain(LIGHT_CLASS);
  expect(html).toContain(DARK_CLASS);
});
```

  The duplication is the cost of avoiding the flash; this test is what stops it
  rotting.

- [ ] **Step 2: Implement** the six-line blocking `<script>` from spec §2.8, in
  `<head>`, before the module script. Wrapped in `try/catch`, adding the class
  only for an explicit `light` or `dark`.
- [ ] **Step 3: Verify** by eye: set dark mode with a light OS, hard-reload, and
  watch for a white flash. Then `npm test -w @nigel/app`.
- [ ] **Step 4: Commit** `fix(web): apply the stored appearance mode before first paint`.

---

## Task 7: Documentation and the manual pass

- [ ] `CLAUDE.md`: a "Web UI (SPA)" line for the switcher — three states,
  `localStorage` key `nigel.color-mode`, and why not `settings.json` (every
  `/api/settings/*` route is behind the locked guard, so the unlock screen could
  not read a server-stored preference). A Key Design Constraints line for the
  print specificity fix, so the next person does not re-break it.
- [ ] `web/README.md`: the print checklist item becomes "black on white in
  **all three** modes"; the theme section gains the mode contract (no class
  means system, and system is tracked by CSS rather than by a listener).
- [ ] **Manual pass:**

  ```bash
  cd web && npm run build && cargo run -- serve --no-open
  ```

  - [ ] Switch to Dark, reload — still dark
  - [ ] Switch to Light on a dark-OS machine, reload — still light
  - [ ] Switch to System, change the OS setting with the app open — the app
        follows with no reload (AC #3)
  - [ ] Keyboard only: Tab to the switcher, arrow through all three
  - [ ] Print preview in each of the three modes — black on white every time
  - [ ] Lock an encrypted database and reload — the gate honours the stored mode
  - [ ] Walk two or three screens in dark mode looking for anything that reads
        badly; dark has never been reviewed with the part overrides applied

- [ ] **Commit** `docs: record the appearance mode contract`.

---

## Definition of done

- [ ] `npm run lint && npm run typecheck && npm test && npm run build` green
      from `web/`, in CI's order
- [ ] `contrast.test.ts` passes unchanged — no palette value moved
- [ ] `describePreviewA11y` green over all five switcher states
- [ ] Print preview verified black-on-white in light, dark and system, on a
      dark-OS machine
- [ ] Every AC in TASK-75 has a check next to it in spec §3
