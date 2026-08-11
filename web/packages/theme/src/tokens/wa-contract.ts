import { css } from 'lit';

/**
 * The `--wa-*` tokens Web Awesome's own component styles read.
 *
 * This app deliberately never loads Web Awesome's stylesheet — theming rides
 * custom properties instead. What that stylesheet also carries, though, is the
 * *structural* half of every component: padding, borders, control heights,
 * transitions, focus rings. WA's compiled component CSS reads those from
 * `--wa-*` custom properties, and a property nothing defines is not a default,
 * it is nothing:
 *
 * - `padding: 0 var(--wa-form-control-padding-inline)` collapses to `0`, so a
 *   button is exactly as wide as its label;
 * - `border-style: var(--wa-form-control-border-style)` is invalid and drops
 *   to `none`, so an input has no field chrome — setting only `border-color`
 *   from a `::part()` rule cannot bring it back, because a border with no
 *   style is not drawn;
 * - `padding-block-start: calc(var(--spacing) - var(--wa-form-control-padding-block))`
 *   is an invalid calc, which voids the whole declaration, so the dialog
 *   header sits flush against the panel edge.
 *
 * So this module is the other half of the bargain. Everything here is defined
 * in terms of the tokens the rest of the package already owns, never as a
 * literal, which has three consequences worth stating: the app's own look
 * drives Web Awesome's rather than the other way round; dark mode and print
 * come free, because `var()` resolves at use time and both of those redefine
 * the tokens underneath; and `contrast.test.ts`, which indexes tokens by their
 * *n*th `#rrggbb` occurrence, is untouched because there is not one hex value
 * in this file.
 *
 * Prefer adding a token here over hand-writing padding or borders into a
 * `::part()` rule in `controls.ts`. A token reaches every `wa-*` component at
 * once through inheritance; a part rule fixes one part of one component.
 * Part rules are for what the token vocabulary genuinely has no word for.
 */
export const waContractCss = css`
  :root {
    /* Spacing. WA reads --wa-space-2xs, and --wa-spacing-xs in two places —
       that inconsistency is upstream's, and both have to be answered. */
    --wa-space-2xs: 4px;
    --wa-spacing-xs: var(--wa-space-xs);

    /* Radii. The package names its scale --wa-radius-*; WA's components read
       --wa-border-radius-*. Same values, both spellings, one source. */
    --wa-border-radius-s: var(--wa-radius-sm);
    --wa-border-radius-m: var(--wa-radius-md);
    --wa-border-radius-l: var(--wa-radius-lg);
    --wa-border-radius-pill: var(--wa-radius-pill);
    --wa-radius-s: var(--wa-radius-sm);
    --wa-radius-m: var(--wa-radius-md);
    --wa-radius-l: var(--wa-radius-lg);

    /* Shadows, same story: shadow.ts names them -sm/-md/-lg while WA and
       print.ts both read -s/-m/-l. Without these the dialog has no shadow. */
    --wa-shadow-s: var(--wa-shadow-sm);
    --wa-shadow-m: var(--wa-shadow-md);
    --wa-shadow-l: var(--wa-shadow-lg);

    --wa-border-style: solid;
    --wa-border-width-s: 1px;
    --wa-border-width-m: 2px;

    --wa-panel-border-style: var(--wa-border-style);
    --wa-panel-border-width: var(--wa-border-width-s);
    --wa-panel-border-radius: var(--wa-radius-lg);

    /* Type. --wa-font-size-l is what the dialog title reads; without it the
       heading renders at the inherited body size. */
    --wa-font-size-m: var(--wa-font-size-base);
    --wa-font-size-l: var(--wa-font-size-lg);
    --wa-font-size-smaller: var(--wa-font-size-s);
    --wa-font-weight-body: var(--wa-font-weight-normal);
    --wa-font-weight-semibold: var(--wa-font-weight-medium);
    --wa-font-weight-heading: var(--wa-font-weight-bold);
    --wa-font-weight-action: var(--wa-font-weight-medium);
    --wa-line-height-normal: var(--wa-line-height);
    --wa-line-height-condensed: 1.25;

    /* Motion. The package's own scale is --nc-duration-*; these are the names
       WA transitions read, so a reduced-motion preference reaches WA too. */
    --wa-transition-fast: var(--nc-duration-fast);
    --wa-transition-normal: var(--nc-duration-base);
    --wa-transition-slow: var(--nc-duration-base);
    --wa-transition-easing: ease;

    /* Focus. Matches the ring controls.ts draws, so a wa-* control and a plain
       one inside the same shadow root are ringed identically. */
    --wa-focus-ring-style: solid;
    --wa-focus-ring-width: 2px;
    --wa-focus-ring-offset: 2px;
    --wa-focus-ring: var(--wa-focus-ring-style) var(--wa-focus-ring-width)
      var(--wa-color-focus);

    /* Surfaces and text, in WA's vocabulary. */
    --wa-color-surface-default: var(--wa-color-surface);
    --wa-color-surface-raised: var(--wa-color-surface);
    --wa-color-surface-lowered: var(--wa-color-surface-alt);
    --wa-color-surface-border: var(--wa-color-border);
    --wa-color-text-normal: var(--wa-color-text);
    --wa-color-text-quiet: var(--wa-color-muted);
    --wa-color-text-link: var(--wa-color-brand);

    /* The dialog scrim. Deliberately a literal black at low alpha rather than
       a token: it is the same darkening in both modes, and a scrim built from
       --wa-color-text would invert to a white wash over a dark app. */
    --wa-color-overlay-modal: rgb(0 0 0 / 45%);
    --wa-color-overlay-inline: rgb(0 0 0 / 20%);

    /* Hover and active are mixed into currentColor by WA as
       color-mix(in oklab, currentColor, var(--wa-color-mix-hover)). */
    --wa-color-mix-hover: var(--wa-color-surface) 12%;
    --wa-color-mix-active: var(--wa-color-surface) 20%;

    /* The fill / on / border × quiet / normal / loud families, for the two
       palettes our components actually ask for. "on" is the readable
       foreground for the matching fill, which is why each one names a token
       the contrast suite already holds against its partner. */
    --wa-color-neutral-fill-quiet: var(--wa-color-surface-alt);
    --wa-color-neutral-fill-normal: var(--wa-color-border-soft);
    --wa-color-neutral-fill-loud: var(--wa-color-muted);
    --wa-color-neutral-on-quiet: var(--wa-color-text);
    --wa-color-neutral-on-normal: var(--wa-color-text);
    --wa-color-neutral-on-loud: var(--wa-color-bg);
    --wa-color-neutral-border-quiet: var(--wa-color-border-soft);
    --wa-color-neutral-border-normal: var(--wa-color-border);
    --wa-color-neutral-border-loud: var(--wa-color-border);

    --wa-color-brand-fill-quiet: var(--wa-color-surface-alt);
    --wa-color-brand-fill-normal: var(--wa-color-border-soft);
    --wa-color-brand-fill-loud: var(--wa-color-brand);
    --wa-color-brand-on-quiet: var(--wa-color-brand);
    --wa-color-brand-on-normal: var(--wa-color-brand);
    --wa-color-brand-on-loud: var(--wa-color-on-brand);
    --wa-color-brand-border-quiet: var(--wa-color-border-soft);
    --wa-color-brand-border-normal: var(--wa-color-brand);
    --wa-color-brand-border-loud: var(--wa-color-brand);

    /* Buttons keep their press feedback; controls.ts translates the brand
       button on top of this. */
    --wa-button-transform-hover: none;
    --wa-button-transform-active: scale(0.9875);

    /* Form controls — the family whose absence left every input, select and
       switch with no chrome and every button with no padding. Upstream's own
       proportions, in em, so they track the base size rather than fighting it. */
    --wa-form-control-background-color: var(--wa-color-surface);
    --wa-form-control-border-color: var(--wa-color-border);
    --wa-form-control-border-style: var(--wa-border-style);
    --wa-form-control-border-width: var(--wa-border-width-s);
    --wa-form-control-border-radius: var(--wa-radius-sm);
    --wa-form-control-activated-color: var(--wa-color-brand);

    --wa-form-control-label-color: var(--wa-color-text);
    --wa-form-control-label-font-weight: var(--wa-font-weight-medium);
    --wa-form-control-label-line-height: var(--wa-line-height-condensed);

    --wa-form-control-value-color: var(--wa-color-text);
    --wa-form-control-value-font-size: var(--wa-font-size-base);
    --wa-form-control-value-font-weight: var(--wa-font-weight-normal);
    --wa-form-control-value-line-height: var(--wa-line-height-condensed);

    --wa-form-control-hint-color: var(--wa-color-muted);
    --wa-form-control-hint-font-weight: var(--wa-font-weight-normal);
    --wa-form-control-hint-line-height: var(--wa-line-height);

    --wa-form-control-placeholder-color: var(--wa-color-muted);

    --wa-form-control-required-content: '*';
    --wa-form-control-required-content-color: var(--wa-color-danger);
    --wa-form-control-required-content-offset: 0.1em;

    --wa-form-control-padding-block: 0.5em;
    --wa-form-control-padding-inline: 0.875em;
    --wa-form-control-height: calc(
      2 * var(--wa-form-control-padding-block) + 1em *
        var(--wa-form-control-value-line-height)
    );
    --wa-form-control-toggle-size: 1.25em;
  }
`;
