import { describe, it, expect, vi } from 'vitest';
import { FetchApiClient, type DragDropEvent } from './client.js';
import { DesktopApiClient, createApiClient, fileManagerLabel } from './desktop-client.js';

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
      listen: eventBus().listen,
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
      listen: eventBus().listen,
      invoke: async (_cmd, args) => {
        saved.push(args as { name: string });
        return null;
      },
    });

    await (client.invoicePreviewTarget(1251) as { run: () => Promise<void> }).run();

    expect(saved[0].name).toBe('invoice-1251.pdf');
  });

  it('documentPreviewTarget saves through the native side', async () => {
    const saved: Array<{ name: string; bytes: number[] }> = [];
    const fetchImpl = vi.fn(async (_input: RequestInfo | URL) =>
      new Response('%PDF-1.7', {
        status: 200,
        headers: { 'content-type': 'application/pdf' },
      }),
    );
    const client = new DesktopApiClient({
      fetchImpl,
      listen: eventBus().listen,
      invoke: async (cmd, args) => {
        expect(cmd).toBe('save_export');
        saved.push(args as { name: string; bytes: number[] });
        return null;
      },
    });

    const target = client.documentPreviewTarget(7);
    expect(target.kind).toBe('action');
    await (target as { run: () => Promise<void> }).run();

    expect(fetchImpl.mock.calls[0][0]).toBe('/api/documents/7/preview.pdf');
    expect(saved).toHaveLength(1);
    expect(saved[0].name).toBe('document-7.pdf');
    expect(saved[0].bytes.length).toBeGreaterThan(0);
  });

  it('documentSigningRecordTarget saves the record under the server’s name', async () => {
    const saved: Array<{ name: string; bytes: number[] }> = [];
    const fetchImpl = vi.fn(
      async (_input: RequestInfo | URL) =>
        new Response('%PDF-1.7', {
          status: 200,
          headers: {
            'content-type': 'application/pdf',
            'content-disposition': 'inline; filename="Website-rebuild-v1-signing-record.pdf"',
          },
        }),
    );
    const client = new DesktopApiClient({
      fetchImpl,
      listen: eventBus().listen,
      invoke: async (cmd, args) => {
        expect(cmd).toBe('save_export');
        saved.push(args as { name: string; bytes: number[] });
        return null;
      },
    });

    const target = client.documentSigningRecordTarget(7);
    expect(target.kind).toBe('action');
    await (target as { run: () => Promise<void> }).run();

    expect(fetchImpl.mock.calls[0][0]).toBe('/api/documents/7/signing-record.pdf');
    expect(saved.map((s) => s.name)).toEqual(['Website-rebuild-v1-signing-record.pdf']);
  });

  it('raises a failed export rather than swallowing it', async () => {
    const fetchImpl = vi.fn(async () => new Response('nope', { status: 500 }));
    const client = new DesktopApiClient({
      fetchImpl,
      listen: eventBus().listen,
      invoke: async () => null,
    });

    await expect(
      (client.exportTarget('pnl', 'pdf') as { run: () => Promise<void> }).run(),
    ).rejects.toThrow();
  });
});

/** A fake `__TAURI__.event.listen`, with the handlers reachable per event. */
function eventBus() {
  const handlers = new Map<string, Array<(event: { payload: unknown }) => void>>();
  const unlistened: string[] = [];

  const listen = async (
    name: string,
    handler: (event: { payload: unknown }) => void,
  ) => {
    const forName = handlers.get(name) ?? [];
    forName.push(handler);
    handlers.set(name, forName);
    return () => unlistened.push(name);
  };

  const emit = (name: string, payload: unknown) => {
    for (const handler of handlers.get(name) ?? []) handler({ payload });
  };

  return { listen, emit, unlistened, names: () => [...handlers.keys()] };
}

describe('DesktopApiClient importSource', () => {
  it('picks through the native dialog and answers the staged upload', async () => {
    const invoked: Array<[string, Record<string, unknown>]> = [];
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: eventBus().listen,
      invoke: async (cmd, args) => {
        invoked.push([cmd, args]);
        return {
          uploadId: 'a1b2',
          filename: 'cedar-april-2025.csv',
          size: 8214,
          path: '/home/books/cedar-april-2025.csv',
        };
      },
    });

    const source = client.importSource();
    if (source.kind !== 'native') throw new Error('expected a native source');
    const staged = await source.pick();

    expect(invoked[0][0]).toBe('pick_import_file');
    expect(staged).toEqual({
      uploadId: 'a1b2',
      filename: 'cedar-april-2025.csv',
      size: 8214,
      path: '/home/books/cedar-april-2025.csv',
    });
  });

  it('reports a cancelled dialog as null rather than as a failure', async () => {
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: eventBus().listen,
      invoke: async () => null,
    });

    const source = client.importSource();
    if (source.kind !== 'native') throw new Error('expected a native source');
    await expect(source.pick()).resolves.toBeNull();
  });

  it('stages a dropped path by its path', async () => {
    const invoked: Array<[string, Record<string, unknown>]> = [];
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: eventBus().listen,
      invoke: async (cmd, args) => {
        invoked.push([cmd, args]);
        return {
          uploadId: 'c3d4',
          filename: 'juniper-may-2025.xlsx',
          size: 41000,
          path: '/home/books/juniper-may-2025.xlsx',
        };
      },
    });

    const source = client.importSource();
    if (source.kind !== 'native') throw new Error('expected a native source');
    const staged = await source.stagePath('/home/books/juniper-may-2025.xlsx');

    expect(invoked[0]).toEqual([
      'stage_import',
      { path: '/home/books/juniper-may-2025.xlsx' },
    ]);
    expect(staged.uploadId).toBe('c3d4');
  });

  it('reduces the four Tauri drag events to over, leave and drop', async () => {
    const bus = eventBus();
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: bus.listen,
      invoke: async () => null,
    });

    const source = client.importSource();
    if (source.kind !== 'native') throw new Error('expected a native source');
    const seen: DragDropEvent[] = [];
    const off = source.onDragDrop((event) => seen.push(event));
    await Promise.resolve();

    expect(bus.names()).toEqual([
      'tauri://drag-enter',
      'tauri://drag-over',
      'tauri://drag-drop',
      'tauri://drag-leave',
    ]);

    bus.emit('tauri://drag-enter', {
      paths: ['/home/books/cedar-april-2025.csv'],
      position: { x: 10, y: 20 },
    });
    bus.emit('tauri://drag-over', { position: { x: 12, y: 24 } });
    bus.emit('tauri://drag-drop', {
      paths: ['/home/books/cedar-april-2025.csv'],
      position: { x: 12, y: 24 },
    });
    bus.emit('tauri://drag-leave', null);

    expect(seen).toEqual([
      { type: 'over' },
      { type: 'over' },
      { type: 'drop', paths: ['/home/books/cedar-april-2025.csv'] },
      { type: 'leave' },
    ]);

    off();
    expect(bus.unlistened).toHaveLength(4);
  });

  it('reports a drop carrying no paths as an empty drop rather than throwing', async () => {
    const bus = eventBus();
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: bus.listen,
      invoke: async () => null,
    });

    const source = client.importSource();
    if (source.kind !== 'native') throw new Error('expected a native source');
    const seen: DragDropEvent[] = [];
    source.onDragDrop((event) => seen.push(event));
    await Promise.resolve();

    bus.emit('tauri://drag-drop', { position: { x: 1, y: 1 } });

    expect(seen).toEqual([{ type: 'drop', paths: [] }]);
  });
});

describe('DesktopApiClient dataDirReveal', () => {
  it('asks the shell to open the books folder, naming no path', async () => {
    const invoked: Array<[string, Record<string, unknown>]> = [];
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: eventBus().listen,
      invoke: async (cmd, args) => {
        invoked.push([cmd, args]);
        return null;
      },
    });

    await client.dataDirReveal().run();
    expect(invoked).toEqual([['reveal_data_dir', {}]]);
  });

  it('passes on what the shell refused', async () => {
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: eventBus().listen,
      invoke: async () => {
        throw "Couldn't open /tmp/acme";
      },
    });
    await expect(client.dataDirReveal().run()).rejects.toBe("Couldn't open /tmp/acme");
  });

  it("calls it by each platform's name", () => {
    expect(fileManagerLabel('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)')).toBe(
      'Reveal in Finder',
    );
    expect(fileManagerLabel('Mozilla/5.0 (Windows NT 10.0; Win64; x64)')).toBe(
      'Show in Explorer',
    );
    expect(fileManagerLabel('Mozilla/5.0 (X11; Linux x86_64)')).toBe('Open folder');
  });
});

describe('FetchApiClient dataDirReveal', () => {
  it('offers none: a browser tab cannot open a folder', () => {
    expect(new FetchApiClient().dataDirReveal()).toBeNull();
  });
});

describe('DesktopApiClient shellChrome', () => {
  it('sends the palette and its source to the shell', async () => {
    const invoked: Array<[string, Record<string, unknown>]> = [];
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: eventBus().listen,
      invoke: async (cmd, args) => {
        invoked.push([cmd, args]);
        return null;
      },
    });

    const chrome = client.shellChrome();
    if (!chrome) throw new Error('expected native chrome');
    chrome.background('dark', 'explicit');
    chrome.background('light', 'system');
    chrome.ready();
    await Promise.resolve();

    expect(invoked).toEqual([
      ['set_chrome_background', { mode: 'dark', source: 'explicit' }],
      ['set_chrome_background', { mode: 'light', source: 'system' }],
      ['frontend_ready', {}],
    ]);
  });
});

describe('createApiClient', () => {
  it('answers a browser client when there is no Tauri global', () => {
    expect(createApiClient()).toBeInstanceOf(FetchApiClient);
    expect(createApiClient().importSource()).toEqual({ kind: 'browser' });
  });

  it('answers a native client when the shell exposes invoke and listen', () => {
    const globals = globalThis as Record<string, unknown>;
    globals.__TAURI__ = {
      core: { invoke: async () => null },
      event: { listen: async () => () => {} },
    };
    try {
      expect(createApiClient().importSource().kind).toBe('native');
    } finally {
      delete globals.__TAURI__;
    }
  });
});
