import { describe, it, expect } from 'vitest';

import { ApiError } from '../api/index.js';
import { documentSendFailureMessage, documentsGuardrailMessage } from './documents-errors.js';

function conflict(reason: string, details: Record<string, unknown> = {}, message = 'refused') {
  return new ApiError({
    code: 'conflict',
    rawCode: 'conflict',
    message,
    status: 409,
    details: { reason, ...details },
  });
}

function upstream(details: Record<string, unknown>, message: string, status = 502) {
  return new ApiError({
    code: 'upstream_failed',
    rawCode: 'upstream_failed',
    message,
    status,
    details: { reason: 'send_failed', ...details },
  });
}

describe('documentsGuardrailMessage', () => {
  const sentences: [string, Record<string, unknown>, string][] = [
    ['document_terminal', {}, 'This document is withdrawn, executed or declined. Nothing more can be done to it.'],
    ['document_accepted', {}, 'This document is accepted. The only step left is to countersign it.'],
    ['document_wrong_state', {}, 'This document is not in a state that allows that. The view has been refreshed.'],
    ['duplicate_document', {}, 'This PDF is already filed for this client.'],
    ['unchanged_revision', {}, 'This PDF is identical to the current version. A revision needs different content.'],
    ['signer_count', {}, 'A document is sent to exactly one signer.'],
    ['signer_name_required', {}, 'The signer needs a name, because accepting means typing it.'],
    ['kind_inactive', {}, 'That document kind is no longer in use. Choose another.'],
    ['file_changed', {}, 'The filed PDF no longer matches the checksum recorded when it was filed.'],
    ['stale_version', {}, 'That response is for a version that is no longer the latest sent.'],
    ['checksum_mismatch', {}, 'That response was given on a different file than the version it names.'],
    ['role_not_allowed', {}, 'A collaborator cannot accept a document. Only the signer can.'],
    ['client_archived', {}, 'This client is archived. Unarchive it before filing or sending documents.'],
    ['send_misconfigured', {}, 'Nigel cannot send with the current email settings.'],
    ['invalid_public_base_url', {}, 'The documents base URL is not a valid public address. Check it in settings.'],
  ];

  for (const [reason, details, sentence] of sentences) {
    it(`explains ${reason} in our words, not the server’s`, () => {
      expect(documentsGuardrailMessage(conflict(reason, details, 'server english'))).toBe(sentence);
    });
  }

  it('names the unset keys for a send, singular and plural', () => {
    expect(
      documentsGuardrailMessage(conflict('send_not_configured', { missing: ['mailgun_api_key'] })),
    ).toBe('Sending documents needs mailgun_api_key, which is not set.');
    expect(
      documentsGuardrailMessage(
        conflict('send_not_configured', { missing: ['mailgun_api_key', 'r2_private_bucket'] }),
      ),
    ).toBe('Sending documents needs mailgun_api_key, r2_private_bucket, which are not set.');
  });

  it('names the unset keys for a sync', () => {
    expect(
      documentsGuardrailMessage(conflict('sync_not_configured', { missing: ['r2_secret_key'] })),
    ).toBe('Syncing responses needs r2_secret_key, which is not set.');
  });

  it('asks for an address when the client has none', () => {
    expect(
      documentsGuardrailMessage(conflict('client_missing_email', { clientName: 'Globex' })),
    ).toBe('Globex has no email address. Add one on the client before sending.');
  });

  it('explains a missing document from a 404 reason', () => {
    const error = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'document 9 not found',
      status: 404,
      details: { reason: 'document_not_found' },
    });
    expect(documentsGuardrailMessage(error)).toBe('This document no longer exists.');
  });

  it('explains an unknown kind from a 404 reason', () => {
    const error = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'Document kind not found: Memo',
      status: 404,
      details: { reason: 'kind_not_found' },
    });
    expect(documentsGuardrailMessage(error)).toBe(
      'That document kind does not exist. Choose another.',
    );
  });

  it('renders the server’s message for a 400, which names the offending value', () => {
    const error = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'Invalid status: bogus',
      status: 400,
    });
    expect(documentsGuardrailMessage(error)).toBe('Invalid status: bogus');
  });

  it('renders the server’s message for an unrecognized 409 reason', () => {
    expect(
      documentsGuardrailMessage(conflict('something_new', {}, 'The server’s own sentence')),
    ).toBe('The server’s own sentence');
  });

  it('falls back to a generic sentence for a non-API error', () => {
    expect(documentsGuardrailMessage(new Error('boom'))).toBe('Could not save that document.');
  });
});

describe('documentSendFailureMessage', () => {
  it('is not retryable once an address was emailed, and names who and the dead links', () => {
    const view = documentSendFailureMessage(
      upstream(
        { step: 'email', service: 'mailgun', emailed: ['pat@cedar.test'], documentStatus: 'draft' },
        'mailgun 500: boom',
      ),
      'Website refresh',
    );
    expect(view.headline).toBe('The pages were published, but Mailgun would not send every email.');
    expect(view.message).toBe('mailgun 500: boom');
    expect(view.retryable).toBe(false);
    expect(view.note).toContain('pat@cedar.test');
    expect(view.note).toContain('links are no longer live');
    expect(view.note).toContain('Website refresh');
  });

  it('lists every emailed address', () => {
    const view = documentSendFailureMessage(
      upstream({ step: 'email', service: 'mailgun', emailed: ['pat@cedar.test', 'sam@cedar.test'] }, 'x'),
      'Website refresh',
    );
    expect(view.note).toContain('pat@cedar.test, sam@cedar.test');
  });

  it('is retryable when nothing was emailed', () => {
    const view = documentSendFailureMessage(
      upstream({ step: 'publish', service: 'r2', emailed: [], documentStatus: 'draft' }, 'r2 403: SignatureDoesNotMatch'),
      'Website refresh',
    );
    expect(view.headline).toBe('Publishing the pages to Cloudflare R2 failed.');
    expect(view.message).toBe('r2 403: SignatureDoesNotMatch');
    expect(view.retryable).toBe(true);
    expect(view.note).toBe('No email was sent, and “Website refresh” is still a draft.');
  });

  it('carries cleanup warnings into the note', () => {
    const view = documentSendFailureMessage(
      upstream({ step: 'publish', cleanupWarnings: ['The manifest could not be closed.'] }, 'x'),
      'Website refresh',
    );
    expect(view.note).toContain('The manifest could not be closed.');
  });

  it('points an unconfigured send at settings and is not retryable', () => {
    const view = documentSendFailureMessage(
      conflict('send_not_configured', { missing: ['r2_private_bucket'] }),
      'Website refresh',
    );
    expect(view.headline).toBe('Sending is not configured yet.');
    expect(view.message).toBe('Sending documents needs r2_private_bucket, which is not set.');
    expect(view.retryable).toBe(false);
    expect(view.actionHref).toBe('#/settings');
  });

  it('uses the reason sentence for another recognized refusal', () => {
    const view = documentSendFailureMessage(conflict('signer_count', {}, 'raw'), 'Website refresh');
    expect(view.headline).toBe('A document is sent to exactly one signer.');
    expect(view.message).toBe('raw');
    expect(view.retryable).toBe(false);
  });

  it('is not retryable on a 501, and says why', () => {
    const view = documentSendFailureMessage(
      upstream({ step: 'render' }, 'pdf feature not built', 501),
      'Website refresh',
    );
    expect(view.retryable).toBe(false);
    expect(view.note).toContain('cannot');
  });

  it('has a headline when the failure names no step', () => {
    const view = documentSendFailureMessage(new Error('network down'), 'Website refresh');
    expect(view.headline).toBe('The document could not be sent.');
    expect(view.message).toBe('Error: network down');
    expect(view.retryable).toBe(true);
  });
});
