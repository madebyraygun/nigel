import { describe, it, expect, vi, afterEach } from 'vitest';
import type { ReactiveController, ReactiveControllerHost } from 'lit';
import { EmailDraftController, EMAIL_PREVIEW_DEBOUNCE_MS, reasonOf } from './email-draft.js';
import { ApiError } from '../api/index.js';
import type { EmailPreview } from '../api/types.js';

function host(): ReactiveControllerHost & { updates: number } {
  const controllers: ReactiveController[] = [];
  return {
    updates: 0,
    addController: (c) => controllers.push(c),
    removeController: () => {},
    requestUpdate() {
      this.updates += 1;
    },
    updateComplete: Promise.resolve(true),
  };
}

function previewOf(greeting: string | undefined): EmailPreview {
  return {
    emails: [
      {
        role: 'client',
        to: 'Acme <ap@acme.test>',
        cc: [],
        subject: 'Invoice #1251',
        body: `${greeting ?? '(saved)'}\n\nInvoice #1251.`,
        attachments: [],
      },
    ],
    linkPlaceholders: [],
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

describe('EmailDraftController', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('starts from the saved default and previews it', async () => {
    const fetchPreview = vi.fn(async (greeting: string | undefined) => previewOf(greeting));
    const draft = new EmailDraftController(host(), {
      loadDefault: async () => 'Thanks, Acme.',
      fetchPreview,
    });

    await draft.open();

    expect(draft.message).toBe('Thanks, Acme.');
    expect(draft.greeting).toBe('Thanks, Acme.');
    expect(fetchPreview).toHaveBeenCalledWith('Thanks, Acme.');
    expect(draft.preview?.emails[0].body).toContain('Thanks, Acme.');
  });

  it('debounces edits into one request carrying the last value', async () => {
    vi.useFakeTimers();
    const fetchPreview = vi.fn(async (greeting: string | undefined) => previewOf(greeting));
    const draft = new EmailDraftController(host(), { loadDefault: async () => '', fetchPreview });
    await draft.open();
    fetchPreview.mockClear();

    draft.edit('H');
    vi.advanceTimersByTime(EMAIL_PREVIEW_DEBOUNCE_MS - 50);
    draft.edit('Hi');
    vi.advanceTimersByTime(EMAIL_PREVIEW_DEBOUNCE_MS - 50);
    expect(fetchPreview).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(50);
    expect(fetchPreview).toHaveBeenCalledTimes(1);
    expect(fetchPreview).toHaveBeenCalledWith('Hi');
  });

  it('drops a preview that arrives after a newer one was asked for', async () => {
    const slow = deferred<EmailPreview>();
    const fetchPreview = vi
      .fn<(greeting: string | undefined) => Promise<EmailPreview>>()
      .mockImplementationOnce(() => slow.promise)
      .mockImplementation(async (greeting) => previewOf(greeting));
    const draft = new EmailDraftController(host(), { loadDefault: async () => 'Old', fetchPreview });

    const opening = draft.open();
    await vi.waitFor(() => expect(fetchPreview).toHaveBeenCalledTimes(1));
    draft.edit('New');
    await draft.refresh();
    slow.resolve(previewOf('Old'));
    await opening;

    expect(draft.preview?.emails[0].body).toContain('New');
    expect(draft.previewLoading).toBe(false);
  });

  it('keeps what was typed while the default was still loading', async () => {
    const saved = deferred<string>();
    const draft = new EmailDraftController(host(), {
      loadDefault: () => saved.promise,
      fetchPreview: async (greeting) => previewOf(greeting),
    });

    const opening = draft.open();
    draft.edit('Typed first');
    saved.resolve('The default');
    await opening;

    expect(draft.message).toBe('Typed first');
  });

  it('leaves the greeting to the server when the default could not be read', async () => {
    const fetchPreview = vi.fn(async (greeting: string | undefined) => previewOf(greeting));
    const draft = new EmailDraftController(host(), {
      loadDefault: async () => {
        throw new Error('offline');
      },
      fetchPreview,
    });

    await draft.open();

    expect(draft.greeting).toBeUndefined();
    expect(draft.messageCaution).toContain('Could not load the saved message');
    expect(draft.messageCaution).toContain('offline');
    expect(draft.messageCaution).not.toContain('Error:');
    expect(fetchPreview).toHaveBeenCalledWith(undefined);

    draft.edit('');
    expect(draft.greeting).toBe('');
  });

  it('lands nothing after it is closed', async () => {
    const slow = deferred<EmailPreview>();
    const draft = new EmailDraftController(host(), {
      loadDefault: async () => '',
      fetchPreview: () => slow.promise,
    });
    const opening = draft.open();
    await Promise.resolve();
    draft.close();
    slow.resolve(previewOf('late'));
    await opening;

    expect(draft.preview).toBeNull();
  });

  it('clears the last preview when a refresh fails', async () => {
    const fetchPreview = vi
      .fn<(greeting: string | undefined) => Promise<EmailPreview>>()
      .mockImplementationOnce(async (greeting) => previewOf(greeting))
      .mockRejectedValueOnce(
        new ApiError({ code: 'internal', rawCode: 'internal', message: 'Template missing.', status: 500 }),
      );
    const draft = new EmailDraftController(host(), { loadDefault: async () => 'Hi', fetchPreview });
    await draft.open();
    expect(draft.preview).not.toBeNull();

    draft.edit('Hi again');
    await draft.refresh();

    expect(draft.preview).toBeNull();
    expect(draft.previewError).toBe('Template missing.');
    expect(draft.previewBlocksSend).toBe(false);
  });

  it('marks an invalid greeting as one the send refuses too', async () => {
    const draft = new EmailDraftController(host(), {
      loadDefault: async () => 'Hi ${nickname}',
      fetchPreview: async () => {
        throw new ApiError({
          code: 'greeting_invalid',
          rawCode: 'greeting_invalid',
          message: 'The greeting names ${nickname}, which is not a placeholder.',
          status: 400,
          details: { placeholder: 'nickname' },
        });
      },
    });
    await draft.open();

    expect(draft.previewBlocksSend).toBe(true);
    expect(draft.previewError).toContain('${nickname}');
  });

  it('shows a plain Error by its message, without the Error prefix', async () => {
    const draft = new EmailDraftController(host(), {
      loadDefault: async () => '',
      fetchPreview: () => Promise.reject(new Error('No document is open.')),
    });
    await draft.open();

    expect(draft.previewError).toBe('No document is open.');
  });
});

describe('reasonOf', () => {
  it('reads an Error by its message and anything else as the fallback', () => {
    expect(reasonOf(new Error('offline'), 'x')).toBe('offline');
    expect(reasonOf('busy', 'x')).toBe('busy');
    expect(reasonOf({}, 'Something failed.')).toBe('Something failed.');
  });
});
