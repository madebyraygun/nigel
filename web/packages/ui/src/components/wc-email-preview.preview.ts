import { html, type TemplateResult } from 'lit';
import './wc-email-preview.js';
import type { EmailPreviewView } from './wc-email-preview.js';
import type { Preview } from '../../preview/types.js';

const PAY = '[pay link created when sent]';
const LINK = '[link created when sent]';

const INVOICE: EmailPreviewView = {
  role: 'client',
  to: 'Acme Accounts <ap@acme.test>',
  subject: 'Invoice #1251 from Initech',
  body: [
    'Thanks again for a great quarter. This covers the March retainer and the extra workshop day.',
    'Invoice #1251 from Initech for USD 1850.00, due 2026-04-14.',
    `View and pay online: ${PAY}`,
    'The invoice PDF is attached.',
    'Thank you,\nInitech',
  ].join('\n\n'),
  attachments: ['Invoice-1251.pdf'],
};

function documentEmail(
  role: 'signer' | 'collaborator',
  name: string,
  email: string,
): EmailPreviewView {
  return {
    role,
    to: `${name} <${email}>`,
    subject: 'Agreement: Cedar Systems services agreement from Initech',
    body: [
      `Hello ${name},`,
      'Here is the agreement we talked through on Tuesday.',
      role === 'signer'
        ? 'Initech has sent you “Cedar Systems services agreement” to review and accept.'
        : 'Initech has shared “Cedar Systems services agreement” with you for review.',
      `Your personal link: ${LINK}`,
      'The PDF is attached.',
      'Thank you,\nInitech',
    ].join('\n\n'),
    attachments: ['Cedar-Systems-services-agreement-v1.pdf'],
  };
}

const NO_GREETING: EmailPreviewView = {
  ...INVOICE,
  body: INVOICE.body.split('\n\n').slice(1).join('\n\n'),
};

/** A send dialog's width, so the preview reads as it does in place. */
const frame = (content: TemplateResult) => html`<div style="width:44rem">${content}</div>`;

const preview: Preview = {
  id: 'wc-email-preview',
  title: 'Email preview',
  group: 'Invoicing',
  description:
    'The email a send would deliver — To, subject, attachments and the plain-text body. Links made at send time are marked.',
  layout: 'stack',
  states: [
    {
      name: 'invoice',
      render: () =>
        frame(html`<wc-email-preview .emails=${[INVOICE]} .placeholders=${[PAY]}></wc-email-preview>`),
    },
    {
      name: 'no-greeting',
      render: () =>
        frame(html`<wc-email-preview
          .emails=${[NO_GREETING]}
          .placeholders=${[PAY]}
        ></wc-email-preview>`),
    },
    {
      name: 'document-recipients',
      render: () =>
        frame(html`<wc-email-preview
        .emails=${[
          documentEmail('signer', 'Pat Example', 'pat@cedar.test'),
          documentEmail('collaborator', 'Sam Example', 'sam@cedar.test'),
        ]}
        .placeholders=${[LINK]}
      ></wc-email-preview>`),
    },
    {
      name: 'loading',
      render: () =>
        frame(html`<wc-email-preview loading></wc-email-preview>`),
    },
    {
      name: 'updating',
      render: () =>
        frame(html`<wc-email-preview
          loading
          .emails=${[INVOICE]}
          .placeholders=${[PAY]}
        ></wc-email-preview>`),
    },
    {
      name: 'error',
      render: () =>
        frame(html`<wc-email-preview error="Juniper Labs has no email address."></wc-email-preview>`),
    },
  ],
};

export default preview;
