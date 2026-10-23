import { LitElement, html, css, nothing, type PropertyValues } from 'lit';
import { customElement, property, state, query } from 'lit/decorators.js';
import { live } from 'lit/directives/live.js';
import '@awesome.me/webawesome/dist/components/dialog/dialog.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import { controlsCss } from '@nigel/theme';
import './wc-money.js';
import './wc-category-picker.js';
import type { CategoryOption } from './category-option.js';
import type { NcCategoryChangeDetail, WcCategoryPicker } from './wc-category-picker.js';
import type {
  NcEditCommitDetail,
  NcRowEventDetail,
  RegisterTableRow,
} from './wc-register-table.js';

/** What Save says when the category box holds text that is not a category. */
export const CHOOSE_CATEGORY_MESSAGE = 'Choose a category from the list.';

/**
 * One register transaction in a dialog: the date, description, amount and
 * account to say which one it is, and the category and vendor to change.
 *
 * It edits what the register's inline editor edits and answers with the same
 * events, `nc-edit-commit` and `nc-edit-cancel`, so the screen saves both
 * through one path. Unlike the inline editor it stays open while the save is
 * in flight and shows a refusal beside the fields, since the fields are still
 * there to correct.
 *
 * The one check made before saving is that the category box names a category:
 * typing past a choice clears it, and saving then would keep the old category
 * while the box shows something else.
 */
@customElement('wc-transaction-dialog')
export class WcTransactionDialog extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        font-family: var(--wa-font-family-sans);
      }

      wa-dialog {
        --width: var(--nc-transaction-dialog-width, 32rem);
      }

      .summary {
        display: grid;
        grid-template-columns: max-content minmax(0, 1fr);
        gap: var(--wa-space-2xs) var(--wa-space-m);
        margin: 0 0 var(--wa-space-l);
        padding: var(--wa-space-s) var(--wa-space-m);
        background: var(--wa-color-surface-alt);
        border: 1px solid var(--wa-color-border-soft);
        border-radius: var(--wa-radius-m);
        font-size: var(--nc-font-size-control);
      }

      dt {
        color: var(--wa-color-muted);
      }

      dd {
        margin: 0;
        overflow-wrap: anywhere;
        user-select: text;
      }

      .fields {
        display: grid;
        gap: var(--wa-space-m);
      }

      .error {
        margin: 0 0 var(--wa-space-m);
        padding: var(--wa-space-xs) var(--wa-space-s);
        border-radius: var(--wa-radius-m);
        background: var(--wa-color-danger-fill-quiet);
        color: var(--wa-color-danger);
        font-size: var(--wa-font-size-s);
      }

      .footer {
        display: flex;
        justify-content: flex-end;
        gap: var(--wa-space-s);
      }
    `,
  ];

  @property({ type: Boolean, reflect: true })
  open = false;

  /** The transaction being edited. Null renders nothing inside the dialog. */
  @property({ attribute: false })
  row: RegisterTableRow | null = null;

  @property({ attribute: false })
  categories: CategoryOption[] = [];

  @property({ type: Boolean, attribute: 'show-account' })
  showAccount = true;

  /** A save in flight: the fields and both buttons wait for it. */
  @property({ type: Boolean, reflect: true })
  busy = false;

  /** The server's answer to the last save. */
  @property({ type: String })
  error: string | null = null;

  /**
   * The message from the check made before saving. Public so a preview can
   * show the state; a Save that fails the check sets it.
   */
  @property({ type: String, attribute: 'validation-error' })
  validationError = '';

  @state() private categoryId: number | null = null;
  @state() private vendor = '';

  @query('wc-category-picker') private picker?: WcCategoryPicker;

  willUpdate(changed: PropertyValues<this>): void {
    const opened = changed.has('open') && this.open;
    if (changed.has('row') || opened) {
      this.categoryId = this.row?.categoryId ?? null;
      this.vendor = this.row?.vendor ?? '';
      if (this.hasUpdated) this.validationError = '';
    }
  }

  /**
   * The picker keeps what was typed into it, so a dialog opened again — on
   * the same row after a Cancel, or on another — starts it from the row.
   */
  updated(changed: PropertyValues<this>): void {
    const opened = changed.has('open') && this.open;
    if (!(changed.has('row') || opened) || !this.picker) return;
    this.picker.reset();
    this.picker.value = this.row?.categoryId ?? null;
  }

  /** Put the cursor in the category box. */
  focusCategory(): void {
    this.picker?.focus();
  }

  private handleCategoryChange = (event: CustomEvent<NcCategoryChangeDetail>): void => {
    this.categoryId = event.detail.categoryId;
    if (event.detail.categoryId !== null) this.validationError = '';
  };

  private handleVendorInput = (event: Event): void => {
    this.vendor = (event.target as HTMLInputElement).value;
  };

  /** Enter saves from anywhere in the form the key is not already spoken for. */
  private handleKeydown = (event: KeyboardEvent): void => {
    if (event.key !== 'Enter' || event.defaultPrevented) return;
    if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
    const target = event.composedPath()[0];
    if (target instanceof HTMLElement && target.closest('button, wa-button, textarea')) return;
    event.preventDefault();
    this.save();
  };

  private save = (): void => {
    const row = this.row;
    if (!row || this.busy) return;

    const typed = this.picker?.queryText.trim() ?? '';
    if (this.categoryId === null && (row.categoryId !== null || typed !== '')) {
      this.validationError = CHOOSE_CATEGORY_MESSAGE;
      this.focusCategory();
      return;
    }

    this.validationError = '';
    const vendor = this.vendor.trim();
    this.dispatchEvent(
      new CustomEvent<NcEditCommitDetail>('nc-edit-commit', {
        detail: { id: row.id, categoryId: this.categoryId, vendor: vendor === '' ? null : vendor },
        bubbles: true,
        composed: true,
      }),
    );
  };

  private cancel = (): void => {
    if (!this.row) return;
    this.dispatchEvent(
      new CustomEvent<NcRowEventDetail>('nc-edit-cancel', {
        detail: { id: this.row.id },
        bubbles: true,
        composed: true,
      }),
    );
  };

  /** wa-dialog's own dismissals — Esc, the close button. */
  private handleHide = (event: Event): void => {
    if (event.target !== event.currentTarget) return;
    if (this.busy) {
      event.preventDefault();
      return;
    }
    if (this.open) this.cancel();
  };

  private handleAfterShow = (event: Event): void => {
    if (event.target !== event.currentTarget) return;
    this.focusCategory();
  };

  private handleAfterHide = (event: Event): void => {
    if (event.target !== event.currentTarget) return;
    this.dispatchEvent(new CustomEvent('nc-dialog-closed', { bubbles: true, composed: true }));
  };

  render() {
    const row = this.row;
    const message = this.validationError || this.error;

    return html`
      <wa-dialog
        label="Edit transaction"
        ?open=${this.open && row !== null}
        @wa-hide=${this.handleHide}
        @wa-after-show=${this.handleAfterShow}
        @wa-after-hide=${this.handleAfterHide}
      >
        ${row
          ? html`
              <dl class="summary">
                <dt>Date</dt>
                <dd>${row.date}</dd>
                <dt>Description</dt>
                <dd>${row.description}</dd>
                <dt>Amount</dt>
                <dd><wc-money .amount=${row.amount}></wc-money></dd>
                ${this.showAccount
                  ? html`<dt>Account</dt>
                      <dd>${row.accountName}</dd>`
                  : nothing}
              </dl>
              ${message ? html`<p class="error" role="alert">${message}</p>` : nothing}
              <div class="fields" @keydown=${this.handleKeydown}>
                <wc-category-picker
                  .options=${this.categories}
                  placeholder="Type to find a category"
                  ?invalid=${this.validationError !== ''}
                  ?disabled=${this.busy}
                  @nc-category-change=${this.handleCategoryChange}
                ></wc-category-picker>
                <wa-input
                  label="Vendor"
                  spellcheck="false"
                  autocomplete="off"
                  .value=${live(this.vendor)}
                  ?disabled=${this.busy}
                  @input=${this.handleVendorInput}
                ></wa-input>
              </div>
            `
          : nothing}
        <div slot="footer" class="footer">
          <wa-button data-cancel appearance="outlined" ?disabled=${this.busy} @click=${this.cancel}>
            Cancel
          </wa-button>
          <wa-button data-save variant="brand" ?disabled=${this.busy} @click=${this.save}>
            ${this.busy ? 'Saving…' : 'Save'}
          </wa-button>
        </div>
      </wa-dialog>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-transaction-dialog': WcTransactionDialog;
  }
  interface HTMLElementEventMap {
    'nc-dialog-closed': CustomEvent<void>;
  }
}
