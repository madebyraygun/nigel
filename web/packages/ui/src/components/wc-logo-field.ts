import { LitElement, html, css } from 'lit';
import { customElement, property, query, state } from 'lit/decorators.js';
import '@awesome.me/webawesome/dist/components/button/button.js';
import { controlsCss } from '@nigel/theme';
import { monogramInitials } from './wc-monogram.js';

export interface NcLogoFileDetail {
  file: File;
}

/**
 * The letterhead's logo: a preview on the white a document is printed on, a
 * line about what the file may be, and Choose… (plus Remove once there is
 * one). A file dropped on the box counts as chosen.
 *
 * The component reads nothing. It hands the chosen `File` up in
 * `nc-logo-file`, because the size cap, the read into a `data:` URI and the
 * error that comes of either are the screen's, as is the request to clear the
 * logo (`nc-logo-clear`). Without a logo the preview shows the initials of
 * `name`, standing in for what the documents print instead.
 */
@customElement('wc-logo-field')
export class WcLogoField extends LitElement {
  static styles = [
    controlsCss,
    css`
      :host {
        display: block;
        font-family: var(--wa-font-family-sans);
        color: var(--wa-color-text);
      }

      .label {
        display: block;
        margin-bottom: var(--wa-space-xs);
        font-size: 12.5px;
        font-weight: var(--wa-font-weight-medium);
      }

      .zone {
        display: flex;
        align-items: center;
        gap: var(--wa-space-m);
        padding: var(--wa-space-m);
        border: 1px dashed var(--wa-color-border);
        border-radius: var(--nc-radius-tile);
        background: var(--wa-color-surface-alt);
        transition: border-color var(--nc-transition-fast);
      }

      .zone.over {
        border-color: var(--wa-color-brand);
      }

      /* Both documents flatten the logo onto white, so the preview shows it
         on that white in either mode. */
      .preview {
        display: grid;
        place-items: center;
        flex: none;
        width: 120px;
        height: 48px;
        box-sizing: border-box;
        padding: var(--wa-space-xs);
        border-radius: var(--wa-radius-sm);
        background: var(--nc-color-document-bg);
        color: var(--nc-color-on-gradient);
        font-size: var(--nc-font-size-control);
        font-weight: var(--wa-font-weight-semibold);
      }

      .preview img {
        max-width: 100%;
        max-height: 100%;
      }

      .text {
        flex: 1;
        min-width: 0;
        display: grid;
        gap: 2px;
      }

      .prompt {
        font-size: var(--nc-font-size-control);
      }

      .help {
        font-size: var(--nc-font-size-meta);
        color: var(--wa-color-muted);
      }

      .actions {
        display: flex;
        flex-wrap: wrap;
        gap: var(--wa-space-s);
      }

      input[type='file'] {
        display: none;
      }

      @media (max-width: 48rem) {
        .zone {
          flex-wrap: wrap;
        }
      }
    `,
  ];

  /** The stored logo as a `data:` URI, or empty. */
  @property({ type: String })
  logo = '';

  /** Whose initials stand in for a missing logo. */
  @property({ type: String })
  name = '';

  /** The headline in the box; a shell that cannot take a drop says Choose instead. */
  @property({ type: String })
  prompt = 'Drop a PNG or JPEG here';

  @property({ type: String })
  help = '';

  @property({ type: Boolean, reflect: true })
  disabled = false;

  @state()
  private over = false;

  @query('input[type="file"]')
  private picker?: HTMLInputElement;

  private hand(file: File | undefined): void {
    if (!file || this.disabled) return;
    this.dispatchEvent(
      new CustomEvent<NcLogoFileDetail>('nc-logo-file', {
        detail: { file },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private onPick = (event: Event) => {
    const input = event.target as HTMLInputElement;
    this.hand(input.files?.[0]);
    // So choosing the same file twice still fires a change event.
    input.value = '';
  };

  private onDragOver = (event: DragEvent) => {
    if (this.disabled) return;
    event.preventDefault();
    this.over = true;
  };

  private onDragLeave = () => {
    this.over = false;
  };

  private onDrop = (event: DragEvent) => {
    event.preventDefault();
    this.over = false;
    this.hand(event.dataTransfer?.files?.[0]);
  };

  private choose = () => {
    this.picker?.click();
  };

  private clear = () => {
    this.dispatchEvent(new CustomEvent('nc-logo-clear', { bubbles: true, composed: true }));
  };

  render() {
    return html`
      <span class="label" id="label">Logo</span>
      <div
        class=${this.over ? 'zone over' : 'zone'}
        role="group"
        aria-labelledby="label"
        @dragover=${this.onDragOver}
        @dragleave=${this.onDragLeave}
        @drop=${this.onDrop}
      >
        <div class="preview">
          ${this.logo
            ? html`<img class="logo-preview" src=${this.logo} alt="The configured logo" />`
            : html`<span aria-hidden="true">${monogramInitials(this.name)}</span>`}
        </div>
        <div class="text">
          <span class="prompt">${this.prompt}</span>
          ${this.help ? html`<span class="help">${this.help}</span>` : ''}
        </div>
        <div class="actions">
          <wa-button
            size="small"
            appearance="outlined"
            ?disabled=${this.disabled}
            @click=${this.choose}
            >Choose…</wa-button
          >
          ${this.logo
            ? html`<wa-button
                size="small"
                appearance="outlined"
                ?disabled=${this.disabled}
                @click=${this.clear}
                >Remove</wa-button
              >`
            : ''}
        </div>
        <input
          type="file"
          accept="image/png,image/jpeg"
          tabindex="-1"
          aria-hidden="true"
          ?disabled=${this.disabled}
          @change=${this.onPick}
        />
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-logo-field': WcLogoField;
  }
  interface HTMLElementEventMap {
    'nc-logo-file': CustomEvent<NcLogoFileDetail>;
    'nc-logo-clear': CustomEvent<void>;
  }
}
