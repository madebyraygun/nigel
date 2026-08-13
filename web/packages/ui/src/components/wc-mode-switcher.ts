import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/radio-group/radio-group.js';
import '@awesome.me/webawesome/dist/components/radio/radio.js';
import { COLOR_MODES, controlsCss, type ColorMode, type ResolvedMode } from '@nigel/theme';

export interface NcColorModeChangeDetail {
  mode: ColorMode;
}

function isColorMode(value: string): value is ColorMode {
  return (COLOR_MODES as readonly string[]).includes(value);
}

/**
 * Light, dark, or follow the system.
 *
 * A radio group rather than a two-way toggle, because the third state is the
 * point: once you have chosen dark, "go back to following the OS" has to stay
 * reachable, and a toggle has nowhere to put it. `role="radiogroup"` is also
 * exactly the semantic — pick one of these — and brings arrow-key navigation
 * and a group label with it, where a hand-built version would mean
 * re-implementing roving tabindex (see `wc-register-table` for how much work
 * that is). A `wa-select` was the other option and hides two of three choices
 * behind a popover.
 *
 * Labels are words, not icons alone: an icon-only control needs a
 * visually-hidden label to pass, which is the same WCAG 1.4.1 reasoning
 * `wc-money` and `wc-invoice-status` already follow.
 *
 * **Fully controlled.** It reads `mode`, emits `nc-color-mode-change`, and
 * never writes storage or touches `<html>`. The container owns persistence —
 * which is also what keeps the preview harness from changing the real app's
 * appearance while someone reviews the states.
 *
 * `resolved` is what `system` currently means, for the hint. It is passed in
 * rather than read here because jsdom's `matchMedia` always answers
 * `matches: false` and never fires `change`, so a component that read it
 * directly could not be tested in either state.
 */
@customElement('wc-mode-switcher')
export class WcModeSwitcher extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .hint {
        margin: var(--wa-space-xs, 6px) 0 0;
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
      }

      /* Screen chrome, not part of the report — see wc-app-shell. */
      @media print {
        :host {
          display: none;
        }
      }
    `,
  ];

  @property({ type: String })
  mode: ColorMode = 'system';

  @property({ type: String })
  resolved: ResolvedMode = 'light';

  private handleChange(event: Event) {
    const value = (event.target as HTMLElement & { value?: string }).value ?? '';
    if (!isColorMode(value) || value === this.mode) return;
    this.dispatchEvent(
      new CustomEvent<NcColorModeChangeDetail>('nc-color-mode-change', {
        detail: { mode: value },
        bubbles: true,
        composed: true,
      }),
    );
  }

  render() {
    return html`
      <wa-radio-group
        label="Appearance"
        orientation="horizontal"
        value=${this.mode}
        @change=${this.handleChange}
      >
        <wa-radio value="light">Light</wa-radio>
        <wa-radio value="dark">Dark</wa-radio>
        <wa-radio value="system">System</wa-radio>
      </wa-radio-group>
      ${this.mode === 'system'
        ? html`<p class="hint">Following your device — currently ${this.resolved}.</p>`
        : nothing}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-mode-switcher': WcModeSwitcher;
  }
}
