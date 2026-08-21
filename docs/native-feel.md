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

## The face the machine already uses

A native app draws its chrome in the system face, and using anything else is
the loudest of the remaining tells — louder than a cursor or a scrollbar,
because every label on screen carries it. `--wa-font-family-sans` is a
`system-ui` stack for exactly that reason, and it is what a component gets by
asking for nothing.

IBM Plex Mono is bundled into the binary and is asked for by job, never
inherited. `--nc-font-figures` (aliased `--nc-font-money`) is for a column of
digits that has to land above the column below it — an amount, a count, a
percentage, a date. `tabular-nums` gets a proportional face most of the way,
but only a mono keeps the separators and the currency symbol aligned too.
`--nc-font-brand` is the wordmark and the company name, wherever it appears:
the sidebar's brand row and the unlock gate alike. `--wa-font-family-mono` is
for the code-shaped strings — a pattern, an account code, a column index, a
filename, an invoice number, a keyboard hint. An identifier is read character
by character and is not a quantity, which is why it takes the code face rather
than the figures one even though both resolve to the same stack.

A `ch` budget measures the element's own zero, so a width reserved in `ch` and
the face that fills it have to be the same one. `wc-period-nav`'s label and
`wc-balance-list`'s amount column both set the figures face for that reason.

On paper the sans token goes back to the bundled stack. A system face is
whatever the machine has, and a report printed from two machines has to be one
document.

No component names a family. `packages/ui/src/__tests__/font-stack-guard.test.ts`
fails the build on a `font-family` that does not resolve through a token,
because a hardcoded stack renders perfectly well and would keep its old face
through a change made everywhere else.

## Chrome that moves has to move

Putting the sidebar away is a change of shape, and a native window animates it
rather than cutting to the result. Which element animates depends on the width,
and only one of the two rules is live at a time.

Above 48rem the sidebar is a docked column and `wc-nav-sidebar` transitions its
own `width` between 232px and the 56px rail over `--nc-transition-base`. The
transition exists only while a toggle holds `data-animating` on the host, so a
window resize — including one dragged across the breakpoint, in either
direction — changes the width in a single frame; only a gesture slides. Labels
keep their layout in both states and the moving edge crops them, which is what
makes collapse read as expand run backwards: rows hold their line, a label is
ellipsised rather than wrapped, and the full text stays reachable as the
button's `title`.

Below 48rem there is no rail — the sidebar is a drawer, and `wc-app-shell`
slides the whole thing off-canvas with `transform`. That rule is written in the
shell's tree against `::slotted()`, and an outer-tree rule beats the inner
tree's `:host` for the same property whatever the specificity, so the shell
decides at this width and the sidebar cancels even a gesture's width transition
to say so.

Reduced motion is answered in `@nigel/theme`: every `--nc-transition-*` and
`--nc-duration-*` collapses to zero under `prefers-reduced-motion: reduce`, so
a component that reads a token gets the preference without asking. Both sliding
surfaces state the cancel in their own media query as well.

## Spellcheck on data fields

A red squiggle under an account code, a regex pattern or a currency code is
noise, not a correction, because none of those are prose. Fields that take
an amount, a date, an account or category name, an invoice number, a vendor
or a pattern carry `spellcheck="false"`. Fields where someone writes actual
sentences — a client's billing address, an invoice's notes and terms, a
memo — keep spellcheck on, because that is exactly the content a spelling
correction is for.
