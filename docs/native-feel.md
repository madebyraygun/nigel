# Native feel

The SPA in `web/` runs both in a browser tab and inside the Tauri desktop
shell (`docs/desktop.md`). A handful of browser behaviours that nobody
notices in a tab read as "this is a website in a box" inside a webview:
rubber-band scrolling past the end of a list, a blue selection painting
across toolbar chrome while dragging, a drag ghost trailing an icon, a link
cursor on a button, a red spell-check squiggle under an account code. None of
these cost anything to fix on the web, so the fixes are unconditional CSS and
attributes rather than a platform check.

These conventions were worked out first in the boxcraft project
(`docs/dev/native-feel-conventions.md` there); they are engine-level rather
than app-specific, so they transfer here unchanged.

## The placement rule

Platform conditionals belong in the shell and the app composition layer —
`crates/nigel-desktop` and `web/apps/app` — never inside `@nigel/ui`. No
component in `web/packages/ui/src/components` sniffs the user agent or
checks whether it is running under Tauri. Every rule below is plain CSS and
HTML attributes, true and harmless in a browser as well as in the shell.

## Document-level rules

`web/apps/app/index.html` carries three rules on `html, body`:

- `overscroll-behavior: none` stops the document rubber-banding past its own
  ends. Scrolling stays inside the panel or list that owns it, which is what
  a native window does — a browser tab's own bounce is the tell this removes.
- `user-select: none` (with the `-webkit-` prefix for WebKitGTK and
  WKWebView) stops a drag across the window's chrome from painting a text
  selection, the way dragging a native toolbar never does.
- `img { -webkit-user-drag: none; }` stops an icon or a decorative image
  producing the browser's own drag ghost when someone drags across it by
  accident.

## Selection has to be given back

`user-select` is an inherited property, and inheritance crosses shadow
boundaries: turning it off on `html, body` turns it off everywhere in the
app, including data a person opens the app specifically to read and copy —
an amount, a transaction description, an account name, an invoice number.
Every place that shows content like that restores `user-select: text` on
itself, and every field a person types into does the same, so typing and
copying keep working exactly as before while chrome — labels, headers,
toolbars, buttons, nav — stays unselectable.

`wc-money` restores it once, on its own host, which covers every amount
anywhere in the app without every table or card that renders one having to
say so again. `@nigel/theme`'s `controlsCss` restores it for every native
`input`, `textarea`, `[contenteditable]` and `wa-input` in one place, because
that sheet is already adopted by every component that hosts a Web Awesome
primitive (`docs/architecture.md` and the component-first workflow in
`CLAUDE.md` say why). Everywhere else — a table's data cells, a definition
list's values, a rendered filename — the component that shows the content
restores its own selection.

## Cursor discipline

A native control shows the platform's arrow cursor; `cursor: pointer` is the
web's own affordance for a link that navigates, and showing it on a button
reads as a promise the control does not keep. Every `wc-*` component in
`web/packages/ui/src/components` uses `cursor: default` on its buttons,
options and disclosure controls. A genuine `<a>` needs no cursor rule at
all — the user agent already shows the pointer on anchors — so the only
components that carry a `cursor` declaration are the ones whose element is
not a link.

## Spellcheck on data fields

A red squiggle under an account code, a regex pattern or a currency code is
noise, not a correction, because none of those are prose. Fields that take
an amount, a date, an account or category name, an invoice number, a vendor
or a pattern carry `spellcheck="false"`. Fields where someone writes actual
sentences — a client's billing address, an invoice's notes and terms, a
memo — keep spellcheck on, because that is exactly the content a spelling
correction is for.

## The shell

The frame follows the app-refresh handoff (`docs/design/app-refresh/README.md`,
section 1) and is built from `@nigel/ui`: `wc-app-shell` draws the top bar
and the content area, `wc-nav-sidebar` is the sidebar slotted into it, and
`nigel-app` hands both what they show.

- **Floating sidebar.** An inset panel, 8px (`--nc-window-inset`) from the
  window's edges, 14px radius, `--nc-shadow-floating`, on
  `--nc-color-sidebar-bg`. 224px wide, folding to a 58px icon rail over
  180ms (`--nc-transition-sidebar`, zero under reduced motion). The
  hamburger and the `nigel` wordmark sit in its header; the nav is grouped
  (an unlabelled top group, then Statements, Billing and Setup, read from
  `navGroup` in the screen registry); Review carries the queue's count; and
  Settings is pinned in the footer. The active item is filled with
  `--nc-color-selected-bg` inside a 1px rainbow hairline: the fill painted to
  the padding box and `--nc-grad-brand-text` to the border box of a
  transparent border.
- **Folded state is per device.** The rail is remembered in `localStorage`
  under `nigel.sidebar-collapsed`, like the colour mode, and only on a wide
  viewport: at phone width collapsed means the drawer is shut, which is where
  it starts anyway. Below 48rem the sidebar is an off-canvas drawer with a
  backdrop and a hamburger in the top bar.
- **Top bar.** 50px, with the breadcrumb on the left — the book's monogram
  (`wc-monogram`: initials on a pastel from `--nc-color-pastel-1…7`, picked by
  hashing the data directory so a book keeps its colour), its name, then the
  screen, and `Invoices / #n` on an open invoice with Invoices a way back to
  the list — then the search pill and refresh. The book and the pill dispatch
  `nc-palette-open` (`{ scope: 'books' | 'all' }`), which bubbles out of
  `nigel-app` to the document for the palette to answer; refresh remounts the
  screen and reloads the status and the Review count.
- **Status line.** A second inset panel under the content (`wc-status-line`,
  34px, the sidebar's material, 12px radius, its bottom edge level with the
  sidebar's): the open book first (monogram and name;
  `nc-palette-open` scoped to books; absent until the book has a name), the
  data directory as selectable text (`dataDirDisplay`, home written as `~`,
  with the full `dataDir` in the tooltip), `encrypted` / `not encrypted`, then
  `▲ n to review` (`nc-navigate` to Review; absent at zero), `⌘K palette`
  (`Ctrl K` off Apple platforms; `nc-palette-open` for all) and the version.
  `nigel-app` hands the shell what it shows from `/api/status` (`dataDirDisplay`,
  `dataDir`, `encrypted`, `version`) and the Review count the sidebar already
  loads. The component omits the version when `distribution` is `"store"`, but
  `/api/status` does not report `distribution` yet: TASK-150.4 adds it, and
  until then the version always shows. As the line narrows the version goes
  first, then the palette hint (container queries), and the book name and lock
  label truncate with an ellipsis. Hosted mode draws no status line, and below
  48rem the component hides itself.
- **Window title.** `{Screen} · {Book}`, innermost level first on an open
  invoice (`#1254 · Invoices · Acme Consulting LLC`). `nigel-app` sets
  `document.title`; the Tauri window follows it (`docs/desktop.md`).
- **Scroll.** A screen change puts the content area back at the top, as a
  native window's new pane would be. The shell resets on a change of
  `scrollKey`, which the app derives from the breadcrumb, so a filter or a
  period change within a screen keeps the reader's place.
- **Hosted mode.** `shell=native` on the URL the page is opened with means
  it is inside the Swift shell, which draws its own sidebar. The host may put
  the flag in the query (`nigel://localhost/?shell=native#/dashboard`) or in
  the hash route's parameters (`nigel://localhost/#/settings?shell=native`,
  how it opens a screen directly); `isHostedShell` in
  `apps/app/src/shell-preferences.ts` accepts either, read once at boot, and
  `parseHash` strips the parameter so no screen sees it. `nigel-app` then
  renders no `wc-nav-sidebar` and sets `hide-sidebar` on the shell, which
  drops the sidebar slot and its toggle and keeps the breadcrumb top bar. The
  flag is read in the app; no `wc-*` component knows which host it is in.

## Focus

Two treatments, both in `controlsCss` so every root that hosts a control
draws them the same way:

- A **field** — a text `input`, `select`, `textarea`, and `wa-input`,
  `wa-select` and `wa-textarea` through the part that carries their border —
  turns its border `--wa-color-brand` and wears a 3px `--nc-color-ring` halo
  on any focus, pointer included, because a caret in a field is the thing
  being worked on. Checkboxes, radios, file pickers, range sliders and colour
  inputs are controls, not fields, and take the outline below. The field
  keeps a transparent 2px outline rather than none: forced-colors mode drops
  the halo and repaints that outline, and a `forced-colors: active` rule
  draws it in `Highlight`, so a high-contrast user still sees which field has
  the caret.
- Everything else uses `:focus-visible`: a 2px `--wa-color-focus` outline
  offset 2px, on keyboard focus only, so a click does not leave a ring
  behind.
