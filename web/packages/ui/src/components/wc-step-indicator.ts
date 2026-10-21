import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/**
 * Where Setup is, as a row of pills: the current step wide, the rest narrow.
 * Completed and current pills are brand-coloured, future ones border-coloured.
 * Announced as one image ("Step 2 of 4") rather than as four decorations.
 */
@customElement('wc-step-indicator')
export class WcStepIndicator extends LitElement {
  static styles = css`
    :host {
      display: inline-block;
    }

    .row {
      display: flex;
      gap: 6px;
      align-items: center;
    }

    .pill {
      width: 6px;
      height: 6px;
      border-radius: 3px;
      background: var(--wa-color-border);
      transition:
        width 200ms ease,
        background-color 200ms ease;
    }

    .pill.done {
      background: var(--wa-color-brand);
    }

    .pill.current {
      width: 22px;
    }

    @media (prefers-reduced-motion: reduce) {
      .pill {
        transition: none;
      }
    }
  `;

  /** How many steps there are. */
  @property({ type: Number })
  total = 4;

  /** The current step, counted from zero. */
  @property({ type: Number })
  current = 0;

  render() {
    const pills = Array.from({ length: this.total }, (_, i) => {
      const cls =
        i < this.current ? 'pill done' : i === this.current ? 'pill done current' : 'pill';
      return html`<span class=${cls}></span>`;
    });
    return html`<div
      class="row"
      role="img"
      aria-label=${`Step ${this.current + 1} of ${this.total}`}
    >
      ${pills}
    </div>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-step-indicator': WcStepIndicator;
  }
}
