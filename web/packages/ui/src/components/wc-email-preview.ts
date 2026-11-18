import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import './wc-notice-bar.js';
import './wc-spinner.js';

/** One email as the server renders it for a preview. */
/** The placeholders an email greeting may name, written `${name}`, per kind. */
export const GREETING_PLACEHOLDERS: Record<'invoice' | 'document', readonly string[]> = {
  invoice: ['firstName', 'fullName', 'clientName', 'companyName', 'invoiceNumber', 'amountDue', 'dueDate'],
  document: ['firstName', 'fullName', 'clientName', 'companyName', 'documentTitle'],
};

/** The placeholders of one kind as the text a person types. */
export function greetingPlaceholderList(kind: 'invoice' | 'document'): string {
  return GREETING_PLACEHOLDERS[kind].map((name) => `\${${name}}`).join(', ');
}

export interface EmailPreviewView {
  role: string;
  /** `Name <email>`. */
  to: string;
  cc?: string[];
  subject: string;
  body: string;
  attachments: string[];
}

const ROLE_LABELS: Record<string, string> = {
  client: 'Client',
  signer: 'Signer',
  collaborator: 'Collaborator',
};

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * The body split around every placeholder, so each one can be marked. Longest
 * first, so a placeholder that contains another is matched whole.
 */
export function splitPlaceholders(
  body: string,
  placeholders: string[],
): { text: string; placeholder: boolean }[] {
  const needles = placeholders.filter((p) => p !== '').sort((a, b) => b.length - a.length);
  if (needles.length === 0) return [{ text: body, placeholder: false }];
  const pattern = new RegExp(`(${needles.map(escapeRegExp).join('|')})`);
  return body
    .split(pattern)
    .filter((part) => part !== '')
    .map((text) => ({ text, placeholder: needles.includes(text) }));
}

/**
 * The email a send would deliver: To, subject, attachments and the plain-text
 * body, one block per recipient. Links that only exist once the send runs are
 * marked, so nobody mistakes the stand-in for the real thing.
 *
 * While a refresh is in flight the last preview stays in place, dimmed, rather
 * than collapsing to a spinner on every keystroke.
 */
@customElement('wc-email-preview')
export class WcEmailPreview extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-sans);
      color: var(--wa-color-text);
    }

    .heading {
      display: flex;
      align-items: baseline;
      gap: var(--wa-space-xs);
      margin: 0 0 var(--wa-space-xs);
      font-size: var(--wa-font-size-s);
      font-weight: var(--wa-font-weight-semibold);
    }

    .updating {
      color: var(--wa-color-muted);
      font-weight: var(--wa-font-weight-normal);
    }

    .emails {
      display: flex;
      flex-direction: column;
      gap: var(--wa-space-s);
      margin: 0;
      padding: 0;
      list-style: none;
      transition: opacity 120ms ease;
    }

    :host([loading]) .emails {
      opacity: 0.6;
    }

    .email {
      border: 1px solid var(--wa-color-border);
      border-radius: var(--wa-radius-m);
      background: var(--wa-color-surface);
      overflow: hidden;
    }

    dl {
      display: grid;
      grid-template-columns: max-content minmax(0, 1fr);
      gap: var(--wa-space-2xs) var(--wa-space-m);
      margin: 0;
      padding: var(--wa-space-s) var(--wa-space-m);
      border-bottom: 1px solid var(--wa-color-border-soft);
      background: var(--wa-color-surface-alt);
      font-size: var(--wa-font-size-s);
    }

    dt {
      color: var(--wa-color-muted);
    }

    dd {
      margin: 0;
      overflow-wrap: anywhere;
      user-select: text;
      -webkit-user-select: text;
    }

    .role {
      color: var(--wa-color-muted);
    }

    .body {
      margin: 0;
      padding: var(--wa-space-m);
      max-height: 18rem;
      overflow: auto;
      font-family: var(--wa-font-family-sans);
      font-size: var(--wa-font-size-m);
      line-height: 1.55;
      white-space: pre-wrap;
      overflow-wrap: anywhere;
      overscroll-behavior: contain;
      user-select: text;
      -webkit-user-select: text;
    }

    .body:focus {
      outline: none;
    }

    .body:focus-visible {
      outline: 2px solid var(--wa-color-focus);
      outline-offset: -2px;
    }

    mark {
      padding: 0 var(--wa-space-2xs);
      border: 1px dashed var(--wa-color-warning-border-normal);
      border-radius: var(--wa-radius-s);
      background: var(--wa-color-warning-fill-quiet);
      color: var(--wa-color-text);
    }

    .loading {
      display: flex;
      justify-content: center;
      padding: var(--wa-space-m);
    }
  `;

  @property({ attribute: false })
  emails: EmailPreviewView[] = [];

  /** The literal stand-ins the body carries for links made at send time. */
  @property({ attribute: false })
  placeholders: string[] = [];

  @property({ type: Boolean, reflect: true })
  loading = false;

  /** Why the preview could not be built — the server's own words. */
  @property({ type: String })
  error = '';

  private renderBody(body: string) {
    return splitPlaceholders(body, this.placeholders).map((part) =>
      part.placeholder
        ? html`<mark data-placeholder title="Created when the email is sent">${part.text}</mark>`
        : part.text,
    );
  }

  private renderEmail(email: EmailPreviewView, multiple: boolean) {
    const role = ROLE_LABELS[email.role] ?? email.role;
    return html`
      <li class="email" data-email data-role=${email.role}>
        <dl>
          <dt>To</dt>
          <dd data-to>
            ${email.to}${multiple ? html` <span class="role">· ${role}</span>` : nothing}
          </dd>
          ${email.cc && email.cc.length > 0
            ? html`<dt>Cc</dt>
                <dd data-cc>${email.cc.join(', ')}</dd>`
            : nothing}
          <dt>Subject</dt>
          <dd data-subject>${email.subject}</dd>
          ${email.attachments.length > 0
            ? html`<dt>Attached</dt>
                <dd data-attachments>${email.attachments.join(', ')}</dd>`
            : nothing}
        </dl>
        <div class="body" data-body tabindex="0" role="region" aria-label=${`Message to ${email.to}`}>${this.renderBody(email.body)}</div>
      </li>
    `;
  }

  render() {
    const count = this.emails.length;
    return html`
      <p class="heading" id="heading">
        ${count > 1 ? `Email preview — ${count} emails` : 'Email preview'}
        ${this.loading && count > 0
          ? html`<span class="updating" data-updating>Updating…</span>`
          : nothing}
      </p>
      ${this.error
        ? html`<wc-notice-bar
            variant="warning"
            data-error
            message=${`Could not preview the email. ${this.error} You can still send.`}
          ></wc-notice-bar>`
        : nothing}
      ${this.loading && count === 0
        ? html`<div class="loading" data-loading>
            <wc-spinner show-label label="Preparing the preview"></wc-spinner>
          </div>`
        : nothing}
      ${count > 0
        ? html`<ul class="emails" aria-labelledby="heading" aria-busy=${this.loading ? 'true' : 'false'}>
            ${this.emails.map((email) => this.renderEmail(email, count > 1))}
          </ul>`
        : nothing}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-email-preview': WcEmailPreview;
  }
}
