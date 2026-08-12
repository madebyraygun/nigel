---
id: TASK-76
title: 'Theme: adopt a mono primary typeface for a terminal visual brand'
status: In Progress
assignee:
  - '@stream-4'
created_date: '2026-08-09 00:46'
updated_date: '2026-08-11 23:15'
labels:
  - enhancement
  - web
  - ui
  - theme
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-11-task-76-mono-typeface-design.md
  - docs/superpowers/plans/2026-08-11-task-76-mono-typeface.md
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Nigel is a terminal tool with a web front end, and the type does not say so. Move the primary typeface to a mono — IBM Plex Mono or Fira Mono — so the browser reads as the same product as the CLI.

This is a token change first: @nigel/theme owns --wa-font-family-sans and the rest of typography.ts, so the switch should happen there rather than in components. Worth checking against the places where type is doing real work — the register and report tables, wc-money (already tabular figures, which a mono only helps), the aging strip labels, and the invoice HTML template, which is a client-facing document and may well want to stay in a text face rather than follow the app.

Fonts must be self-hosted and bundled: the SPA is embedded in the binary by rust-embed and nigel serve is a localhost server with no network guarantee, so a webfont CDN is not an option. Weight matters to binary size — subset to the ranges actually used.

Decide explicitly whether this is mono everywhere or mono for UI chrome and data with a text face for prose, and whether the published invoice page follows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The primary typeface is a self-hosted mono, bundled rather than fetched from a CDN
- [x] #2 No network request is made for a font at runtime
- [x] #3 A decision is recorded on whether the client-facing invoice template follows the app or keeps a text face
- [ ] #4 Register and report tables, wc-money figures and the aging strip stay legible and aligned at the new metrics
- [ ] #5 Line lengths and control heights are re-checked — mono is wider, and the manager tables and dialogs must not overflow
- [x] #6 The contrast and a11y suites still pass
- [x] #7 Added font weight to the embedded bundle is measured and noted
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decisions taken as ruled: IBM Plex Mono, weights 400/500/600, mono everywhere
in the app, the client-facing invoice keeps its text face, marketing site out
of scope, target <200KB added. The family and the scope remain Sam's to
override at review.

MEASUREMENTS (same machine, same toolchain, --release both sides)

  fonts on disk     21,676 + 22,264 + 22,348  =     66,288 B
  web/dist             648,126 ->    715,713  =    +67,587 B
  target/release/nigel  25,606,416 -> 25,676,408 = +69,992 B  (+0.27%)

Comfortably inside the 200KB target. The binary delta is measured, not derived
— it exceeds the font bytes by ~3.7KB, which is rust-embed's path metadata.

TOOLING DEPARTURE: this environment has no pip, no ensurepip, no venv, no uv
and no pipx, so pyftsubset was unreachable. Subsetting was done instead with
`subset-font` (harfbuzz wasm) via npx, driven by a committed script at
packages/theme/scripts/subset-fonts.mjs. Same model the plan asked for — run
by hand, output committed, never a build or CI dependency — and the script is
in the repo, so it is more reproducible than a README command. Source is the
complete upstream faces from @ibm/plex-mono@2.5.0, not fontsource's
pre-subset files (those are already stripped and would have lost more).

SPEC DISCREPANCY, and the notable finding of this task: spec §4.5 lists
`✓ ✗ × ⟳ ◻ ◑ ● ◆ ▲ ⊘` among the symbols to include in the subset. IBM Plex Mono
does not contain eight of them — `✗ ⟳ ◑ ● ◆ ▲ ⊘ ◻` are absent from the
*complete* upstream font (checked with fontkit against the unsubset woff2), so
no subset could have included them. `✓ × ← ↑ → ↓ · • — – …` are all present.

Consequence: wc-invoice-status (all six status markers), wc-send-dialog (⟳, ✗)
and wc-reconciliation-history (✗) fall back per glyph. A reconciled row and a
discrepancy row now draw their marks from two different fonts, and the invoice
table's status column loses exact width alignment. Recommended fix is
wc-icon-* SVGs through the existing WcIconBase rather than hunting a mono with
dingbat coverage — a component change, deliberately not folded in here.
Recorded in CLAUDE.md and web/README.md.

METRICS NOT CHANGED, deliberately. Spec §5 says decide the base size after
looking, in a separate commit. There is no browser in this environment, so
guessing 13px would have been exactly the thing that commit exists to avoid.
Left at 14px/1.5 for Sam. The arithmetic for the decision: Plex Mono
advances every glyph at 0.600em (measured), so 8.40px per character at 14px
and 7.80px at 13px — a 60-character description wants 504px against 468px.
The `longest-realistic-row` canary state is declared so the judgement has
something concrete to be made against.

ACs #4 and #5 are left unchecked: both are "look at it at 1280 and 1024" and
neither can be claimed from here. Everything else is asserted in tests.
The slashed-zero question (§5) is also unanswered — it needs eyes on figures.

RE-MEASURED after the --wa-* token contract landed on the base branch (that
shifted the baseline, so the earlier absolute figures were stale). Both sides
measured on the same machine and toolchain, with and without the fonts:

  fonts on disk                                    66,288 B  (unchanged)
  web/dist      659,310 ->    726,897  =          +67,587 B
  release bin  25,618,704 -> 25,684,688 =          +65,984 B  (+0.26%)

The font delta on web/dist is byte-identical to the earlier run; the binary
delta moved from 69,992 to 65,984, which is why the spec says to measure it
rather than derive it from the file sizes. Still far inside the 200KB target.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
IBM Plex Mono 400/500/600, subset to Latin plus the punctuation and arrows the
UI draws, committed under `packages/theme/src/fonts/` and served from the same
binary that serves the app. Weights chosen to match
`--wa-font-weight-normal`/`-medium`/`-bold` exactly, which is the whole
argument for Plex over Fira: Fira has no 600, so every table header and field
label would be browser-synthesised or the token would move to 700 and the UI
would get heavier for a reason unrelated to design.

No component changed. One token has always owned the primary face, and both
family tokens now point at the bundled stack with the system mono behind it —
mono rather than the old sans, so a face that fails to load still aligns money
columns.

`@font-face` URLs are relative, resolved against `dist/css/nigel.css`, which
the app and the preview harness both alias. An absolute `/fonts/…` would have
worked in the app and 404'd on :9090, so every component state would have been
reviewed in the wrong typeface with nothing failing to say so. Three tests
carry the offline guarantee: no remote URL in the composed sheet, no font host
or preconnect hint in any HTML entry point, and every declared face actually
present in `dist/fonts/` — that last one because a `@font-face` pointing at a
missing file falls back silently and would ship a system-font UI with a green
suite.

Weight added to the binary: **69,992 bytes, +0.27%** (25,606,416 -> 25,676,408
on the same machine and toolchain). Fonts on disk are 66,288; `web/dist` grows
67,587. Well inside the 200KB target.

The invoice template does not follow, and CLAUDE.md now says why so nobody
tidies up the inconsistency: the published page has nowhere to get a font,
a CDN link would be a third-party request in someone else's browser opening a
bill from us, `pdf.rs` renders through printpdf's built-in faces so the two
halves of one document would diverge, and it is not our brand surface.

One thing the spec had wrong, found by actually checking coverage: IBM Plex
Mono has no glyph for `✗ ⟳ ◑ ● ◆ ▲ ⊘ ◻`, in the complete upstream font, not
just the subset. `wc-invoice-status`, `wc-send-dialog` and
`wc-reconciliation-history` draw all eight and fall back per glyph — so a
reconciled row (`✓`, present) and a discrepancy row (`✗`, absent) now draw
their marks from two different fonts. The fix is SVG icons through the
existing `WcIconBase`, filed rather than folded in.

Base metrics deliberately unchanged at 14px/1.5. That decision belongs after
looking at the harness, which is what the review is for.
<!-- SECTION:FINAL_SUMMARY:END -->
