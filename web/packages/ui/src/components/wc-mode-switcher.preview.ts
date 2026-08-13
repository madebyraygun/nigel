import { html } from 'lit';
import './wc-mode-switcher.js';
import '../../src/components/wc-app-shell.js';
import type { Preview } from '../../preview/types.js';

const preview: Preview = {
  id: 'wc-mode-switcher',
  title: 'Appearance mode switcher',
  group: 'Settings',
  description:
    'Light, dark, or follow the system. Three states rather than a toggle, so following the OS stays reachable once a choice has been made. Fully controlled — it never writes storage, which is why driving it here cannot change the real app.',
  layout: 'stack',
  states: [
    {
      name: 'system',
      render: () => html`<wc-mode-switcher mode="system" resolved="light"></wc-mode-switcher>`,
    },
    {
      name: 'system-dark',
      // The hint is the only reason the component knows what `system` resolves
      // to. Injected here rather than read, since jsdom always answers light.
      render: () => html`<wc-mode-switcher mode="system" resolved="dark"></wc-mode-switcher>`,
    },
    {
      name: 'light',
      render: () => html`<wc-mode-switcher mode="light" resolved="light"></wc-mode-switcher>`,
    },
    {
      name: 'dark',
      render: () => html`<wc-mode-switcher mode="dark" resolved="dark"></wc-mode-switcher>`,
    },
    {
      name: 'in-a-settings-panel',
      // Where it actually lives: a row on the settings screen, beside the
      // other preferences rather than in the shell header.
      render: () => html`
        <div
          style="max-width: 34rem; padding: 1rem; border: 1px solid var(--wa-color-border); border-radius: var(--wa-radius-m, 8px); background: var(--wa-color-surface);"
        >
          <wc-mode-switcher mode="system" resolved="dark"></wc-mode-switcher>
        </div>
      `,
    },
  ],
};

export default preview;
