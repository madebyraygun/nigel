# Task 75 — A three-state light / dark / system mode switcher

Stream 4 (Web theme), PR-4b. Depends on PR-4a (TASK-72): the switcher renders a
`wa-radio-group` inside a `wc-*` shadow root, which is unpainted until the part
overrides reach shadow roots, and "print is unaffected by the selected mode"
cannot be checked while the print sheet cannot reach the chrome it hides.

**Objective:** an explicit, persistent, keyboard-reachable choice between light,
dark and follow-the-system — with print immune to all three.

---

## 1. Where the codebase actually is

**Both palettes already exist and the class hooks are already written.**
`web/packages/theme/src/tokens/color.ts` defines the light tokens at `:root` and
a `darkTokens` block emitted twice by `colorDarkCss`:

```css
@media (prefers-color-scheme: dark) { :root:not(.light-mode) { …dark… } }
:root.dark-mode { …dark… }
```

So the CSS contract for a three-state switcher is already in place: no class
means "follow the system", `.light-mode` opts out of it, `.dark-mode` forces
dark. `__tests__/nigel-theme.test.ts` even asserts all three exist.

**Nothing sets either class.** `rg "localStorage|matchMedia|dark-mode"` across
`web/` finds the token file, the theme test, and the placeholder page's own
`@media (prefers-color-scheme: dark)` block. There is no writer.

**`main.ts` is three import lines** (`web/apps/app/src/main.ts`) and the app's
`index.html` paints `html, body` from `var(--wa-color-bg, #fdfcfb)` before any
module runs. The preview harness has the same shape
(`web/packages/ui/preview/main.ts`, `preview/index.html`).

**The app shell already has the slot.** `wc-app-shell` exposes
`<slot name="header-actions">` (`web/packages/ui/src/components/wc-app-shell.ts`),
covered by a preview state and a slot test, and nothing fills it today.

**`wa-radio` / `wa-radio-group` already work under jsdom.**
`wc-category-form.ts` uses both, with a passing `describePreviewA11y` run — so a
radio-based control needs no new test-setup shims.

**Print does not currently win, and that is not a latent bug.** `printCss`
redefines the tokens under a bare `:root` (specificity 0,1,0) inside
`@media print`. The two dark selectors are `:root:not(.light-mode)` and
`:root.dark-mode`, both (0,2,0). Specificity beats source order, so:

- on a machine whose OS prefers dark, **printing today produces the dark
  palette**, because `:root:not(.light-mode)` outranks print's `:root`;
- the moment `.dark-mode` exists, an explicit dark choice would do the same.

`__tests__/print.test.ts` asserts the print block comes *after* the dark block
("last wins in a flat cascade"), which is only true when specificity ties. It
does not. `web/README.md`'s manual print checklist says "black on white in both
light and dark mode" — that item has been failing.

This is AC #5's whole content, and it is a real fix rather than a precaution.

---

## 2. Design decisions

### 2.1 The preference lives in `localStorage`, not `settings.json`

The task suspects this; three things settle it.

1. **The gate.** Every `/api/settings/*` route is behind the locked guard
   (CLAUDE.md is explicit, and deliberately so). On an encrypted database the
   unlock screen renders *before* anything can be read from the server, so a
   server-stored preference could not be honoured on the first screen the user
   sees. A theme that snaps to the chosen mode only after the password is typed
   is worse than no switcher.
2. **`settings.json` is shared with the CLI and the TUI**, which have no notion
   of a browser colour scheme; adding a key means a Rust struct field, a route,
   `docs/api.md`, and a settings surface in two other front ends that must
   ignore it.
3. **The preference is per-browser by nature.** A laptop and a desktop pointed
   at the same books can legitimately disagree, and the multiuser plan
   (TASK-32) would otherwise have to decide whose mode wins.

Key: `nigel.color-mode`. Values: `light` | `dark` | `system`. An unreadable or
unrecognised value is treated as `system` and rewritten — never thrown; a
private-mode browser that refuses `localStorage` must degrade to system, not to
a blank screen. Every read and write is wrapped, because `localStorage` throws
rather than returning null when storage is disabled.

### 2.2 `system` writes no class, and that is what makes AC #3 free

In `system` mode the applier removes both classes and lets
`@media (prefers-color-scheme: dark)` do the work. The browser re-evaluates a
media query the moment the OS setting changes, with no listener and no reload —
AC #3 is satisfied by not writing JavaScript.

A `matchMedia` listener is still wanted for one thing only: the control shows
which mode is *resolved* while `system` is selected ("System — currently dark").
That listener is cosmetic; if it fails, the colours are still right. It is
injectable so jsdom can drive it (jsdom's `matchMedia` always answers
`matches: false` and never fires `change`), following the
`initializeAppStore(client, { reload })` precedent.

The alternative — always writing an explicit class, resolving `system` in JS —
was considered and rejected: it makes the CSS media query dead code, puts the
OS-tracking behaviour behind a listener that can fail, and means a page with
JavaScript broken renders light on a dark desktop.

### 2.3 Print wins by specificity, not by order

```css
@media print {
  /* :root:root matches the same element with the specificity the mode
     selectors carry, so source order decides and print is last. */
  :root:root { …the existing print tokens, unchanged… }
}
```

`:root:root` is (0,2,0) — a tie with both `:root:not(.light-mode)` and
`:root.dark-mode`, and `printCss` is composed last, so it wins in both. A
three-selector list (`:root, :root.light-mode, :root.dark-mode`) was the first
idea and is wrong: for an unclassed root in `system` mode on a dark OS, only the
bare `:root` arm matches, and it still loses to `:root:not(.light-mode)`.

`print.test.ts` gains an assertion that the print token block's selector carries
at least the mode selectors' specificity, replacing the ordering assertion that
is currently doing a job it cannot do.

### 2.4 `color-scheme` follows the choice

`colorCss` sets `color-scheme: light dark` at `:root`, which tells the UA to
render native widgets (scrollbars, the `wa-input type="month"` picker on the
reconcile screen, form control defaults) for whichever scheme is active. With an
explicit override, that must be pinned:

```css
:root.light-mode { color-scheme: light; }
:root.dark-mode  { color-scheme: dark; }
```

These declarations carry no hex values, so they cannot disturb
`__tests__/contrast.test.ts`, which selects tokens by the *n*th `#rrggbb`
occurrence. **Nothing in this task may add a third copy of `darkTokens`** — that
would shift every occurrence index and break the contrast suite in a way whose
error message points nowhere near the cause. Noted here because it is the one
edit in this area that looks harmless and is not.

### 2.5 Where the code lives

| Piece | Home | Why |
|---|---|---|
| `COLOR_MODES`, `ColorMode`, `readMode`, `writeMode`, `applyMode`, `resolveMode`, `initColorMode` | `web/packages/theme/src/color-mode.ts` | the class names it toggles are defined in the same package; no Lit element involved, so it fits a token package |
| `wc-mode-switcher` | `web/packages/ui/src/components/` | mandatory component-first workflow |
| The wiring | `web/apps/app/src/main.ts` (one call) and `nigel-app.ts` (one slotted element) | composition only, per the app's charter |
| The same call | `web/packages/ui/preview/main.ts` | so the harness can be reviewed in both modes |

`@nigel/theme` currently ships CSS only. Adding one behaviour module is a real
change to its character, and the alternative (a `@nigel/ui` module) was
considered. The class names, the media query and the print interaction all live
in the theme package; splitting the writer from the contract it writes against
is how the two drift. The module imports nothing — not even Lit.

### 2.6 The control

`wc-mode-switcher`, a `wa-radio-group` with three `wa-radio`s, labelled
"Appearance", rendered as a compact segmented control in the shell header.

- Radio group, not a toggle: the task asks for three states, and a
  radiogroup is the role that means "pick one of these", with arrow-key
  navigation and a group label for free. Web Awesome's radio group already
  passes axe under jsdom in `wc-category-form`.
- Not a `wa-select`: a select for three mutually exclusive options costs a
  popover and hides two of the three choices behind a click.
- Not hand-built buttons: a hand-built `role="radiogroup"` means re-implementing
  roving tabindex, which `wc-register-table` shows is real work.
- Labels are words, not icons alone — "Light", "Dark", "System". Icons may be
  added beside them; an icon-only control needs a visually-hidden label and
  fails the same WCAG 1.4.1 reasoning `wc-money` and `wc-invoice-status` already
  follow in this codebase.
- It hides itself on paper: `@media print { :host { display: none } }`, in the
  component, per PR-4a's rule that a component hides its own chrome.

Properties: `mode: ColorMode`, `resolved: 'light' | 'dark'` (what `system`
currently means, for the hint). Event: `nc-color-mode-change` with
`{ mode }`. Fully controlled, like every other form component in the library —
the component never writes storage; `nigel-app` does.

### 2.7 Placement: the shell header, and nowhere else

`nigel-app` slots it into `wc-app-shell`'s `header-actions`, which exists
unused. Reachable from every screen, one tab stop from the top of the page, and
it does not spend a nav slot.

Two deliberate omissions:

- **Not on the settings screen.** A second control means two places to keep in
  sync and a decision about which one wins. If Sam wants it in settings
  instead, it is a one-line move; both is the option to avoid.
- **Not on the unlock gate.** The gate replaces the shell entirely (that is the
  boot design — with no shell, nothing can fetch before the password arrives),
  so there is no header to put it in. The gate still *honours* the stored mode,
  because the class is applied at boot before anything renders.

### 2.8 First paint

`main.ts` is a module script, so it runs after HTML parsing. `index.html` paints
`html, body` from the tokens immediately. For a user whose stored choice differs
from their OS setting, that is a visible flash of the wrong palette on every
load.

Recommendation: a **six-line blocking inline script in `<head>`** of
`web/apps/app/index.html`, before the stylesheet-bearing module, that reads the
key and sets the class:

```html
<script>
  try {
    var m = localStorage.getItem('nigel.color-mode');
    if (m === 'light' || m === 'dark') {
      document.documentElement.classList.add(m + '-mode');
    }
  } catch (e) { /* storage disabled: fall through to the system preference */ }
</script>
```

It duplicates the storage key and the class-name convention in HTML, which is
the cost. A node-environment test pins the duplication: it reads `index.html`
and asserts the key string and both class names match the constants exported by
`color-mode.ts`. `initColorMode()` in `main.ts` then does the same work
idempotently and takes over.

If Sam would rather not have a script tag in the shell, the fallback is to
accept the flash — it is one frame on a localhost app, and only for users who
override their OS. Flagged in §6.

---

## 3. Acceptance criteria, mapped

| AC | How it is met | How it is checked |
|---|---|---|
| #1 light, dark, follow the system | `wc-mode-switcher`, three radios | component test + preview states |
| #2 persists across a reload | `localStorage` key `nigel.color-mode`, read in `initColorMode` | unit tests on `readMode`/`writeMode` with a fake storage, including the throwing one; an app-level test that re-boots and finds the class |
| #3 system tracks `prefers-color-scheme` live | `system` writes no class; the CSS media query does the tracking | asserted structurally (`applyMode('system')` removes both classes) plus a browser pass; the resolved-mode hint is tested through an injected `MediaQueryList` |
| #4 both palettes keep passing contrast | no palette value changes; no third `darkTokens` copy | the existing `packages/theme/__tests__/contrast.test.ts`, unchanged |
| #5 print unaffected by the mode | `:root:root` in the print block (§2.3) | new `print.test.ts` assertion; browser print preview in all three modes on a dark-OS machine |
| #6 keyboard reachable, axe passes in both modes | `wa-radio-group` semantics; `describePreviewA11y` over states that include a dark-mode wrapper | `wc-mode-switcher.test.ts`; keyboard walk in the harness |

On AC #6, the same honesty note as TASK-72: axe under jsdom does not evaluate
colour contrast, so "axe passes in both modes" is a structural claim. The real
contrast guarantee is `contrast.test.ts`, which holds both palettes to AA
already.

---

## 4. Preview states

`wc-mode-switcher.preview.ts`:

| State | What it shows |
|---|---|
| `system` | default selection, hint reading "currently light" |
| `system-dark` | same, hint reading "currently dark" (injected resolved value) |
| `light` | explicit light selected |
| `dark` | explicit dark selected |
| `in-a-header` | slotted into a `wc-app-shell` header, which is where it actually lives |

`wc-app-shell.preview.ts` gains nothing — its `with-header-actions` state
already covers the slot.

---

## 5. Documentation

- `CLAUDE.md` — a "Web UI (SPA)" line: the mode switcher, the `localStorage`
  key, why not `settings.json` (the unlock gate reasoning), and that `system`
  writes no class. A Key Design Constraints line for the print specificity fix,
  since "print wins" is a property the next person will otherwise re-break.
- `web/README.md` — the Printing section's checklist gains "in all three modes",
  and the theme section gains the mode contract.
- `docs/api.md` — nothing. This task adds no endpoint, deliberately.

---

## 6. Open questions for Sam

1. **Inline head script, or accept the first-paint flash?** §2.8. The script is
   six lines and a duplication test; the flash is one frame for users who
   override their OS setting.

2. **Header or settings screen?** §2.7 puts it in the header. Settings is the
   conventional home and is one screen away; the header is one tab stop from
   anywhere. Both is the option to refuse.

3. **Does the preview harness get the switcher too?** Recommended yes — the
   harness is where component states are reviewed, and half of them have never
   been looked at in dark mode. It costs one line in `preview/main.ts` plus a
   control in the harness chrome.

4. **`system` label wording.** "System", "Auto", or "Match system"? The task
   says "follow the system". Whatever is chosen goes in the radio label and in
   the hint, so it is worth picking once.

5. **Should the resolved-mode hint exist at all?** It is the only reason the
   `matchMedia` listener exists. Dropping it removes the listener, the injection
   seam and two preview states, at the cost of the control being unable to say
   what "System" currently means.
