import { LitElement, html, css, nothing } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/input/input.js';
import '@awesome.me/webawesome/dist/components/radio-group/radio-group.js';
import '@awesome.me/webawesome/dist/components/radio/radio.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import {
  ACCOUNT_CLASSES,
  accountClassLabel,
  type AccountClassValue,
} from './account-class.js';
import { controlsCss } from '@nigel/theme';

export interface CategoryFormValue {
  name: string;
  categoryType: string;
  class: string;
  taxLine: string;
  formLine: string;
}

export interface CategoryFormErrors {
  name?: string;
}

export interface NcCategoryFormChangeDetail {
  value: CategoryFormValue;
}

/**
 * Expense first, which is what `CATEGORY_TYPES` in `category_manager.rs` seeds
 * a new category with. (The list endpoint sorts income first; that is the
 * reading order, not the default for a new row.)
 */
export const EMPTY_CATEGORY_FORM: CategoryFormValue = {
  name: '',
  categoryType: 'expense',
  class: 'expense',
  taxLine: '',
  formLine: '',
};

/**
 * The form-line values `reports::resolve_k1_mapping` reads by name, as opposed
 * to by prefix. The rest of the vocabulary is a pattern rather than a list, so
 * the suggestions are completed at runtime from the form lines the chart of
 * accounts already uses — a hard-coded copy of the stock chart would drift from
 * whatever the migrations actually backfilled.
 */
export const FORM_LINE_ANCHORS = ['1120S-1a', '1120S-2', '1120S-5', 'excluded'];

const FORM_LINE_PATTERN = /^(1120S-\d{1,2}[a-z]?|K-\d{1,2}[a-z]?|excluded)$/;

export function validateCategoryForm(value: CategoryFormValue): CategoryFormErrors {
  return value.name.trim() === '' ? { name: 'Name is required' } : {};
}

/**
 * A form line nigel's K-1 worksheet will not recognize, or null.
 *
 * Advisory, never a block: `form_line` is free text in the CLI, the TUI, the
 * data layer and the API, and the report has defined behaviour for a value it
 * does not know — the category lands in "Needs mapping". Worth saying out loud
 * because the matching is literal, so `1120s-19` silently misses.
 */
export function formLineWarning(formLine: string): string | null {
  const trimmed = formLine.trim();
  if (trimmed === '' || FORM_LINE_PATTERN.test(trimmed)) return null;
  return "Nigel will not recognize this on the K-1 worksheet — the category shows under “Needs mapping”.";
}

/** Suggestions: the anchors plus whatever the chart of accounts already uses. */
export function formLineSuggestions(inUse: (string | null)[]): string[] {
  const all = [...FORM_LINE_ANCHORS, ...inUse.filter((line): line is string => !!line)];
  return [...new Set(all)].sort();
}

/**
 * The category add/edit field group — every field the TUI's form has.
 *
 * Controlled, like the import form: it renders `value` and emits each edit, so
 * the screen can send the smallest legal PATCH rather than the whole row.
 */
@customElement('wc-category-form')
export class WcCategoryForm extends LitElement {
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
        gap: var(--wa-space-m);
        min-width: 0;
      }

      /* Label and field drawn from the form-control tokens, as a wa-input
         is, so the native field sits in the column without a seam. */
      label.field {
        display: block;
        font-size: var(--wa-font-size-base);
        font-weight: var(--wa-form-control-label-font-weight);
        color: var(--wa-form-control-label-color);
        margin-bottom: var(--wa-space-2xs);
      }

      input.native {
        width: 100%;
        box-sizing: border-box;
        min-height: var(--wa-form-control-height);
        font: inherit;
        font-family: var(--wa-font-family-mono);
        font-size: var(--wa-form-control-value-font-size);
        color: var(--wa-form-control-value-color);
        background: var(--wa-form-control-background-color);
        border: var(--wa-form-control-border-width) var(--wa-form-control-border-style)
          var(--wa-form-control-border-color);
        border-radius: var(--wa-form-control-border-radius);
        padding: 0 var(--wa-form-control-padding-inline);
      }

      .error {
        margin: var(--wa-space-2xs) 0 0;
        color: var(--wa-color-danger);
        font-size: var(--wa-font-size-s);
      }

      .warning {
        margin: var(--wa-space-2xs) 0 0;
        color: var(--wa-color-warning);
        font-size: var(--wa-font-size-s);
      }

      .hint {
        margin: var(--wa-space-2xs) 0 0;
        color: var(--wa-color-muted);
        font-size: var(--wa-font-size-s);
      }

      .hint code {
        font-family: var(--wa-font-family-mono);
      }
    `,
  ];

  @property({ attribute: false })
  value: CategoryFormValue = EMPTY_CATEGORY_FORM;

  @property({ attribute: false })
  errors: CategoryFormErrors = {};

  /** Form lines to suggest — `formLineSuggestions()` builds the list. */
  @property({ attribute: false })
  suggestions: string[] = FORM_LINE_ANCHORS;

  @property({ type: Boolean })
  disabled = false;

  private emit(next: Partial<CategoryFormValue>): void {
    this.dispatchEvent(
      new CustomEvent<NcCategoryFormChangeDetail>('nc-category-form-change', {
        detail: { value: { ...this.value, ...next } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private handleField(field: keyof CategoryFormValue) {
    return (event: Event) => {
      const input = event.target as HTMLInputElement;
      this.emit({ [field]: String(input.value ?? '') } as Partial<CategoryFormValue>);
    };
  }

  render() {
    const warning = formLineWarning(this.value.formLine);
    const unknownClass = !ACCOUNT_CLASSES.includes(
      this.value.class as AccountClassValue,
    );

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

        <wa-radio-group
          data-type
          label="Type"
          orientation="horizontal"
          value=${this.value.categoryType}
          ?disabled=${this.disabled}
          @change=${this.handleField('categoryType')}
        >
          <wa-radio value="income">Income</wa-radio>
          <wa-radio value="expense">Expense</wa-radio>
        </wa-radio-group>

        <wa-select
          data-class
          label="Class"
          hint="Where this sits in the accounting structure."
          value=${this.value.class}
          ?disabled=${this.disabled}
          @change=${this.handleField('class')}
        >
          ${ACCOUNT_CLASSES.map(
            (value) =>
              html`<wa-option value=${value}>${accountClassLabel(value)}</wa-option>`,
          )}
          ${unknownClass
            ? html`<wa-option value=${this.value.class}
                >${this.value.class}</wa-option
              >`
            : nothing}
        </wa-select>

        <wa-input
          data-tax-line
          label="Tax line"
          autocomplete="off"
          spellcheck="false"
          hint="Free text, as on Schedule C."
          value=${this.value.taxLine}
          ?disabled=${this.disabled}
          @input=${this.handleField('taxLine')}
        ></wa-input>

        <!--
          A native input, not wa-input: the list attribute resolves inside one
          tree, and wa-input's real input lives in its own shadow root, where a
          datalist declared here cannot reach it.
        -->
        <div>
          <label class="field" for="form-line">Form line</label>
          <input
            id="form-line"
            class="native"
            data-form-line
            type="text"
            list="form-line-options"
            autocomplete="off"
            spellcheck="false"
            .value=${this.value.formLine}
            ?disabled=${this.disabled}
            @input=${this.handleField('formLine')}
          />
          <datalist id="form-line-options">
            ${this.suggestions.map((line) => html`<option value=${line}></option>`)}
          </datalist>
          <p class="hint">
            <code>1120S-1a</code> gross receipts · <code>1120S-2</code> cost of goods
            sold · <code>1120S-5</code> other income · <code>1120S-&lt;line&gt;</code>
            deduction lines 7–19 · <code>K-&lt;item&gt;</code> Schedule K ·
            <code>excluded</code> to leave it off the worksheet.
          </p>
          ${warning ? html`<p class="warning" role="status">${warning}</p>` : nothing}
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-category-form': WcCategoryForm;
  }
  interface HTMLElementEventMap {
    'nc-category-form-change': CustomEvent<NcCategoryFormChangeDetail>;
  }
}
