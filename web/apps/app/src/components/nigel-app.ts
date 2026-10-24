import { LitElement, html, css, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import { keyed } from 'lit/directives/keyed.js';
import { ifDefined } from 'lit/directives/if-defined.js';
import { controlsCss } from '@nigel/theme';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@nigel/ui';
import {
  dispatchNcToast,
  narrowViewport,
  type NcPaletteOpenDetail,
  type NcPaletteSelectDetail,
  type PaletteScope,
} from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { appUnauthorized, type ApiClient, type MenuCommand } from '../api/index.js';
import { createApiClient } from '../api/desktop-client.js';
import { confirmDiscard, unsavedChanges } from '../state/unsaved.js';
import { dropMenuIntentUnlessFor, requestMenuIntent } from '../state/menu-intent.js';
import {
  getAppStore,
  initializeAppStore,
  type AppStore,
} from '../state/app-store.js';
import { parseHash, routeToHash, screenToHash, type Route } from '../screens/hash-route.js';
import {
  DEFAULT_SCREEN,
  isScreenId,
  navItems,
  screenDef,
  shellHeading,
  type ScreenId,
} from '../screens/registry.js';
import { trailFor, windowTitle } from '../screens/trail.js';
import {
  isHostedShell,
  isApplePlatform,
  paletteShortcut,
  readSidebarCollapsed,
  statusLineOf,
  writeSidebarCollapsed,
} from '../shell-preferences.js';
import type { ScreenContext } from '../screens/context.js';
import { isPaletteShortcut, paletteBooks, paletteScreens } from '../palette.js';
import {
  deepActiveElement,
  hasOpenModal,
  isSnakeTrigger,
  snakeAllowedOnBoot,
} from '../snake-trigger.js';

/**
 * Root container: owns the api client, the store, and the route.
 *
 * Routing is one-directional. Navigation writes `location.hash` and nothing
 * else; the hashchange listener is the only thing that updates `route`. That
 * is what makes the back button and a pasted deep link behave the same as a
 * click.
 */
@customElement('nigel-app')
export class NigelApp extends SignalWatcher(LitElement) {
  static styles = [controlsCss, css`
    :host {
      display: block;
      height: 100vh;
    }

    .boot {
      display: grid;
      place-items: center;
      height: 100vh;
      background: var(--wa-color-bg);
    }

    .gate {
      height: 100vh;
      overflow: auto;
    }

    .banner {
      color: var(--wa-color-text);
    }

    .banner strong {
      color: var(--wa-color-danger);
    }

    .retry {
      margin-left: var(--wa-space-s, 8px);
    }
  `];

  /** Overridable so tests can drive the app with a fake transport. */
  client: ApiClient = createApiClient();

  @state()
  private route: Route = { screen: DEFAULT_SCREEN, params: new URLSearchParams() };

  /** The easter egg. Nothing on screen says it is here. */
  @state()
  private snakeOpen = false;

  /**
   * Whether the sidebar is put away — the rail on a wide viewport, where it
   * starts as this device last left it, and off-canvas on a phone, where it
   * starts that way because it would otherwise cover the screen. The sidebar
   * and the shell render the controls and ask; the sidebar is this element's
   * slotted child, so only this element can pass the answer down.
   */
  @state()
  private sidebarCollapsed = narrowViewport() || readSidebarCollapsed();

  /** How many transactions are waiting in Review, for the sidebar's count. */
  @state()
  private reviewCount: number | undefined = undefined;

  /** The ⌘K palette, and which part of it was asked for. */
  @state()
  private paletteOpen = false;

  @state()
  private paletteScope: PaletteScope = 'all';

  /** The control that opened the palette; focus goes back to it on close. */
  private paletteOpener: HTMLElement | null = null;

  /**
   * A screen was picked from the palette. The opener may be in the screen
   * that pick replaces, so focus is checked again once the route has swapped.
   */
  private refocusAfterRoute = false;

  /** Bumped by the top bar's refresh, which remounts the screen so it loads afresh. */
  @state()
  private refreshes = 0;

  /**
   * A native window is hosting the page and draws its own sidebar, so this
   * one is not rendered. Read once: the query string is fixed for the life of
   * the page.
   */
  private readonly hosted = isHostedShell();

  private readonly shortcut = paletteShortcut();
  private readonly apple = isApplePlatform();
  private wasReady = false;

  private store: AppStore | null = null;
  private reportedError: string | null = null;
  private focusBeforeSnake: HTMLElement | null = null;
  private unsubscribeMenu: (() => void) | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    this.store = initializeAppStore(this.client);
    window.addEventListener('hashchange', this.handleHashChange);
    window.addEventListener('keydown', this.handleGlobalKeydown);
    // On the Mac the native shell draws the palette and owns the chord.
    if (!this.hosted) window.addEventListener('keydown', this.handlePaletteKeydown);
    const menu = this.client.menuSource();
    if (menu.kind === 'native') {
      this.unsubscribeMenu = menu.onCommand(this.handleMenuCommand);
    }
    this.syncRouteFromHash();
    void this.store.refreshStatus();
  }

  disconnectedCallback(): void {
    window.removeEventListener('hashchange', this.handleHashChange);
    window.removeEventListener('keydown', this.handleGlobalKeydown);
    window.removeEventListener('keydown', this.handlePaletteKeydown);
    this.unsubscribeMenu?.();
    this.unsubscribeMenu = null;
    super.disconnectedCallback();
  }

  /** The hash the screen on show was routed from; where a refused leave goes back to. */
  private routedHash = '';

  /**
   * A selection from the shell's menu bar.
   *
   * Navigation validates the screen id here rather than in the api layer,
   * which does not know the registry; an id this build has never heard of, or
   * a screen the sidebar does not list, is dropped, so a newer shell degrades to inert items rather than a blank
   * screen. `find` (on the register only) and `import` reach their screens as
   * one-shot intents the screen consumes — a route parameter could not re-fire
   * on a repeated chord.
   */
  private handleMenuCommand = (command: MenuCommand): void => {
    // The gates and the boot screen render without the shell: no sidebar to
    // toggle, no screen to receive an intent, and the untouched hash is what
    // returns the user where they were headed once the app is up. A selection
    // before then is dropped whole — an intent stored now would fire on
    // whatever screen mounts after unlock.
    if ((this.store ?? getAppStore()).boot.get() !== 'ready') return;
    // A dialog holds the top layer and makes the page behind it inert, which
    // stops the sidebar; the menu is held to the same rule, so a chord cannot
    // swap the screen out from under the register's edit dialog or the palette.
    if (hasOpenModal()) return;

    switch (command.kind) {
      case 'navigate':
        // Only what the sidebar offers: setup and unlock are gates the boot
        // phase routes to, never destinations a menu item may pick.
        if (isScreenId(command.screen) && screenDef(command.screen).inNav) {
          this.navigate(command.screen);
        }
        return;
      case 'new-invoice':
        this.navigate('invoices', new URLSearchParams({ new: '1' }));
        return;
      case 'find':
        // Find searches the screen in front of the user; only the register
        // has a search box, and navigating there from elsewhere would throw
        // away whatever that screen held, an unsaved invoice draft included.
        if (this.route.screen === 'register') requestMenuIntent('find');
        return;
      case 'import':
        this.navigate('import');
        requestMenuIntent('pick-import');
        return;
      case 'toggle-sidebar':
        // A hosting native window draws its own sidebar and toggles it itself.
        if (!this.hosted) this.setSidebarCollapsed(!this.sidebarCollapsed);
        return;
      default:
        command satisfies never;
    }
  };

  private handleHashChange = (): void => {
    const target = window.location.hash;
    if (unsavedChanges() && this.routedHash && target !== this.routedHash) {
      // The address bar has already moved. Put it back without a second
      // hashchange, ask, and only then go where it was going.
      const back = new URL(window.location.href);
      back.hash = this.routedHash;
      history.replaceState(history.state, '', back);
      this.refocusAfterRoute = false;
      void confirmDiscard().then((discard) => {
        if (discard) window.location.hash = target;
        // Staying put: an intent raised for the screen that was refused dies
        // here, as it would have on the route change.
        else dropMenuIntentUnlessFor(this.route.screen);
      });
      return;
    }
    // A route change takes the screen out from under the game, so the game
    // goes with it rather than staying up over a screen nobody navigated to.
    this.closeSnake();
    this.paletteOpen = false;
    this.syncRouteFromHash();
    if (this.refocusAfterRoute) {
      this.refocusAfterRoute = false;
      this.settlePaletteFocus();
    }
    void this.loadReviewCount();
    // An intent belongs to the screen it was raised for; one still parked
    // when the user leaves for another screen must not fire on a later visit.
    dropMenuIntentUnlessFor(this.route.screen);
  };

  protected updated(): void {
    // The count is only worth asking for once there are books to count in,
    // and asking for it before then would be a query the gates exist to stop.
    const ready = (this.store ?? getAppStore()).boot.get() === 'ready';
    const arrived = ready && !this.wasReady;
    this.wasReady = ready;
    if (arrived) void this.loadReviewCount();
  }

  private async loadReviewCount(): Promise<void> {
    if (!this.wasReady) return;
    try {
      this.reviewCount = (await this.client.getReviewQueue()).length;
    } catch {
      // The count is a hint; without it the sidebar shows none.
      this.reviewCount = undefined;
    }
  }

  protected willUpdate(): void {
    // The other way the dashboard disappears: locking, a failed status, a
    // reload. Each of those swaps the render branch, and without this the
    // overlay would only stop being *rendered* — the open flag and the focus
    // capture would survive to strand the next one.
    const boot = (this.store ?? getAppStore()).boot.get();
    if (this.snakeOpen && !snakeAllowedOnBoot(boot)) this.closeSnake();
    if (boot !== 'ready') this.paletteOpen = false;
  }

  private syncRouteFromHash(): void {
    if (!window.location.hash) {
      // Seed the address bar so a reload lands on the same screen.
      window.location.hash = screenToHash(DEFAULT_SCREEN);
      return;
    }
    this.route = parseHash(window.location.hash);
    this.routedHash = window.location.hash;
  }

  private handleNavigate = (event: CustomEvent<{ id: string }>): void => {
    window.location.hash = `#/${event.detail.id}`;
  };

  private handleSidebarToggle = (event: CustomEvent<{ collapsed: boolean }>): void => {
    this.setSidebarCollapsed(event.detail.collapsed);
  };

  private setSidebarCollapsed(collapsed: boolean): void {
    this.sidebarCollapsed = collapsed;
    // On a phone collapsed means the drawer is shut, which is where it starts
    // anyway; only the rail is a choice worth remembering.
    if (!narrowViewport()) writeSidebarCollapsed(collapsed);
  }

  private handleRefresh = async (): Promise<void> => {
    // Refresh remounts the screen, which would drop a draft as surely as
    // leaving it.
    if (!(await confirmDiscard())) return;
    void this.store?.refreshStatus();
    void this.loadReviewCount();
    this.refreshes += 1;
  };

  /** Navigation for screens: the same one-directional path the sidebar takes. */
  private navigate = (screen: ScreenId, params?: URLSearchParams): void => {
    window.location.hash = routeToHash({ screen, params: params ?? new URLSearchParams() });
  };

  private screenContext(): ScreenContext {
    return {
      client: this.client,
      params: this.route.params,
      navigate: this.navigate,
    };
  }

  /**
   * The one shortcut the app has, and it is a secret.
   *
   * `s` on the dashboard, which is the screen and the key the TUI puts Snake
   * on. It is bound at the window rather than on a screen because the game
   * covers the whole app and outlives whatever was underneath, and every guard
   * that keeps it out of somebody's typing lives in `snake-trigger.ts`.
   */
  private handleGlobalKeydown = (event: KeyboardEvent): void => {
    if (this.snakeOpen) return;
    if (this.route.screen !== 'dashboard') return;
    if (!snakeAllowedOnBoot((this.store ?? getAppStore()).boot.get())) return;
    if (!isSnakeTrigger(event)) return;

    event.preventDefault();
    const active = deepActiveElement();
    this.focusBeforeSnake = active instanceof HTMLElement ? active : null;
    this.snakeOpen = true;
  };

  /**
   * The only way out, and every exit goes through it: Escape, a route change,
   * and the boot phase moving off `ready`.
   *
   * One path because the open flag and the captured focus have to fall
   * together. A render branch that simply stopped rendering the overlay —
   * which is what locking does — would leave both behind, and unlocking would
   * put a fresh game back over the app with a focus capture pointing at an
   * element that no longer exists.
   */
  private closeSnake = (): void => {
    if (!this.snakeOpen) return;

    const restore = this.focusBeforeSnake;
    this.snakeOpen = false;
    this.focusBeforeSnake = null;

    void this.updateComplete.then(() => {
      // Whatever the game was covering may have been unmounted while it was
      // up, so the shell is the fallback: focus has to land somewhere, and a
      // `focus()` on a detached element lands on the body.
      if (restore?.isConnected) {
        restore.focus();
        return;
      }
      this.focusShell();
    });
  };

  /** Where focus lands when what had it is gone: a `focus()` on a detached element lands on the body. */
  private focusShell(): void {
    const shell = this.shadowRoot?.querySelector('wc-app-shell');
    if (!shell) return;
    if (!shell.hasAttribute('tabindex')) shell.setAttribute('tabindex', '-1');
    shell.focus();
  }

  /**
   * The platform's chord toggles the palette from anywhere, a field included
   * — but not over another dialog, which holds the top layer and would leave
   * the palette drawn underneath it.
   */
  private handlePaletteKeydown = (event: KeyboardEvent): void => {
    if (!isPaletteShortcut(event, this.apple) || this.snakeOpen) return;
    if ((this.store ?? getAppStore()).boot.get() !== 'ready') return;
    if (this.paletteOpen) {
      event.preventDefault();
      this.closePalette();
      return;
    }
    if (hasOpenModal()) return;
    event.preventDefault();
    const active = deepActiveElement();
    this.openPalette('all', active instanceof HTMLElement ? active : null);
  };

  private handlePaletteOpen = (event: CustomEvent<NcPaletteOpenDetail>): void => {
    if (this.hosted) return;
    // The request comes from a control's host; the opener is the focusable
    // control inside it, which a WebKit click leaves unfocused.
    const origin = event
      .composedPath()
      .find((node): node is HTMLElement => node instanceof HTMLElement && node.tabIndex >= 0);
    this.openPalette(event.detail.scope, origin ?? null);
  };

  private openPalette(scope: PaletteScope, opener: HTMLElement | null): void {
    this.paletteScope = scope;
    this.paletteOpener = opener;
    this.refocusAfterRoute = false;
    this.paletteOpen = true;
  }

  private closePalette = (): void => {
    this.paletteOpen = false;
    this.settlePaletteFocus();
  };

  /** A screen navigates; the open book is where the reader already is. */
  private handlePaletteSelect = (event: CustomEvent<NcPaletteSelectDetail>): void => {
    if (event.detail.kind === 'screen') {
      this.refocusAfterRoute = true;
      this.navigate(event.detail.id as ScreenId);
    }
    this.closePalette();
  };

  /**
   * The palette hands focus back to its opener; when the opener is gone —
   * the screen it sat in was swapped out — the shell takes it instead.
   */
  private settlePaletteFocus(): void {
    const palette = this.shadowRoot?.querySelector('wc-command-palette');
    void this.updateComplete
      .then(() => palette?.updateComplete)
      .then(() => {
        if (this.paletteOpen || this.paletteOpener?.isConnected) return;
        this.paletteOpener = null;
        this.focusShell();
      });
  }

  private handleRetry = (): void => {
    this.reportedError = null;
    void this.store?.refreshStatus();
  };

  /** Surface a status failure once per occurrence rather than every render. */
  private announceError(message: string): void {
    if (this.reportedError === message) return;
    this.reportedError = message;
    dispatchNcToast(this, { message, variant: 'danger', duration: 0 });
  }

  render() {
    const store = this.store ?? getAppStore();
    const error = store.statusError.get();

    if (error) this.announceError(error.message);

    const boot = store.boot.get();
    const ctx = this.screenContext();

    if (boot === 'starting') {
      return html`
        <div class="boot">
          <wc-spinner size="l" show-label label="Connecting to nigel"></wc-spinner>
        </div>
      `;
    }

    // The gate replaces the shell rather than sitting inside it: with no
    // sidebar and no screen rendered, nothing exists that could fetch data
    // before the password arrives. The hash is left alone, so unlocking returns
    // the user to wherever they were headed.
    if (boot === 'locked') {
      const gate = screenDef('unlock');
      document.title = `${gate.title} · ${store.companyName.get()}`;
      return html`<div class="gate">${gate.render(ctx)}</div>`;
    }

    // Same treatment as the unlock gate, and for the same reason: with no
    // database there is no screen that could fetch anything.
    if (boot === 'needs-setup') {
      const gate = screenDef('setup');
      document.title = `${gate.title} · ${store.companyName.get()}`;
      return html`<div class="gate">${gate.render(ctx)}</div>`;
    }

    const screen = screenDef(this.route.screen);
    const heading = shellHeading(screen);
    const book = store.companyName.get();
    const trail = trailFor(this.route);
    document.title = windowTitle(trail, book);

    return html`
      <wc-app-shell
        screen-title=${heading.title}
        ?title-hidden=${heading.hidden}
        book-name=${book}
        book-key=${store.status.get()?.dataDir ?? book}
        .trail=${trail}
        .scrollKey=${trail.map((item) => item.label).join('/')}
        search-shortcut=${this.shortcut.keycap}
        search-key-shortcuts=${this.shortcut.aria}
        ?hide-sidebar=${this.hosted}
        .statusLine=${this.hosted ? undefined : statusLineOf(store.status.get())}
        review-count=${ifDefined(this.reviewCount)}
        ?inert=${this.snakeOpen || this.paletteOpen}
        ?sidebar-collapsed=${this.sidebarCollapsed}
        @nc-sidebar-toggle=${this.handleSidebarToggle}
        @nc-navigate=${this.handleNavigate}
        @nc-refresh=${this.handleRefresh}
        @nc-palette-open=${this.handlePaletteOpen}
      >
        ${this.hosted
          ? nothing
          : html`<wc-nav-sidebar
              slot="sidebar"
              ?collapsed=${this.sidebarCollapsed}
              .items=${navItems({ counts: { review: this.reviewCount } })}
              active=${screen.id}
            ></wc-nav-sidebar>`}
        ${this.renderBanner(error?.message ?? null)}
        ${screen.id === 'dashboard' && this.reviewCount
          ? html`<wc-review-pill
              slot="page-actions"
              .count=${this.reviewCount}
            ></wc-review-pill>`
          : nothing}
        ${keyed(this.refreshes, screen.render(ctx))}
      </wc-app-shell>
      ${this.hosted
        ? nothing
        : html`<wc-command-palette
            ?open=${this.paletteOpen}
            scope=${this.paletteScope}
            .opener=${this.paletteOpener}
            .books=${paletteBooks(store.status.get(), book)}
            .screens=${paletteScreens()}
            @nc-palette-close=${this.closePalette}
            @nc-palette-select=${this.handlePaletteSelect}
          ></wc-command-palette>`}
      ${this.snakeOpen
        ? html`<wc-snake fullscreen @nc-snake-exit=${this.closeSnake}></wc-snake>`
        : nothing}
    `;
  }

  private renderBanner(errorMessage: string | null) {
    if (appUnauthorized.get()) {
      return html`
        <div slot="banner" class="banner">
          <strong>Session expired.</strong> Reopen the link
          <code>nigel serve</code> printed — the browser cannot mint a new
          session token on its own.
        </div>
      `;
    }
    if (errorMessage) {
      return html`
        <div slot="banner" class="banner">
          <strong>Could not load status.</strong> ${errorMessage}
          <wa-button class="retry" size="s" @click=${this.handleRetry}>
            Retry
          </wa-button>
        </div>
      `;
    }
    return nothing;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-app': NigelApp;
  }
}
