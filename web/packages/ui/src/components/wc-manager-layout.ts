import { LitElement, html, css, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '../icons/icons.js';
import './wc-spinner.js';
import { controlsCss } from '@nigel/theme';

/**
 * The frame the manager screens share: the page's title row (its h1 and an
 * Add button), a filter row, a list, and one place where a guardrail lands.
 * A screen built on it registers `heading: 'screen'`, so the shell draws no
 * second title above this one.
 *
 * One layout for clients, accounts, categories and rules because the screens are
 * the same screen with different columns — a list you add to, edit rows of, and
 * delete rows from. The part worth sharing is not the box: it is `error`, the
 * inline region a refused delete renders into. `confirmDialog()` resolves and
 * removes itself before the request is sent, so a 409 raised by a delete has no
 * dialog left to appear in, and a toast would take the count away before it had
 * been read. It goes here instead, as a `role="alert"`, above the row the user
 * was just looking at.
 */
@customElement('wc-manager-layout')
export class WcManagerLayout extends LitElement {
  static styles = [
    controlsCss,
    css`
      /* A column, and one that takes the whole screen: the empty slot holds a
         wc-empty-state, which centres itself in the space under the header
         only if there is space under the header to have. The shell's content
         area already pads the screen, so the frame adds none of its own. */
      :host {
        display: flex;
        flex-direction: column;
        flex: 1 1 auto;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      /* The refresh's title row: the page h1 on the left, the screen's
         primary action on the right, wrapping under the title on a phone. */
      header {
        display: flex;
        flex-wrap: wrap;
        align-items: flex-start;
        justify-content: space-between;
        gap: 12px;
        margin-bottom: 18px;
      }

      .heading {
        margin: 0;
        font-size: var(--nc-font-size-display);
        font-weight: var(--wa-font-weight-medium);
        letter-spacing: var(--nc-letter-spacing-tight);
        line-height: 1.2;
      }

      .count {
        color: var(--wa-color-muted);
        font-weight: var(--wa-font-weight-normal);
        font-variant-numeric: tabular-nums;
      }

      .description {
        margin: 4px 0 0;
        color: var(--wa-color-muted);
        font-size: var(--nc-font-size-control);
        max-width: 68ch;
      }

      .error {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--wa-space-s);
        margin-bottom: var(--nc-toolbar-gap);
        padding: 10px 14px;
        border: 1px solid var(--wa-color-danger);
        border-radius: var(--wa-radius-lg);
        background: var(--wa-color-danger-fill-quiet);
        font-size: var(--nc-font-size-control);
      }

      .error p {
        margin: 0;
        flex: 1 1 16rem;
      }

      .error-actions {
        display: flex;
        gap: var(--wa-space-xs);
        flex: 0 0 auto;
      }

      /* The list's filter row, laid out as wc-filtered-list lays out its own:
         one wrapping, vertically centred row, --nc-toolbar-gap above the
         table. Nothing slotted leaves no row and no gap. */
      .toolbar {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--nc-filter-row-gap);
        margin-bottom: var(--nc-toolbar-gap);
        font-size: var(--nc-font-size-control);
      }

      .toolbar[hidden] {
        display: none;
      }

      .busy {
        display: flex;
        justify-content: center;
        padding: var(--wa-space-xl);
      }

      /* Block flow for the sheets: a flex container is not required to
         fragment, and a client list long enough to print is exactly where
         Safari and older Chromium slice through a row. */
      @media print {
        :host {
          display: block;
        }
      }
    `,
  ];

  @property({ type: String })
  heading = '';

  @property({ type: String })
  description = '';

  /** Rendered beside the heading, as the TUI titles do. Null hides it. */
  @property({ type: Number })
  count: number | null = null;

  @property({ type: String, attribute: 'add-label' })
  addLabel = 'Add';

  @property({ type: Boolean, attribute: 'add-disabled' })
  addDisabled = false;

  /** A first load in flight: the list is replaced by a spinner. */
  @property({ type: Boolean, reflect: true })
  busy = false;

  /** Show the `empty` slot in place of the list. */
  @property({ type: Boolean, reflect: true })
  empty = false;

  /** Screen-level feedback — a refused delete, or a list that would not load. */
  @property({ type: String })
  error: string | null = null;

  /** An optional button beside the error, e.g. "Show those rules". */
  @property({ type: String, attribute: 'error-action-label' })
  errorActionLabel = '';

  private emit(name: string): void {
    this.dispatchEvent(new CustomEvent(name, { bubbles: true, composed: true }));
  }

  @state()
  private hasToolbar = false;

  private handleToolbarChange(event: Event): void {
    this.hasToolbar = (event.target as HTMLSlotElement).assignedElements().length > 0;
  }

  private handleAdd = () => this.emit('nc-manager-add');
  private handleErrorAction = () => this.emit('nc-manager-error-action');
  private handleErrorDismiss = () => this.emit('nc-manager-error-dismiss');

  render() {
    return html`
      <header>
        <div>
          <h1 class="heading">
            ${this.heading}
            ${this.count === null
              ? nothing
              : html`<span class="count">(${this.count})</span>`}
          </h1>
          ${this.description
            ? html`<p class="description">${this.description}</p>`
            : nothing}
        </div>
        <wa-button
          data-add
          variant="brand"
          ?disabled=${this.addDisabled}
          @click=${this.handleAdd}
        >
          <wc-icon-plus slot="start"></wc-icon-plus>
          ${this.addLabel}
        </wa-button>
      </header>

      ${this.error
        ? html`
            <div class="error" role="alert">
              <p>${this.error}</p>
              <div class="error-actions">
                ${this.errorActionLabel
                  ? html`<wa-button
                      data-error-action
                      size="s"
                      appearance="outlined"
                      @click=${this.handleErrorAction}
                      >${this.errorActionLabel}</wa-button
                    >`
                  : nothing}
                <wa-button
                  data-error-dismiss
                  size="s"
                  appearance="plain"
                  @click=${this.handleErrorDismiss}
                  >Dismiss</wa-button
                >
              </div>
            </div>
          `
        : nothing}

      <div class="toolbar" ?hidden=${!this.hasToolbar}>
        <slot name="toolbar" @slotchange=${this.handleToolbarChange}></slot>
      </div>

      ${this.busy
        ? html`<div class="busy">
            <wc-spinner size="l" label="Loading" show-label></wc-spinner>
          </div>`
        : this.empty
          ? html`<slot name="empty"></slot>`
          : html`<slot></slot>`}

      <slot name="overlay"></slot>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-manager-layout': WcManagerLayout;
  }
}
