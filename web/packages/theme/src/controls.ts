import { css } from 'lit';

/**
 * The brand treatment for Web Awesome primitives, adopted by the components
 * that render them.
 *
 * `::part()` reaches exactly one shadow boundary down, and it reaches it from
 * the tree the rule is written in. Every `wa-*` primitive here lives inside a
 * `wc-*` shadow root, which lives inside a screen's, which lives inside
 * `nigel-app`'s — so a document-level copy of these rules cannot even select
 * the host, let alone its parts. The sheet therefore has to be adopted by the
 * component that *hosts* the primitive; an ancestor's copy is already one
 * boundary too far.
 *
 * `static styles = [controlsCss, css\`…\`]` is the shape, and the order is
 * load bearing: a component's own rules come after so it can still override
 * the shared treatment. A source-scan guard test in each package fails the
 * build when a file imports a `wa-*` component module without adopting this.
 *
 * Tokens are the other way through the boundary and need none of this — custom
 * properties inherit, which is why the document sheet still owns them.
 */
export const controlsCss = css`
  wa-button[variant='brand']::part(base),
  wa-button[variant='primary']::part(base) {
    background: var(--nc-grad-brand);
    color: var(--nc-color-on-gradient);
    border-color: transparent;
  }

  wa-button[variant='brand']:hover::part(base),
  wa-button[variant='primary']:hover::part(base) {
    background: var(--nc-grad-brand-hover);
    filter: brightness(1.04);
  }

  wa-button[variant='brand']:active:not([disabled])::part(base),
  wa-button[variant='primary']:active:not([disabled])::part(base) {
    transform: translateY(1px);
  }

  /* Hover and focus-visible put a glow under a button in the ramp's own hues.
     The token flips to the pastels in dark mode; the ink ramp is what reads on
     a pale surface.

     The pseudo-class is written twice on purpose. wa-button sets
     delegatesFocus, so the host matches :focus-visible when the inner control
     does — and the part itself is that control, which is the direct statement
     of the same thing.

     No transition is declared here. wa-button's own base part already
     transitions box-shadow over --wa-transition-fast, which wa-contract.ts
     points at --nc-duration-fast — 0ms under prefers-reduced-motion. A
     transition in this rule would replace that whole list rather than add to
     it, taking the background and colour transitions with it. */
  :is(wa-button[variant='brand'], wa-button[variant='primary']):not([disabled]):is(
      :hover,
      :focus-visible
    )::part(base),
  :is(wa-button[variant='brand'], wa-button[variant='primary']):not(
      [disabled]
    )::part(base):focus-visible {
    box-shadow: var(--nc-glow-brand);
  }

  /* Secondary: the neutral variant, filled or outlined. A plain button is a
     row action drawn as bare text and a semantic variant is its own colour —
     neither takes the brand halo. */
  :is(wa-button:not([variant]), wa-button[variant='neutral']):not(
      [appearance='plain'],
      [disabled]
    ):is(:hover, :focus-visible)::part(base),
  :is(wa-button:not([variant]), wa-button[variant='neutral']):not(
      [appearance='plain'],
      [disabled]
    )::part(base):focus-visible {
    box-shadow: var(--nc-glow-neutral);
  }

  wa-button::part(label) {
    font-family: var(--wa-font-family-sans);
    font-weight: var(--wa-font-weight-medium);
    letter-spacing: 0.01em;
  }

  /* Field chrome and label type are deliberately *not* here. They belong to
     the --wa-form-control-* tokens in tokens/wa-contract.ts, and a part rule
     would be worse than redundant: a rule written in the outer tree beats the
     shadow tree's own for the same property whatever the specificity, so an
     unconditional background on an input's base part overrides Web Awesome's
     disabled and filled-appearance treatments as well as its default one.
     The token reaches the same declaration and leaves the variants intact. */

  wa-dialog::part(header),
  wa-dialog::part(body),
  wa-dialog::part(footer) {
    background: var(--wa-color-surface);
    color: var(--wa-color-text);
  }

  wa-dialog::part(header) {
    border-bottom: 1px solid var(--wa-color-border);
  }

  :focus-visible {
    outline: 2px solid var(--wa-color-focus);
    outline-offset: 2px;
  }

  /* Print, for content that lives inside a shadow root.
     @nigel/theme's print sheet carries the same rules for content in the
     document; these are the copy that reaches the other side of the boundary,
     and this sheet is adopted by every root that hosts a control. */
  @media print {
    wa-button,
    wa-select {
      display: none;
    }

    [data-print='hide'] {
      display: none;
    }

    /* A report that runs over a page break keeps its column headings. */
    thead {
      display: table-header-group;
    }

    tr {
      break-inside: avoid;
    }

    a {
      text-decoration: none;
    }
  }
`;
