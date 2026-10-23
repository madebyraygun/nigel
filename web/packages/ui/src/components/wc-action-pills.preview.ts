import { html } from 'lit';
import './wc-action-pills.js';
import type { ActionPill } from './wc-action-pills.js';
import type { Preview } from '../../preview/types.js';

const LIST: ActionPill[] = [
  {
    key: 'sync',
    label: 'Sync now',
    icon: 'refresh',
    hint: 'Pull paid Stripe checkouts and record them',
  },
  { key: 'new', label: 'New invoice', icon: 'plus', tone: 'primary' },
];

const DRAFT: ActionPill[] = [
  { key: 'send', label: 'Send…', tone: 'primary' },
  {
    key: 'pay',
    label: 'Record payment…',
    disabled: true,
    reason: 'A payment can be recorded once the invoice is sent.',
  },
  { key: 'edit', label: 'Edit' },
  { key: 'duplicate', label: 'Duplicate' },
  { key: 'delete', label: 'Delete…', tone: 'danger' },
];

const SCHEDULED: ActionPill[] = [
  { key: 'send', label: 'Send…', tone: 'primary' },
  {
    key: 'pay',
    label: 'Record payment…',
    disabled: true,
    reason: 'A payment can be recorded once the invoice is sent.',
  },
  { key: 'edit', label: 'Edit' },
  { key: 'duplicate', label: 'Duplicate' },
  {
    key: 'delete',
    label: 'Delete…',
    tone: 'danger',
    disabled: true,
    reason:
      'A recurring schedule generated this invoice and keeps it as the record of what that period billed. Void it instead.',
  },
  { key: 'void', label: 'Void…', tone: 'danger' },
];

const PAID: ActionPill[] = [
  {
    key: 'send',
    label: 'Resend…',
    tone: 'primary',
    disabled: true,
    reason: 'A paid invoice has nothing left to bill for.',
  },
  {
    key: 'pay',
    label: 'Record payment…',
    disabled: true,
    reason: 'Nothing is outstanding.',
  },
  { key: 'edit', label: 'Edit', disabled: true, reason: 'A paid invoice cannot be edited.' },
  { key: 'duplicate', label: 'Duplicate' },
  { key: 'void', label: 'Void…', tone: 'danger', disabled: true },
];

const preview: Preview = {
  id: 'wc-action-pills',
  title: 'Action pills',
  group: 'Controls',
  description:
    '32px pill buttons for a title row: filled brand for the primary action, outlined for the rest, outlined in the expense colour for a destructive one. A disabled action sits at 40% and, when it carries a reason, says why underneath.',
  layout: 'stack',
  states: [
    { name: 'list-title', render: () => html`<wc-action-pills .actions=${LIST}></wc-action-pills>` },
    {
      name: 'draft',
      render: () => html`<wc-action-pills align="end" .actions=${DRAFT}></wc-action-pills>`,
    },
    {
      name: 'schedule-generated-draft',
      render: () => html`<wc-action-pills align="end" .actions=${SCHEDULED}></wc-action-pills>`,
    },
    {
      name: 'paid',
      render: () => html`<wc-action-pills align="end" .actions=${PAID}></wc-action-pills>`,
    },
  ],
};

export default preview;
