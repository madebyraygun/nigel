import { html } from 'lit';
import './wc-security-card.js';
import './wc-password-form.js';
import type { Preview } from '../../preview/types.js';
import type { SecurityPanel } from './wc-security-card.js';

const width = 'width:min(720px, calc(100vw - 32px));';

const encrypted = (panel: SecurityPanel, error = '') =>
  html`<div style=${width}>
    <wc-security-card encrypted .panel=${panel}>
      <wc-password-form
        slot="change"
        mode="change"
        layout="inline"
        error=${panel === 'change' ? error : ''}
      ></wc-password-form>
      <wc-password-form
        slot="remove"
        mode="remove"
        layout="inline"
        error=${panel === 'remove' ? error : ''}
      ></wc-password-form>
    </wc-security-card>
  </div>`;

const preview: Preview = {
  id: 'wc-security-card',
  title: 'Security Card',
  group: 'Forms',
  description:
    "The database's encryption state, and the password operations it allows as inline panels, one open at a time.",
  layout: 'stack',
  states: [
    {
      name: 'not encrypted',
      render: () =>
        html`<div style=${width}>
          <wc-security-card>
            <wc-password-form slot="set" mode="set" layout="inline"></wc-password-form>
          </wc-security-card>
        </div>`,
    },
    { name: 'encrypted', render: () => encrypted(null) },
    { name: 'change open', render: () => encrypted('change') },
    { name: 'remove open', render: () => encrypted('remove') },
    { name: 'wrong password', render: () => encrypted('change', 'Wrong password.') },
  ],
};

export default preview;
