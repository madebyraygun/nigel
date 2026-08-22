---
id: TASK-72
title: >-
  Web UI: dialogs and form fields render unpainted — theme ::part() overrides
  never match
status: Done
assignee:
  - '@stream-4'
created_date: '2026-08-09 00:45'
updated_date: '2026-08-12 17:50'
labels:
  - bug
  - web
  - ui
  - theme
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-72-theme-shadow-parts-design.md
  - docs/superpowers/plans/2026-08-11-task-72-theme-shadow-parts.md
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Edit client dialog renders with no panel background: the clients table behind it shows straight through the header, body and footer, and the Name, Email and Billing address fields have no visible field chrome. Only the focused Notes textarea looks painted, and that is Web Awesome own focus styling rather than ours.

Root cause is not the dialog. @nigel/theme ships the intended rules — wa-dialog::part(header|body|footer) and wa-input/wa-select/wa-textarea::part(base) both set background: var(--wa-color-surface) in global.ts, and they do reach the built stylesheet. But main.ts loads that stylesheet at document level, while the wa-* primitives live inside wc-* shadow roots (wa-dialog inside wc-manager-dialog, wa-input inside wc-client-form). ::part() crosses exactly one shadow boundary and only for parts exposed in the same tree, and nothing calls exportparts, so those rules ship and never match. Design tokens still work because custom properties inherit across shadow boundaries, which is why colour and type look correct while the surfaces do not.

nigelTheme, the CSSResult a component would adopt into its own shadow root to make these rules apply, currently has zero consumers anywhere in apps/ or packages/ui.

Affects every wc-manager-dialog consumer — accounts, categories, rules and clients — not just clients, and every wa-* form field in a wc-* wrapper.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The Add/Edit client dialog renders an opaque panel; nothing behind it is legible through it
- [x] #2 Name, Email and Billing address fields carry the same visible field chrome as the focused Notes field
- [x] #3 The mechanism reaches wa-* primitives nested inside wc-* shadow roots, rather than relying on document-level ::part() rules that cannot match
- [x] #4 The same fix holds for the accounts, categories and rules dialogs, not only clients
- [x] #5 Either nigelTheme gains its consumers or the dead export is removed — the theme does not keep shipping rules that never apply
- [x] #6 A preview state covers a dialog open over a populated list, and describePreviewA11y passes with zero violations
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Diagnosis confirmed by source survey rather than a browser (no browser in the
implementation environment; the browser pass is the review itself, since this
is a visual PR). `rg` finds zero `createRenderRoot` overrides and zero
`exportparts` anywhere in web/, so every component has an open shadow root and
nothing publishes a part upward; the built `dist/css/nigel.css` carried 25
`::part(` rules, all of them at document level, and exactly 23 source files
import a `@awesome.me/webawesome/dist/components/` module. That matches the
spec's table file for file.

Option C as ruled: `global.ts` -> `controls.ts` exporting `controlsCss`, adopted
by all 23 hosts as `[controlsCss, css`…`]`, with a source-scan guard test per
package. Print chrome relocation folded in per the same ruling.

Two departures from the plan, both forced by the code:

1. The plan recommended replacing the brand button's `color: #2b2b33` literal
   with `var(--wa-color-text)`. That token is `#ece9f5` in dark mode and the
   brand gradient is the same pastel ramp in both modes (gradient.ts defines it
   only at `:root`), so it would have shipped a 1.06:1 button label in dark — on
   the one button this PR makes visible for the first time. Added
   `--nc-color-on-gradient: #2b2b33` beside the ramp instead: one value, no dark
   override, identical to what was hardcoded. contrast.test.ts now holds it
   against all seven stops; worst case is 7.73:1.

2. `@nigel/ui` builds could not resolve `@nigel/theme` once its src imported it.
   The root tsconfig maps the package to its source, which is outside ui's
   `rootDir: ./src`, so declaration emit failed with TS6059 on every token
   module. tsconfig.build.json now maps it to `../theme/dist/index.d.ts` — the
   workspace build order already builds theme first. Separately, `@nigel/theme`
   moved from ui's devDependencies to dependencies: it is a runtime import now
   and was resolving only via npm workspace hoisting.

Not done: the plan's Step 0 browser checks and Task 7's manual pass against
demo data. Both need a browser. Everything they would have confirmed is
asserted in tests except the two things tests cannot reach — that the dialog
panel is actually opaque (axe under jsdom does not evaluate colour contrast)
and what a printer does. Those are the review.

REVIEW ROUND 2 — four visual defects and AC #2 not met. One root cause, and it
was not the part rules.

Web Awesome splits each component into compiled CSS inside the shadow root and
the --wa-* properties that CSS reads, which ship in the stylesheet this app
never loads. The theme defined a colour and type vocabulary and never the
structural one: of the 74 --wa-* tokens the nine primitives we render actually
read, 68 were undefined. An undefined custom property is not a default — CSS
discards the declaration referencing it.

  buttons, no padding      padding: 0 var(--wa-form-control-padding-inline)
                           height: var(--wa-form-control-height)
  dialog header flush      padding-block-start:
                             calc(var(--spacing) - var(--wa-form-control-padding-block))
                           -> invalid calc voids the whole declaration
  heading at body size     var(--wa-font-size-l), var(--wa-font-weight-heading)
  inputs, no border        var(--wa-form-control-border-style)
                           -> a border with no style is not drawn, which is why
                              controlsCss setting border-color looked inert

--wa-shadow-s/m/l were a third variant: defined only inside print.ts's
@media print block, so the dialog had no shadow on screen.

Fix is tokens/wa-contract.ts, per the ruling — tokens over per-part rules. It
inherits to every component at once, dark mode and print follow for free
because var() resolves at use time, and it carries no hex so contrast.test.ts's
nth-#rrggbb indexing passes unchanged.

Also removed the wa-input/select/textarea::part(base) and
::part(form-control-label) blocks from controls.ts. Not only redundant: an
outer-tree ::part() rule beats the shadow tree's own for the same property
regardless of specificity — the property this whole task relies on — so those
unconditional rules were overriding Web Awesome's disabled and
appearance="filled" treatments too.

wa-contract.test.ts reads the required token list out of the installed package
rather than a written-down copy, counts only bare var(--x) (var(--x, fallback)
is WA's per-variant indirection, and defining those at :root would pin every
button to one variant), and stops scanning at @media print. Confirmed to fail
naming the token and its five consumers when one is deleted.

Preview coverage needed nothing new: over-a-populated-list already renders the
dialog with four populated, unfocused fields, and wc-client-form's saving state
covers the disabled variant the part-rule removal restores.

Spec §8 carries the reasoning as an addendum.

Not verified: no browser here. All four defects are argued from the Web
Awesome source and asserted in tests, and none has been seen. Body and footer
dialog padding come from --wa-space-l, which was already defined — if those
still read flush after this, it is a different cause.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The theme's treatment for Web Awesome primitives has always shipped and has
never rendered. `::part()` reaches one shadow boundary down, from the tree the
rule is written in; the sheet was loaded at document level while every `wa-*`
primitive sits inside a `wc-*` shadow root inside a screen inside `nigel-app`.
The rules were correct — they were in the wrong tree. Tokens looked right
throughout because custom properties inherit through boundaries, which is why
the app read as half-painted rather than unstyled.

`global.ts` becomes `controls.ts`, exporting `controlsCss`, adopted by the 23
files that render a primitive as `static styles = [controlsCss, css`…`]` —
controls first, so a component can still override the shared treatment.
`nigelTheme` keeps the tokens and print and drops the part rules it could never
deliver; a test asserts it carries no `::part(` at all. A source-scan guard in
each package fails the build when a file imports a `wa-*` module without
adopting the sheet, checking both the import and the use, with no exemption
list and its own detector tests so excluding a file cannot silently disarm it.

The same root cause killed half of `print.ts`: it hid the shell with
`wc-app-shell::part(sidebar)` and a list of `wc-*` tag names, none of which
could match, so printing put the full application chrome on the paper. Token
repainting stays in the document sheet — it works. Hiding moves to the elements:
`wc-app-shell` hides its header, banner and sidebar slot and unclamps the
`100vh`/`overflow: hidden` box that would crop a long report to one screenful;
five chrome components hide themselves; `controlsCss` carries the
`wa-button`/`wa-select` and table-heading rules into every root with a control.

Visible for the first time: the brand gradient on every primary button. Its
label was a `#2b2b33` literal. The plan called for `var(--wa-color-text)`, but
that flips to `#ece9f5` in dark while the pastel ramp does not flip at all —
1.06:1. `--nc-color-on-gradient` holds the original value with no dark
override, and contrast.test.ts checks it against all seven stops (worst 7.73:1).

Preview: `wc-manager-dialog` gains `over-a-populated-list`, the bug as reported
— a six-row table under an open dialog, where a transparent panel and an opaque
one finally look different. axe passes on it, though that proves structure, not
opacity: under jsdom axe does not evaluate colour contrast.
<!-- SECTION:FINAL_SUMMARY:END -->
