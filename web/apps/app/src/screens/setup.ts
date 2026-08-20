import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import { prefersReducedMotion } from '@nigel/ui';
import { controlsCss } from '@nigel/theme';

import { SignalWatcher } from '../mixins/signal-watcher.js';
import { getAppStore, type AppStore } from '../state/app-store.js';
import type { BooksProfile, SetupAction } from '../api/types.js';
import type { ScreenContext } from './context.js';

/** How long the wordmark takes to assemble before the first question arrives. */
const REVEAL_MS = 1200;
const REVEAL_TICK_MS = 40;

type Step = 'arrival' | 'profile' | 'identity' | 'first-move';

/**
 * The first-run gate: the four answers a set of books is created from.
 *
 * Rendered instead of the app shell, not inside it — with no sidebar and no
 * screen there is nothing that could fetch data from a database that does not
 * exist yet. One question is visible at a time, in the terminal onboarding's
 * order, and nothing reaches the server until the last step: the first three
 * are entirely local, so a wrong turn costs a click rather than a set of books.
 */
@customElement('nigel-setup-screen')
export class NigelSetupScreen extends SignalWatcher(LitElement) {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        min-height: 100vh;
        background: var(--wa-color-bg);
        color: var(--wa-color-text);
        font-family: var(--wa-font-family-sans);
      }

      .stage {
        position: relative;
        display: grid;
        place-items: center;
        min-height: 100vh;
        padding: var(--wa-space-xl, 24px);
        overflow: hidden;
      }

      wc-particle-field {
        position: absolute;
        inset: 0;
      }

      .panel {
        position: relative;
        width: 100%;
        max-width: 42rem;
        display: grid;
        gap: var(--wa-space-l, 16px);
        justify-items: center;
        text-align: center;
      }

      wc-wordmark {
        --nc-wordmark-size: var(--wa-font-size-s, 13px);
      }

      h1 {
        margin: 0;
        font-size: var(--wa-font-size-2xl, 24px);
        font-weight: var(--wa-font-weight-semibold, 600);
      }

      p {
        margin: 0;
        color: var(--wa-color-muted);
        max-width: 34rem;
      }

      .cards {
        display: grid;
        gap: var(--wa-space-m, 12px);
        width: 100%;
        text-align: left;
      }

      .card {
        display: grid;
        gap: var(--wa-space-2xs, 4px);
        padding: var(--wa-space-l, 16px);
        background: var(--wa-color-surface);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-l, 12px);
        cursor: pointer;
        font: inherit;
        color: inherit;
        text-align: left;
      }

      .card:hover,
      .card:focus-visible {
        border-color: var(--wa-color-brand);
      }

      .card strong {
        font-size: var(--wa-font-size-l, 15px);
      }

      .card span {
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
      }

      .form {
        display: grid;
        gap: var(--wa-space-m, 12px);
        width: 100%;
        max-width: 26rem;
        text-align: left;
      }

      .actions {
        display: flex;
        gap: var(--wa-space-s, 8px);
        justify-content: center;
      }

      .footnote {
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
      }

      /*
       * The arrival's copy and buttons wait for the wordmark to finish
       * assembling, then fade in a beat apart. Hidden rather than merely
       * transparent: a button nobody can see is a button nobody should be
       * able to click or tab to.
       */
      .intro {
        opacity: 0;
        visibility: hidden;
      }

      .intro-ready {
        animation: intro-fade-in 320ms ease-out both;
        animation-delay: calc(var(--intro-order, 0) * 70ms);
      }

      .intro-order-1 {
        --intro-order: 1;
      }

      .intro-order-2 {
        --intro-order: 2;
      }

      .intro-instant {
        opacity: 1;
        visibility: visible;
        animation: none;
      }

      @keyframes intro-fade-in {
        from {
          opacity: 0;
          visibility: hidden;
        }
        to {
          opacity: 1;
          visibility: visible;
        }
      }

      @media (prefers-reduced-motion: reduce) {
        .intro {
          opacity: 1;
          visibility: visible;
          animation: none;
        }
      }

      .error {
        color: var(--wa-color-danger);
        font-size: var(--wa-font-size-s, 13px);
        margin: 0;
      }
    `,
  ];

  @state() private step: Step = 'arrival';
  @state() private reveal = 0;
  @state() private introSkipped = false;
  @state() private profile: BooksProfile = 'business';
  @state() private userName = '';
  @state() private companyName = '';
  @state() private password = '';
  @state() private confirm = '';
  @state() private dataDir = '';
  @state() private error = '';
  @state() private busy: SetupAction | 'load' | null = null;

  private store: AppStore = getAppStore();
  private ticker: ReturnType<typeof setInterval> | null = null;
  private elapsed = 0;

  connectedCallback(): void {
    super.connectedCallback();
    // At the window rather than on the stage: until the arrival's buttons
    // arrive nothing here takes focus, so a listener on the gate's own markup
    // would never be reached by the key it is waiting for.
    window.addEventListener('keydown', this.finishIntro);
    // `wc-wordmark` draws itself whole when motion is unwelcome and ignores
    // `reveal` entirely, so a ticker would re-render the gate thirty times
    // over for nothing.
    if (prefersReducedMotion()) {
      this.reveal = 1;
      return;
    }
    // Elapsed milliseconds rather than an accumulated fraction: adding
    // 40/1200 thirty times lands just short of 1, which leaves the mark a tick
    // shy of drawn and the copy waiting on it.
    this.ticker = setInterval(() => {
      this.elapsed += REVEAL_TICK_MS;
      this.reveal = Math.min(1, this.elapsed / REVEAL_MS);
      if (this.reveal >= 1) this.stopReveal();
    }, REVEAL_TICK_MS);
  }

  disconnectedCallback(): void {
    window.removeEventListener('keydown', this.finishIntro);
    this.stopReveal();
    super.disconnectedCallback();
  }

  private stopReveal(): void {
    if (this.ticker !== null) clearInterval(this.ticker);
    this.ticker = null;
  }

  /**
   * Any key during the intro lands it where it stands: the wordmark completes
   * and the copy arrives at once, on the step the reader is already on.
   */
  private finishIntro = (): void => {
    if (this.reveal >= 1) return;
    this.stopReveal();
    this.introSkipped = true;
    this.reveal = 1;
  };

  private beginSetup = (): void => {
    this.step = 'profile';
  };

  /** How the arrival's own elements enter, and in what order. */
  private introClass(order: number): string {
    if (this.introSkipped) return `intro intro-instant intro-order-${order}`;
    return this.reveal >= 1 ? `intro intro-ready intro-order-${order}` : 'intro';
  }

  private chooseProfile(profile: BooksProfile): void {
    this.profile = profile;
    this.step = 'identity';
  }

  private readField(
    event: Event,
    key: 'userName' | 'companyName' | 'password' | 'confirm' | 'dataDir',
  ): void {
    this[key] = (event.target as HTMLInputElement).value;
  }

  private submitIdentity(): void {
    if (this.password && this.password !== this.confirm) {
      this.error = "Those two don't match. Have another go.";
      return;
    }
    this.error = '';
    this.step = 'first-move';
  }

  private plan(action: SetupAction) {
    return {
      userName: this.userName.trim(),
      companyName: this.companyName.trim(),
      profile: this.profile,
      ...(this.password ? { password: this.password } : {}),
      action,
    };
  }

  private runSetup = async (action: SetupAction): Promise<void> => {
    this.busy = action;
    this.error = '';
    const outcome = await this.store.runSetup(this.plan(action));
    this.busy = null;
    // A success unmounts this screen: the boot phase moves to `ready` and the
    // shell takes over, so there is nothing to render here afterwards.
    if (!outcome.ok) this.error = `That didn't take. ${outcome.message}`;
  };

  private loadExisting = async (): Promise<void> => {
    const path = this.dataDir.trim();
    if (!path) {
      this.error = 'I need a directory to look in.';
      return;
    }
    this.busy = 'load';
    this.error = '';
    const outcome = await this.store.switchDataDir(path);
    this.busy = null;
    if (!outcome.ok) this.error = outcome.message;
  };

  render() {
    return html`
      <div class="stage">
        <wc-particle-field></wc-particle-field>
        <div class="panel">
          <wc-wordmark animated .reveal=${this.reveal}></wc-wordmark>
          ${this.renderStep()}
          ${this.error ? html`<p class="error" role="alert">${this.error}</p>` : nothing}
        </div>
      </div>
    `;
  }

  private renderStep() {
    switch (this.step) {
      case 'arrival':
        return html`
          <h1 class=${this.introClass(0)}>Hello. I'm Nigel.</h1>
          <p class=${this.introClass(1)}>
            I keep the books, privacy first. Just a few simple questions to start.
          </p>
          <div class=${`actions ${this.introClass(2)}`}>
            <wa-button
              appearance="outlined"
              ?disabled=${this.busy !== null}
              @click=${() => this.runSetup('demo')}
              >${this.busy === 'demo' ? 'Loading the demo…' : 'Show me the demo'}</wa-button
            >
            <wa-button variant="brand" @click=${this.beginSetup}>Set up my books</wa-button>
          </div>
        `;
      case 'profile':
        return html`
          <h1>Who are we keeping books for?</h1>
          <div class="cards">
            <button
              class="card"
              data-profile="business"
              @click=${() => this.chooseProfile('business')}
            >
              <strong>A business</strong>
              <span>
                Schedule C or 1120-S chart of accounts, with tax lines already
                mapped. Invoices, clients, the lot.
              </span>
            </button>
            <button
              class="card"
              data-profile="personal"
              @click=${() => this.chooseProfile('personal')}
            >
              <strong>Personal finances</strong>
              <span>A household chart. No tax mapping or invoices to chase.</span>
            </button>
          </div>
          <p class="footnote">
            This decides your chart of accounts, and can't be changed later. You can always create a new profile to manage another set of books.
          </p>
        `;
      case 'identity':
        return this.renderIdentity();
      case 'first-move':
        return this.renderFirstMove();
    }
  }

  private renderIdentity() {
    const personal = this.profile === 'personal';
    const companyLabel = personal ? 'Household name' : 'Business name';
    const companyHint = personal
      ? 'It goes on your reports.'
      : 'It goes on your reports and invoices.';
    return html`
      <h1>Tell me about yourself.</h1>
      <div class="form">
        <wa-input
          label="Your name"
          hint="So I know who I'm greeting. First name is plenty."
          .value=${this.userName}
          @input=${(e: Event) => this.readField(e, 'userName')}
        ></wa-input>
        <wa-input
          label=${companyLabel}
          hint=${companyHint}
          .value=${this.companyName}
          @input=${(e: Event) => this.readField(e, 'companyName')}
        ></wa-input>
        <wa-input
          type="password"
          label="Password (optional but recommended)"
          autocomplete="new-password"
          password-toggle
          hint="Encrypts the database file. Keep this safe — lose it and the books are gone."
          .value=${this.password}
          @input=${(e: Event) => this.readField(e, 'password')}
        ></wa-input>
        ${this.password
          ? html`<wa-input
              type="password"
              label="Type it again"
              autocomplete="new-password"
              .value=${this.confirm}
              @input=${(e: Event) => this.readField(e, 'confirm')}
            ></wa-input>`
          : nothing}
      </div>
      <div class="actions">
        <wa-button appearance="outlined" @click=${() => (this.step = 'profile')}>Back</wa-button>
        <wa-button variant="brand" @click=${() => this.submitIdentity()}>Carry on</wa-button>
      </div>
    `;
  }

  private renderFirstMove() {
    return html`
      <h1>How shall we start?</h1>
      <div class="cards">
        <div class="card">
          <strong>Start from scratch</strong>
          <span>
            An empty ledger and a chart of accounts. Import a statement when
            you're ready.
          </span>
          <wa-button
            variant="brand"
            ?disabled=${this.busy !== null}
            @click=${() => this.runSetup('fresh')}
            >${this.busy === 'fresh' ? 'Setting up…' : 'Start fresh'}</wa-button
          >
        </div>
        <div class="card">
          <strong>Load books I already have</strong>
          <span>Point me at a directory with a nigel.db in it.</span>
          <wa-input
            label="Data directory"
            placeholder="~/Documents/nigel"
            .value=${this.dataDir}
            @input=${(e: Event) => this.readField(e, 'dataDir')}
          ></wa-input>
          <wa-button ?disabled=${this.busy !== null} @click=${this.loadExisting}
            >${this.busy === 'load' ? 'Looking…' : 'Load them'}</wa-button
          >
        </div>
      </div>
      <div class="actions">
        <wa-button
          appearance="outlined"
          ?disabled=${this.busy !== null}
          @click=${() => (this.step = 'identity')}
          >Back</wa-button
        >
      </div>
    `;
  }
}

export function renderSetup(_ctx: ScreenContext): TemplateResult {
  return html`<nigel-setup-screen></nigel-setup-screen>`;
}

declare global {
  interface HTMLElementTagNameMap {
    'nigel-setup-screen': NigelSetupScreen;
  }
}
