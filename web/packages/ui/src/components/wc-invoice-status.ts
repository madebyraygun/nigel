import { LitElement, html, css, type TemplateResult } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '../icons/icons.js';

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
  draft: html`<wc-icon-status-draft inline class="mark"></wc-icon-status-draft>`,
  sent: html`<wc-icon-status-sent inline class="mark"></wc-icon-status-sent>`,
  partial: html`<wc-icon-status-partial inline class="mark"></wc-icon-status-partial>`,
  paid: html`<wc-icon-status-paid inline class="mark"></wc-icon-status-paid>`,
  overdue: html`<wc-icon-status-overdue inline class="mark"></wc-icon-status-overdue>`,
  void: html`<wc-icon-status-void inline class="mark"></wc-icon-status-void>`,
} satisfies Record<InvoiceStatusWord, TemplateResult>;

/**
 * The same six, keyed for lookup by a string nobody vouched for.
 *
 * `status` is whatever the server wrote, and `invoices.status` has no CHECK
 * constraint: `constructor` and `toString` are values a hand-edited row can
 * hold, and an object lookup would answer those with an inherited function.
 */
const ICON_FOR_STATUS = new Map<string, TemplateResult>(Object.entries(STATUS_ICONS));

/** What a status the six do not cover gets: a neutral mark, and its own word. */
const UNKNOWN_STATUS_ICON = html`<wc-icon-dot inline class="mark"></wc-icon-dot>`;

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
        ${ICON_FOR_STATUS.get(this.status) ?? UNKNOWN_STATUS_ICON}
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
