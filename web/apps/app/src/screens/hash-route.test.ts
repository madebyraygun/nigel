import { describe, it, expect } from 'vitest';
import { parseHash, routeToHash, screenToHash } from './hash-route.js';

describe('parseHash', () => {
  it('keeps the hosting flag out of the screen params', () => {
    const route = parseHash('#/settings?shell=native');
    expect(route.screen).toBe('settings');
    expect(route.params.has('shell')).toBe(false);

    const invoice = parseHash('#/invoices?number=1254&shell=native');
    expect(invoice.params.toString()).toBe('number=1254');
  });

  it('reads a screen id', () => {
    expect(parseHash('#/register').screen).toBe('register');
  });

  it('tolerates a missing slash', () => {
    expect(parseHash('#register').screen).toBe('register');
  });

  it('falls back to the dashboard for an empty hash', () => {
    expect(parseHash('').screen).toBe('dashboard');
    expect(parseHash('#').screen).toBe('dashboard');
    expect(parseHash('#/').screen).toBe('dashboard');
  });

  it('falls back to the dashboard for an unknown screen', () => {
    // A stale bookmark should land somewhere useful, not on a blank page.
    expect(parseHash('#/nonsense').screen).toBe('dashboard');
  });

  it('preserves query parameters for the screen tasks', () => {
    const route = parseHash('#/register?year=2025&account=BofA%20Checking');
    expect(route.screen).toBe('register');
    expect(route.params.get('year')).toBe('2025');
    expect(route.params.get('account')).toBe('BofA Checking');
  });

  it('yields empty params when there is no query', () => {
    expect([...parseHash('#/register').params]).toEqual([]);
  });

  it('reads an invoice number from the path, the way the breadcrumb spells it', () => {
    const route = parseHash('#/invoices/1254');
    expect(route.screen).toBe('invoices');
    expect(route.params.get('number')).toBe('1254');

    const edit = parseHash('#/invoices/1254?edit=1&shell=native');
    expect(edit.params.get('number')).toBe('1254');
    expect(edit.params.get('edit')).toBe('1');
    expect(edit.params.has('shell')).toBe(false);
  });

  it('still opens an invoice from the query form', () => {
    expect(parseHash('#/invoices?number=1254').params.get('number')).toBe('1254');
  });

  it('refuses a path segment on a screen that takes none', () => {
    expect(parseHash('#/register/1254').screen).toBe('dashboard');
    expect(parseHash('#/invoices/1254/extra').screen).toBe('dashboard');
  });

  it('falls back to the dashboard for a segment that is not valid percent-encoding', () => {
    expect(() => parseHash('#/invoices/%')).not.toThrow();
    expect(parseHash('#/invoices/%').screen).toBe('dashboard');
    expect(parseHash('#/invoices/%E0').screen).toBe('dashboard');
  });

  it('does not treat a param-only hash as a screen', () => {
    expect(parseHash('#/?year=2025').screen).toBe('dashboard');
  });
});

describe('routeToHash', () => {
  it('serialises a bare screen', () => {
    expect(routeToHash({ screen: 'reports', params: new URLSearchParams() })).toBe(
      '#/reports',
    );
  });

  it('appends params when present', () => {
    expect(
      routeToHash({ screen: 'register', params: new URLSearchParams({ year: '2025' }) }),
    ).toBe('#/register?year=2025');
  });

  it('writes an invoice number into the path', () => {
    expect(
      routeToHash({ screen: 'invoices', params: new URLSearchParams('number=1254') }),
    ).toBe('#/invoices/1254');
    expect(
      routeToHash({ screen: 'invoices', params: new URLSearchParams('number=1254&edit=1') }),
    ).toBe('#/invoices/1254?edit=1');
    expect(
      routeToHash({ screen: 'invoices', params: new URLSearchParams('status=paid') }),
    ).toBe('#/invoices?status=paid');
  });

  it('keeps a number that is not one in the query, where the screen refuses it', () => {
    expect(
      routeToHash({ screen: 'invoices', params: new URLSearchParams('number=abc') }),
    ).toBe('#/invoices?number=abc');
  });

  it('round-trips an invoice route', () => {
    const hash = '#/invoices/1254?edit=1';
    expect(routeToHash(parseHash(hash))).toBe(hash);
  });

  it('round-trips', () => {
    const hash = '#/register?year=2025';
    expect(routeToHash(parseHash(hash))).toBe(hash);
  });
});

describe('screenToHash', () => {
  it('builds the hash a nav click writes', () => {
    expect(screenToHash('undo')).toBe('#/undo');
  });
});
