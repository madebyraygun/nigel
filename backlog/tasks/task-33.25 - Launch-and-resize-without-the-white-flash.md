---
id: TASK-33.25
title: Launch and resize without the white flash
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-20 23:49'
updated_date: '2026-08-21 14:34'
labels:
  - tauri
  - macos
milestone: m-0
dependencies: []
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The window paints white before the SPA's first frame and shows white at the edges when a resize outruns the webview — the webview's default background peeking out from behind the page. A native window is never a different color than its content.

Two shell-side changes. Give the window a background color matching the theme bg, resolving dark mode at build time the way the SPA's pre-paint script does, so a dark-mode launch does not flash light — the OS appearance is the shell's best signal, with the stored in-app override applied by the pre-paint script an instant later. And create the window hidden, showing it once the frontend signals ready, so first paint is the app rather than a blank sheet. Show-on-ready trades a flash for a beat of nothing; the beat has to stay short enough to read as instant.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Launching in light and in dark shows no wrong-color flash before first paint
- [ ] #2 Fast resizes show the theme background at the edges, never white
- [ ] #3 The window appears promptly — perceived launch is not slower
- [ ] #4 Windows and Linux are unaffected or equally improved; no @nigel/ui change
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Branch feat/desktop-launch-paint stacked on feat/desktop-lifecycle — both restructure main.rs, so the PR's base is the lifecycle branch, retargeted to main when that merges. Same worktree, sequential.
2. New src/chrome.rs with BG_LIGHT #f3f2f7 and BG_DARK #17171d; the builder gains .visible(false), and immediately after build the shell sets the background color from window.theme(), the OS signal. Drift is pinned by a source-text test reading ../../web/packages/theme/src/tokens/color.ts and asserting both hexes appear — the window_size.rs convention pointed at the theme.
3. Frontend-ready and palette refinement through the api seam, wired in web/apps/app only. Two commands: frontend_ready (shell shows and focuses the window; a 4-second fallback timer from setup shows it regardless, so a wedged frontend never leaves an invisible process) and set_chrome_background(mode), which refines the window color to the SPA's actual resolved palette — stored override included — at boot and on mode toggle. desktop-client.ts grows the capability, the browser client no-ops it, main.ts fires ready after first paint, and a documentElement class observer reports mode changes. No @nigel/ui or @nigel/theme change.
4. index.html's pre-token background fallback #fdfcfb is corrected to the token value #f3f2f7, the same drift family the color-mode bootstrap test pins — the pre-token frame must not flash a stray color.
5. Tests: Rust source-text (visible(false) before build, the fallback timer present, the theme drift pin); vitest for the new client capability (ready fires once; the observer reports mode). Gates: cargo fmt --check and cargo test in crates/nigel-desktop; npm typecheck, lint, test, build from web/.
6. Verifiable on Linux: resize-edge background and show-on-ready timing. macOS list: dark-mode launch color, and the title-bar region once 33.20 lands.
7. Merge points, flagged: the menu lane (33.22) also adds builder-chain and invoke-handler lines in main.rs (additive, low risk); PRs 38 and 40 touch web/apps/app/src/api/client.ts, where this adds one interface method (additive, low risk).
<!-- SECTION:PLAN:END -->
