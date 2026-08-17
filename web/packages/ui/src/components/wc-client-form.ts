import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import { controlsCss } from '@nigel/theme';

/** One row of the contacts repeater. */
export interface ClientFormContact {
  email: string;
  name: string;
  title: string;
}

export interface ClientFormValue {
  name: string;
  /**
   * Every address, in order. The one at `billingIndex` is the invoice's `To`
   * and the address every list shows; the rest are copied on every invoice.
   *
   * There is no separate Email field. `POST`/`PATCH /api/clients` accept
   * `email` *or* `contacts` and refuse both, so a form carrying both controls
   * would have to decide which one wins — and the list is the more general of
   * the two.
   */
  contacts: ClientFormContact[];
  /** Which contact is the billing recipient. Meaningless on an empty list. */
  billingIndex: number;
  billingAddress: string;
  notes: string;
}

export interface ClientFormErrors {
  name?: string;
  /** Keyed by row index, so a refusal lands beside the address it is about. */
  contacts?: Record<number, string>;
}

export interface NcClientFormChangeDetail {
  value: ClientFormValue;
}

export const EMPTY_CONTACT: ClientFormContact = { email: '', name: '', title: '' };

export const EMPTY_CLIENT_FORM: ClientFormValue = {
  name: '',
  contacts: [],
  billingIndex: 0,
  billingAddress: '',
  notes: '',
};

/**
 * What the form can refuse before the server sees it.
 *
 * Only the name, because that is the only field `add_client` requires. The
 * email is deliberately **not** shape-checked: `nigel client add` does not
 * check it either, and `client_manager.rs` says as much — a web form that
 * rejected an address the CLI accepts would make the two surfaces disagree
 * about what a client is.
 */
export function validateClientForm(value: ClientFormValue): ClientFormErrors {
  const errors: ClientFormErrors = {};
  if (value.name.trim() === '') errors.name = 'Name is required';

  const contacts: Record<number, string> = {};
  const seen = new Map<string, number>();
  value.contacts.forEach((contact, index) => {
    const email = contact.email.trim();
    if (email === '') {
      contacts[index] = 'An email address is required';
      return;
    }
    // Case-insensitive, because that is what the database's unique index is
    // and a duplicate address is a duplicate delivery, not a second recipient.
    const key = email.toLowerCase();
    if (seen.has(key)) {
      contacts[index] = 'This address is already on the list';
      return;
    }
    seen.set(key, index);
  });
  if (Object.keys(contacts).length > 0) errors.contacts = contacts;

  return errors;
}

/**
 * The client add/edit field group — the four columns the table has.
 *
 * The email carries inline helper text because `client_missing_email` is the
 * send failure most likely to be hit and the cheapest one to prevent: an
 * invoice for a client with no address refuses at precheck, before any network
 * call, and the only fix is here.
 */
@customElement('wc-client-form')
export class WcClientForm extends LitElement {
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
        min-width: 20rem;
      }

      .error {
        margin: var(--wa-space-2xs, 4px) 0 0;
        color: var(--wa-color-danger, #b3261e);
        font-size: var(--wa-font-size-s, 13px);
      }

      .hint {
        margin: var(--wa-space-2xs, 4px) 0 0;
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s, 13px);
      }

      /*
       * A plain textarea rather than wa-textarea: Web Awesome's auto-sizing one
       * needs a ResizeObserver, which jsdom has not got, and the component-first
       * workflow means every state of this form is mounted in a jsdom axe run.
       */
      .stacked {
        display: grid;
        gap: var(--wa-space-2xs, 4px);
      }

      .label {
        font-size: var(--wa-font-size-s, 13px);
        font-weight: var(--wa-font-weight-medium, 500);
      }

      textarea {
        font: inherit;
        width: 100%;
        box-sizing: border-box;
        padding: var(--wa-space-xs, 6px) var(--wa-space-s, 8px);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-m, 8px);
        background: var(--wa-color-surface);
        color: inherit;
        resize: vertical;
      }

      textarea:focus-visible {
        outline: 2px solid var(--wa-color-focus);
        outline-offset: 1px;
      }

      fieldset.contacts {
        display: grid;
        gap: var(--wa-space-s, 8px);
        margin: 0;
        padding: var(--wa-space-s, 8px);
        border: 1px solid var(--wa-color-border);
        border-radius: var(--wa-radius-m, 8px);
        justify-items: start;
      }

      .contact {
        display: grid;
        grid-template-columns: auto 1fr 1fr 1fr auto;
        gap: var(--wa-space-xs, 6px);
        align-items: end;
        width: 100%;
      }

      .contact .billing {
        display: grid;
        justify-items: center;
        gap: var(--wa-space-2xs, 4px);
        font-size: var(--wa-font-size-s, 13px);
      }

      .contact .row-actions {
        display: flex;
        gap: var(--wa-space-2xs, 4px);
      }
    `,
  ];

  @property({ attribute: false })
  value: ClientFormValue = EMPTY_CLIENT_FORM;

  @property({ attribute: false })
  errors: ClientFormErrors = {};

  @property({ type: Boolean })
  disabled = false;

  private emit(next: Partial<ClientFormValue>): void {
    this.dispatchEvent(
      new CustomEvent<NcClientFormChangeDetail>('nc-client-form-change', {
        detail: { value: { ...this.value, ...next } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleField(field: 'name' | 'billingAddress' | 'notes') {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit({ [field]: input.value } as Partial<ClientFormValue>);
    };
  }

  private handleContactField(index: number, field: keyof ClientFormContact) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      const contacts = this.value.contacts.map((contact, i) =>
        i === index ? { ...contact, [field]: input.value } : contact,
      );
      this.emit({ contacts });
    };
  }

  private addContact = (): void => {
    this.emit({ contacts: [...this.value.contacts, { ...EMPTY_CONTACT }] });
  };

  private removeContact(index: number) {
    return (): void => {
      const contacts = this.value.contacts.filter((_, i) => i !== index);
      // The billing row follows what is left rather than pointing past the end.
      const billingIndex =
        index < this.value.billingIndex
          ? this.value.billingIndex - 1
          : Math.min(this.value.billingIndex, Math.max(contacts.length - 1, 0));
      this.emit({ contacts, billingIndex });
    };
  }

  private moveContact(index: number, delta: number) {
    return (): void => {
      const target = index + delta;
      if (target < 0 || target >= this.value.contacts.length) return;
      const contacts = [...this.value.contacts];
      [contacts[index], contacts[target]] = [contacts[target], contacts[index]];
      // Up/down buttons rather than a drag handle: a drag has no keyboard
      // equivalent that passes axe without building these anyway, which is the
      // reasoning `wc-line-items` already records.
      let billingIndex = this.value.billingIndex;
      if (billingIndex === index) billingIndex = target;
      else if (billingIndex === target) billingIndex = index;
      this.emit({ contacts, billingIndex });
    };
  }

  private chooseBilling(index: number) {
    return (): void => this.emit({ billingIndex: index });
  }

  private renderContact(contact: ClientFormContact, index: number) {
    const error = this.errors.contacts?.[index];
    const last = this.value.contacts.length - 1;

    return html`
      <div class="contact" data-contact=${index}>
        <span class="billing">
          <input
            type="radio"
            name="billing-contact"
            data-billing=${index}
            aria-label=${`Bill ${contact.email || `contact ${index + 1}`}`}
            .checked=${this.value.billingIndex === index}
            ?disabled=${this.disabled}
            @change=${this.chooseBilling(index)}
          />
        </span>
        <wa-input
          data-contact-email
          label="Email"
          type="email"
          autocomplete="off"
          spellcheck="false"
          value=${contact.email}
          ?disabled=${this.disabled}
          @input=${this.handleContactField(index, 'email')}
        ></wa-input>
        <wa-input
          data-contact-name
          label="Name"
          autocomplete="off"
          spellcheck="false"
          value=${contact.name}
          ?disabled=${this.disabled}
          @input=${this.handleContactField(index, 'name')}
        ></wa-input>
        <wa-input
          data-contact-title
          label="Title"
          autocomplete="off"
          spellcheck="false"
          value=${contact.title}
          ?disabled=${this.disabled}
          @input=${this.handleContactField(index, 'title')}
        ></wa-input>
        <span class="row-actions">
          <wa-button
            data-move-up
            size="s"
            appearance="plain"
            aria-label=${`Move ${contact.email || `contact ${index + 1}`} up`}
            ?disabled=${this.disabled || index === 0}
            @click=${this.moveContact(index, -1)}
            >↑</wa-button
          >
          <wa-button
            data-move-down
            size="s"
            appearance="plain"
            aria-label=${`Move ${contact.email || `contact ${index + 1}`} down`}
            ?disabled=${this.disabled || index === last}
            @click=${this.moveContact(index, 1)}
            >↓</wa-button
          >
          <wa-button
            data-remove-contact
            size="s"
            appearance="plain"
            variant="danger"
            aria-label=${`Remove ${contact.email || `contact ${index + 1}`}`}
            ?disabled=${this.disabled}
            @click=${this.removeContact(index)}
            >Remove</wa-button
          >
        </span>
      </div>
      ${error ? html`<p class="error" role="alert">${error}</p>` : nothing}
    `;
  }

  render() {
    const noAddress = this.value.contacts.every((c) => c.email.trim() === '');

    return html`
      <div class="fields">
        <div>
          <wa-input
            data-name
            label="Name"
            autocomplete="off"
            spellcheck="false"
            value=${this.value.name}
            ?disabled=${this.disabled}
            @input=${this.handleField('name')}
          ></wa-input>
          ${this.errors.name
            ? html`<p class="error" role="alert">${this.errors.name}</p>`
            : nothing}
        </div>

        <fieldset class="contacts">
          <legend class="label">Contacts</legend>
          ${noAddress
            ? html`<p class="hint" data-email-hint>
                An invoice cannot be sent to a client with no email address.
              </p>`
            : nothing}
          ${this.value.contacts.map((contact, index) => this.renderContact(contact, index))}
          <wa-button
            data-add-contact
            size="s"
            appearance="outlined"
            ?disabled=${this.disabled}
            @click=${this.addContact}
            >Add contact</wa-button
          >
        </fieldset>

        <wa-input
          data-address
          label="Billing address"
          autocomplete="off"
          value=${this.value.billingAddress}
          ?disabled=${this.disabled}
          @input=${this.handleField('billingAddress')}
        ></wa-input>

        <label class="stacked">
          <span class="label">Notes</span>
          <textarea
            data-notes
            rows="2"
            .value=${this.value.notes}
            ?disabled=${this.disabled}
            @input=${this.handleField('notes')}
          ></textarea>
        </label>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-client-form': WcClientForm;
  }
  interface HTMLElementEventMap {
    'nc-client-form-change': CustomEvent<NcClientFormChangeDetail>;
  }
}
