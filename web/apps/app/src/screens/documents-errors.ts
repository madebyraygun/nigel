import type { SendFailureView } from '@nigel/ui';

import { ApiError } from '../api/index.js';
import type {
  ConflictDetails,
  DocumentSendErrorDetails,
  DocumentSendStep,
} from '../api/types.js';
import { conflictDetailsOf } from './manager-errors.js';

/** The `details` of a document send failure, whatever status it came back as. */
export function documentSendDetailsOf(error: unknown): DocumentSendErrorDetails | null {
  if (!(error instanceof ApiError)) return null;
  const details = error.details;
  if (typeof details !== 'object' || details === null) return null;
  return details as DocumentSendErrorDetails;
}

function missingKeysSentence(subject: string, missing: string[]): string {
  if (missing.length === 0) return `${subject} is not configured yet.`;
  return `${subject} needs ${missing.join(', ')}, which ${
    missing.length === 1 ? 'is' : 'are'
  } not set.`;
}

/**
 * What to tell the user about a refused documents write.
 *
 * Built the way `invoicing-errors.ts` is: the reason code is the contract and
 * the sentence is ours. The same two deliberate exceptions: a **400** renders
 * the server's message because it names the offending value, and an
 * **unrecognized 409 reason** does too, because inventing a sentence would
 * hide the only information there is.
 */
export function documentsGuardrailMessage(error: unknown): string {
  const details = conflictDetailsOf(error);
  if (details) {
    const sentence = conflictSentence(details);
    if (sentence) return sentence;
  }

  if (error instanceof ApiError) {
    if (error.status === 404) {
      const reason = notFoundReason(error);
      if (reason === 'document_not_found') return 'This document no longer exists.';
      if (reason === 'kind_not_found') return 'That document kind does not exist. Choose another.';
    }
    return error.message;
  }
  return 'Could not save that document.';
}

function notFoundReason(error: ApiError): string | undefined {
  const details = error.details;
  if (typeof details !== 'object' || details === null) return undefined;
  return (details as { reason?: string }).reason;
}

function conflictSentence(details: ConflictDetails): string | null {
  switch (details.reason) {
    case 'document_terminal':
      return 'This document is withdrawn, executed or declined. Nothing more can be done to it.';
    case 'document_accepted':
      return 'This document is accepted. The only step left is to countersign it.';
    case 'document_wrong_state':
      return 'This document is not in a state that allows that. The view has been refreshed.';
    case 'duplicate_document':
      return 'This PDF is already filed for this client.';
    case 'unchanged_revision':
      return 'This PDF is identical to the current version. A revision needs different content.';
    case 'signer_count':
      return 'A document is sent to exactly one signer.';
    case 'version_sent':
      return 'That version has already been sent, so its recipients are fixed.';
    case 'signer_name_required':
      return 'The signer needs a name, because accepting means typing it.';
    case 'kind_inactive':
      return 'That document kind is no longer in use. Choose another.';
    case 'file_changed':
      return 'The filed PDF no longer matches the checksum recorded when it was filed.';
    case 'stale_version':
      return 'That response is for a version that is no longer the latest sent.';
    case 'checksum_mismatch':
      return 'That response was given on a different file than the version it names.';
    case 'role_not_allowed':
      return 'A collaborator cannot accept a document. Only the signer can.';
    case 'client_archived':
      return 'This client is archived. Unarchive it before filing or sending documents.';
    case 'client_missing_email':
      return details.clientName
        ? `${details.clientName} has no email address. Add one on the client before sending.`
        : 'This client has no email address. Add one before sending.';
    case 'send_not_configured':
      return missingKeysSentence('Sending documents', details.missing ?? []);
    case 'send_misconfigured':
      return 'Nigel cannot send with the current email settings.';
    case 'sync_not_configured':
      return missingKeysSentence('Syncing responses', details.missing ?? []);
    case 'invalid_public_base_url':
      return 'The documents base URL is not a valid public address. Check it in settings.';
    default:
      return null;
  }
}

const SERVICE_NAMES: Record<string, string> = {
  r2: 'Cloudflare R2',
  mailgun: 'Mailgun',
};

const STEP_HEADLINES: Record<DocumentSendStep, (service: string) => string> = {
  config: () => 'Nigel could not read the document settings.',
  load: () => 'The document could not be loaded.',
  render: () => 'The document pages could not be rendered.',
  freeze: () => 'This document cannot be sent.',
  publish: (service) => `Publishing the pages${service ? ` to ${service}` : ''} failed.`,
  manifest: (service) => `Opening the response manifest${service ? ` on ${service}` : ''} failed.`,
  email: (service) =>
    `The pages were published, but ${service || 'the mail gateway'} would not send every email.`,
  record: () => 'The send could not be recorded.',
};

/**
 * A document send failure, turned into the strings the dialog renders.
 *
 * Our words for what failed, the upstream's own for why. `retryable` is false
 * for a refusal that would refuse again identically, for a build that cannot
 * render PDFs (501), and once any address was emailed: the send rolled back, so those links are dead and a second send is
 * a fresh decision rather than a repeat.
 */
export function documentSendFailureMessage(error: unknown, title: string): SendFailureView {
  const message = error instanceof ApiError ? error.message : String(error);
  const featureMissing = error instanceof ApiError && error.status === 501;

  const conflict = conflictDetailsOf(error);
  if (conflict?.reason === 'send_not_configured') {
    return {
      headline: 'Sending is not configured yet.',
      message: missingKeysSentence('Sending documents', conflict.missing ?? []),
      note: 'These are settings, not something this document can fix.',
      retryable: false,
      actionLabel: 'Open settings',
      actionHref: '#/settings',
    };
  }
  const sentence = conflict ? conflictSentence(conflict) : null;
  if (sentence) return { headline: sentence, message, retryable: false };

  const details = documentSendDetailsOf(error);
  const step = details?.step;
  if (!step) {
    return {
      headline: 'The document could not be sent.',
      message,
      retryable: !featureMissing,
    };
  }

  const service = details.service ? (SERVICE_NAMES[details.service] ?? details.service) : '';
  const emailed = details.emailed ?? [];
  const cleanup = (details.cleanupWarnings ?? []).join(' ');

  let note: string;
  if (emailed.length > 0) {
    note = `Already emailed: ${emailed.join(', ')}. The send was rolled back, so their links are no longer live and “${title}” is still a draft.`;
  } else if (featureMissing) {
    note = 'No email was sent. This build of Nigel cannot render a PDF, so no document can be sent from it.';
  } else {
    note = `No email was sent${
      details.documentStatus ? `, and “${title}” is still a ${details.documentStatus}` : ''
    }.`;
  }
  if (cleanup) note = `${note} ${cleanup}`;

  return {
    headline: STEP_HEADLINES[step](service),
    message,
    note,
    retryable: emailed.length === 0 && !featureMissing,
  };
}
