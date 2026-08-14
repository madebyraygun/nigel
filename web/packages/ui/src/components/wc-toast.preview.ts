import { html } from 'lit';
import './wc-toast.js';
import type { Preview } from '../../preview/types.js';
import type { NcToastDetail } from './wc-toast.js';

/**
 * Long enough to hold still while a state is being looked at, and short of
 * `duration: 0`, which is the never-expires case that earns a close button.
 */
const PREVIEW_DURATION_MS = 10 * 60 * 1000;

const seeded = (initial: NcToastDetail | NcToastDetail[]) => {
  const held = (Array.isArray(initial) ? initial : [initial]).map((detail) => ({
    duration: PREVIEW_DURATION_MS,
    ...detail,
  }));
  return html`<wc-toast .initial=${held}></wc-toast>`;
};

const preview: Preview = {
  id: 'wc-toast',
  title: 'Toast',
  group: 'Feedback',
  description:
    'The polite live region terminating the nc-toast bus. It pins to the bottom-right corner of the viewport, clear of the sidebar and header, and stacks up to three toasts; a danger toast is its own alert. States seed toasts via .initial with a long duration so they stay visible — except never-expires, which is the duration-zero case and carries a close button.',
  layout: 'stack',
  states: [
    {
      name: 'info',
      render: () => seeded({ message: 'Rules re-applied.' } satisfies NcToastDetail),
    },
    {
      name: 'success',
      render: () =>
        seeded({
          message: '42 transactions imported.',
          variant: 'success',
        } satisfies NcToastDetail),
    },
    {
      name: 'danger',
      render: () =>
        seeded({
          message: 'Could not reach the nigel server.',
          variant: 'danger',
        } satisfies NcToastDetail),
    },
    {
      name: 'with-action',
      render: () =>
        seeded({
          message: 'Import undone.',
          action: { label: 'Redo', onClick: () => {} },
        } satisfies NcToastDetail),
    },
    {
      name: 'long-message',
      render: () =>
        seeded({
          message:
            'The category "Office Supplies" could not be deleted because 37 transactions still reference it, and every one of them would be left without a category.',
          variant: 'danger',
        } satisfies NcToastDetail),
    },
    {
      name: 'never-expires',
      render: () =>
        seeded({
          message: 'Could not reach the nigel server.',
          variant: 'danger',
          duration: 0,
        } satisfies NcToastDetail),
    },
    {
      name: 'stacked',
      render: () =>
        seeded([
          { message: 'Rules re-applied.' },
          { message: '42 transactions imported.', variant: 'success' },
          {
            message: 'Import undone.',
            action: { label: 'Redo', onClick: () => {} },
          },
        ] satisfies NcToastDetail[]),
    },
  ],
};

export default preview;
