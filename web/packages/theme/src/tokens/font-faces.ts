import { css } from 'lit';

/**
 * The bundled primary face.
 *
 * IBM Plex Mono, self-hosted, subset, and committed under `../fonts/`. Nothing
 * is fetched at runtime: `nigel serve` is a single binary that may be running
 * with no network at all, and a CDN link would make the app's typography
 * depend on something outside the binary that ships it.
 *
 * The URLs are **relative**, and that is load bearing. They resolve against
 * `dist/css/nigel.css`, which both Vite roots — the app and the preview
 * harness on :9090 — alias and import. An absolute `/fonts/…` would work in
 * the app and 404 in the harness, so every component state would be reviewed
 * in the wrong typeface with nothing to say so. Vite rewrites these into
 * hashed files under `web/dist/assets/`, which rust-embed bakes in and
 * `static_files.rs` serves as `font/woff2` with an immutable cache header.
 *
 * Regenerate with `scripts/subset-fonts.mjs`; see web/README.md.
 */

export const FONT_FAMILY = 'IBM Plex Mono';

/**
 * Exactly `--wa-font-weight-normal`, `-medium` and `-bold`. Plex Mono ships a
 * real 600, which is the reason it was chosen over Fira Mono — whose absence
 * of one would leave every table header and field label synthesised.
 */
export const BUNDLED_FONT_WEIGHTS = [400, 500, 600] as const;

export const fontFacesCss = css`
  @font-face {
    font-family: 'IBM Plex Mono';
    font-style: normal;
    font-weight: 400;
    font-display: swap;
    src: url('../fonts/ibm-plex-mono-400.woff2') format('woff2');
  }

  @font-face {
    font-family: 'IBM Plex Mono';
    font-style: normal;
    font-weight: 500;
    font-display: swap;
    src: url('../fonts/ibm-plex-mono-500.woff2') format('woff2');
  }

  @font-face {
    font-family: 'IBM Plex Mono';
    font-style: normal;
    font-weight: 600;
    font-display: swap;
    src: url('../fonts/ibm-plex-mono-600.woff2') format('woff2');
  }
`;
