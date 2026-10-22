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
  /* The standard button. Every wa-button in the app is the refresh's pill
     (docs/design/app-refresh/README.md, "Radius": 999px for pills, buttons
     and switches), and this sheet is the only place that says so: a guard
     test in each package fails the build when a component sets a button's
     radius, height or fill itself.

     Size, shape and type ride the custom properties Web Awesome's own
     .button rule reads, set on the host so they inherit into the shadow
     root. A declaration here, in the tree that hosts the button, wins over
     the component's :host([size]) rules for the same property whatever the
     specificity, which is what keeps the label at the 13px control size at
     every size. The host carries the radius too, so the focus outline drawn
     on it follows the pill. */
  wa-button {
    --wa-form-control-height: var(--nc-button-height);
    --wa-form-control-border-radius: var(--wa-radius-pill);
    --wa-form-control-padding-inline: var(--nc-button-padding-inline);
    --wa-font-weight-action: var(--wa-font-weight-normal);
    border-radius: var(--wa-radius-pill);
    font-size: var(--nc-font-size-control);
  }

  wa-button[size='l'] {
    --wa-form-control-height: var(--nc-button-height-lg);
  }

  wa-button[size='s'] {
    --wa-form-control-height: var(--nc-button-height-sm);
    --wa-form-control-padding-inline: var(--nc-button-padding-inline-sm);
  }

  /* A filled button — the brand primary, the destructive confirm — sets its
     label in 500; an outline sits at the body weight beside it. */
  wa-button:is([variant='brand'], [variant='danger']):is(:not([appearance]), [appearance='accent']) {
    --wa-font-weight-action: var(--wa-font-weight-medium);
  }

  /* Neutral is the design's secondary: an outline in the border colour on a
     card's ground, whether or not the author asked for appearance=outlined.
     Plain stays bare text. An outer-tree part rule beats the shadow tree's
     own for the same property, Web Awesome's hover included, so the hover
     fill is stated here as well. */
  wa-button:is(:not([variant]), [variant='neutral']):not([appearance='plain'])::part(base) {
    background-color: var(--wa-color-surface);
    border-color: var(--wa-color-border);
    color: var(--wa-color-text);
  }

  wa-button:is(:not([variant]), [variant='neutral']):not([disabled], [loading]):hover::part(base) {
    background-color: var(--nc-color-hover);
  }

  /* The brand primary is a flat --wa-color-brand fill under
     --wa-color-on-brand text: Web Awesome's accent appearance read through
     the brand family in wa-contract.ts. Only its hover is stated here. */
  wa-button[variant='brand']:is(:not([appearance]), [appearance='accent']):not(
      [disabled],
      [loading]
    ):hover::part(base) {
    background-color: var(--wa-color-brand-hover);
  }

  /* Danger needs no rule of its own. appearance=outlined is the outline in
     --wa-color-danger with danger text; the default accent appearance is the
     filled destructive confirm, --wa-color-danger under on-brand text. Both
     come from the danger family in wa-contract.ts. */

  /* Disabled is 40% and refuses hover: every hover rule above excludes it,
     and Web Awesome's own exclude its .disabled class. */
  wa-button[disabled] {
    opacity: 0.4;
  }

  /* Text for assistive technology alone. A wa-button does not pass its
     host's aria-label to the button inside it, so a name longer than the
     visible label goes in the content: Retry<span class="sr-only"> Revenue</span>. */
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
    border: 0;
  }

  /* An icon-only button is a circle the button's height across. Its icon is
     the only thing in the default slot and carries the accessible name as
     its label — a button with text leaves its icon decorative or puts it in
     the start slot — so a labelled icon there is what marks one. */
  wa-button:has(> [label]:not([slot])) {
    --wa-form-control-padding-inline: 0;
    --nc-icon-size: var(--nc-button-icon-size);
    width: var(--wa-form-control-height);
  }

  wa-button[appearance='plain']:has(> [label]:not([slot]))::part(base) {
    color: var(--wa-color-muted);
  }

  /* Forced colors drops every fill, so each button keeps an edge, and the
     focus outline is repainted in the system colour rather than lost. */
  @media (forced-colors: active) {
    wa-button::part(base) {
      border-color: ButtonBorder;
    }

    wa-button::part(base):focus-visible {
      outline: 2px solid Highlight;
      outline-offset: 2px;
    }
  }

  /* The standard switch, drawn here and nowhere else (the same guard test
     covers it): a 38×22 pill track and a 16px white knob. Off is a light
     track with a mid-grey edge, and the knob carries the same edge, both
     held to 3:1 by contrast.test.ts. On fills the track with the brand and
     puts a check from the icon set in the knob, so the state never rests on
     colour alone. The knob's slide rides the track's transition, which Web
     Awesome passes down with transition: inherit. */
  wa-switch {
    --width: var(--nc-switch-width);
    --height: var(--nc-switch-height);
    --thumb-size: var(--nc-switch-knob-size);
  }

  wa-switch::part(control) {
    box-sizing: border-box;
    border: 1px solid var(--nc-color-switch-border);
    border-radius: var(--wa-radius-pill);
    background-color: var(--wa-color-surface-alt);
    transition:
      translate var(--nc-transition-switch),
      background-color var(--nc-transition-switch),
      border-color var(--nc-transition-switch);
  }

  wa-switch::part(thumb) {
    box-sizing: border-box;
    border: 1px solid var(--nc-color-switch-border);
    background-color: var(--nc-color-switch-knob);
  }

  wa-switch:state(checked)::part(control) {
    border-color: var(--wa-color-brand);
    background-color: var(--wa-color-brand);
  }

  wa-switch:state(checked)::part(thumb) {
    border-color: var(--nc-color-switch-check);
    background-image: var(--nc-switch-check-image);
    background-position: center;
    background-repeat: no-repeat;
    background-size: 10px;
  }

  /* 40%, on the part Web Awesome dims itself so the two do not multiply. */
  wa-switch[disabled]::part(base) {
    opacity: 0.4;
  }

  /* Forced colors drops the fills, so the switch is redrawn in system
     colours: both edges in ButtonText, the knob filled ButtonText when on
     (the brand-inked check would be the one author colour left), and the
     knob's position and fill say which state it is in. The knob keeps a
     focus outline in Highlight. */
  @media (forced-colors: active) {
    wa-switch::part(control),
    wa-switch::part(thumb) {
      border-color: ButtonText;
    }

    wa-switch:state(checked)::part(thumb) {
      background-color: ButtonText;
      background-image: none;
    }

    wa-switch:focus-within::part(thumb) {
      outline: 2px solid Highlight;
      outline-offset: 2px;
    }
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

  /* Buttons, links and anything else that takes the keyboard: a brand
     outline clear of the edge, on keyboard focus only. A wa-button draws the
     same ring on its own base part from the --wa-focus-ring tokens. */
  :focus-visible {
    outline: 2px solid var(--wa-color-focus);
    outline-offset: 2px;
  }

  /* A field someone is typing in is ringed rather than outlined: its own
     border turns brand and a translucent brand halo sits outside it. That is
     on any focus, pointer included, because a caret in a field is the thing
     being worked on. Checkboxes, radios and file pickers are controls, not
     fields — as are range and colour inputs — and keep the outline above.

     The outline is transparent rather than none: forced-colors mode drops
     the halo and repaints the outline in the system colour, so it is the
     focus indicator a high-contrast user still sees.

     Web Awesome draws the field's chrome inside its own shadow root, so the
     same ring reaches it through the part that carries the border; the
     outline WA puts on that part is made transparent the same way, or a
     field would wear both. */
  input:not([type='checkbox'], [type='radio'], [type='file'], [type='range'], [type='color']):focus,
  select:focus,
  textarea:focus {
    border-color: var(--wa-color-brand);
    box-shadow: 0 0 0 3px var(--nc-color-ring);
    outline: 2px solid transparent;
    outline-offset: 0;
  }

  wa-input:focus-within::part(base),
  wa-textarea:focus-within::part(base),
  wa-select:focus-within::part(combobox),
  wa-select[open]::part(combobox) {
    border-color: var(--wa-color-brand);
    box-shadow: 0 0 0 3px var(--nc-color-ring);
    outline-color: transparent;
  }

  @media (forced-colors: active) {
    input:not([type='checkbox'], [type='radio'], [type='file'], [type='range'], [type='color']):focus,
    select:focus,
    textarea:focus,
    wa-input:focus-within::part(base),
    wa-textarea:focus-within::part(base),
    wa-select:focus-within::part(combobox),
    wa-select[open]::part(combobox) {
      outline: 2px solid Highlight;
    }
  }

  /* The document turns off user-select so dragging the desktop shell's chrome
     does not paint a selection, and that choice inherits through every shadow
     boundary underneath it — including into a field someone is typing in. A
     plain selector reaches a native input written directly in the adopting
     component's own shadow root; wa-input needs the host element named here,
     because the selection actually happens one boundary further in, inside
     its own shadow root, and an inherited property is read from the host's
     computed value rather than needing a part of its own. */
  input,
  textarea,
  [contenteditable],
  wa-input {
    user-select: text;
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
