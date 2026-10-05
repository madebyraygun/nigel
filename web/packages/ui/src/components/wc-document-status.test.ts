import { describe, it, expect, afterEach } from 'vitest';
import './wc-document-status.js';
import { documentStatusLabel } from './wc-document-status.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-document-status.preview.js';

describe('wc-document-status', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('words changes_requested for a person', () => {
    expect(documentStatusLabel('changes_requested')).toBe('changes requested');
    expect(documentStatusLabel('constructor')).toBe('constructor');
  });

  it('renders the word beside the mark', async () => {
    const el = document.createElement('wc-document-status');
    el.status = 'accepted';
    document.body.appendChild(el);
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.word')?.textContent).toBe('accepted');
    expect(el.shadowRoot?.querySelector('wc-icon-check')).toBeTruthy();
  });
});

describePreviewA11y(preview);
