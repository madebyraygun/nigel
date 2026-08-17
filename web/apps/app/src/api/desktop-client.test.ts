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

  it('opens an inline PDF externally on linux and frames it elsewhere', async () => {
    const calls: string[] = [];
    const linux = new DesktopApiClient({
      fetchImpl: async () => new Response('%PDF-1.4', { status: 200 }),
      invoke: async (cmd) => {
        calls.push(cmd);
        return cmd === 'platform' ? 'linux' : null;
      },
      platform: 'linux',
    });

    await linux.openInvoicePreview(1251);

    expect(calls).toContain('open_external');
  });

  it('saves a non-linux preview through the native dialog rather than navigating', async () => {
    const assign = vi.fn();
    vi.spyOn(globalThis, 'location', 'get').mockReturnValue({ assign } as unknown as Location);
    const saved: Array<{ name: string; bytes: number[] }> = [];
    const mac = new DesktopApiClient({
      fetchImpl: async () =>
        new Response('%PDF-1.4', {
          status: 200,
          headers: { 'content-disposition': 'attachment; filename="invoice-1251.pdf"' },
        }),
      invoke: async (cmd, args) => {
        if (cmd === 'save_export') saved.push(args as { name: string; bytes: number[] });
        return null;
      },
      platform: 'darwin',
    });

    await mac.openInvoicePreview(1251);

    expect(assign).not.toHaveBeenCalled();
    expect(saved).toHaveLength(1);
    expect(saved[0].name).toBe('invoice-1251.pdf');
    expect(saved[0].bytes.length).toBeGreaterThan(0);
  });

  it('raises a failed non-linux preview save rather than swallowing it', async () => {
    const mac = new DesktopApiClient({
      fetchImpl: async () => new Response('nope', { status: 500 }),
      invoke: async () => null,
      platform: 'darwin',
    });

    await expect(mac.openInvoicePreview(1251)).rejects.toThrow();
  });

  it('asks the native side for the platform at most once, when none is injected', async () => {
    const calls: string[] = [];
    const noPlatform = new DesktopApiClient({
      fetchImpl: async () => new Response('%PDF-1.4', { status: 200 }),
      invoke: async (cmd) => {
        calls.push(cmd);
        return cmd === 'platform' ? 'linux' : null;
      },
    });

    await noPlatform.openInvoicePreview(1251);
    await noPlatform.openInvoicePreview(1251);

    expect(calls.filter((cmd) => cmd === 'platform')).toHaveLength(1);
  });

  it('retries the platform lookup after a rejection instead of poisoning the client', async () => {
    let platformCalls = 0;
    const client = new DesktopApiClient({
      fetchImpl: async () => new Response('%PDF-1.4', { status: 200 }),
      invoke: async (cmd) => {
        if (cmd !== 'platform') return null;
        platformCalls += 1;
        if (platformCalls === 1) throw new Error('ipc not ready yet');
        return 'linux';
      },
    });

    await expect(client.openInvoicePreview(1251)).rejects.toThrow('ipc not ready yet');
    await expect(client.openInvoicePreview(1251)).resolves.toBeUndefined();

    expect(platformCalls).toBe(2);
  });
});
