import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/**
 * The six derived statuses, in the order `refresh_status` reasons about them.
 *
 * Kept as data here rather than imported from the app: `@nigel/ui` never sees
 * an API type, and a status the server invents tomorrow still renders — as its
 * own word, in the neutral colour.
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
 * An invoice's status as a pill: a 1px border, a 6px dot and the word, all in
 * the status colour.
 *
 * The word is what carries the meaning — colour is never the only channel
 * (WCAG 1.4.1) — so the dot is decoration and the word is announced as
 * written. It is capitalised by CSS rather than in the text, so a screen reader
 * and a copy both get the server's own word.
 */
@customElement('wc-invoice-status')
export class WcInvoiceStatus extends LitElement {
  static styles = css`
    :host {
      display: inline-block;
      vertical-align: middle;
    }

    .chip {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      box-sizing: border-box;
      height: 22px;
      padding: 0 9px;
      border: 1px solid currentColor;
      border-radius: var(--wa-radius-pill);
      font-family: var(--wa-font-family-sans);
      font-size: var(--nc-font-size-meta);
      font-weight: var(--wa-font-weight-medium);
      line-height: 1;
      white-space: nowrap;
      color: var(--wa-color-muted);
    }

    :host([size='l']) .chip {
      height: 24px;
      padding: 0 10px;
    }

    .dot {
      flex: none;
      width: 6px;
      height: 6px;
      border-radius: 50%;
      background: currentColor;
    }

    .word {
      text-transform: capitalize;
    }

    .chip[data-status='sent'] {
      color: var(--wa-color-info);
    }

    /*
     * Partial reads as the flagged token rather than a warning colour of its
     * own. Every value here must be a token @nigel/theme defines in both
     * schemes and holds to WCAG AA — a literal fallback that can actually win
     * is a colour the contrast test never sees.
     */
    .chip[data-status='partial'] {
      color: var(--nc-color-flagged);
    }

    .chip[data-status='paid'] {
      color: var(--nc-color-income);
    }

    .chip[data-status='overdue'] {
      color: var(--nc-color-expense);
    }

    .chip[data-status='void'] .word {
      text-decoration: line-through;
    }
  `;

  /** The status word as the server spelled it; unknown values render as-is. */
  @property({ type: String, reflect: true })
  status = 'draft';

  /** `l` is the 24px pill beside a page title; the default is the 22px row pill. */
  @property({ type: String, reflect: true })
  size: 'm' | 'l' = 'm';

  render() {
    return html`
      <span class="chip" part="chip" data-status=${this.status}>
        <span class="dot" aria-hidden="true"></span>
        <span class="word">${this.status}</span>
      </span>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-invoice-status': WcInvoiceStatus;
  }
}
