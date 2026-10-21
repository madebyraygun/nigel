import { LitElement, html, css } from "lit";
import { customElement, property } from "lit/decorators.js";
import { controlsCss } from "@nigel/theme";
import { dispatchPaletteOpen } from "./wc-breadcrumb.js";
import "../icons/icons.js";

/**
 * The top bar's search control: a pill that asks for the whole palette
 * (`nc-palette-open` with scope `all`). The keycap shows the shortcut that
 * does the same; which key that is on this platform is the app's to say.
 */
@customElement("wc-search-pill")
export class WcSearchPill extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: inline-flex;
        flex: none;
      }

      button {
        display: inline-flex;
        align-items: center;
        gap: var(--wa-space-s);
        width: 240px;
        max-width: 100%;
        height: 32px;
        padding: 0 5px 0 12px;
        box-sizing: border-box;
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-pill);
        background: var(--wa-color-surface);
        color: var(--wa-color-muted);
        font-family: var(--wa-font-family-sans);
        font-size: 12.5px;
        cursor: default;
        transition: border-color var(--nc-transition-fast);
      }

      button:hover {
        border-color: var(--wa-color-brand);
      }

      wc-icon-search {
        --nc-icon-size: 15px;
      }

      .word {
        flex: 1;
        text-align: left;
      }

      kbd {
        padding: 0 7px;
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-pill);
        background: var(--wa-color-bg);
        font-family: inherit;
        font-size: var(--nc-font-size-caption);
        line-height: 18px;
      }

      /* A phone has no room for the pill, so it folds to its magnifier. */
      @media (max-width: 48rem) {
        button {
          justify-content: center;
          width: 32px;
          padding: 0;
        }

        .word,
        kbd {
          display: none;
        }
      }

      @media print {
        :host {
          display: none;
        }
      }
    `,
  ];

  /** The keycap's text. */
  @property({ type: String })
  shortcut = "⌘K";

  /** The same chord in `aria-keyshortcuts` form, e.g. `Meta+K`. */
  @property({ type: String, attribute: "key-shortcuts" })
  keyShortcuts = "Meta+K";

  render() {
    return html`
      <button
        type="button"
        aria-label="Search"
        aria-haspopup="dialog"
        aria-keyshortcuts=${this.keyShortcuts}
        @click=${(event: Event) => dispatchPaletteOpen(event.currentTarget as HTMLElement, "all")}
      >
        <wc-icon-search></wc-icon-search>
        <span class="word">Search</span>
        <kbd aria-hidden="true">${this.shortcut}</kbd>
      </button>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "wc-search-pill": WcSearchPill;
  }
}
