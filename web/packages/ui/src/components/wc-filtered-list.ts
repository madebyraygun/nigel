import { LitElement, html, css } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

/**
 * A list with its filter row: the row on top, the table under it, and
 * `--nc-toolbar-gap` between them — the same gap `wc-manager-layout` puts
 * under its toolbar, so every list screen spaces its filters alike.
 *
 * Everything with `slot="filters"` sits on one wrapping row, vertically
 * centred: a `.label` span ("Status") before its chips, a `wa-select` whose
 * label `controlsCss` sets inline to its left. The default slot is the list.
 * With nothing in the filters slot the row and its gap go away.
 */
@customElement('wc-filtered-list')
export class WcFilteredList extends LitElement {
  static styles = css`
    :host {
      display: flex;
      flex-direction: column;
      gap: var(--nc-toolbar-gap);
      min-width: 0;
    }

    .filters {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--nc-filter-row-gap);
      font-size: var(--nc-font-size-control);
    }

    .filters[hidden] {
      display: none;
    }

    ::slotted(.label) {
      color: var(--wa-color-muted);
    }
  `;

  /** The accessible name of the filter row. */
  @property({ type: String })
  label = 'Filters';

  @state()
  private hasFilters = false;

  private handleSlotChange(event: Event): void {
    const slot = event.target as HTMLSlotElement;
    this.hasFilters = slot.assignedElements().length > 0;
  }

  render() {
    return html`
      <div class="filters" part="filters" role="group" aria-label=${this.label} ?hidden=${!this.hasFilters}>
        <slot name="filters" @slotchange=${this.handleSlotChange}></slot>
      </div>
      <slot></slot>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-filtered-list': WcFilteredList;
  }
}
