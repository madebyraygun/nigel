import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/**
 * A bordered card for a choice or a small form, in Setup's refreshed style.
 *
 * `interactive` draws it as one button: a title and a description that pick
 * something when clicked. Without it the card is a plain container, its
 * default slot holding whatever the choice needs — an input, a button. The
 * host receives the click either way, so a caller listens on the element.
 */
@customElement('wc-choice-card')
export class WcChoiceCard extends LitElement {
  static styles = css`
    :host {
      display: block;
    }

    .card {
      display: grid;
      gap: var(--wa-space-xs);
      align-content: start;
      box-sizing: border-box;
      width: 100%;
      height: 100%;
      margin: 0;
      padding: 18px;
      background: var(--wa-color-surface);
      border: 1px solid var(--wa-color-border);
      border-radius: 14px;
      color: inherit;
      font: inherit;
      text-align: left;
      transition:
        border-color var(--wa-transition-fast),
        box-shadow var(--wa-transition-fast);
    }

    button.card {
      cursor: pointer;
    }

    button.card:hover,
    :host([preview-state='hover']) button.card {
      border-color: var(--wa-color-brand);
      box-shadow: 0 0 0 3px var(--nc-color-ring);
    }

    button.card:focus-visible,
    :host([preview-state='focus']) button.card {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 2px;
    }

    /* Forced colours drop the ring and repaint borders and outlines in system
       colours, so the hover edge and the focus outline are stated in them. */
    @media (forced-colors: active) {
      button.card:hover,
      :host([preview-state='hover']) button.card {
        border-color: Highlight;
      }

      button.card:focus-visible,
      :host([preview-state='focus']) button.card {
        outline-color: Highlight;
      }
    }

    .title {
      font-size: 15px;
      font-weight: 600;
    }

    .description {
      color: var(--wa-color-muted);
      font-size: 13px;
      line-height: 1.5;
    }

    .body {
      display: grid;
      gap: var(--wa-space-s);
    }
  `;

  @property({ type: String })
  heading = '';

  @property({ type: String })
  description = '';

  /** Pins the hover or focus treatment, for the preview harness. */
  @property({ type: String, reflect: true, attribute: 'preview-state' })
  previewState: '' | 'hover' | 'focus' = '';

  /** Draws the card as a single button. */
  @property({ type: Boolean, reflect: true })
  interactive = false;

  private content() {
    return html`
      <span class="title">${this.heading}</span>
      ${this.description ? html`<span class="description">${this.description}</span>` : ''}
      <span class="body"><slot></slot></span>
    `;
  }

  render() {
    return this.interactive
      ? html`<button class="card" type="button">${this.content()}</button>`
      : html`<div class="card">${this.content()}</div>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-choice-card': WcChoiceCard;
  }
}
