import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { live } from 'lit/directives/live.js';
import { controlsCss, type ColorMode } from '@nigel/theme';
import type { NcColorModeChangeDetail } from './wc-mode-switcher.js';

const CHOICES: { mode: ColorMode; label: string }[] = [
  { mode: 'system', label: 'System' },
  { mode: 'light', label: 'Light' },
  { mode: 'dark', label: 'Dark' },
];

/**
 * System, Light or Dark as three cards, each a miniature window drawn in that
 * mode — System split corner to corner between the two.
 *
 * A native radio group under the cards, so the arrow keys and the group's
 * name come from the platform. Controlled like `wc-mode-switcher`: it reads
 * `mode`, emits `nc-color-mode-change`, and leaves storage and `<html>` to
 * its owner.
 */
@customElement('wc-appearance-picker')
export class WcAppearancePicker extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      fieldset {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        gap: var(--wa-space-m);
        margin: 0;
        padding: 0;
        border: 0;
      }

      legend {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip: rect(0, 0, 0, 0);
        white-space: nowrap;
      }

      .card {
        display: grid;
        gap: var(--wa-space-s);
        padding: var(--wa-space-s);
        border: 1.5px solid var(--wa-color-border);
        border-radius: var(--wa-radius-lg);
        background: var(--wa-color-surface);
        cursor: default;
      }

      .card.selected {
        border-color: transparent;
        background:
          linear-gradient(var(--wa-color-surface), var(--wa-color-surface)) padding-box,
          var(--nc-grad-brand-text) border-box;
      }

      .card:focus-within {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: 2px;
      }

      input {
        position: absolute;
        opacity: 0;
        width: 1px;
        height: 1px;
        margin: 0;
      }

      .window,
      .sidebar,
      .line,
      .mini-card {
        display: block;
      }

      .window {
        position: relative;
        height: 78px;
        border-radius: var(--wa-radius-md);
        overflow: hidden;
      }

      .scene {
        position: absolute;
        inset: 0;
        display: flex;
        gap: 5px;
        padding: 6px;
        border: 1px solid var(--pv-line);
        border-radius: inherit;
        background: var(--pv-bg);
      }

      .scene.light {
        --pv-bg: var(--nc-color-preview-light-bg);
        --pv-panel: var(--nc-color-preview-light-panel);
        --pv-card: var(--nc-color-preview-light-card);
        --pv-line: var(--nc-color-preview-light-line);
        --pv-accent: var(--nc-color-preview-light-accent);
      }

      .scene.dark {
        --pv-bg: var(--nc-color-preview-dark-bg);
        --pv-panel: var(--nc-color-preview-dark-panel);
        --pv-card: var(--nc-color-preview-dark-card);
        --pv-line: var(--nc-color-preview-dark-line);
        --pv-accent: var(--nc-color-preview-dark-accent);
      }

      /* System: the dark scene laid over the light one and cut on the
         diagonal, so each half shows one mode. */
      .scene.split {
        clip-path: polygon(100% 0, 100% 100%, 0 100%);
      }

      .sidebar {
        flex: none;
        width: 26%;
        border: 1px solid var(--pv-line);
        border-radius: 5px;
        background: var(--pv-panel);
      }

      .main {
        flex: 1;
        display: grid;
        align-content: start;
        gap: 5px;
        padding-top: 3px;
      }

      .line {
        height: 4px;
        border-radius: var(--wa-radius-pill);
        background: var(--pv-line);
      }

      .line.accent {
        width: 45%;
        background: var(--pv-accent);
      }

      .line.short {
        width: 70%;
      }

      .mini-card {
        height: 24px;
        border: 1px solid var(--pv-line);
        border-radius: 4px;
        background: var(--pv-card);
      }

      .caption {
        display: flex;
        align-items: center;
        gap: var(--wa-space-s);
        padding: 0 2px 2px;
        font-size: var(--nc-font-size-control);
      }

      .dot {
        flex: none;
        width: 14px;
        height: 14px;
        box-sizing: border-box;
        border: 1.5px solid var(--wa-color-border);
        border-radius: 50%;
      }

      .selected .dot {
        border: 4px solid var(--wa-color-brand);
      }

      @media (max-width: 30rem) {
        fieldset {
          grid-template-columns: 1fr;
        }
      }

      @media print {
        :host {
          display: none;
        }
      }
    `,
  ];

  @property({ type: String })
  mode: ColorMode = 'system';

  private choose(mode: ColorMode): void {
    this.dispatchEvent(
      new CustomEvent<NcColorModeChangeDetail>('nc-color-mode-change', {
        detail: { mode },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private scene(tone: 'light' | 'dark', split = false) {
    return html`<span class="scene ${tone} ${split ? 'split' : ''}">
      <span class="sidebar"></span>
      <span class="main">
        <span class="line accent"></span>
        <span class="line short"></span>
        <span class="mini-card"></span>
      </span>
    </span>`;
  }

  render() {
    return html`
      <fieldset>
        <legend>Appearance</legend>
        ${CHOICES.map(({ mode, label }) => {
          const selected = this.mode === mode;
          return html`<label class=${selected ? 'card selected' : 'card'}>
            <input
              type="radio"
              name="appearance"
              value=${mode}
              .checked=${live(selected)}
              @change=${() => this.choose(mode)}
            />
            <span class="window" aria-hidden="true">
              ${mode === 'system'
                ? html`${this.scene('light')}${this.scene('dark', true)}`
                : this.scene(mode)}
            </span>
            <span class="caption"><span class="dot" aria-hidden="true"></span>${label}</span>
          </label>`;
        })}
      </fieldset>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-appearance-picker': WcAppearancePicker;
  }
}
