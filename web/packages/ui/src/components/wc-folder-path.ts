import { LitElement, html, css } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';

/**
 * A folder on disk, set as selectable code, with whatever can be done about it
 * slotted underneath as buttons.
 */
@customElement('wc-folder-path')
export class WcFolderPath extends LitElement {
  static styles = css`
    :host {
      display: grid;
      gap: var(--wa-space-m);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    code {
      display: block;
      padding: var(--wa-space-s) var(--wa-space-m);
      border: 1px solid var(--wa-color-border-soft);
      border-radius: var(--wa-radius-md);
      background: var(--wa-color-surface-alt);
      font-family: var(--wa-font-family-mono);
      font-size: var(--nc-font-size-control);
      overflow-wrap: anywhere;
      user-select: text;
    }

    .actions {
      display: flex;
      flex-wrap: wrap;
      gap: var(--wa-space-s);
    }

    .actions.empty {
      display: none;
    }
  `;

  @property({ type: String })
  path = '';

  @state()
  private empty = true;

  private onSlotChange = (event: Event) => {
    const slot = event.target as HTMLSlotElement;
    this.empty = slot.assignedElements().length === 0;
  };

  render() {
    return html`
      <code>${this.path}</code>
      <div class=${this.empty ? 'actions empty' : 'actions'}>
        <slot @slotchange=${this.onSlotChange}></slot>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-folder-path': WcFolderPath;
  }
}
