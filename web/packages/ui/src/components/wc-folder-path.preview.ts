import { html } from 'lit';
import '@awesome.me/webawesome/dist/components/button/button.js';
import './wc-folder-path.js';
import type { Preview } from '../../preview/types.js';

const width = 'width:min(720px, calc(100vw - 32px));';

const preview: Preview = {
  id: 'wc-folder-path',
  title: 'Folder Path',
  group: 'Data',
  description: 'A folder on disk as selectable code, with what can be done about it underneath.',
  layout: 'stack',
  states: [
    {
      name: 'with an action',
      render: () =>
        html`<div style=${width}>
          <wc-folder-path path="~/Documents/Nigel/acme-consulting">
            <wa-button>Reveal in Finder</wa-button>
          </wc-folder-path>
        </div>`,
    },
    {
      name: 'path only',
      render: () =>
        html`<div style=${width}>
          <wc-folder-path path="~/Documents/Nigel/juniper-labs"></wc-folder-path>
        </div>`,
    },
    {
      name: 'long path',
      render: () =>
        html`<div style="width:280px;">
          <wc-folder-path
            path="~/Library/Mobile Documents/com~apple~CloudDocs/Bookkeeping/Nigel/harbor-and-vale"
          ></wc-folder-path>
        </div>`,
    },
  ],
};

export default preview;
