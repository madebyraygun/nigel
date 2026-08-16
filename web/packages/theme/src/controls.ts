import { css } from 'lit';
import { brandCycleKeyframes } from './tokens/gradient.js';

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
  ${brandCycleKeyframes}

  wa-button[variant='brand']::part(base) {
    background: var(--nc-grad-brand);
    background-size: var(--nc-grad-brand-size);
    background-position: 0% 50%;
    color: var(--nc-color-on-gradient);
    border-color: transparent;
  }

  /* background-image, not the background shorthand: the shorthand resets
     background-size to auto, and the drift below is a position shift measured
     against that size. */
  wa-button[variant='brand']:hover::part(base) {
    background-image: var(--nc-grad-brand-hover);
    filter: brightness(1.04);
  }

  /* The hover treatment for the one button that has a gradient to move: the
     ramp drifts by exactly one period per iteration and starts over where it
     began. Same seven colours — nothing is rotated or recoloured, the image
     is scrolled. */
  wa-button[variant='brand']:not([appearance='plain'], [disabled], [loading]):is(
      :hover,
      :focus-visible
    )::part(base) {
    animation: nc-brand-cycle 2.4s linear infinite;
  }

  @media (prefers-reduced-motion: reduce) {
    /* The border below still draws, instantly, so hover and focus keep an
       indication here — the motion is what goes, not the feedback. */
    wa-button[variant='brand']:is(:hover, :focus-visible)::part(base) {
      animation: none;
    }
  }

  wa-button[variant='brand']:active:not([disabled])::part(base) {
    transform: translateY(1px);
  }

  /* Which edge a button draws on hover. One line per variant, so the rule
     that applies it below is written once and every button is excluded on the
     same terms. Neutral names the text colour because its own border tokens
     are where an outlined button already sits — hovering to them is no change
     at all. */
  wa-button {
    --nc-hover-border: var(--wa-color-text);
  }

  wa-button[variant='brand'] {
    --nc-hover-border: var(--wa-color-brand);
  }

  wa-button[variant='danger'] {
    --nc-hover-border: var(--wa-color-danger);
  }

  wa-button[variant='success'] {
    --nc-hover-border: var(--wa-color-success);
  }

  wa-button[variant='warning'] {
    --nc-hover-border: var(--wa-color-warning);
  }

  /* Web Awesome already gives the base part a --wa-border-width-s border in
     transparent and transitions it, along with five other properties, over
     --wa-transition-fast. A transition declared out here replaces that whole
     list rather than adding to it, so the list is restated with the two
     properties the edge is drawn from on their own longer duration and
     everything else left where WA had it. controls.test.ts pins which
     properties this is answerable for. */
  wa-button::part(base) {
    transition:
      background var(--wa-transition-fast),
      border-color var(--nc-duration-slow),
      box-shadow var(--nc-duration-slow),
      color var(--wa-transition-fast),
      opacity var(--wa-transition-fast),
      transform var(--wa-transition-fast);
  }

  /* A plain button is a row action drawn as bare text, a disabled one is
     refusing, and a loading one drops the click in handleClick — none of the
     three should be carrying the strongest click invitation in the theme.

     The edge is --wa-border-width-m, drawn in two halves so the box never
     changes: the border WA already reserved space for, plus the rest of the
     way as an inset ring. Widening the border itself would move every
     neighbour a pixel on hover, and an inset shadow is clipped to the padding
     edge, so the ring lands flush inside the border and the two read as one
     solid 2px edge. The remainder is subtracted rather than written as 1px,
     so a change to either width token stays a --wa-border-width-m edge. */
  wa-button:not([appearance='plain'], [disabled], [loading]):is(:hover, :focus-visible)::part(base) {
    border-color: var(--nc-hover-border);
    box-shadow: inset 0 0 0
      calc(var(--wa-border-width-m) - var(--wa-form-control-border-width)) var(--nc-hover-border);
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
