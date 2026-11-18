import { describe, it, expect, afterEach } from 'vitest';
import './wc-email-preview.js';
import {
  splitPlaceholders,
  type EmailPreviewView,
  type WcEmailPreview,
} from './wc-email-preview.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-email-preview.preview.js';

const LINK = '[link created when sent]';

const EMAIL: EmailPreviewView = {
  role: 'signer',
  to: 'Pat Example <pat@cedar.test>',
  subject: 'Agreement from Initech',
  body: `Hello Pat Example,\n\nHere it is.\n\nYour link: ${LINK}`,
  attachments: ['Agreement-v1.pdf', 'Schedule-A.pdf'],
};

async function mount(props: Partial<WcEmailPreview>): Promise<WcEmailPreview> {
  const el = document.createElement('wc-email-preview');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('splitPlaceholders', () => {
  it('splits around every placeholder and keeps the text between', () => {
    expect(splitPlaceholders(`a ${LINK} b ${LINK}`, [LINK])).toEqual([
      { text: 'a ', placeholder: false },
      { text: LINK, placeholder: true },
      { text: ' b ', placeholder: false },
      { text: LINK, placeholder: true },
    ]);
  });

  it('leaves the body whole when there is nothing to mark', () => {
    expect(splitPlaceholders('plain (text) [x]', [])).toEqual([
      { text: 'plain (text) [x]', placeholder: false },
    ]);
  });
});

describe('wc-email-preview', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('shows the To, subject, attachments and the body', async () => {
    const el = await mount({ emails: [EMAIL], placeholders: [LINK] });
    const root = el.shadowRoot!;
    expect(root.querySelector('[data-to]')?.textContent).toContain('Pat Example <pat@cedar.test>');
    expect(root.querySelector('[data-subject]')?.textContent).toBe('Agreement from Initech');
    expect(root.querySelector('[data-attachments]')?.textContent).toBe(
      'Agreement-v1.pdf, Schedule-A.pdf',
    );
    expect(root.querySelector('[data-body]')?.textContent).toContain('Here it is.');
  });

  it('marks each placeholder in the body', async () => {
    const el = await mount({ emails: [EMAIL], placeholders: [LINK] });
    const marks = el.shadowRoot!.querySelectorAll('[data-body] mark[data-placeholder]');
    expect([...marks].map((m) => m.textContent)).toEqual([LINK]);
  });

  it('gives each recipient its own block and names its role', async () => {
    const el = await mount({
      emails: [EMAIL, { ...EMAIL, role: 'collaborator', to: 'Sam Example <sam@cedar.test>' }],
    });
    const blocks = el.shadowRoot!.querySelectorAll('[data-email]');
    expect(blocks).toHaveLength(2);
    expect(blocks[1].querySelector('[data-to]')?.textContent).toContain('Collaborator');
  });

  it('keeps the last preview on screen while a refresh runs', async () => {
    const el = await mount({ emails: [EMAIL], loading: true });
    const root = el.shadowRoot!;
    expect(root.querySelector('[data-email]')).toBeTruthy();
    expect(root.querySelector('[data-updating]')).toBeTruthy();
    expect(root.querySelector('[data-loading]')).toBeNull();
  });

  it('shows a spinner before the first preview arrives', async () => {
    const el = await mount({ loading: true });
    expect(el.shadowRoot!.querySelector('[data-loading]')).toBeTruthy();
  });

  it('says a failure does not stop the send', async () => {
    const el = await mount({ error: 'Cedar Systems has no email address.' });
    expect(el.shadowRoot!.querySelector('[data-error]')?.getAttribute('message')).toContain(
      'You can still send.',
    );
  });
});

describePreviewA11y(preview);
