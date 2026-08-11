# Task 76 — A bundled mono primary typeface

Stream 4 (Web theme), PR-4c. Last of the three, after TASK-72 (the theme reaches
shadow roots) and TASK-75 (mode switcher). Deliberately last: this is the change
with the widest visual blast radius and the least functional value, and it wants
to be measured against a UI that is finally painting the way it was designed to.

**Objective:** the browser reads as the same product as the CLI, with the font
bundled into the binary and no network request for type — and with the register
and report tables still legible at the new metrics.

Two decisions in here need Sam's sign-off before implementation: **which
family** (§2) and **how far the mono goes** (§3). Both are argued to a
recommendation rather than left open.

---

## 1. Where the codebase actually is

**One token owns the primary face.** `web/packages/theme/src/tokens/typography.ts`
defines `--wa-font-family-sans` (a system sans stack), `--wa-font-family-mono` (a
system mono stack), five sizes, three weights (`normal: 400`, `medium: 500`,
`bold: 600`), `--wa-line-height: 1.5`, and `--nc-font-money:
var(--wa-font-family-mono)`. Every component reads
`font-family: var(--wa-font-family-sans)` — no component names a family.

**Its doc comment is a promise this task breaks**, and says so in as many words:
"System stacks only — nigel bundles no webfonts, so nothing is added to the
embedded binary and nothing is fetched at runtime". That comment is the current
design decision. Replacing it is part of the work, not an afterthought.

**Three HTML files hardcode a fallback stack** for the pre-token first paint:
`web/apps/app/index.html` (`var(--wa-font-family-sans, ui-sans-serif, …)`),
`web/packages/ui/preview/index.html` (tokens only, no literal fallback), and
`web/placeholder/index.html` (a plain literal stack — this is the "SPA not
built" page `build.rs` seeds, served with no theme and no font assets at all).

**The client-facing invoice does not read app tokens.**
`src/invoicing/templates/invoice.html` inlines
`body{font-family:system-ui,sans-serif;…}`. The PDF half of the same document is
rendered by `src/pdf.rs` through printpdf's built-in faces; CLAUDE.md already
records why printpdf's optional asset features were refused.

**Static assets are embedded and typed correctly.**
`src/server/static_files.rs` serves `web/dist` through rust-embed and sets
`Content-Type` from the embedded file's own mimetype metadata, so `.woff2`
answers `font/woff2` with no code change. Anything under `assets/` gets
`public, max-age=31536000, immutable`; anything else gets `no-store`. That is an
argument for letting Vite hash the fonts into `assets/`.

**There is a precedent next door.** `~/Dev/boxcraft/main/packages/theme` has
`src/tokens/font-faces.ts` — a `css` module of `@font-face` rules for
self-hosted variable woff2 files, composed into the theme, with
`__tests__/font-faces.test.ts` asserting one face per family, the weight ranges,
`font-display: swap`, and the URLs. Boxcraft's mono is Chivo Mono at
`--wa-font-size-base: 13px` / `--wa-line-height: 1.35`. The mechanism is worth
copying wholesale; the family is not a constraint.

**Non-ASCII glyphs the UI renders** (from a scan of `packages/ui/src` and
`apps/app/src`, comments included, so this is a superset): `— – … “ ” ’ · • ✓ ✗
× ← ↑ → ↓ ⟳ ◻ ◑ ● ◆ ▲ ⊘`. Plus whatever arrives in bank descriptions and client
names, which is real Latin text and not predictable.

---

## 2. Which family: IBM Plex Mono

**Recommendation: IBM Plex Mono.** Both candidates are SIL OFL 1.1, both are
self-hostable, both look like a terminal. The tiebreakers:

| | IBM Plex Mono | Fira Mono |
|---|---|---|
| Weights shipped | Regular 400, Medium 500, SemiBold 600, Bold 700 (+ italics) | Regular 400, Medium 500, Bold 700 |
| Fits `--wa-font-weight-bold: 600`? | exactly | no — 600 would synthesise or the token would have to become 700 |
| Italic | yes | none — italics synthesise (oblique-by-shear) |
| Latin coverage | Latin-1 + Extended-A and beyond | Latin-1 + Extended-A |
| Reads as | a terminal, and specifically as a *ledger* terminal | a terminal |

The weight row decides it. `--wa-font-weight-bold: 600` is used across the
component library for table headers, field labels and emphasis. Fira Mono has no
600, so every one of those would either be browser-synthesised (a smeared fake
bold that looks worst at small sizes on a mono) or the token would move to 700 —
a heavier UI than the one that exists, changed for a reason that has nothing to
do with design.

Weights to bundle: **400, 500, 600**. Not 700 (no token asks for it), not
italics (nothing in the UI is italic — worth confirming during the harness walk;
if something is, add the 400 italic and note the cost).

**Verify at implementation time whether the pinned version ships a variable
woff2.** If it does, one file covering 400–600 replaces three and is smaller
than the sum. If it does not, three static subsets. This is a fact about a
package version, not a design decision, and the plan measures it rather than
assuming it.

---

## 3. How far the mono goes: everywhere in the app, nowhere in the invoice

### 3.1 The app: mono everywhere, via the one token

`--wa-font-family-sans` becomes `'IBM Plex Mono', ui-monospace, SFMono-Regular,
Menlo, Consolas, monospace`, and `--wa-font-family-mono` becomes the same stack.
No component changes.

The alternative — mono for chrome and data, a text face for prose — was
considered seriously and rejected:

- The app has almost no prose. The longest strings in it are two-sentence
  guardrail explanations, field hints, and empty states. A second face for those
  buys very little.
- It costs a second token (`--nc-font-prose`) and a per-component judgement call
  about which side of the line each string falls on. Every future component then
  makes a typographic decision, which is exactly the drift the token system
  exists to prevent — and the reason a component "never inlines a brand value"
  is already a rule here.
- Two families is two font budgets in the binary, and this task has a
  measurement AC.

The honest cost of going mono everywhere is width and density, addressed in §5.

### 3.2 The published invoice: keeps its text face

**AC #3's recorded decision: the client-facing invoice template does not
follow.** Four independent reasons, any one of which would be enough:

1. **The published page has nowhere to get the font.** `AssetPublisher` uploads
   `index.html` and `invoice.pdf` to R2 and nothing else — CLAUDE.md is explicit
   that `publish_page` rewrites index.html alone. Shipping a font would mean new
   objects, new paths, and cache-busting for a document whose whole design is
   two static files behind a token.
2. **A CDN link is worse here than it is in the app.** In the app it is banned
   by the offline constraint; on a client's invoice it would be a third-party
   request made by someone else's browser when they open a bill from us.
3. **The PDF cannot follow.** `pdf.rs` renders through printpdf's built-in
   faces. The HTML and the PDF are one pair produced from one seam
   (`invoicing::render::render_invoice`) and are supposed to look like the same
   document; changing one is how they diverge.
4. **It is not our brand surface.** The invoice is read by people who have never
   seen the CLI. A terminal face there is a statement about us made in a document
   that is about them.

A cheap half-step exists and is **not** taken in this task: giving the amounts
column in `invoice.html` a system mono stack for alignment, with no bundled
font. Worth a follow-up if the columns bother anyone; out of scope here because
it changes a client-facing document for a reason unrelated to this task.

### 3.3 The placeholder page: unchanged

`web/placeholder/index.html` is served when the SPA has not been built. There is
no `web/dist` to serve a font from, by definition. It keeps its system stack.

### 3.4 The marketing site

`site/` is deployed separately by `.github/workflows/pages.yml` and is out of
scope. If the mono becomes the brand, the site should follow — as its own task.

---

## 4. Bundling

### 4.1 Where the bytes live

`web/packages/theme/src/fonts/*.woff2`, **committed**, with the OFL text beside
them in `web/packages/theme/src/fonts/LICENSE`.

Committed rather than pulled from `@fontsource/…` at build time because the
files must be *subset* (AC's weight concern), and subsetting needs `fonttools`,
which is not available in CI and must not become a build dependency. The subset
is produced once, locally, by a command recorded in `web/README.md` and repeated
only when the glyph inventory changes.

### 4.2 How they reach the browser

Copy boxcraft's shape, with one deliberate difference:

```ts
// web/packages/theme/src/tokens/font-faces.ts
export const fontFacesCss = css`
  @font-face {
    font-family: 'IBM Plex Mono';
    font-style: normal;
    font-weight: 400;
    font-display: swap;
    src: url('../fonts/ibm-plex-mono-400.woff2') format('woff2');
  }
  /* …500, 600 — or one variable face spanning 400 600 */
`;
```

composed into `nigelTheme` **first**, before the token modules.

The difference from boxcraft: **relative URLs, not `/fonts/…` absolute ones.**
Boxcraft serves the files from `apps/web/public/fonts/`, which means the app
owns bytes the theme package declares. Here that would break the preview harness
outright — it is a separate Vite root on :9090 and would 404 every face, so
every component state would be reviewed in the wrong typeface, silently.

With a relative URL, the resolution chain is:

1. `build-css.js` writes `dist/css/nigel.css` and copies `src/fonts/*.woff2` to
   `dist/fonts/`;
2. both apps alias `@nigel/theme/css/nigel.css` to that built file
   (`apps/app/vite.config.ts`, `packages/ui/preview/vite.config.ts`), so Vite
   resolves `url('../fonts/…')` against it and emits the woff2 into the build's
   hashed `assets/`;
3. `web/dist/assets/ibm-plex-mono-400.<hash>.woff2` is embedded by rust-embed
   and served with the immutable cache header and `font/woff2`.

The preview harness gets the same faces from the same declaration, which is the
whole point.

### 4.3 No network, provably

- `font-faces.test.ts` asserts every `src:` is a relative `url(` with
  `format('woff2')`, that no `http`/`//` appears anywhere in `nigelTheme.cssText`,
  and that every face sets `font-display: swap`.
- A build assertion (extending `__tests__/build-css.test.ts`) that
  `dist/fonts/` contains one file per declared face — a `@font-face` pointing at
  a missing file fails silently in a browser, which is the failure mode that
  would ship a system-font UI while every test passed.
- `apps/app/index.html` gains no `<link>` of any kind. A grep-style assertion in
  the app's guard tests keeps it that way: no `fonts.googleapis`, no
  `preconnect`, no `@import url(http`.

`font-display: swap` rather than `block` or `optional`: the bytes come from the
same binary over loopback, so the swap window is imperceptible, and `optional`
can permanently settle on the fallback on a slow first paint — which would mean
the brand face sometimes just does not appear.

### 4.4 Measuring the weight (AC #7)

Recorded in the task notes and in `web/README.md`:

- `ls -l` each subset woff2, and the total;
- `web/dist` total before and after;
- `cargo build --release` binary size before and after, same machine, same
  toolchain, `--release` both times.

The third is the number the AC is actually about, and it is not the sum of the
first — rust-embed stores the bytes as-is, but a release binary's size is not a
linear function of its embedded assets. Measure it, do not derive it.

Expectation, to be confirmed rather than trusted: a Latin subset of one static
weight lands in the 15–25 KB range, so three weights are roughly 50–75 KB, and a
variable subset spanning 400–600 is usually smaller than two statics. If the
measured total exceeds ~150 KB, something is wrong with the subset and the plan
stops to look rather than shipping it.

### 4.5 The subset

Include: Basic Latin, Latin-1 Supplement, Latin Extended-A (client names and
bank descriptions are real-world text — a subset that drops `é` renders half a
client's name in a fallback face, mid-word, which is worse than not bundling at
all), General Punctuation, Currency Symbols, and the specific symbols the UI
draws: `✓ ✗ × ⟳ ◻ ◑ ● ◆ ▲ ⊘ ← ↑ → ↓ · •`.

Anything outside that falls back per-glyph, which is the correct behaviour and
is why the Latin ranges are generous.

---

## 5. Metrics, and the things most likely to break

IBM Plex Mono advances every glyph at a fixed width, so ordinary UI strings
render meaningfully wider than the proportional system sans they replace —
expect on the order of 10–20% for mixed-case text, more for text that is mostly
lowercase. That is the entire risk of this task, and it lands on:

| Surface | What to look at |
|---|---|
| `wc-register-table` | the densest table in the app; description + vendor + category on one row at 1280px and at 1024px |
| `wc-report-table` | eight reports, section/subtotal/total rows, long category names |
| `wc-manager-table` | rules (pattern, match type, category, vendor, priority) is the widest |
| `wc-invoice-table` | client names plus four money columns |
| `wc-money` | already tabular; a mono can only help, but check the sign column still aligns |
| `wc-aging-bars` | five bucket labels across the strip |
| `wc-manager-dialog` + forms | `wc-client-form` sets `min-width: 20rem` on its field grid; buttons grow with their labels |
| `wc-nav-sidebar` | `--nc-sidebar-width` against the longest nav label |
| `wc-app-shell` header | `--nc-header-height: 48px` against a taller line box |

**Recommended response: change the token, look at it, then decide the size.**
Two commits inside PR-4c:

1. the face swap alone, at today's `14px / 1.5`;
2. a metrics commit — the candidate is `--wa-font-size-base: 13px` and
   `--wa-line-height: 1.45`, which is roughly where boxcraft landed with Chivo
   Mono (13px / 1.35) — kept separate so it can be reverted without reverting
   the face.

Doing it the other way round (guessing the metrics up front) makes the harness
walk unable to tell which change caused what.

Two things to check while walking, both cheap to get wrong:

- **The zero.** IBM Plex Mono offers a slashed-zero alternate. On a bookkeeping
  screen a distinguishable zero is worth having; if it is a stylistic set rather
  than the default, decide whether to enable it via `font-feature-settings` on
  `--nc-font-money`'s consumers.
- **`--nc-font-money` becomes redundant.** It currently points at the mono stack
  to make money columns align against a proportional UI. With a mono UI it
  points at the same thing everything else does. Keep the token (it names an
  intent, and a future non-mono UI would need it again), but say so in the
  comment.

---

## 6. Acceptance criteria, mapped

| AC | How it is met | How it is checked |
|---|---|---|
| #1 self-hosted mono, bundled not CDN | §4.1–4.2 | `font-faces.test.ts`; the build assertion that `dist/fonts/` is populated |
| #2 no runtime network request for a font | relative URLs, no `<link>`, no `@import` | the no-network assertions in §4.3; devtools network tab with the browser offline during the manual pass |
| #3 a decision recorded on the invoice template | §3.2 — it does not follow, with four reasons | this document, plus a `CLAUDE.md` line |
| #4 tables, `wc-money`, aging strip legible and aligned | §5 | harness walk at :9090 over every listed component, then the real app on demo data |
| #5 line lengths and control heights re-checked | §5 | the same walk at 1280 and 1024, plus every manager dialog opened |
| #6 contrast and a11y suites still pass | nothing about colour changes; nothing about structure changes | `npm test` (contrast in `@nigel/theme`, axe in `@nigel/ui`) |
| #7 added weight measured and noted | §4.4 | the three measurements, recorded in the task notes and `web/README.md` |

---

## 7. Preview states

This task re-renders existing states rather than inventing behaviour, so it adds
one canary rather than a set:

| Component | New state | Why |
|---|---|---|
| `wc-register-table` | `longest-realistic-row` — a 60-character description, a long vendor, the longest stock category name, a six-figure amount | the densest row in the app, at the width where a mono first overflows |

Everything else is covered by the states already declared, which is the point of
driving axe off the preview object. The review is the harness walk, not new
files.

---

## 8. Documentation

- `web/packages/theme/src/tokens/typography.ts` — the doc comment currently
  states the opposite policy and must be rewritten: what is bundled, why it is
  bundled rather than fetched, and where the files live.
- `web/README.md` — a "Typefaces" section: the family, the weights, the subset
  command verbatim, the measured sizes, and the rule that a new glyph outside
  the subset falls back rather than failing.
- `CLAUDE.md` — the `@nigel/theme` architecture entry (the theme now ships
  fonts), and a Key Design Constraints line carrying §3.2's decision, so the
  next person does not "fix" the invoice's inconsistency.

---

## 9. Open questions for Sam

1. **IBM Plex Mono or Fira Mono?** §2 recommends Plex on the 600-weight
   argument. If Fira is preferred for its look, `--wa-font-weight-bold` moves to
   700 and the UI gets heavier.

2. **Mono everywhere, or mono for chrome and data only?** §3.1 recommends
   everywhere. The counter-argument is that guardrail explanations and empty
   states are the only text anyone *reads* rather than scans, and mono is
   measurably slower to read in prose.

3. **Base size: hold at 14px, or drop to 13px?** §5 recommends deciding after
   looking, in a separate commit. If 13px, every screen gets denser at once —
   which some will read as an improvement and some as a regression.

4. **Does the marketing site follow?** Out of scope here (§3.4), but if the mono
   is the brand then the site being sans is the inconsistency, not the invoice.

5. **Is ~50–100 KB of binary acceptable for a typeface?** The measurement AC
   implies a threshold nobody has stated. Worth naming one before the work, so
   the answer is not negotiated after the fonts are committed.
