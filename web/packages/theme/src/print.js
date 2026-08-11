import { css } from 'lit';
/**
 * What the app looks like on paper.
 *
 * A printed report is the artifact an accountant keeps, so the page has to be
 * the report and nothing else: no sidebar, no toolbars, no export buttons, no
 * screen-sized colour.
 *
 * This sheet does the half of that a document-level stylesheet can actually
 * do: redefine the tokens at `:root`. Custom properties inherit through shadow
 * boundaries, which is the one thing that reaches inside every `wc-*` element
 * at once — a print sheet cannot select into a shadow root, but it can change
 * what the shadow root reads.
 *
 * Hiding the chrome is the other half and is *not* here, because a rule that
 * hides an element has to live in the tree that element is in. `wc-app-shell`
 * hides its own header, banner and sidebar slot; `wc-nav-sidebar`, `wc-toast`,
 * `wc-export-links`, `wc-period-nav` and `wc-register-toolbar` each hide
 * themselves; and `controlsCss` carries the `wa-button`/`wa-select` and table
 * rules into every root that hosts a control. What stays here is what applies
 * to content in the document itself.
 */
export const printCss = css `
  @page {
    margin: 1.5cm;
  }

  @media print {
    :root {
      color-scheme: light;

      --wa-color-bg: #ffffff;
      --wa-color-surface: #ffffff;
      --wa-color-surface-alt: #ffffff;
      --wa-color-border: #999999;
      --wa-color-border-soft: #cccccc;
      --wa-color-text: #000000;
      --wa-color-muted: #333333;
      --wa-color-brand: #000000;
      --wa-color-brand-hover: #000000;
      --wa-color-on-brand: #ffffff;
      --wa-color-danger: #000000;
      --wa-color-success: #000000;
      --wa-color-warning: #000000;
      --wa-color-info: #000000;

      /* Income and expense stop being a colour pair on paper. wc-money always
         renders the sign, so the direction survives the loss of the hue. */
      --nc-color-income: #000000;
      --nc-color-expense: #000000;
      --nc-color-flagged: #000000;
      --nc-color-selected-bg: transparent;

      --nc-grad-brand: none;
      --nc-grad-brand-hover: none;

      --wa-shadow-s: none;
      --wa-shadow-m: none;
      --wa-shadow-l: none;
    }

    html,
    body {
      background: #ffffff;
      color: #000000;
    }

    [data-print='hide'] {
      display: none;
    }

    /* A report that runs over a page break keeps its column headings. */
    thead {
      display: table-header-group;
    }

    tr,
    wc-panel,
    wc-stat-card,
    wc-notice-bar {
      break-inside: avoid;
    }

    wc-panel,
    wc-stat-card {
      box-shadow: none;
    }

    a {
      text-decoration: none;
    }
  }
`;
//# sourceMappingURL=print.js.map