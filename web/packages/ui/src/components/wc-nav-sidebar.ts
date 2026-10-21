import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { controlsCss } from '@nigel/theme';
import '../icons/icons.js';

export interface NavItem {
  id: string;
  label: string;
  /** Tag name of a `wc-icon-*` element. */
  icon?: string;
  disabled?: boolean;
  /**
   * The heading of the group the item sits in. Consecutive items with the
   * same group are drawn together; items with none form the unlabelled group
   * at the top.
   */
  group?: string;
  /** A figure drawn at the item's right edge — Review's queue. Hidden at 0. */
  count?: number;
  /** Drawn in the footer, below every group, rather than in the list. */
  pinned?: boolean;
}

interface NavGroup {
  label?: string;
  items: NavItem[];
}

function groupsOf(items: NavItem[]): NavGroup[] {
  const groups: NavGroup[] = [];
  for (const item of items) {
    const current = groups[groups.length - 1];
    if (current && current.label === item.group) current.items.push(item);
    else groups.push({ label: item.group, items: [item] });
  }
  return groups;
}

/**
 * Primary navigation: the floating sidebar panel.
 *
 * Presentational: it renders the items it is given and announces intent.
 * Which items exist, which one is active and whether the panel is folded to
 * its rail are the app's business. The hamburger in its header asks for the
 * other state with `nc-sidebar-toggle`, the same request the shell's own
 * toggle makes.
 */
@customElement('wc-nav-sidebar')
export class WcNavSidebar extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: flex;
        flex-direction: column;
        flex: none;
        box-sizing: border-box;
        width: var(--nc-sidebar-width);
        margin: var(--nc-window-inset) 0 var(--nc-window-inset) var(--nc-window-inset);
        background: var(--nc-color-sidebar-bg);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-xl);
        box-shadow: var(--nc-shadow-floating);
        overflow: hidden;
        font-family: var(--wa-font-family-sans);
        transition: width var(--nc-transition-sidebar);
      }

      :host([collapsed]) {
        width: var(--nc-sidebar-collapsed-width);
      }

      .head {
        display: flex;
        align-items: center;
        gap: var(--wa-space-s);
        flex: none;
        height: var(--nc-header-height);
        padding: 0 10px;
        box-sizing: border-box;
      }

      .hamburger {
        display: inline-grid;
        place-items: center;
        flex: none;
        width: 32px;
        height: 32px;
        padding: 0;
        border: 0;
        border-radius: var(--wa-radius-md);
        background: transparent;
        color: var(--wa-color-text);
        cursor: default;
        transition: background var(--nc-transition-fast);
      }

      .hamburger:hover {
        background: var(--nc-color-hover);
      }

      .wordmark {
        font-size: 15px;
        font-weight: var(--wa-font-weight-bold);
        letter-spacing: -0.01em;
        background: var(--nc-grad-brand-text);
        -webkit-background-clip: text;
        background-clip: text;
        color: transparent;
        white-space: nowrap;
      }

      nav {
        display: flex;
        flex-direction: column;
        flex: 1;
        min-height: 0;
      }

      .groups {
        display: flex;
        flex-direction: column;
        gap: 10px;
        flex: 1;
        min-height: 0;
        /* 4px of room above the first item for its focus outline, which
           the scroll box would otherwise clip, taken back by the margin. */
        margin-top: -4px;
        padding: 4px var(--wa-space-s) var(--wa-space-s);
        overflow-x: hidden;
        overflow-y: auto;
      }

      .group-label {
        margin: 0;
        padding: 4px 10px 3px;
        font-size: var(--nc-font-size-label);
        font-weight: var(--wa-font-weight-medium);
        letter-spacing: var(--nc-letter-spacing-label);
        text-transform: uppercase;
        color: var(--wa-color-muted);
        white-space: nowrap;
      }

      ul {
        display: grid;
        gap: 1px;
        margin: 0;
        padding: 0;
        list-style: none;
      }

      .footer {
        flex: none;
        padding: var(--wa-space-s);
        border-top: 1px solid var(--wa-color-border-soft);
      }

      .item {
        display: flex;
        align-items: center;
        gap: 10px;
        width: 100%;
        height: 32px;
        padding: 0 9px;
        box-sizing: border-box;
        border: 1px solid transparent;
        border-radius: var(--wa-radius-md);
        background: transparent;
        color: var(--wa-color-text);
        font: inherit;
        font-size: var(--nc-font-size-body);
        text-align: left;
        white-space: nowrap;
        cursor: default;
        transition: background var(--nc-transition-fast);
        --nc-icon-size: 17px;
      }

      .item:hover:not(.disabled, .active) {
        background: var(--nc-color-hover);
      }

      /* The rainbow hairline: the fill painted to the padding edge and the
         ramp to the border edge, so the transparent 1px border shows the
         ramp and nothing else does. */
      .item.active {
        background:
          linear-gradient(var(--nc-color-selected-bg), var(--nc-color-selected-bg)) padding-box,
          var(--nc-grad-brand-text) border-box;
        font-weight: var(--wa-font-weight-medium);
      }

      .item.disabled {
        opacity: 0.4;
        cursor: not-allowed;
      }

      .label {
        flex: 1;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
      }

      .count {
        font-size: 11px;
        color: var(--nc-color-flagged);
      }

      :host([collapsed]) .wordmark,
      :host([collapsed]) .group-label,
      :host([collapsed]) .label,
      :host([collapsed]) .count {
        display: none;
      }

      :host([collapsed]) .item {
        justify-content: center;
        padding: 0;
      }

      @media (prefers-reduced-motion: reduce) {
        :host {
          transition: none;
        }
      }

      /* At phone width the shell slides the whole sidebar off-canvas instead
         of narrowing it, so collapsed means "away" rather than "58px of
         icons" and the rail styling stands down: what slides back in is the
         full nav with its words. */
      @media (max-width: 48rem) {
        :host([collapsed]) {
          width: var(--nc-sidebar-width);
        }

        :host([collapsed]) .wordmark,
        :host([collapsed]) .group-label,
        :host([collapsed]) .label,
        :host([collapsed]) .count {
          display: revert;
        }

        :host([collapsed]) .item {
          justify-content: revert;
          padding: 0 9px;
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
    `,
  ];

  @property({ attribute: false })
  items: NavItem[] = [];

  @property({ type: String })
  active = '';

  @property({ type: Boolean, reflect: true })
  collapsed = false;

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

  private requestToggle(): void {
    this.dispatchEvent(
      new CustomEvent<{ collapsed: boolean }>('nc-sidebar-toggle', {
        detail: { collapsed: !this.collapsed },
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

  private renderItem(item: NavItem) {
    const isActive = item.id === this.active;
    const classes = ['item', isActive ? 'active' : '', item.disabled ? 'disabled' : '']
      .filter(Boolean)
      .join(' ');
    const count = item.count && item.count > 0 ? item.count : 0;
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
          ${count ? html`<span class="count">${count}</span>` : nothing}
        </button>
      </li>
    `;
  }

  render() {
    const listed = this.items.filter((item) => !item.pinned);
    const pinned = this.items.filter((item) => item.pinned);
    return html`
      <div class="head">
        <button
          type="button"
          class="hamburger"
          aria-label="Sidebar"
          aria-expanded=${this.collapsed ? 'false' : 'true'}
          @click=${this.requestToggle}
        >
          <wc-icon-menu></wc-icon-menu>
        </button>
        <span class="wordmark" aria-hidden="true">nigel</span>
      </div>
      <nav aria-label="Primary">
        <div class="groups">
          ${groupsOf(listed).map(
            (group, index) => html`
              <div class="group">
                ${group.label
                  ? html`<p class="group-label" id="group-${index}">${group.label}</p>`
                  : nothing}
                <ul aria-labelledby=${group.label ? `group-${index}` : nothing}>
                  ${group.items.map((item) => this.renderItem(item))}
                </ul>
              </div>
            `,
          )}
        </div>
        ${pinned.length
          ? html`<div class="footer">
              <ul>
                ${pinned.map((item) => this.renderItem(item))}
              </ul>
            </div>`
          : nothing}
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
    'nc-sidebar-toggle': CustomEvent<{ collapsed: boolean }>;
  }
}
