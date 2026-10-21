import { css } from 'lit';
import { brandCycle, brandRamp } from './gradient.js';
import { shadowDarkTokens } from './shadow.js';

/**
 * The brand, danger, success, warning and info entries are darkened
 * derivations of the `effects.rs` pastels (lavender, pink, mint, yellow,
 * cyan). `__tests__/contrast.test.ts` holds every pairing to WCAG AA.
 *
 * Light mode is deliberately **not** paper-white with near-black text. The
 * canvas carries a slight lavender-grey tint and the text is a soft charcoal,
 * which puts text-on-surface at about 11.6:1 — still well above AAA, and a
 * good deal less stark than pure black on white. Three planes are meant to be
 * distinguishable at a glance: the app canvas sits lowest, the floating
 * sidebar and status panels just above it, and cards are the only true white.
 *
 * The values are the app-refresh handoff's table
 * (`docs/design/app-refresh/README.md`, "Design tokens"); `web/README.md`
 * maps each of its shorthand names onto the role here.
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
    --wa-color-info: #266ba1;

    /* Signed money, mirroring crates/nigel/src/tui.rs money_span: income reads green,
     * expense reads red. The TUI can lean on color alone; wc-money also
     * renders the sign. */
    --nc-color-income: #17683a;
    --nc-color-expense: #b3283f;
    --nc-color-flagged: #855508;
    /* The active nav item's fill. */
    --nc-color-selected-bg: #f1edff;

    /* The keyboard or pointer row in a list, the selected table row, the
       active chip. The same value as the active nav fill, kept as its own
       role because the two are chosen for different reasons. */
    --nc-color-highlight: #f1edff;

    /* What a row, a nav item or an icon button sits on under the pointer. */
    --nc-color-hover: #efedf5;

    /* The floating sidebar and status panels: a plane just off-white, so
       they read as panels over the canvas rather than as more card. */
    --nc-color-sidebar-bg: #fbfafe;

    /* An input's own ground. White in light mode like a card; a shade below
       the card in dark mode, so a field reads as recessed into it. */
    --nc-color-field: #ffffff;

    /* The halo around a focused field, drawn outside its brand-coloured
       border. Translucent brand, so it tints whatever it lands on. */
    --nc-color-ring: rgb(90 63 214 / 22%);

    /* What dims the app behind a dialog, the palette or the open drawer. A
       tinted, light veil on the light canvas; a heavy black on the dark one,
       where a light veil would read as a white wash. */
    --wa-color-overlay-modal: rgb(40 36 60 / 18%);

    /* A chip that stays dark in both modes — the toast, which paints over
       cards and dialogs that already use --wa-color-surface and would vanish
       into them in dark mode if it reused it. Not mode-dependent. */
    --nc-color-inverse-bg: #1f1f28;
    --nc-color-inverse-text: #ece9f5;
    --nc-color-inverse-border: rgb(255 255 255 / 10%);
    --nc-color-inverse-hover: rgb(255 255 255 / 15%);

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

    /* The Snake board is a terminal, not a document, and these three do not
       flip with the colour mode. The pastels the snake is drawn in only read
       on a dark ground — on a light one the whole snake is seven near-white
       smudges — so the board keeps the dark canvas whichever mode is on. The
       food green is crates/nigel/src/tui.rs GREEN and the ink is the dark-mode text, which
       is what the TUI draws the head in. */
    --nc-color-arcade-bg: #17171d;
    --nc-color-arcade-ink: #ece9f5;
    --nc-color-arcade-food: #50dc64;

    /* A switch's knob: white on the brand track and on the off track, in
       both modes. */
    --nc-color-knob: #ffffff;

    /* The Appearance picker's miniature windows. Each draws one mode while
       the app is in either, so neither flips with the colour mode: the light
       set is the light canvas, sidebar, card, border and brand, the dark set
       the same roles in dark. */
    --nc-color-preview-light-bg: #f3f2f7;
    --nc-color-preview-light-panel: #fbfafe;
    --nc-color-preview-light-card: #ffffff;
    --nc-color-preview-light-line: #dedaeb;
    --nc-color-preview-light-accent: #5a3fd6;
    --nc-color-preview-dark-bg: #17171d;
    --nc-color-preview-dark-panel: #22222b;
    --nc-color-preview-dark-card: #1f1f28;
    --nc-color-preview-dark-line: #2e2e3c;
    --nc-color-preview-dark-accent: #c4b7ff;
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
  --wa-color-danger: #ff9fa8;
  --wa-color-success: #8ee6a0;
  --wa-color-warning: #ffe0a3;
  --wa-color-info: #bae1ff;

  --nc-color-income: #7fe0a0;
  --nc-color-expense: #ff9fa8;
  --nc-color-flagged: #ffe0a3;
  --nc-color-selected-bg: #2a2740;
  --nc-color-highlight: #2a2740;
  --nc-color-hover: #2a2a35;
  --nc-color-sidebar-bg: #22222b;
  --nc-color-field: #1b1b22;
  --nc-color-ring: rgb(196 183 255 / 25%);
  --wa-color-overlay-modal: rgb(0 0 0 / 45%);

  --nc-color-income-fill: #6fcf92;
  --nc-color-expense-fill: #f0919c;

  ${shadowDarkTokens}

  /* The pastel ramp is legible on a dark surface, so the wordmark keeps it.
     It names the plain ramp rather than --nc-grad-brand, which is periodic and
     sized for the button's hover drift. */
  --nc-grad-brand-text: ${brandRamp};
  --nc-grad-brand-text-cycle: ${brandCycle};
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
