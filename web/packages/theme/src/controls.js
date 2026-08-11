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
export const controlsCss = css `
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

  wa-button::part(label) {
    font-family: var(--wa-font-family-sans);
    font-weight: var(--wa-font-weight-medium);
    letter-spacing: 0.01em;
  }

  wa-input::part(form-control-label),
  wa-select::part(form-control-label),
  wa-switch::part(form-control-label),
  wa-checkbox::part(form-control-label),
  wa-radio::part(form-control-label),
  wa-radio-group::part(form-control-label),
  wa-textarea::part(form-control-label) {
    font-family: var(--wa-font-family-sans);
    font-weight: var(--wa-font-weight-medium);
    color: var(--wa-color-text);
  }

  wa-input::part(base),
  wa-select::part(base),
  wa-textarea::part(base) {
    background: var(--wa-color-surface);
    border-color: var(--wa-color-border);
    border-radius: var(--wa-radius-sm);
    color: var(--wa-color-text);
  }

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
//# sourceMappingURL=controls.js.map