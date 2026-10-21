import { describe, it, expect } from 'vitest';
import { parseHash } from './hash-route.js';
import { trailFor, windowTitle } from './trail.js';

describe('trailFor', () => {
  it('is the screen title alone for a screen', () => {
    expect(trailFor(parseHash('#/dashboard'))).toEqual([{ label: 'Dashboard' }]);
    expect(trailFor(parseHash('#/undo'))).toEqual([{ label: 'Undo last import' }]);
  });

  it('is the screen alone for the invoice list, filtered or not', () => {
    expect(trailFor(parseHash('#/invoices'))).toEqual([{ label: 'Invoices' }]);
    expect(trailFor(parseHash('#/invoices?status=draft'))).toEqual([{ label: 'Invoices' }]);
  });

  it('goes a level down on an open invoice, with Invoices the way back', () => {
    expect(trailFor(parseHash('#/invoices?number=1254'))).toEqual([
      { label: 'Invoices', id: 'invoices' },
      { label: '#1254' },
    ]);
  });

  it('keeps the invoice level while it is being edited', () => {
    expect(trailFor(parseHash('#/invoices?number=1254&edit=1'))).toEqual([
      { label: 'Invoices', id: 'invoices' },
      { label: '#1254' },
    ]);
  });

  it('does not invent a level for a number no invoice could have', () => {
    expect(trailFor(parseHash('#/invoices?number=abc'))).toEqual([{ label: 'Invoices' }]);
  });
});

describe('windowTitle', () => {
  it('is the screen, then the book', () => {
    expect(windowTitle([{ label: 'Dashboard' }], 'Acme Consulting LLC')).toBe(
      'Dashboard · Acme Consulting LLC',
    );
  });

  it('reads innermost first on an open invoice', () => {
    expect(
      windowTitle([{ label: 'Invoices', id: 'invoices' }, { label: '#1254' }], 'Acme Consulting LLC'),
    ).toBe('#1254 · Invoices · Acme Consulting LLC');
  });
});
