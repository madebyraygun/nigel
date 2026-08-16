import { html } from 'lit';
import './wc-send-dialog.js';
import type { SendStepView } from './wc-send-dialog.js';
import type { Preview } from '../../preview/types.js';

const LABELS: Record<string, string> = {
  config: 'Reading the invoicing settings',
  load: 'Loading the invoice',
  precheck: 'Checking the invoice can be sent',
  payment_link: 'Creating the Stripe payment link',
  render: 'Rendering the invoice',
  publish: 'Publishing to R2',
  email: 'Emailing the client',
  record: 'Recording the send',
};

const ORDER = Object.keys(LABELS);

function trace(done: string[], running?: string, failed?: string): SendStepView[] {
  return ORDER.map((step) => ({
    step,
    label: LABELS[step],
    state:
      step === failed
        ? 'failed'
        : step === running
          ? 'running'
          : done.includes(step)
            ? step === 'payment_link'
              ? 'reused'
              : 'ok'
            : 'pending',
  }));
}

const base = html`
  <wc-send-dialog
    open
    .number=${1251}
    .total=${1850}
    .recipient=${'ap@acme.test'}
    .publishHost=${'billing.example.com'}
    .subject=${'Invoice #1251 from Bluepeak'}
  ></wc-send-dialog>
`;

const PREVIEW_PAGE =
  '<h1>Invoice #1251</h1><p>Billed to: Acme Co</p><p><strong>Total: USD 1850.00</strong></p>';

const preview: Preview = {
  id: 'wc-send-dialog',
  title: 'Send dialog',
  group: 'Invoicing',
  description:
    'The confirmation, the step trace, and the outcome. The one dialog that survives its own request — a step trace has nowhere else to be rendered.',
  layout: 'stack',
  states: [
    { name: 'confirm', render: () => base },
    {
      name: 'confirm-with-preview',
      render: () => html`
        <wc-send-dialog
          open
          number="1251"
          .total=${1850}
          .client=${'Acme Co'}
          .recipient=${'ap@acme.test'}
          .publishHost=${'billing.example.com'}
          .subject=${'Invoice #1251 from Bluepeak LLC'}
          .pdfHref=${'#pdf'}
          .previewHtml=${PREVIEW_PAGE}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'preview-loading',
      render: () => html`
        <wc-send-dialog
          open
          number="1251"
          .total=${1850}
          .client=${'Acme Co'}
          .recipient=${'ap@acme.test'}
          .previewLoading=${true}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'preview-failed',
      render: () => html`
        <wc-send-dialog
          open
          number="1251"
          .total=${1850}
          .client=${'Acme Co'}
          .recipient=${'ap@acme.test'}
          .previewError=${'Invoice template /books/templates/invoice.html is empty.'}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'blocked-no-pdf',
      render: () => html`
        <wc-send-dialog
          open
          number="1251"
          .total=${1850}
          .client=${'Acme Co'}
          .recipient=${'ap@acme.test'}
          .previewHtml=${PREVIEW_PAGE}
          .pdfAvailable=${false}
          .blocked=${'This build cannot send — PDF support is not compiled in, and the invoice PDF is attached to the email.'}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'config-caution',
      render: () => html`
        <wc-send-dialog
          open
          number="1251"
          .total=${1850}
          .client=${'Acme Co'}
          .recipient=${'ap@acme.test'}
          .previewHtml=${PREVIEW_PAGE}
          .configCautions=${[
            'public_base_url does not end in /i — Nigel writes objects under the i/ prefix, so published links will 404 unless that prefix is what this address serves.',
          ]}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'blocked-no-email',
      render: () => html`
        <wc-send-dialog
          open
          .number=${1249}
          .total=${960}
          .recipient=${''}
          .blocked=${'Globex has no email address. Add one on the client before sending.'}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'in-flight',
      render: () => html`
        <wc-send-dialog
          open
          phase="sending"
          .number=${1251}
          .total=${1850}
          .recipient=${'ap@acme.test'}
          .steps=${trace(['config', 'load', 'precheck', 'payment_link', 'render'], 'publish')}
        ></wc-send-dialog>
      `,
    },
    {
      // A step label wide enough to wrap in the dialog. The mark belongs on
      // the label's first line and in line with the single-line row above it.
      name: 'in-flight-wrapping-label',
      render: () => html`
        <wc-send-dialog
          open
          phase="sending"
          .number=${1251}
          .total=${1850}
          .recipient=${'ap@acme.test'}
          .steps=${[
            { step: 'config', label: LABELS.config, state: 'ok' },
            {
              step: 'publish',
              label:
                'Publishing the rendered invoice page and its PDF attachment to the configured R2 bucket',
              state: 'running',
            },
            { step: 'email', label: LABELS.email, state: 'pending' },
          ] satisfies SendStepView[]}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'sent',
      render: () => html`
        <wc-send-dialog
          open
          phase="sent"
          .number=${1251}
          .total=${1850}
          .recipient=${'ap@acme.test'}
          .publicUrl=${'https://billing.example.com/i/aBc123XyZ/index.html'}
          .steps=${trace(ORDER)}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'failed-at-publish',
      render: () => html`
        <wc-send-dialog
          open
          phase="failed"
          .number=${1251}
          .total=${1850}
          .recipient=${'ap@acme.test'}
          .steps=${trace(
            ['config', 'load', 'precheck', 'payment_link', 'render'],
            undefined,
            'publish',
          )}
          .failure=${{
            headline: 'Publishing the invoice page failed.',
            message: 'r2 403: SignatureDoesNotMatch',
            note: 'No email was sent, and invoice #1251 is still a draft.',
            retryable: true,
          }}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'failed-not-configured',
      render: () => html`
        <wc-send-dialog
          open
          phase="failed"
          .number=${1251}
          .total=${1850}
          .recipient=${'ap@acme.test'}
          .steps=${trace([], undefined, 'config')}
          .failure=${{
            headline: 'Sending is not configured yet.',
            message: 'Sending needs r2_bucket, public_base_url, which are not set.',
            note: 'These are settings, not something this invoice can fix.',
            retryable: false,
            actionLabel: 'Open settings',
            actionHref: '#/settings',
          }}
        ></wc-send-dialog>
      `,
    },
    {
      name: 'failed-at-record',
      render: () => html`
        <wc-send-dialog
          open
          phase="failed"
          .number=${1251}
          .total=${1850}
          .recipient=${'ap@acme.test'}
          .steps=${trace(
            ['config', 'load', 'precheck', 'payment_link', 'render', 'publish', 'email'],
            undefined,
            'record',
          )}
          .failure=${{
            headline: 'The send could not be recorded.',
            message: 'database is locked',
            note: 'The invoice was emailed but Nigel could not record it. Run `nigel invoice show 1251` to check before sending again.',
            retryable: false,
          }}
        ></wc-send-dialog>
      `,
    },
  ],
};

export default preview;
