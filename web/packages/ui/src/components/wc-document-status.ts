import { LitElement, html, css, type TemplateResult } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '../icons/icons.js';

/** The seven document statuses, as the server spells them. */
export const DOCUMENT_STATUS_WORDS = [
  'draft',
  'sent',
  'changes_requested',
  'accepted',
  'declined',
  'executed',
  'withdrawn',
] as const;

export type DocumentStatusWord = (typeof DOCUMENT_STATUS_WORDS)[number];

/** A status as a person reads it; one the list does not cover comes back unchanged. */
export function documentStatusLabel(status: string): string {
  return status === 'changes_requested' ? 'changes requested' : status;
}

/**
 * Decorative marks: the word beside each one carries the meaning. Keyed through
 * a `Map` because `status` is unvetted text and an object lookup would answer
 * `constructor` with an inherited function.
 */
const ICON_FOR_STATUS = new Map<string, TemplateResult>([
  ['draft', html`<wc-icon-status-draft inline class="mark"></wc-icon-status-draft>`],
  ['sent', html`<wc-icon-status-sent inline class="mark"></wc-icon-status-sent>`],
  [
    'changes_requested',
    html`<wc-icon-status-partial inline class="mark"></wc-icon-status-partial>`,
  ],
  ['accepted', html`<wc-icon-check inline class="mark"></wc-icon-check>`],
  ['declined', html`<wc-icon-close inline class="mark"></wc-icon-close>`],
  ['executed', html`<wc-icon-status-paid inline class="mark"></wc-icon-status-paid>`],
  ['withdrawn', html`<wc-icon-status-void inline class="mark"></wc-icon-status-void>`],
]);

const UNKNOWN_STATUS_ICON = html`<wc-icon-dot inline class="mark"></wc-icon-dot>`;

@customElement('wc-document-status')
export class WcDocumentStatus extends LitElement {
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

    .chip[data-status='changes_requested'] {
      color: var(--nc-color-flagged);
      border-color: currentColor;
    }

    .chip[data-status='accepted'],
    .chip[data-status='executed'] {
      color: var(--nc-color-income);
      border-color: currentColor;
    }

    .chip[data-status='declined'] {
      color: var(--nc-color-expense);
      border-color: currentColor;
    }

    .chip[data-status='withdrawn'] {
      color: var(--wa-color-muted);
      text-decoration: line-through;
    }
  `;

  /** The status as the server spelled it; unknown values render as-is. */
  @property({ type: String, reflect: true })
  status = 'draft';

  render() {
    return html`
      <span class="chip" part="chip" data-status=${this.status}>
        ${ICON_FOR_STATUS.get(this.status) ?? UNKNOWN_STATUS_ICON}
        <span class="word">${documentStatusLabel(this.status)}</span>
      </span>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-document-status': WcDocumentStatus;
  }
}
