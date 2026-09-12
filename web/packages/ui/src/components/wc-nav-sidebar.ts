import { LitElement, html, css, nothing } from 'lit';
import type { PropertyValues } from 'lit';
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
      width: var(--nc-sidebar-width, 232px);
      flex-shrink: 0;
      background: var(--nc-color-sidebar-bg, var(--wa-color-surface));
      border-right: 1px solid var(--wa-color-border);
      font-family: var(--wa-font-family-sans);
      overflow-y: auto;
      /* Clipping is what turns the width change into a wipe, in both
         directions: a label keeps its layout and the moving edge crops it.
         overflow-y: auto on its own computes overflow-x to auto, which is a
         scrollbar for the length of the animation. */
      overflow-x: hidden;
    }

    /* Only a gesture animates: toggling sets data-animating for the
       transition's lifetime, so a window resize — including one across the
       48rem breakpoint, in either direction — changes width in one frame. */
    :host([data-animating]) {
      transition: width var(--nc-transition-base, 200ms ease);
    }

    @media (prefers-reduced-motion: reduce) {
      :host([data-animating]) {
        transition: none;
      }
    }

    :host([collapsed]) {
      width: var(--nc-sidebar-collapsed-width, 56px);
    }

    /* Each row holds its line, so a label reaching past the box is clipped to
       an ellipsis rather than wrapping onto a second line — which at 56px
       would make the rail grow taller before it grew wider. The full text
       stays reachable as the button's title in both states. */
    .brand,
    button {
      white-space: nowrap;
    }

    .label {
      overflow: hidden;
      text-overflow: ellipsis;
      min-width: 0;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      height: var(--nc-header-height, 48px);
      padding: 0 var(--wa-space-m, 12px);
      border-bottom: 1px solid var(--wa-color-border);
      box-sizing: border-box;
    }

    .brand-name {
      overflow: hidden;
      text-overflow: ellipsis;
      min-width: 0;
      font-family: var(--nc-font-brand);
      font-weight: var(--wa-font-weight-bold, 600);
      font-size: var(--wa-font-size-lg, 16px);
      background: var(--nc-grad-brand-text, var(--nc-grad-brand));
      -webkit-background-clip: text;
      background-clip: text;
      color: transparent;
    }

    ul {
      list-style: none;
      margin: 0;
      padding: var(--wa-space-s, 8px);
      display: grid;
      gap: 2px;
    }

    button {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      width: 100%;
      padding: 8px 10px;
      border: 0;
      border-radius: var(--wa-radius-sm, 6px);
      background: transparent;
      color: var(--wa-color-text);
      font: inherit;
      font-size: var(--wa-font-size-base, 14px);
      text-align: left;
      cursor: default;
      transition: background var(--nc-transition-fast, 120ms ease);
    }

    /* The icon holds its size; the label is what gives way, so the crop
       lands on text rather than squeezing the glyph. */
    button > :not(.label) {
      flex-shrink: 0;
    }

    button:hover:not(.disabled) {
      background: var(--wa-color-surface-alt);
    }

    button.active {
      background: var(--nc-color-selected-bg);
      font-weight: var(--wa-font-weight-medium, 500);
    }

    button.disabled {
      opacity: 0.4;
      cursor: not-allowed;
    }

    /* At phone width the shell slides the whole sidebar off-canvas instead of
       narrowing it, so collapsed means "away" rather than "56px of icons"
       and the rail styling stands down: what slides back in is the full nav
       with its words. */
    @media (max-width: 48rem) {
      /* The rail does not exist at this width: collapsed means the shell has
         slid the whole sidebar off-canvas with transform from the outer
         tree, and what slides back in is the full column. */
      :host([data-animating]) {
        transition: none;
      }

      :host([collapsed]) {
        width: var(--nc-sidebar-width, 232px);
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

  private animationDone = 0;

  protected updated(changed: PropertyValues<this>): void {
    // A toggle is the only thing that changes collapsed after first render,
    // so the attribute marks a gesture; the timeout stands in for the
    // transitionend that reduced motion never fires.
    if (!changed.has('collapsed') || changed.get('collapsed') === undefined) return;
    this.setAttribute('data-animating', '');
    window.clearTimeout(this.animationDone);
    this.animationDone = window.setTimeout(() => this.removeAttribute('data-animating'), 300);
  }

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
                  title=${item.label}
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
