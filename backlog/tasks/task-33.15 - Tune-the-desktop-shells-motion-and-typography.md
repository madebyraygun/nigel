---
id: TASK-33.15
title: Tune the desktop shell's motion and typography
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-18 01:56'
updated_date: '2026-08-21 00:48'
labels:
  - tauri
  - ui
milestone: m-0
dependencies: []
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The shell reads as more native since the web tells were removed, and what remains is typographic and motion work rather than convention-following.

Three specific pieces the operator named after using the desktop app:

The sidebar toggle snaps. It should animate, respecting prefers-reduced-motion, which the app already honours in seventeen places.

The hamburger sits after the company name. On a desktop window it belongs at the far left, before the name, where a title bar's controls live.

The typeface balance leans mono. IBM Plex Mono is currently the default body face — --wa-font-family-sans falls back to a mono stack — so chrome, labels and prose all render in it. A native app uses the system face for its chrome. Plex Mono should stay where it earns its place: figures, where digits must align, and the brand's own character. This is a change to the theme tokens rather than to components, since a component reads the token and never a hardcoded stack.

The third is the substantial one and the biggest remaining tell after the conventions work: system-ui for chrome is most of what makes an app look like it belongs on the machine.

Two notes from a later read of the code. wc-app-shell already renders the nav toggle before the header title (wc-app-shell.ts:312-320), and the title there is the screen title with the company name living in the sidebar — so AC #2 may already hold; verify in the running shell rather than re-doing it. And while in the tokens: native macOS chrome text is 13px (NSFont.systemFontSize), the base here is 14px today — worth trying 13px for chrome-level text alongside the face change.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The sidebar toggle animates, and does not animate under prefers-reduced-motion
- [ ] #2 The menu control sits at the far left of the header, before the company name
- [ ] #3 Chrome, labels and prose render in the system face; figures and the brand keep IBM Plex Mono
- [ ] #4 The change is made in @nigel/theme tokens; no component names a font stack directly
- [ ] #5 Money columns still align digit for digit, and the mono-glyph coverage test still passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Worktree: git worktree add /home/dalton/Dev/nigel/wt-typeface -b feat/shell-typography from main, then npm ci in its web/. Touches only web/packages/theme, web/packages/ui, web/apps/app/index.html and four docs — no collision with PRs #37-#40 except the shared docs files and wc-import-history.ts, which this task leaves alone.

2. Split the faces in web/packages/theme/src/tokens/typography.ts. --wa-font-family-sans becomes a system stack (system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Helvetica Neue, Arial, Noto Sans, sans-serif); --wa-font-family-mono keeps the IBM Plex Mono stack unchanged. Keep the sans name: it means the primary UI face and Web Awesome's own internals read it.

3. Add two semantic tokens beside the existing --nc-font-money, so no component ever points at --wa-font-family-mono for a brand or figures reason. --nc-font-figures: var(--wa-font-family-mono) is the aligned-figures face (money, counts, dates in columns); --nc-font-money: var(--nc-font-figures) keeps the name wc-money and wc-reconcile-form already read; --nc-font-brand: var(--wa-font-family-mono) is the wordmark and the sidebar's brand name. Semantic tokens are the cleaner answer here because they say why Plex is being asked for, which is what a later face change needs to know.

4. Update the pre-theme fallback in web/apps/app/index.html: font-family: var(--wa-font-family-sans, <system stack>) instead of the mono fallback, so the first paint before the token sheet arrives is the system face too.

5. Point the figure and date surfaces at --nc-font-figures. The rule is: every selector that already declares font-variant-numeric: tabular-nums, plus explicit date cells. wc-register-table td.date, wc-reconciliation-history td.month, wc-invoice-table td.number, wc-manager-table td.end/th.end, wc-count-grid dd, wc-reconcile-result dd, wc-line-items td.figure input, wc-sample-table .date (already mono, renamed to the semantic token). wc-import-history td.count is deliberately skipped — PR #37 rewrites that file; file it as a follow-up.

6. Point the brand surfaces at --nc-font-brand: wc-nav-sidebar .brand-name and wc-wordmark .art.

7. Animate the wide-viewport rail (AC #1). wc-nav-sidebar :host gets transition: width var(--nc-transition-base), which is the only thing that changes between 232px and 56px. Reduced motion is already answered at the token level (--nc-transition-base collapses to 0ms linear), and an explicit @media (prefers-reduced-motion: reduce) { :host { transition: none } } is added to mirror the shell's existing belt-and-braces rule and to give the CSS-text test something to assert. Add overflow-x: hidden and white-space: nowrap on the nav rows so labels are clipped and wiped in rather than wrapping inside a 56px box while the width slides. Below 48rem nothing changes: the shell already slides the sidebar off-canvas with transform var(--nc-transition-base) and already cancels it under reduced motion.

8. AC #2 is verification only, plus a flag. wc-app-shell already renders .nav-toggle as the first child of the header, before the h1 screen title, and no company name is in the header — the company name is wc-nav-sidebar's brand row, fed by nigel-app as app-name. So the AC as worded has no company name in the header to sit before. Add a test pinning nav-toggle-before-title order, and confirm in the preview harness and the running shell. Do NOT restructure the header: the residual observation (the sidebar's brand row shares the 48px top band and sits left of the toggle) is a separate design decision and is raised for the operator rather than acted on here.

9. Tests. Rewrite the two nigel-theme.test.ts cases that pin Plex as the primary face: assert --wa-font-family-sans starts at system-ui and ends at sans-serif and names no Plex, and move the ui-monospace/monospace assertions onto --wa-font-family-mono. Add --nc-font-figures and --nc-font-brand to that file's nigel-token list, and resolve --nc-font-money through to the Plex stack with the existing token-resolution helper. In font-faces.test.ts, the ordering assertion now measures against --wa-font-family-mono, the token that names the family. Add a source-scan guard (packages/ui/src/__tests__/font-stack-guard.test.ts, in the style of mono-glyph-coverage) that fails if any file under packages/ui/src, packages/ui/preview, apps/app/src or apps/app/index.html declares a font-family whose first value is not a var(), which is AC #4 made enforceable. Add wc-nav-sidebar CSS-text tests for the width transition and the reduced-motion cancel, and a toggling preview state so the animation can actually be watched in the harness (describePreviewA11y picks it up automatically). Correct the stale premise in mono-glyph-coverage.test.ts's header comment — Plex is the figures and brand face now, not the primary face — without changing what it forbids; AC #5 holds because that test is a source scan and wc-money keeps --nc-font-money plus tabular-nums.

10. Docs, per the documentation policy. docs/design-constraints.md line 90 asserts the web UI is Plex throughout and must be rewritten to the split; the invoice's own system stack and its reasons stay. docs/architecture.md's sentence that the two family tokens are the same Plex stack must be rewritten. web/README.md's Typefaces section says Plex everywhere and must say chrome-is-system, Plex-for-figures-and-brand. Add a short Typeface and rail-motion section to docs/native-feel.md, which is where these conventions live.

11. Verify: from web/, npm ci, npm run typecheck, npm run lint, npm test, npm run build; then npm run preview on :9090 and walk every preview for a face regression (money columns, tables, forms, dialogs, wordmark), and npm run dev plus cargo run -- serve --no-open to check the rail animation, the reduced-motion path and the header order in the real app. ./scripts/check-no-real-data.sh --staged before each commit, judged by exit status.

12. Out of scope, flagged not silently taken: the description's 13px chrome-size suggestion (no AC covers it, and it ripples through spacing and the contrast suite) and dropping the now possibly unused Plex 500 subset (would change README's measured byte table). Neither is done here. font-faces.ts itself needs no change: the woff2 subsets and the three @font-face declarations are unaffected by Plex's role narrowing.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented on feat/shell-typography; PR #41.

The token split. --wa-font-family-sans is a system-ui stack (with -apple-system, BlinkMacSystemFont, Segoe UI, Roboto and a sans-serif tail, because WebKitGTK and older WKWebView are both shells nigel ships in and neither reliably answers system-ui alone). --wa-font-family-mono is unchanged. Three semantic tokens carry Plex's remaining jobs: --nc-font-figures for columns of digits, --nc-font-money aliased to it where wc-money and wc-reconcile-form already read one, --nc-font-brand for the wordmark and the company name. Naming the job rather than the family is what lets a component ask for alignment without knowing which face provides it.

Eight figure and date columns were getting Plex only by inheritance and now ask for it: wc-register-table td.date, wc-reconciliation-history td.month, wc-invoice-table td.number, wc-manager-table td.end, wc-count-grid dd, wc-reconcile-result dd, wc-line-items td.figure input, wc-sample-table .date. wc-import-history td.count matches the same rule and was skipped to avoid conflicting with PR #37 — TASK-33.28.

The rail. wc-nav-sidebar transitions its own width over --nc-transition-base, which the theme already zeroes under prefers-reduced-motion; the component repeats the cancel in a media query so a reader of the file sees it. overflow-x: hidden and white-space: nowrap on the two rows stop a full-width label reflowing inside the 56px box during the slide. Nothing changed below 48rem, where wc-app-shell already slid the drawer with transform.

AC #2 needed no code. wc-app-shell already renders .nav-toggle as the header's first child before the screen title, and no company name is in the header — nigel-app feeds it to wc-nav-sidebar as app-name. A test pins the order. Measured in the running app, the brand row sits at x=12 and the toggle at x=249, so the window's top band still reads company name then toggle; that is a design decision rather than an ordering bug and is TASK-33.27.

New guard. packages/ui/src/__tests__/font-stack-guard.test.ts fails the build on any font-family in packages/ui/{src,preview}, apps/app/src or apps/app/index.html whose value does not resolve through a token. Verified to bite by planting a hardcoded stack. This is AC #4 made enforceable rather than assumed.

Verification beyond the suites. Both the preview harness and the real app (this branch's binary, an isolated fixture book on a spare port, never the operator's instance on 5731) were driven in Chromium and measured: 21/21 harness checks and 11/11 app checks. The rail was caught genuinely mid-slide one frame after the real toggle is pressed (233px to 220px to 57px over 0.2s) and lands at once under reduced motion (233px to 57px, 0s).

Gates from web/: typecheck, lint, test (931 app tests, 163 files, zero failures) and build all pass; check-no-real-data.sh --staged exits 0.

font-faces.ts needed no change — the woff2 subsets and the three @font-face declarations are unaffected by Plex's role narrowing. Two things stay deliberately undone: the 13px chrome size the description floats (no AC covers it, and it ripples through spacing and the contrast suite) and dropping the Plex 500 subset, which may now have no consumer but would change the measured byte table in web/README.md.
<!-- SECTION:NOTES:END -->
