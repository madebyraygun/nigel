import { LitElement, html, css, nothing } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import '@nigel/ui';
import { dispatchNcToast, narrowViewport } from '@nigel/ui';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { appUnauthorized, type ApiClient, type MenuCommand } from '../api/index.js';
import { createApiClient } from '../api/desktop-client.js';
import { requestMenuIntent } from '../state/menu-intent.js';
import {
  getAppStore,
  initializeAppStore,
  type AppStore,
} from '../state/app-store.js';
import { parseHash, screenToHash, type Route } from '../screens/hash-route.js';
import {
  DEFAULT_SCREEN,
  isScreenId,
  navItems,
  screenDef,
  type ScreenId,
} from '../screens/registry.js';
import type { ScreenContext } from '../screens/context.js';
import {
  deepActiveElement,
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
  static styles = css`
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
  `;

  /** Overridable so tests can drive the app with a fake transport. */
  client: ApiClient = createApiClient();

  @state()
  private route: Route = { screen: DEFAULT_SCREEN, params: new URLSearchParams() };

  /** The easter egg. Nothing on screen says it is here. */
  @state()
  private snakeOpen = false;

  /**
   * Whether the sidebar is put away — the rail on a wide viewport, off-canvas
   * on a phone, where it starts that way because it would otherwise cover the
   * screen. The shell renders the control and asks; the sidebar is this
   * element's slotted child, so only this element can pass the answer down.
   */
  @state()
  private sidebarCollapsed = narrowViewport();

  private store: AppStore | null = null;
  private reportedError: string | null = null;
  private focusBeforeSnake: HTMLElement | null = null;
  private unsubscribeMenu: (() => void) | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    this.store = initializeAppStore(this.client);
    window.addEventListener('hashchange', this.handleHashChange);
    window.addEventListener('keydown', this.handleGlobalKeydown);
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
    this.unsubscribeMenu?.();
    this.unsubscribeMenu = null;
    super.disconnectedCallback();
  }

  /**
   * A selection from the shell's menu bar.
   *
   * Navigation validates the screen id here rather than in the api layer,
   * which does not know the registry; an id this build has never heard of is
   * dropped, so a newer shell degrades to inert items rather than a blank
   * screen. `find` and `import` land on their screens as one-shot intents the
   * screen consumes — a route parameter could not re-fire on a repeated chord.
   */
  private handleMenuCommand = (command: MenuCommand): void => {
    // The gates and the boot screen render without the shell: no sidebar to
    // toggle, no screen to receive an intent, and the untouched hash is what
    // returns the user where they were headed once the app is up. A selection
    // before then is dropped whole — an intent stored now would fire on
    // whatever screen mounts after unlock.
    if ((this.store ?? getAppStore()).boot.get() !== 'ready') return;

    switch (command.kind) {
      case 'navigate':
        if (isScreenId(command.screen)) this.navigate(command.screen);
        return;
      case 'new-invoice':
        this.navigate('invoices', new URLSearchParams({ new: '1' }));
        return;
      case 'find':
        // Focusing search must not cost a filtered register its filters: the
        // bare-hash navigate is only for arriving from somewhere else.
        if (this.route.screen !== 'register') this.navigate('register');
        requestMenuIntent('find');
        return;
      case 'import':
        this.navigate('import');
        requestMenuIntent('pick-import');
        return;
      case 'toggle-sidebar':
        this.sidebarCollapsed = !this.sidebarCollapsed;
        return;
      default:
        command satisfies never;
    }
  };

  private handleHashChange = (): void => {
    // A route change takes the screen out from under the game, so the game
    // goes with it rather than staying up over a screen nobody navigated to.
    this.closeSnake();
    this.syncRouteFromHash();
  };

  protected willUpdate(): void {
    // The other way the dashboard disappears: locking, a failed status, a
    // reload. Each of those swaps the render branch, and without this the
    // overlay would only stop being *rendered* — the open flag and the focus
    // capture would survive to strand the next one.
    const boot = (this.store ?? getAppStore()).boot.get();
    if (this.snakeOpen && !snakeAllowedOnBoot(boot)) this.closeSnake();
  }

  private syncRouteFromHash(): void {
    if (!window.location.hash) {
      // Seed the address bar so a reload lands on the same screen.
      window.location.hash = screenToHash(DEFAULT_SCREEN);
      return;
    }
    this.route = parseHash(window.location.hash);
  }

  private handleNavigate = (event: CustomEvent<{ id: string }>): void => {
    window.location.hash = `#/${event.detail.id}`;
  };

  private handleSidebarToggle = (event: CustomEvent<{ collapsed: boolean }>): void => {
    this.sidebarCollapsed = event.detail.collapsed;
  };

  /** Navigation for screens: the same one-directional path the sidebar takes. */
  private navigate = (screen: ScreenId, params?: URLSearchParams): void => {
    const query = params?.toString();
    window.location.hash = query ? `#/${screen}?${query}` : `#/${screen}`;
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
      const shell = this.shadowRoot?.querySelector('wc-app-shell');
      if (!shell) return;
      if (!shell.hasAttribute('tabindex')) shell.setAttribute('tabindex', '-1');
      shell.focus();
    });
  };

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
    document.title = `${screen.title} · ${store.companyName.get()}`;

    return html`
      <wc-app-shell
        screen-title=${screen.title}
        ?inert=${this.snakeOpen}
        ?sidebar-collapsed=${this.sidebarCollapsed}
        @nc-sidebar-toggle=${this.handleSidebarToggle}
      >
        <wc-nav-sidebar
          slot="sidebar"
          ?collapsed=${this.sidebarCollapsed}
          .items=${navItems()}
          active=${screen.id}
          app-name=${store.companyName.get()}
          @nc-navigate=${this.handleNavigate}
        ></wc-nav-sidebar>
        ${this.renderBanner(error?.message ?? null)} ${screen.render(ctx)}
      </wc-app-shell>
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
          <button class="retry" type="button" @click=${this.handleRetry}>
            Retry
          </button>
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
