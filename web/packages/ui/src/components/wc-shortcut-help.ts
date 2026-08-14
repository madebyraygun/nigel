import { LitElement, html, css } from 'lit';
import { customElement, property, query } from 'lit/decorators.js';

let instances = 0;

/** One line of a shortcut legend. */
export interface ShortcutHint {
  /**
   * The `KeyboardEvent.key` values this line stands for. The legend does not
   * print them — it prints `display` — but naming them here is what lets a
   * screen's tests walk the legend and prove each key does something.
   */
  keys: string[];
  /** How the keys read on screen, e.g. `↑ ↓`. */
  display: string;
  description: string;
}

/**
 * A screen's keyboard legend, behind a trigger, in a popover.
 *
 * The panel is absolutely positioned against the trigger, so opening it moves
 * nothing on the page — the previous inline `<details>` pushed the toolbar and
 * the table down every time it was opened.
 *
 * It is a disclosure rather than a dialog: the content is a definition list
 * with nothing focusable in it, so focus stays on the trigger and the panel
 * follows it in reading order, which is what a screen reader wants. `Escape`
 * and a click outside both close it, and closing always leaves focus on the
 * trigger the user opened it from.
 *
 * The trigger is a plain `<button>` rather than `wa-button` because
 * `aria-expanded` and `aria-controls` have to land on the real button, and a
 * Web Awesome button keeps its own inside a shadow root this component cannot
 * reach.
 */
@customElement('wc-shortcut-help')
export class WcShortcutHelp extends LitElement {
  static styles = css`
    :host {
      display: inline-block;
      position: relative;
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-s, 13px);
      color: var(--wa-color-muted);
    }

    .trigger {
      font: inherit;
      color: inherit;
      background: none;
      border: 1px solid transparent;
      border-radius: var(--wa-radius-sm, 6px);
      padding: var(--wa-space-2xs, 4px) var(--wa-space-xs, 6px);
      cursor: pointer;
    }

    .trigger:hover,
    .trigger[aria-expanded='true'] {
      color: var(--wa-color-text);
      border-color: var(--wa-color-border);
    }

    .trigger:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: 1px;
    }

    /* Out of flow, so opening the legend never moves the toolbar or the table
       under it. */
    .panel {
      position: absolute;
      z-index: 20;
      top: calc(100% + var(--wa-space-2xs, 4px));
      inset-inline-end: 0;
      min-width: 16rem;
      max-width: min(24rem, 90vw);
      padding: var(--wa-space-s, 8px) var(--wa-space-m, 12px);
      background: var(--wa-color-surface);
      color: var(--wa-color-text);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-md, 8px);
      box-shadow: var(--wa-shadow-m, 0 4px 12px rgba(0, 0, 0, 0.25));
    }

    .panel h2 {
      margin: 0 0 var(--wa-space-xs, 6px);
      font-size: var(--wa-font-size-s, 13px);
      font-weight: var(--wa-font-weight-medium, 500);
      color: var(--wa-color-muted);
    }

    dl {
      display: grid;
      grid-template-columns: max-content 1fr;
      gap: var(--wa-space-2xs, 4px) var(--wa-space-s, 8px);
      margin: 0;
    }

    dt {
      font-family: var(--wa-font-family-mono, monospace);
      color: var(--wa-color-text);
      white-space: nowrap;
    }

    dd {
      margin: 0;
      color: var(--wa-color-muted);
    }

    /* Screen chrome, not part of the report. */
    @media print {
      :host {
        display: none;
      }
    }
  `;

  @property({ attribute: false })
  shortcuts: ShortcutHint[] = [];

  /** The trigger's text. */
  @property({ type: String })
  label = 'Keyboard';

  /** The panel's heading, which is also its accessible name. */
  @property({ type: String })
  heading = 'Keyboard shortcuts';

  @property({ type: Boolean, reflect: true })
  open = false;

  /** Unique per instance, because `aria-controls` needs an id to point at. */
  private readonly panelId = `shortcut-help-${(instances += 1)}`;

  @query('.trigger') private trigger?: HTMLButtonElement;

  connectedCallback(): void {
    super.connectedCallback();
    // On the document rather than on the host: a pointer press anywhere else
    // dismisses, and Escape has to work even after a click on the panel's own
    // text has moved focus off the trigger.
    document.addEventListener('pointerdown', this.handleDocumentPointerDown, true);
    document.addEventListener('keydown', this.handleDocumentKeydown, true);
  }

  disconnectedCallback(): void {
    document.removeEventListener('pointerdown', this.handleDocumentPointerDown, true);
    document.removeEventListener('keydown', this.handleDocumentKeydown, true);
    super.disconnectedCallback();
  }

  /** Close, and hand focus back to the trigger the user opened this from. */
  hide(): void {
    if (!this.open) return;
    this.open = false;
    void this.updateComplete.then(() => this.trigger?.focus());
  }

  show(): void {
    this.open = true;
  }

  private toggle = (): void => {
    if (this.open) this.hide();
    else this.show();
  };

  private handleDocumentPointerDown = (event: Event): void => {
    if (!this.open) return;
    // `composedPath` is the only way to ask whether a press landed inside a
    // shadow root; `event.target` is retargeted to this host for our own DOM.
    if (event.composedPath().includes(this)) return;
    this.open = false;
  };

  private handleDocumentKeydown = (event: KeyboardEvent): void => {
    if (!this.open || event.key !== 'Escape') return;
    event.stopPropagation();
    event.preventDefault();
    this.hide();
  };

  render() {
    return html`
      <button
        class="trigger"
        type="button"
        aria-expanded=${this.open ? 'true' : 'false'}
        aria-controls=${this.panelId}
        @click=${this.toggle}
      >
        ${this.label}
      </button>
      <div
        class="panel"
        id=${this.panelId}
        role="group"
        aria-label=${this.heading}
        ?hidden=${!this.open}
      >
        <h2>${this.heading}</h2>
        <dl>
          ${this.shortcuts.map(
            (hint) => html`
              <dt>${hint.display}</dt>
              <dd>${hint.description}</dd>
            `,
          )}
        </dl>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-shortcut-help': WcShortcutHelp;
  }
}
