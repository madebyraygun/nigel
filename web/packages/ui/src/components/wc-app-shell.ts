import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-toast.js';
import '../icons/icons.js';

/** The width at which the sidebar stops being a column and becomes a drawer. */
export const NARROW_QUERY = '(max-width: 48rem)';

/**
 * Enough of a `MediaQueryList` to answer "is this a phone" and say when that
 * changes. Injectable because jsdom's `matchMedia` answers false to
 * everything, the way `@nigel/theme`'s dark-mode query is.
 */
export interface NarrowQuery {
  matches: boolean;
  addEventListener?(type: 'change', listener: () => void): void;
  removeEventListener?(type: 'change', listener: () => void): void;
}

function defaultNarrowQuery(): NarrowQuery | undefined {
  return typeof matchMedia === 'undefined' ? undefined : matchMedia(NARROW_QUERY);
}

/**
 * Whether the sidebar should start out of the way. The app owns the state but
 * not the breakpoint, so it asks here rather than naming the width again.
 */
export function narrowViewport(query: NarrowQuery | undefined = defaultNarrowQuery()): boolean {
  return query?.matches ?? false;
}

/**
 * The application frame: sidebar rail, header, banner slot, content area, and
 * the single toast region.
 *
 * Purely structural. Unlike boxcraft's app-shell it does not decide what to
 * render — nigel routes from a screen registry in the app, so the shell only
 * provides the slots and lets the container fill them.
 */
@customElement('wc-app-shell')
export class WcAppShell extends LitElement {
  static styles = css`
    :host {
      display: flex;
      height: 100vh;
      background: var(--wa-color-bg);
      color: var(--wa-color-text);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-base);
      line-height: var(--wa-line-height);
    }

    .main {
      flex: 1;
      display: flex;
      flex-direction: column;
      overflow: hidden;
      min-width: 0;
    }

    header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: var(--wa-space-m, 12px);
      min-height: var(--nc-header-height, 48px);
      padding: 0 var(--wa-space-l, 16px);
      background: var(--wa-color-surface);
      border-bottom: 1px solid var(--wa-color-border);
      box-sizing: border-box;
    }

    .title {
      font-size: var(--wa-font-size-lg, 16px);
      font-weight: var(--wa-font-weight-medium, 500);
      margin: 0;
    }

    .heading {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
      min-width: 0;
    }

    .nav-toggle {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
      width: 32px;
      height: 32px;
      padding: 0;
      border: 0;
      border-radius: var(--wa-radius-sm, 6px);
      background: transparent;
      color: var(--wa-color-text);
      cursor: pointer;
      transition: background var(--nc-transition-fast, 120ms ease);
    }

    .nav-toggle:hover {
      background: var(--wa-color-surface-alt);
    }

    /* Below the breakpoint the sidebar leaves the flow and lies over the
       content, so the backdrop is what says the rest of the app is not the
       thing being interacted with. Escape and the toggle close the drawer
       too, which is what keeps a pointer-only affordance from being the only
       way out. */
    .backdrop {
      position: fixed;
      inset: 0;
      z-index: 1;
      background: var(--nc-color-backdrop, rgb(0 0 0 / 45%));
    }

    .actions {
      display: flex;
      align-items: center;
      gap: var(--wa-space-s, 8px);
    }

    /* A column flex container, so a screen that wants the window can ask for
       it with flex-grow on its own host and get a definite height to divide
       up. A screen that does not stays content-sized, as a block child would. */
    .content {
      flex: 1;
      overflow: auto;
      padding: var(--wa-space-l, 16px);
      display: flex;
      flex-direction: column;
      min-height: 0;
    }

    /* A 232px column out of a 390px viewport leaves the content a
       40-character strip, so on a phone the sidebar stops being a column: it
       lies over the content at full width and slides away when it is not
       wanted. The collapsed rail is the wide-viewport answer and stands down
       here — wc-nav-sidebar cancels its own rail styling at the same width. */
    @media (max-width: 48rem) {
      ::slotted([slot='sidebar']) {
        position: fixed;
        inset: 0 auto 0 0;
        z-index: 2;
        transition: transform var(--nc-transition-base, 200ms ease);
      }

      :host([sidebar-collapsed]) ::slotted([slot='sidebar']) {
        transform: translateX(-100%);
      }

      /* 16px a side is a tenth of a phone's width spent on margin. */
      .content {
        padding: 10px;
      }
    }

    @media (prefers-reduced-motion: reduce) {
      ::slotted([slot='sidebar']) {
        transition: none;
      }
    }

    .banner:not(:empty) {
      padding: var(--wa-space-s, 8px) var(--wa-space-l, 16px);
      background: var(--wa-color-surface-alt);
      border-bottom: 1px solid var(--wa-color-border);
    }

    /* A printed report is the artifact an accountant keeps, so the page has to
       be the report and nothing else. This lives here rather than in the print
       sheet because a rule that hides an element has to be in the tree that
       element is in, and the shell is inside nigel-app's shadow root. Token
       recolouring is the other way through the boundary and stays in
       @nigel/theme, where it reaches every component at once by inheritance. */
    @media print {
      :host {
        display: block;
        height: auto;
      }

      header,
      .banner {
        display: none;
      }

      ::slotted([slot='sidebar']) {
        display: none;
      }

      .main {
        overflow: visible;
      }

      .content {
        display: block;
        padding: 0;
        overflow: visible;
      }
    }
  `;

  @property({ type: String, attribute: 'screen-title' })
  screenTitle = '';

  @property({ type: Boolean, reflect: true, attribute: 'sidebar-collapsed' })
  sidebarCollapsed = false;

  @property({ attribute: false })
  narrowQuery: NarrowQuery | undefined = defaultNarrowQuery();

  private get narrow(): boolean {
    return this.narrowQuery?.matches ?? false;
  }

  /** Open here means the drawer is over the content, not the docked column. */
  private get drawerOpen(): boolean {
    return this.narrow && !this.sidebarCollapsed;
  }

  private readonly onViewportChange = () => this.requestUpdate();

  private readonly onKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Escape' && this.drawerOpen) this.requestSidebar(true);
  };

  private readonly onNavigate = () => {
    // The drawer covers the screen it just navigated to. A docked sidebar
    // covers nothing, so navigating there leaves it alone.
    if (this.drawerOpen) this.requestSidebar(true);
  };

  connectedCallback(): void {
    super.connectedCallback();
    this.narrowQuery?.addEventListener?.('change', this.onViewportChange);
    window.addEventListener('keydown', this.onKeyDown);
    this.addEventListener('nc-navigate', this.onNavigate);
  }

  disconnectedCallback(): void {
    this.narrowQuery?.removeEventListener?.('change', this.onViewportChange);
    window.removeEventListener('keydown', this.onKeyDown);
    this.removeEventListener('nc-navigate', this.onNavigate);
    super.disconnectedCallback();
  }

  updated(changed: Map<string, unknown>): void {
    // Focus went into the drawer; hand it back to the control that opened it
    // rather than dropping it on the body. Collapsing to the rail on a wide
    // viewport is not a dismissal and takes no focus with it.
    if (changed.has('sidebarCollapsed') && this.sidebarCollapsed && this.narrow) {
      this.shadowRoot?.querySelector<HTMLButtonElement>('.nav-toggle')?.focus();
    }
  }

  /**
   * The state is nigel-app's: the sidebar is its slotted child and only it can
   * pass `collapsed` down. The shell asks, and re-renders when the answer
   * comes back as a property.
   */
  private requestSidebar(collapsed: boolean): void {
    this.dispatchEvent(
      new CustomEvent<{ collapsed: boolean }>('nc-sidebar-toggle', {
        detail: { collapsed },
        bubbles: true,
        composed: true,
      }),
    );
  }

  render() {
    // The parts exist so a document-level stylesheet can reach the shell's
    // furniture — `::part()` pierces the shadow boundary, nothing else does.
    // The print sheet hides all three and lets the content have the page.
    return html`
      <slot name="sidebar" part="sidebar"></slot>
      ${this.drawerOpen
        ? html`<div
            class="backdrop"
            part="backdrop"
            aria-hidden="true"
            @click=${() => this.requestSidebar(true)}
          ></div>`
        : nothing}
      <div class="main">
        <header part="header">
          <div class="heading">
            <button
              type="button"
              class="nav-toggle"
              part="nav-toggle"
              aria-label="Navigation"
              aria-expanded=${this.sidebarCollapsed ? 'false' : 'true'}
              @click=${() => this.requestSidebar(!this.sidebarCollapsed)}
            >
              <wc-icon-menu></wc-icon-menu>
            </button>
            <h1 class="title">${this.screenTitle}</h1>
          </div>
          <div class="actions"><slot name="header-actions"></slot></div>
        </header>
        <div class="banner" part="banner"><slot name="banner"></slot></div>
        <main class="content" part="content"><slot></slot></main>
      </div>
      <wc-toast></wc-toast>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-app-shell': WcAppShell;
  }
  interface HTMLElementEventMap {
    'nc-sidebar-toggle': CustomEvent<{ collapsed: boolean }>;
  }
}
