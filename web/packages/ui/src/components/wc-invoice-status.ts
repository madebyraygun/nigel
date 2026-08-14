import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '../icons/icons.js';
import type { IconTag } from '../icons/icons.js';

/**
 * The six derived statuses, in the order `refresh_status` reasons about them.
 *
 * Kept as data here rather than imported from the app: `@nigel/ui` never sees
 * an API type, and a status the server invents tomorrow still renders — as its
 * own word, with a neutral glyph.
 */
export const INVOICE_STATUS_WORDS = [
  'draft',
  'sent',
  'partial',
  'paid',
  'overdue',
  'void',
] as const;

export type InvoiceStatusWord = (typeof INVOICE_STATUS_WORDS)[number];

/**
 * An icon per status, matching the wireframes and the TUI's own shorthand.
 *
 * The icon is decorative — the word beside it is what carries the meaning.
 * Both are rendered because colour alone cannot be the only channel (WCAG
 * 1.4.1), which is the same reason `wc-money` always prints its sign.
 *
 * They are SVGs rather than characters because IBM Plex Mono, the app's
 * primary face, has a glyph for none of the six: drawn as text they each come
 * from whatever fallback face the browser finds.
 */
const STATUS_ICONS = {
  draft: 'wc-icon-status-draft',
  sent: 'wc-icon-status-sent',
  partial: 'wc-icon-status-partial',
  paid: 'wc-icon-status-paid',
  overdue: 'wc-icon-status-overdue',
  void: 'wc-icon-status-void',
} satisfies Record<InvoiceStatusWord, IconTag>;

/** What a status the six do not cover gets: a neutral mark, and its own word. */
const UNKNOWN_STATUS_ICON: IconTag = 'wc-icon-dot';

@customElement('wc-invoice-status')
export class WcInvoiceStatus extends LitElement {
  static styles = css`
    :host {
      display: inline-block;
    }

    .chip {
      display: inline-flex;
      align-items: center;
      gap: var(--wa-space-2xs, 4px);
      padding: 0 var(--wa-space-xs, 6px);
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-pill, 999px);
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-s, 13px);
      line-height: 1.7;
      white-space: nowrap;
      color: var(--wa-color-text);
    }

    .glyph {
      --nc-icon-size: 1em;
    }

    .chip[data-status='draft'] {
      color: var(--wa-color-muted);
    }

    .chip[data-status='sent'] {
      color: var(--wa-color-brand);
      border-color: currentColor;
    }

    /*
     * Partial reads as the flagged token rather than a warning colour of its
     * own. Every value here must be a token @nigel/theme defines in both
     * schemes and holds to WCAG AA — a literal fallback that can actually win
     * is a colour the contrast test never sees.
     */
    .chip[data-status='partial'] {
      color: var(--nc-color-flagged);
      border-color: currentColor;
    }

    .chip[data-status='paid'] {
      color: var(--nc-color-income);
      border-color: currentColor;
    }

    .chip[data-status='overdue'] {
      color: var(--nc-color-expense);
      border-color: currentColor;
    }

    .chip[data-status='void'] {
      color: var(--wa-color-muted);
      text-decoration: line-through;
    }
  `;

  /** The status word as the server spelled it; unknown values render as-is. */
  @property({ type: String, reflect: true })
  status = 'draft';

  render() {
    return html`
      <span class="chip" part="chip" data-status=${this.status}>
        ${this.renderGlyph()}
        <span class="word">${this.status}</span>
      </span>
    `;
  }

  private renderGlyph() {
    const tag =
      STATUS_ICONS[this.status as InvoiceStatusWord] ?? UNKNOWN_STATUS_ICON;
    // The icon names a custom element at runtime, so it is created
    // imperatively rather than through a static template tag — wc-empty-state
    // resolves its `icon` property the same way.
    const el = document.createElement(tag);
    el.classList.add('glyph');
    return el;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-status': WcInvoiceStatus;
  }
}
