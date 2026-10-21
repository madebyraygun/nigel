import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '../icons/icons.js';

export interface NavItem {
  id: string;
  label: string;
  /** Tag name of a `wc-icon-*` element. */
  icon?: string;
  disabled?: boolean;
}

/**
 * Primary navigation. Presentational: it renders the items it is given and
 * announces intent. Which items exist and which one is active is the app's
 * screen registry's business.
 */
@customElement('wc-nav-sidebar')
export class WcNavSidebar extends LitElement {
  static styles = css`
    :host {
      display: block;
      width: var(--nc-sidebar-width);
      flex-shrink: 0;
      background: var(--nc-color-sidebar-bg);
      border-right: 1px solid var(--wa-color-border);
      font-family: var(--wa-font-family-sans);
      overflow-y: auto;
    }

    :host([collapsed]) {
      width: var(--nc-sidebar-collapsed-width);
    }

    .brand {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s);
      height: var(--nc-header-height);
      padding: 0 var(--wa-space-m);
      border-bottom: 1px solid var(--wa-color-border);
      box-sizing: border-box;
    }

    .brand-name {
      font-weight: var(--wa-font-weight-bold);
      font-size: var(--wa-font-size-lg);
      background: var(--nc-grad-brand-text);
      -webkit-background-clip: text;
      background-clip: text;
      color: transparent;
    }

    :host([collapsed]) .brand-name {
      display: none;
    }

    ul {
      list-style: none;
      margin: 0;
      padding: var(--wa-space-s);
      display: grid;
      gap: 2px;
    }

    button {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s);
      width: 100%;
      padding: 8px 10px;
      border: 0;
      border-radius: var(--wa-radius-sm);
      background: transparent;
      color: var(--wa-color-text);
      font: inherit;
      font-size: var(--wa-font-size-base);
      text-align: left;
      cursor: default;
      transition: background var(--nc-transition-fast);
    }

    button:hover:not(.disabled) {
      background: var(--wa-color-surface-alt);
    }

    button.active {
      background: var(--nc-color-selected-bg);
      font-weight: var(--wa-font-weight-medium);
    }

    button.disabled {
      opacity: 0.4;
      cursor: not-allowed;
    }

    :host([collapsed]) .label {
      display: none;
    }

    :host([collapsed]) button {
      justify-content: center;
      padding: 8px 0;
    }

    /* At phone width the shell slides the whole sidebar off-canvas instead of
       narrowing it, so collapsed means "away" rather than "56px of icons"
       and the rail styling stands down: what slides back in is the full nav
       with its words. */
    @media (max-width: 48rem) {
      :host([collapsed]) {
        width: var(--nc-sidebar-width);
      }

      :host([collapsed]) .brand-name,
      :host([collapsed]) .label {
        display: revert;
      }

      :host([collapsed]) button {
        justify-content: revert;
        padding: 8px 10px;
      }
    }

    /* Screen chrome, not part of the report. This lives here rather than in
       @nigel/theme's print sheet because a rule that hides an element has to
       be in the tree that element is in, and every wc-* here sits inside
       nigel-app's shadow root where a document rule cannot reach it. */
    @media print {
      :host {
        display: none;
      }
    }
  `;

  @property({ attribute: false })
  items: NavItem[] = [];

  @property({ type: String })
  active = '';

  @property({ type: Boolean, reflect: true })
  collapsed = false;

  @property({ type: String, attribute: 'app-name' })
  appName = 'Nigel';

  private handleClick(item: NavItem): void {
    if (item.disabled) return;
    this.dispatchEvent(
      new CustomEvent<{ id: string }>('nc-navigate', {
        detail: { id: item.id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private renderIcon(item: NavItem) {
    if (!item.icon) return nothing;
    // Icon tags come from data, so the element is created imperatively.
    return document.createElement(item.icon);
  }

  render() {
    return html`
      <div class="brand">
        <span class="brand-name">${this.appName}</span>
      </div>
      <nav aria-label="Primary">
        <ul>
          ${this.items.map((item) => {
            const isActive = item.id === this.active;
            const classes = [
              isActive ? 'active' : '',
              item.disabled ? 'disabled' : '',
            ]
              .filter(Boolean)
              .join(' ');
            return html`
              <li>
                <button
                  type="button"
                  class=${classes}
                  data-nav=${item.id}
                  aria-current=${isActive ? 'page' : 'false'}
                  aria-disabled=${item.disabled ? 'true' : 'false'}
                  title=${this.collapsed ? item.label : nothing}
                  @click=${() => this.handleClick(item)}
                >
                  ${this.renderIcon(item)}
                  <span class="label">${item.label}</span>
                </button>
              </li>
            `;
          })}
        </ul>
      </nav>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-nav-sidebar': WcNavSidebar;
  }
  interface HTMLElementEventMap {
    'nc-navigate': CustomEvent<{ id: string }>;
  }
}
