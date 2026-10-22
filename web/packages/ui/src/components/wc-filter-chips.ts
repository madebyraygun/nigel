import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/** One chip: where it goes, what it says, and how many rows it would show. */
export interface FilterChip {
  value: string;
  label: string;
  href: string;
  /** Omitted draws no count — a chip that clears something has none to give. */
  count?: number;
}

/**
 * A labelled row of filter chips, each a link with an optional count.
 *
 * Links rather than buttons because a filter here navigates: a filtered list
 * is a URL somebody can keep, and the route — not this component — decides
 * what is shown. The current chip is the one whose `value` matches `active`,
 * and it says so with `aria-current` as well as with colour.
 */
@customElement('wc-filter-chips')
export class WcFilterChips extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--nc-filter-row-gap);
      font-size: var(--nc-font-size-control);
    }

    .label {
      margin-right: 2px;
      color: var(--wa-color-muted);
    }

    a {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      box-sizing: border-box;
      height: 28px;
      padding: 0 12px;
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-pill);
      background: var(--wa-color-surface);
      color: inherit;
      text-decoration: none;
      white-space: nowrap;
    }

    a:hover {
      background: var(--nc-color-hover);
    }

    a[aria-current='true'] {
      border-color: var(--wa-color-brand);
      background: var(--nc-color-highlight);
      color: var(--wa-color-brand);
      font-weight: var(--wa-font-weight-medium);
    }

    a:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    .count {
      color: var(--wa-color-muted);
      font-weight: var(--wa-font-weight-normal);
    }
  `;

  /** The word before the chips, which also names the group for a screen reader. */
  @property({ type: String })
  label = '';

  @property({ attribute: false })
  chips: FilterChip[] = [];

  /** The `value` of the chip in effect. */
  @property({ type: String })
  active = '';

  render() {
    return html`
      <div class="row" role="group" aria-labelledby=${this.label ? 'label' : nothing}>
        ${this.label ? html`<span class="label" id="label">${this.label}</span>` : nothing}
        ${this.chips.map((chip) => {
          const current = chip.value === this.active;
          return html`<a
            data-filter=${chip.value}
            href=${chip.href}
            aria-current=${current ? 'true' : nothing}
            >${chip.label}${chip.count === undefined
              ? nothing
              : html` <span class="count">${chip.count}</span>`}</a
          >`;
        })}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-filter-chips': WcFilterChips;
  }
}
