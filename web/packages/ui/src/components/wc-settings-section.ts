import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/**
 * One section of the Settings page: a title, a line saying what it is for,
 * and the controls in the default slot. Sections stack under a hairline.
 *
 * `saved` puts a "✓ Saved" beside the title. The status region is always
 * there, empty when there is nothing to say, so a save that completes is
 * announced rather than only shown.
 */
@customElement('wc-settings-section')
export class WcSettingsSection extends LitElement {
  static styles = css`
    :host {
      display: block;
      padding: 22px 0 26px;
      border-top: 1px solid var(--wa-color-border);
      scroll-margin-top: var(--wa-space-s);
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    header {
      display: flex;
      align-items: baseline;
      gap: var(--wa-space-m);
    }

    h3 {
      margin: 0;
      font-size: var(--nc-font-size-section);
      font-weight: var(--wa-font-weight-semibold);
    }

    .saved {
      font-size: var(--nc-font-size-meta);
      color: var(--nc-color-income);
    }

    .description {
      margin: var(--wa-space-xs) 0 0;
      font-size: var(--nc-font-size-control);
      color: var(--wa-color-muted);
    }

    .body {
      margin-top: var(--wa-space-l);
    }
  `;

  @property({ type: String })
  heading = '';

  @property({ type: String })
  description = '';

  /** Shows "✓ Saved" beside the title while true. */
  @property({ type: Boolean, reflect: true })
  saved = false;

  render() {
    return html`
      <section aria-labelledby="heading">
        <header>
          <h3 id="heading">${this.heading}</h3>
          <span class="saved" role="status">${this.saved ? '✓ Saved' : ''}</span>
        </header>
        ${this.description ? html`<p class="description">${this.description}</p>` : nothing}
        <div class="body"><slot></slot></div>
      </section>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-settings-section': WcSettingsSection;
  }
}
