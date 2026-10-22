import { LitElement, html, css, nothing, type TemplateResult } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import '../icons/icons.js';
import { controlsCss } from '@nigel/theme';

/** The icons a pill can lead with. */
export type ActionPillIcon = 'plus' | 'refresh';

/** One action in the row. */
export interface ActionPill {
  /** What `nc-action` reports, and the button's `data-action`. */
  key: string;
  label: string;
  /** `primary` is the brand button, `danger` the outlined destructive trigger. */
  tone?: 'default' | 'primary' | 'danger';
  icon?: ActionPillIcon;
  disabled?: boolean;
  /**
   * Why a disabled action is off, in one sentence. Printed under the row and
   * tied to the button with `aria-describedby`, so the explanation is there
   * for a pointer, a keyboard and a screen reader alike. Actions that share a
   * sentence share one note.
   */
  reason?: string;
  /** A tooltip for an enabled action — what it will do. */
  hint?: string;
}

export interface NcActionDetail {
  key: string;
}

const ICONS: Record<ActionPillIcon, TemplateResult> = {
  plus: html`<wc-icon-plus slot="start"></wc-icon-plus>`,
  refresh: html`<wc-icon-refresh slot="start"></wc-icon-refresh>`,
};

/**
 * A row of buttons for a screen's title row — and, under it, why each
 * disabled one is disabled.
 *
 * The buttons are the theme's standard `wa-button` (docs/native-feel.md,
 * "Buttons"): `primary` is `variant="brand"`, `danger` is the outlined
 * destructive trigger, and the rest are the outlined secondary. This
 * component styles none of them; it lays out the row and the notes.
 *
 * The enablement is the caller's: this draws what it is told and reports
 * clicks as `nc-action` with the action's key. A disabled action carrying a
 * `reason` prints it; one without a reason is simply off.
 */
@customElement('wc-action-pills')
export class WcActionPills extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .row {
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
      }

      :host([align='end']) .row {
        justify-content: flex-end;
      }

      /* The notes wrap to whatever width the buttons take rather than widening
         the row: a long reason must not push a title row's actions onto a line
         of their own. */
      .notes {
        contain: inline-size;
        display: grid;
        gap: 2px;
        margin: 8px 0 0;
        padding: 0;
        list-style: none;
        color: var(--wa-color-muted);
        font-size: var(--nc-font-size-meta);
      }

      :host([align='end']) .notes {
        text-align: end;
      }
    `,
  ];

  @property({ attribute: false })
  actions: ActionPill[] = [];

  /** `end` right-aligns the row and its notes, for a title row. */
  @property({ type: String, reflect: true })
  align: 'start' | 'end' = 'start';

  private fire(key: string): void {
    this.dispatchEvent(
      new CustomEvent<NcActionDetail>('nc-action', {
        detail: { key },
        bubbles: true,
        composed: true,
      }),
    );
  }

  render() {
    // One note per distinct sentence: four actions a void invoice turns off
    // for the same reason say it once, and all four point at it.
    const reasons = [
      ...new Set(
        this.actions
          .filter((action) => action.disabled && action.reason)
          .map((action) => action.reason as string),
      ),
    ];

    return html`
      <div class="row">
        ${this.actions.map((action) => {
          const index = action.disabled && action.reason ? reasons.indexOf(action.reason) : -1;
          const note = index >= 0 ? `note-${index}` : '';
          const tone = action.tone ?? 'default';
          return html`<wa-button
            data-action=${action.key}
            data-tone=${tone}
            variant=${tone === 'primary' ? 'brand' : tone === 'danger' ? 'danger' : nothing}
            appearance=${tone === 'primary' ? nothing : 'outlined'}
            ?disabled=${action.disabled ?? false}
            title=${(action.disabled ? action.reason : action.hint) || nothing}
            aria-describedby=${note || nothing}
            @click=${() => {
              if (!action.disabled) this.fire(action.key);
            }}
          >
            ${action.icon ? ICONS[action.icon] : nothing}${action.label}
          </wa-button>`;
        })}
      </div>
      ${reasons.length > 0
        ? html`<ul class="notes">
            ${reasons.map(
              (reason, index) =>
                html`<li
                  id=${`note-${index}`}
                  data-note=${this.actions
                    .filter((action) => action.disabled && action.reason === reason)
                    .map((action) => action.key)
                    .join(' ')}
                >
                  ${reason}
                </li>`,
            )}
          </ul>`
        : nothing}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-action-pills': WcActionPills;
  }
}
