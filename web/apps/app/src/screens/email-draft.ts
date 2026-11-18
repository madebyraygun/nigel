import type { ReactiveController, ReactiveControllerHost } from 'lit';
import { ApiError } from '../api/index.js';
import type { EmailPreview } from '../api/types.js';

/** How long the message box has to sit still before the preview is asked for again. */
export const EMAIL_PREVIEW_DEBOUNCE_MS = 300;

export interface EmailDraftOptions {
  /** The saved default greeting the message box starts from. */
  loadDefault: () => Promise<string>;
  /** The preview for this greeting; `undefined` asks for the saved default. */
  fetchPreview: (greeting: string | undefined) => Promise<EmailPreview>;
  debounceMs?: number;
}

/**
 * The message box and the live email preview of a send dialog.
 *
 * Until the saved default has been read, or someone has typed, the box does
 * not know what goes out — so `greeting` is `undefined` and the send omits the
 * field, which the server reads as "use the saved default". After that it is
 * the box's exact contents, and an empty box is `""`: no greeting at all.
 *
 * A preview that comes back after a newer one was asked for is dropped, so a
 * slow response never overwrites what the operator just typed.
 */
export class EmailDraftController implements ReactiveController {
  message = '';
  messageLoading = false;
  /** Why the saved default could not be read, or empty. */
  messageCaution = '';
  preview: EmailPreview | null = null;
  previewLoading = false;
  previewError = '';

  private known = false;
  private session = 0;
  private sequence = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;

  constructor(
    private readonly host: ReactiveControllerHost,
    private readonly options: EmailDraftOptions,
  ) {
    host.addController(this);
  }

  hostDisconnected(): void {
    this.close();
  }

  /** What a send or a preview carries as `greeting`. */
  get greeting(): string | undefined {
    return this.known ? this.message : undefined;
  }

  /** Start a fresh draft: read the default into the box, then preview it. */
  async open(): Promise<void> {
    this.close();
    const session = this.session;
    this.message = '';
    this.known = false;
    this.messageCaution = '';
    this.preview = null;
    this.previewError = '';
    this.messageLoading = true;
    this.host.requestUpdate();
    try {
      const saved = await this.options.loadDefault();
      if (session !== this.session) return;
      // Typing while the default loads wins over the default.
      if (!this.known) {
        this.message = saved;
        this.known = true;
      }
    } catch (error) {
      if (session !== this.session) return;
      const reason = error instanceof ApiError ? ` ${error.message}` : '';
      this.messageCaution = `Could not load the saved message. It still goes out unless you type your own.${reason}`;
    } finally {
      if (session === this.session) {
        this.messageLoading = false;
        this.host.requestUpdate();
      }
    }
    if (session === this.session) await this.refresh();
  }

  /** The box changed: keep the value and preview it once typing pauses. */
  edit(value: string): void {
    this.message = value;
    this.known = true;
    this.host.requestUpdate();
    this.schedule();
  }

  /** Ask for a fresh preview once things have been still for the debounce window. */
  schedule(): void {
    clearTimeout(this.timer);
    this.timer = setTimeout(
      () => void this.refresh(),
      this.options.debounceMs ?? EMAIL_PREVIEW_DEBOUNCE_MS,
    );
  }

  /** Ask for the preview now. */
  async refresh(): Promise<void> {
    clearTimeout(this.timer);
    this.timer = undefined;
    const sequence = ++this.sequence;
    this.previewLoading = true;
    this.previewError = '';
    this.host.requestUpdate();
    try {
      const preview = await this.options.fetchPreview(this.greeting);
      if (sequence !== this.sequence) return;
      this.preview = preview;
    } catch (error) {
      if (sequence !== this.sequence) return;
      this.previewError = error instanceof ApiError ? error.message : String(error);
    } finally {
      if (sequence === this.sequence) {
        this.previewLoading = false;
        this.host.requestUpdate();
      }
    }
  }

  /** Drop any pending request; nothing that was in flight lands any more. */
  close(): void {
    clearTimeout(this.timer);
    this.timer = undefined;
    this.session += 1;
    this.sequence += 1;
    this.messageLoading = false;
    this.previewLoading = false;
  }
}
