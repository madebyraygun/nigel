import { describe, it, expect, vi } from 'vitest';
import { DesktopApiClient } from './desktop-client.js';

describe('DesktopApiClient', () => {
  it('answers an action target that fetches the bytes and hands them to the native side', async () => {
    const saved: Array<{ name: string; bytes: number[] }> = [];
    const fetchImpl = vi.fn(async () =>
      new Response('date,amount\n2026-01-05,10.00\n', {
        status: 200,
        headers: {
          'content-type': 'text/csv',
          'content-disposition': 'attachment; filename="pnl.csv"',
        },
      }),
    );
    const client = new DesktopApiClient({
      fetchImpl,
      invoke: async (_cmd, args) => {
        saved.push(args as { name: string; bytes: number[] });
        return null;
      },
    });

    const target = client.exportTarget('pnl', 'text', { year: 2026 });
    expect(target.kind).toBe('action');

    await (target as { run: () => Promise<void> }).run();

    expect(saved).toHaveLength(1);
    expect(saved[0].name).toBe('pnl.csv');
    expect(saved[0].bytes.length).toBeGreaterThan(0);
  });

  it('names the file from Content-Disposition rather than guessing', async () => {
    const saved: Array<{ name: string }> = [];
    const fetchImpl = vi.fn(async () =>
      new Response('%PDF-1.4', {
        status: 200,
        headers: {
          'content-type': 'application/pdf',
          'content-disposition': 'attachment; filename="invoice-1251.pdf"',
        },
      }),
    );
    const client = new DesktopApiClient({
      fetchImpl,
      invoke: async (_cmd, args) => {
        saved.push(args as { name: string });
        return null;
      },
    });

    await (client.invoicePreviewTarget(1251) as { run: () => Promise<void> }).run();

    expect(saved[0].name).toBe('invoice-1251.pdf');
  });

  it('raises a failed export rather than swallowing it', async () => {
    const fetchImpl = vi.fn(async () => new Response('nope', { status: 500 }));
    const client = new DesktopApiClient({ fetchImpl, invoke: async () => null });

    await expect(
      (client.exportTarget('pnl', 'pdf') as { run: () => Promise<void> }).run(),
    ).rejects.toThrow();
  });
});
