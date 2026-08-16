import { css } from 'lit';
import { brandRamp } from './gradient.js';

/**
 * The brand, danger, success, warning and info entries are darkened
 * derivations of the `effects.rs` pastels (lavender, pink, mint, yellow,
 * cyan). `__tests__/contrast.test.ts` holds every pairing to WCAG AA.
 *
 * Light mode is deliberately **not** paper-white with near-black text. The
 * canvas carries a slight lavender-grey tint and the text is a soft charcoal,
 * which puts text-on-surface at about 11.6:1 — still well above AAA, and a
 * good deal less stark than the 14:1 it used to be. Three planes are meant to
 * be distinguishable at a glance: the sidebar sits lowest, the app canvas
 * above it, and cards are the only true white.
 */
export const colorCss = css`
  :root {
    color-scheme: light dark;

    --wa-color-bg: #f3f2f7;
    --wa-color-surface: #ffffff;
    --wa-color-surface-alt: #f6f4fb;
    --wa-color-border: #dedaeb;
    --wa-color-border-soft: #ebe8f3;
    --wa-color-text: #383843;
    --wa-color-muted: #63636f;
    --wa-color-brand: #5a3fd6;
    --wa-color-brand-hover: #4a32b8;
    --wa-color-on-brand: #ffffff;
    --wa-color-focus: #5a3fd6;
    --wa-color-danger: #b3283f;
    --wa-color-success: #17683a;
    --wa-color-warning: #855508;
    --wa-color-info: #1a5c8c;

    /* Signed money, mirroring src/tui.rs money_span: income reads green,
     * expense reads red. The TUI can lean on color alone; wc-money also
     * renders the sign. */
    --nc-color-income: #17683a;
    --nc-color-expense: #b3283f;
    --nc-color-flagged: #855508;
    --nc-color-selected-bg: #f1edff;

    /* The sidebar is its own plane. It used to be --wa-color-surface, which is
       the same white the cards use, so the navigation and the content it
       navigates read as one continuous sheet. */
    --nc-color-sidebar-bg: #e8e6f0;

    /* The ground a client-facing document is printed on. Deliberately not
       mode-dependent, for --nc-color-on-gradient's reason inverted: a stored
       logo may be transparent and both documents flatten it onto white, so a
       preview of one on a dark card would show the operator something no
       client will ever see. */
    --nc-color-document-bg: #ffffff;

    /* Bar fills, separate from the text tokens above on purpose. A chart bar
       is a large block of colour and only has to clear the 3:1 that WCAG asks
       of a graphic, so it can be much lighter than a figure printed in the
       same hue — which has to clear 4.5:1 and stays where it was. Direction is
       never carried by hue alone anyway: wc-money always renders the sign. */
    --nc-color-income-fill: #5a9473;
    --nc-color-expense-fill: #cb6b7b;
  }
`;

const darkTokens = css`
  --wa-color-bg: #17171d;
  --wa-color-surface: #1f1f28;
  --wa-color-surface-alt: #25252f;
  --wa-color-border: #2e2e3c;
  --wa-color-border-soft: #26262f;
  --wa-color-text: #ece9f5;
  --wa-color-muted: #a5a2b5;
  --wa-color-brand: #c4b7ff;
  --wa-color-brand-hover: #d5cbff;
  --wa-color-on-brand: #17171d;
  --wa-color-focus: #c4b7ff;
  --wa-color-danger: #ffb3ba;
  --wa-color-success: #8ee6a0;
  --wa-color-warning: #ffe0a3;
  --wa-color-info: #bae1ff;

  --nc-color-income: #7fe0a0;
  --nc-color-expense: #ff9fa8;
  --nc-color-flagged: #ffe0a3;
  --nc-color-selected-bg: #2a2740;

  /* Dark mode keeps exactly what it rendered before these three tokens
     existed: the sidebar was --wa-color-surface, and the bars were drawn in
     the same colours as the figures. Dark was not the problem being solved. */
  --nc-color-sidebar-bg: #1f1f28;
  --nc-color-income-fill: #7fe0a0;
  --nc-color-expense-fill: #ff9fa8;

  /* The pastel ramp is legible on a dark surface, so the wordmark keeps it.
     It names the plain ramp rather than --nc-grad-brand, which is periodic and
     sized for the button's hover drift. */
  --nc-grad-brand-text: ${brandRamp};
`;

/**
 * Dark mode by system preference, with `.light-mode` able to opt back out and
 * `.dark-mode` able to force it on regardless of the system setting.
 *
 * `:root` declares `color-scheme: light dark`, which is what lets the UA draw
 * scrollbars, the reconcile screen's `type="month"` picker and form-control
 * defaults from the OS setting. That is right while we are following the OS,
 * and wrong the moment someone overrides it — so an explicit choice pins the
 * scheme to match. The system arm needs no pin: `light dark` already resolves
 * the way the OS asks.
 *
 * `darkTokens` is interpolated exactly twice here and must stay that way.
 * `contrast.test.ts` selects a token by the *n*th `#rrggbb` occurrence in the
 * composed sheet, so a third copy shifts every index and fails that suite with
 * a message pointing nowhere near the cause. Declarations carrying no hex, as
 * below, are safe.
 */
export const colorDarkCss = css`
  @media (prefers-color-scheme: dark) {
    :root:not(.light-mode) {
      ${darkTokens}
    }
  }

  :root.light-mode {
    color-scheme: light;
  }

  :root.dark-mode {
    color-scheme: dark;

    ${darkTokens}
  }
`;
