---
id: TASK-124
title: 'Web app manifest: the SPA installs to the iPad Home Screen'
status: To Do
assignee: []
created_date: '2026-08-19 20:55'
labels:
  - enhancement
  - frontend
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The SPA ships no web-app manifest and no Apple touch metadata — `web/apps/app/index.html` links neither, and there is no `public/` asset directory. On an iPad, Add to Home Screen therefore produces a bookmark with browser chrome and a screenshot-derived icon, not an app-shaped client. One manifest and a handful of head tags turn Safari-against-a-tailnet-`nigel serve` into a full-screen, home-screen iPad client with no Apple developer account, no signing, and no new native code.

This is the cheap half of the iPad story: TASK-33.8 (remote mode against a standalone serve) supplies the reachable server, and this task makes the browser client it enables feel like an app. It also benefits desktop browsers (installable window) and costs nothing when unused. It works over the current reverse-proxy arrangement too, so nothing here waits on 33.8 — the payoff just grows when it lands.

## Shape

- A `manifest.webmanifest` — name, short name, `display: standalone`, `start_url: /`, scope, icons, and `theme_color`/`background_color` drawn from `@nigel/theme` tokens (both light and dark, via the `media` attribute where supported), served like any other static asset through the vite build and rust-embed.
- `<link rel="manifest">`, `apple-touch-icon`, and the status-bar/theme meta tags in `index.html`.
- Icons generated from the existing mark at the sizes iOS and the manifest spec want; assets live in the app package, not hand-placed in `dist`.
- The placeholder page (`web/placeholder/index.html`) is untouched — it is the "SPA not built" notice, not an installable app.

## What to verify rather than assume

- Standalone mode and the session: launching from the home screen must land somewhere sane when the session cookie is absent or expired (the token/auth flow renders inside standalone mode rather than trapping the user in a chromeless dead end).
- The locked-database screen renders correctly in standalone display.
- No service worker is added here. Offline support is its own decision with real cache-invalidation consequences for money data; this task is metadata and icons only.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The served SPA carries a web-app manifest (standalone display, start_url, scope, icons) and the Apple touch/status-bar metadata, embedded through the normal vite build
- [ ] #2 theme_color and background_color come from @nigel/theme tokens for both light and dark; no brand value is duplicated inline
- [ ] #3 Launched from the iPad Home Screen, the app opens full-screen, and the auth and locked-database flows render usably in standalone mode rather than dead-ending
- [ ] #4 No service worker and no offline caching are added
- [ ] #5 The placeholder page is unchanged
- [ ] #6 Update test coverage
- [ ] #7 Create or update documentation, making sure to remove any out of date information
- [ ] #8 All linting checks pass
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
