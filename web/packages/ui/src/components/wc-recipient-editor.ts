import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import { controlsCss } from '@nigel/theme';

export interface RecipientValue {
  name: string;
  email: string;
}

export interface RecipientEditorValue {
  signer: RecipientValue;
  collaborators: RecipientValue[];
}

export interface RecipientContactOption {
  name: string | null;
  email: string;
  isBilling: boolean;
}

export interface RecipientErrors {
  signerName?: string;
  signerEmail?: string;
  /** Keyed by collaborator index, so a refusal lands beside its row. */
  collaborators?: Record<number, string>;
  duplicate?: string;
}

export interface NcRecipientsChangeDetail {
  value: RecipientEditorValue;
}

const BLANK: RecipientValue = { name: '', email: '' };

/** The billing contact as signer, and nobody else. */
export function defaultRecipients(contacts: RecipientContactOption[]): RecipientEditorValue {
  const billing = contacts.find((contact) => contact.isBilling);
  return {
    signer: billing ? { name: billing.name ?? '', email: billing.email } : { ...BLANK },
    collaborators: [],
  };
}

function emailProblem(email: string): string | undefined {
  const trimmed = email.trim();
  if (trimmed === '') return 'Enter an email address';
  if (!trimmed.includes('@')) return 'An email address needs an @';
  return undefined;
}

/**
 * What the editor can refuse before the server sees it. The signer must be
 * named because accepting means typing that name.
 */
export function validateRecipients(value: RecipientEditorValue): RecipientErrors {
  const errors: RecipientErrors = {};
  if (value.signer.name.trim() === '') errors.signerName = 'The signer needs a name';
  const signerEmail = emailProblem(value.signer.email);
  if (signerEmail) errors.signerEmail = signerEmail;

  const collaborators: Record<number, string> = {};
  value.collaborators.forEach((collaborator, index) => {
    const problem = emailProblem(collaborator.email);
    if (problem) collaborators[index] = problem;
  });
  if (Object.keys(collaborators).length > 0) errors.collaborators = collaborators;

  const seen = new Set<string>();
  for (const recipient of [value.signer, ...value.collaborators]) {
    const key = recipient.email.trim().toLowerCase();
    if (key === '') continue;
    if (seen.has(key)) {
      errors.duplicate = `${recipient.email.trim()} is listed more than once`;
      break;
    }
    seen.add(key);
  }

  return errors;
}

/**
 * Who a document is sent to: exactly one signer, never removable, and any
 * number of collaborators, typed in or picked from the client's contacts.
 *
 * Controlled, like the other form components: it renders `value` and emits
 * the whole value on every edit.
 */
@customElement('wc-recipient-editor')
export class WcRecipientEditor extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .fields {
        display: grid;
        gap: var(--wa-space-m, 12px);
      }

      fieldset {
        display: grid;
        gap: var(--wa-space-s, 8px);
        margin: 0;
        padding: var(--wa-space-s, 8px);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-m, 8px);
        justify-items: start;
      }

      legend {
        font-size: var(--wa-font-size-s, 13px);
        font-weight: var(--wa-font-weight-medium, 500);
      }

      .row {
        display: grid;
        grid-template-columns: 1fr 1fr auto;
        gap: var(--wa-space-xs, 6px);
        align-items: end;
        width: 100%;
      }

      .signer {
        grid-template-columns: 1fr 1fr;
        align-items: start;
      }

      .hint {
        margin: 0;
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
      }

      .actions {
        display: flex;
        flex-wrap: wrap;
        gap: var(--wa-space-xs, 6px);
      }

      .error {
        margin: var(--wa-space-2xs, 4px) 0 0;
        color: var(--wa-color-danger, #b3261e);
        font-size: var(--wa-font-size-s, 13px);
      }
    `,
  ];

  @property({ attribute: false })
  value: RecipientEditorValue = { signer: { name: '', email: '' }, collaborators: [] };

  @property({ attribute: false })
  contacts: RecipientContactOption[] = [];

  @property({ attribute: false })
  errors: RecipientErrors = {};

  @property({ type: Boolean, reflect: true })
  disabled = false;

  private emit(next: Partial<RecipientEditorValue>): void {
    this.dispatchEvent(
      new CustomEvent<NcRecipientsChangeDetail>('nc-recipients-change', {
        detail: { value: { ...this.value, ...next } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleSigner(field: keyof RecipientValue) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit({ signer: { ...this.value.signer, [field]: input.value } });
    };
  }

  private handleCollaborator(index: number, field: keyof RecipientValue) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit({
        collaborators: this.value.collaborators.map((collaborator, i) =>
          i === index ? { ...collaborator, [field]: input.value } : collaborator,
        ),
      });
    };
  }

  private addCollaborator = (): void => {
    this.emit({ collaborators: [...this.value.collaborators, { ...BLANK }] });
  };

  private addContact(contact: RecipientContactOption) {
    return (): void => {
      this.emit({
        collaborators: [
          ...this.value.collaborators,
          { name: contact.name ?? '', email: contact.email },
        ],
      });
    };
  }

  private removeCollaborator(index: number) {
    return (): void => {
      this.emit({ collaborators: this.value.collaborators.filter((_, i) => i !== index) });
    };
  }

  private unlistedContacts(): RecipientContactOption[] {
    const listed = new Set(
      [this.value.signer, ...this.value.collaborators].map((r) => r.email.trim().toLowerCase()),
    );
    return this.contacts.filter((contact) => !listed.has(contact.email.trim().toLowerCase()));
  }

  private renderError(message: string | undefined) {
    return message ? html`<p class="error" role="alert">${message}</p>` : nothing;
  }

  private renderCollaborator(collaborator: RecipientValue, index: number) {
    const label =
      collaborator.name.trim() || collaborator.email.trim() || `collaborator ${index + 1}`;
    return html`
      <div class="row" data-collaborator=${index}>
        <wa-input
          data-collaborator-name
          label=${`Collaborator ${index + 1} name`}
          autocomplete="off"
          spellcheck="false"
          value=${collaborator.name}
          ?disabled=${this.disabled}
          @input=${this.handleCollaborator(index, 'name')}
        ></wa-input>
        <wa-input
          data-collaborator-email
          label=${`Collaborator ${index + 1} email`}
          type="email"
          autocomplete="off"
          spellcheck="false"
          value=${collaborator.email}
          ?disabled=${this.disabled}
          @input=${this.handleCollaborator(index, 'email')}
        ></wa-input>
        <wa-button
          data-remove-collaborator
          size="s"
          appearance="plain"
          variant="danger"
          aria-label=${`Remove ${label}`}
          ?disabled=${this.disabled}
          @click=${this.removeCollaborator(index)}
          >Remove</wa-button
        >
      </div>
      ${this.renderError(this.errors.collaborators?.[index])}
    `;
  }

  render() {
    const { signer, collaborators } = this.value;
    const unlisted = this.unlistedContacts();

    return html`
      <div class="fields">
        <fieldset data-signer>
          <legend>Signer</legend>
          <div class="row signer">
            <div>
              <wa-input
                data-signer-name
                label="Signer name"
                autocomplete="off"
                spellcheck="false"
                value=${signer.name}
                ?disabled=${this.disabled}
                @input=${this.handleSigner('name')}
              ></wa-input>
              ${this.renderError(this.errors.signerName)}
            </div>
            <div>
              <wa-input
                data-signer-email
                label="Signer email"
                type="email"
                autocomplete="off"
                spellcheck="false"
                value=${signer.email}
                ?disabled=${this.disabled}
                @input=${this.handleSigner('email')}
              ></wa-input>
              ${this.renderError(this.errors.signerEmail)}
            </div>
          </div>
        </fieldset>

        <fieldset data-collaborators>
          <legend>Collaborators</legend>
          ${collaborators.length === 0
            ? html`<p class="hint">No collaborators. They can request changes but not sign.</p>`
            : nothing}
          ${collaborators.map((collaborator, index) =>
            this.renderCollaborator(collaborator, index),
          )}
          <div class="actions">
            <wa-button
              data-add-collaborator
              size="s"
              appearance="outlined"
              ?disabled=${this.disabled}
              @click=${this.addCollaborator}
              >Add collaborator</wa-button
            >
            ${unlisted.map(
              (contact) => html`
                <wa-button
                  data-add-contact
                  size="s"
                  appearance="outlined"
                  ?disabled=${this.disabled}
                  @click=${this.addContact(contact)}
                  >${`Add ${contact.name?.trim() || contact.email}`}</wa-button
                >
              `,
            )}
          </div>
        </fieldset>

        ${this.renderError(this.errors.duplicate)}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-recipient-editor': WcRecipientEditor;
  }
  interface HTMLElementEventMap {
    'nc-recipients-change': CustomEvent<NcRecipientsChangeDetail>;
  }
}
