import { LitElement, html, css, nothing, type PropertyValues } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { controlsCss } from '@nigel/theme';
import './wc-toast.js';
import './wc-breadcrumb.js';
import './wc-search-pill.js';
import type { BreadcrumbItem } from './wc-breadcrumb.js';
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
 * The application frame: the floating sidebar slot, the top bar, a banner
 * slot, the scrolling content area, and the single toast region.
 *
 * The top bar is drawn here from properties — the breadcrumb (book, screen,
 * any level below it), the search pill and the refresh button — and the
 * shell says what was asked for with events rather than acting on it:
 * `nc-palette-open` from the book and the pill, `nc-navigate` from an earlier
 * breadcrumb level, `nc-refresh` from refresh, `nc-sidebar-toggle` from the
 * drawer's toggle. Which screen is showing is the app's screen registry's
 * business; the shell is handed its title and trail.
 */
@customElement('wc-app-shell')
export class WcAppShell extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: flex;
        height: 100vh;
        background: var(--wa-color-bg);
        color: var(--wa-color-text);
        font-family: var(--wa-font-family-sans);
        font-size: var(--wa-font-size-base);
        line-height: var(--wa-line-height);
      }

      :host([hide-sidebar]) ::slotted([slot='sidebar']) {
        display: none;
      }

      .column {
        flex: 1;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        min-width: 0;
      }

      header {
        display: flex;
        align-items: center;
        gap: 10px;
        flex: none;
        height: var(--nc-header-height);
        margin-top: var(--nc-window-inset);
        padding: 0 16px 0 20px;
        box-sizing: border-box;
      }

      wc-breadcrumb {
        flex: 0 1 auto;
      }

      .spacer {
        flex: 1;
      }

      .icon-button {
        display: inline-grid;
        place-items: center;
        flex: none;
        width: 32px;
        height: 32px;
        padding: 0;
        border: 0;
        background: transparent;
        color: var(--wa-color-muted);
        cursor: default;
        transition: background var(--nc-transition-fast);
        --nc-icon-size: 17px;
      }

      .icon-button:hover {
        background: var(--nc-color-hover);
      }

      .refresh {
        border-radius: var(--wa-radius-pill);
      }

      /* The drawer's toggle. Above the breakpoint the hamburger lives in the
         sidebar's own header, so this one only exists on a phone. */
      .nav-toggle {
        display: none;
        border-radius: var(--wa-radius-md);
        color: var(--wa-color-text);
      }

      /* Below the breakpoint the sidebar leaves the flow and lies over the
         content, so the backdrop is what says the rest of the app is not the
         thing being interacted with. Escape and the toggle close the drawer
         too, which is what keeps a pointer-only affordance from being the
         only way out. */
      .backdrop {
        position: fixed;
        inset: 0;
        z-index: 1;
        background: var(--wa-color-overlay-modal);
      }

      .banner {
        display: none;
      }

      .banner.filled {
        display: block;
        margin: 0 16px var(--wa-space-s) 20px;
        padding: var(--wa-space-s) var(--wa-space-m);
        background: var(--wa-color-surface-alt);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-md);
      }

      /* A column flex container, so a screen that wants the window can ask
         for it with flex-grow on its own host and get a definite height to
         divide up. A screen that does not stays content-sized, as a block
         child would. */
      .content {
        flex: 1;
        display: flex;
        flex-direction: column;
        gap: 18px;
        overflow: auto;
        padding: 4px 16px 16px 20px;
        min-height: 0;
      }

      .page-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
        flex-wrap: wrap;
        gap: var(--wa-space-m);
        flex: none;
      }

      .title {
        margin: 0;
        font-size: var(--nc-font-size-display);
        font-weight: var(--wa-font-weight-medium);
        letter-spacing: var(--nc-letter-spacing-tight);
        line-height: 1.2;
      }

      /* The screen is the only thing in the default slot, and it gets the
         rest of the content area rather than as much of it as its own content
         fills — a screen has nothing to centre anything in otherwise.
         Content taller than the area still grows past it and scrolls,
         because a flex item's automatic minimum size is its content. */
      .content ::slotted(*) {
        flex: 1 1 auto;
      }

      /* A 224px column out of a 390px viewport leaves the content a
         40-character strip, so on a phone the sidebar stops being a column:
         it lies over the content and slides away when it is not wanted. The
         collapsed rail is the wide-viewport answer and stands down here —
         wc-nav-sidebar cancels its own rail styling at the same width. */
      @media (max-width: 48rem) {
        ::slotted([slot='sidebar']) {
          position: fixed;
          inset: 0 auto 0 0;
          z-index: 2;
          transition: transform var(--nc-transition-base);
        }

        /* Its own width plus the inset either side, so neither the panel's
           edge nor its shadow is left on screen. */
        :host([sidebar-collapsed]) ::slotted([slot='sidebar']) {
          transform: translateX(calc(-100% - 2 * var(--nc-window-inset)));
        }

        .nav-toggle {
          display: inline-grid;
        }

        :host([hide-sidebar]) .nav-toggle {
          display: none;
        }

        header {
          padding: 0 10px;
          gap: 6px;
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

      /* A printed report is the artifact an accountant keeps, so the page has
         to be the report and nothing else. This lives here rather than in the
         print sheet because a rule that hides an element has to be in the
         tree that element is in, and the shell is inside nigel-app's shadow
         root. Token recolouring is the other way through the boundary and
         stays in @nigel/theme, where it reaches every component at once by
         inheritance. */
      @media print {
        :host {
          display: block;
          height: auto;
        }

        header,
        .banner,
        .page-head {
          display: none;
        }

        ::slotted([slot='sidebar']) {
          display: none;
        }

        .column {
          overflow: visible;
        }

        .content {
          display: block;
          padding: 0;
          overflow: visible;
        }

        /* The screen goes back to block flow for the sheets. A flex container
           is not required to fragment and Safari and older Chromium slice
           through a row rather than break between two, and the leftover
           height a column hands out is a property of a viewport, which paper
           does not have. A normal declaration in the outer tree beats the
           inner tree's own :host rule, so this reaches every screen from
           here. */
        .content ::slotted(*) {
          display: block;
        }
      }
    `,
  ];

  /** The page heading over the content. Empty draws none, for a screen with its own. */
  @property({ type: String, attribute: 'screen-title' })
  screenTitle = '';

  /** The open book's name, first in the breadcrumb. */
  @property({ type: String, attribute: 'book-name' })
  bookName = '';

  /** What picks the book's monogram colour. */
  @property({ type: String, attribute: 'book-key' })
  bookKey = '';

  /** The breadcrumb after the book: the screen, then any level below it. */
  @property({ attribute: false })
  trail: BreadcrumbItem[] = [];

  /** The search pill's keycap, and the same chord for `aria-keyshortcuts`. */
  @property({ type: String, attribute: 'search-shortcut' })
  searchShortcut = '⌘K';

  @property({ type: String, attribute: 'search-key-shortcuts' })
  searchKeyShortcuts = 'Meta+K';

  @property({ type: Boolean, reflect: true, attribute: 'sidebar-collapsed' })
  sidebarCollapsed = false;

  /** No sidebar and no toggle for one: the host window draws its own. */
  @property({ type: Boolean, reflect: true, attribute: 'hide-sidebar' })
  hideSidebar = false;

  /**
   * Changing this puts the content area back at the top. The app hands in
   * something that changes with the screen, so arriving on one never starts
   * partway down it.
   */
  @property({ attribute: false })
  scrollKey: unknown = undefined;

  @property({ attribute: false })
  narrowQuery: NarrowQuery | undefined = defaultNarrowQuery();

  /** Whether anything is in the banner slot; an empty banner draws nothing. */
  @state()
  private hasBanner = false;

  private readonly onBannerChange = (event: Event) => {
    const slot = event.target as HTMLSlotElement;
    this.hasBanner = slot.assignedNodes({ flatten: true }).length > 0;
  };

  private get narrow(): boolean {
    return this.narrowQuery?.matches ?? false;
  }

  /** Open here means the drawer is over the content, not the docked column. */
  private get drawerOpen(): boolean {
    return this.narrow && !this.hideSidebar && !this.sidebarCollapsed;
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

  updated(changed: PropertyValues<this>): void {
    // Focus went into the drawer; hand it back to the control that opened it
    // rather than dropping it on the body. Collapsing to the rail on a wide
    // viewport is not a dismissal and takes no focus with it.
    if (changed.has('sidebarCollapsed') && this.sidebarCollapsed && this.narrow) {
      this.shadowRoot?.querySelector<HTMLButtonElement>('.nav-toggle')?.focus();
    }
    if (changed.has('scrollKey') && changed.get('scrollKey') !== undefined) {
      const content = this.shadowRoot?.querySelector<HTMLElement>('.content');
      if (content) content.scrollTop = 0;
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

  private requestRefresh(): void {
    this.dispatchEvent(new CustomEvent('nc-refresh', { bubbles: true, composed: true }));
  }

  render() {
    // The parts exist so a document-level stylesheet can reach the shell's
    // furniture — `::part()` pierces the shadow boundary, nothing else does.
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
      <div class="column">
        <header part="header">
          ${this.hideSidebar
            ? nothing
            : html`<button
                type="button"
                class="icon-button nav-toggle"
                part="nav-toggle"
                aria-label="Navigation"
                aria-expanded=${this.sidebarCollapsed ? 'false' : 'true'}
                @click=${() => this.requestSidebar(!this.sidebarCollapsed)}
              >
                <wc-icon-menu></wc-icon-menu>
              </button>`}
          <wc-breadcrumb
            book-name=${this.bookName}
            book-key=${this.bookKey}
            .trail=${this.trail}
          ></wc-breadcrumb>
          <span class="spacer"></span>
          <wc-search-pill
            shortcut=${this.searchShortcut}
            key-shortcuts=${this.searchKeyShortcuts}
          ></wc-search-pill>
          <button
            type="button"
            class="icon-button refresh"
            aria-label="Refresh"
            title="Refresh"
            @click=${this.requestRefresh}
          >
            <wc-icon-refresh></wc-icon-refresh>
          </button>
        </header>
        <div class=${this.hasBanner ? 'banner filled' : 'banner'} part="banner">
          <slot name="banner" @slotchange=${this.onBannerChange}></slot>
        </div>
        <main class="content" part="content">
          ${this.screenTitle
            ? html`<div class="page-head" part="page-head">
                <h1 class="title">${this.screenTitle}</h1>
                <slot name="page-actions"></slot>
              </div>`
            : nothing}
          <slot></slot>
        </main>
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
    'nc-refresh': CustomEvent<void>;
  }
}
