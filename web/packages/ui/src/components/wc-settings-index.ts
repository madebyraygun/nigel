import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { controlsCss } from '@nigel/theme';

export interface SettingsIndexItem {
  id: string;
  label: string;
}

export interface SettingsIndexGroup {
  label: string;
  items: SettingsIndexItem[];
}

export interface NcSettingsSectionDetail {
  id: string;
}

/**
 * The Settings page's left column: its heading over an index of the sections,
 * grouped by what they apply to. It sticks to the top of the scrolling area
 * while the sections scroll past.
 *
 * Which item is active is the screen's to say: it follows the scroll position
 * there, and a click here only asks for a section with `nc-settings-section`.
 * Items are buttons rather than `#` links because the app routes on the hash.
 */
@customElement('wc-settings-index')
export class WcSettingsIndex extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        position: sticky;
        top: 0;
        align-self: start;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      h1 {
        margin: 0 0 var(--wa-space-l);
        font-size: var(--nc-font-size-display);
        font-weight: var(--wa-font-weight-medium);
        letter-spacing: var(--nc-letter-spacing-tight);
        line-height: 1.2;
      }

      .group + .group {
        margin-top: var(--wa-space-l);
      }

      .group-label {
        margin: 0;
        padding: 4px 10px 3px;
        font-size: var(--nc-font-size-label);
        font-weight: var(--wa-font-weight-medium);
        letter-spacing: var(--nc-letter-spacing-label);
        text-transform: uppercase;
        color: var(--wa-color-muted);
      }

      ul {
        list-style: none;
        margin: 0;
        padding: 0;
        display: grid;
        gap: 1px;
      }

      button {
        position: relative;
        display: flex;
        align-items: center;
        width: 100%;
        height: 30px;
        padding: 0 10px 0 14px;
        border: 0;
        border-radius: var(--wa-radius-md);
        background: transparent;
        color: var(--wa-color-muted);
        font: inherit;
        font-size: var(--nc-font-size-control);
        text-align: left;
        cursor: default;
        transition:
          background var(--nc-transition-fast),
          color var(--nc-transition-fast);
      }

      button:hover {
        background: var(--nc-color-hover);
        color: var(--wa-color-text);
      }

      button[aria-current='true'] {
        background: var(--wa-color-surface-alt);
        color: var(--wa-color-text);
      }

      /* The active item's tick: the rainbow run top to bottom, drawn
         across and turned upright since the ramp itself runs sideways. */
      button[aria-current='true']::before {
        content: '';
        position: absolute;
        left: 0.5px;
        top: 13.5px;
        width: 14px;
        height: 3px;
        border-radius: var(--wa-radius-pill);
        background: var(--nc-grad-brand-text);
        transform: rotate(90deg);
      }

      /* One column on a phone: the index stops sticking and sits above the
         sections it lists. */
      @media (max-width: 48rem) {
        :host {
          position: static;
        }
      }

      @media print {
        nav {
          display: none;
        }
      }
    `,
  ];

  /** The page heading drawn above the index. */
  @property({ type: String })
  heading = 'Settings';

  @property({ attribute: false })
  groups: SettingsIndexGroup[] = [];

  /** The id of the section the reader is in. */
  @property({ type: String })
  active = '';

  private select(id: string): void {
    this.dispatchEvent(
      new CustomEvent<NcSettingsSectionDetail>('nc-settings-section', {
        detail: { id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  render() {
    return html`
      <h1>${this.heading}</h1>
      <nav aria-label="${this.heading} sections">
        ${this.groups.map(
          (group, index) => html`
            <div class="group">
              <p class="group-label" id="group-${index}">${group.label}</p>
              <ul aria-labelledby="group-${index}">
                ${group.items.map(
                  (item) => html`<li>
                    <button
                      type="button"
                      aria-current=${item.id === this.active ? 'true' : 'false'}
                      @click=${() => this.select(item.id)}
                    >
                      ${item.label}
                    </button>
                  </li>`,
                )}
              </ul>
            </div>
          `,
        )}
      </nav>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-settings-index': WcSettingsIndex;
  }
  interface HTMLElementEventMap {
    'nc-settings-section': CustomEvent<NcSettingsSectionDetail>;
  }
}
