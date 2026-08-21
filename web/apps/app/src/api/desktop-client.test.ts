import { describe, it, expect, vi } from 'vitest';
import { FetchApiClient, MENU_COMMAND_IDS, type DragDropEvent } from './client.js';
import { DesktopApiClient, createApiClient } from './desktop-client.js';

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

describe('the menu source', () => {
  function menuClient() {
    const bus = eventBus();
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: bus.listen,
      invoke: async () => null,
    });
    return { bus, client };
  }

  it('is native, and maps navigation ids to navigate commands', async () => {
    const { bus, client } = menuClient();
    const source = client.menuSource();
    expect(source.kind).toBe('native');
    if (source.kind !== 'native') return;

    const seen: unknown[] = [];
    source.onCommand((command) => seen.push(command));
    await Promise.resolve();

    bus.emit('menu-command', 'navigate:register');
    expect(seen).toEqual([{ kind: 'navigate', screen: 'register' }]);
  });

  it('maps the command ids and drops what this build does not know', async () => {
    const { bus, client } = menuClient();
    const source = client.menuSource();
    if (source.kind !== 'native') throw new Error('expected a native menu');

    const seen: unknown[] = [];
    source.onCommand((command) => seen.push(command));
    await Promise.resolve();

    for (const id of MENU_COMMAND_IDS) {
      bus.emit('menu-command', id);
    }
    bus.emit('menu-command', 'navigate:');
    bus.emit('menu-command', 'a-menu-item-from-the-future');
    bus.emit('menu-command', { id: 'find' });

    expect(seen).toEqual(MENU_COMMAND_IDS.map((kind) => ({ kind })));
  });

  it('stops delivering once unsubscribed', async () => {
    const { bus, client } = menuClient();
    const source = client.menuSource();
    if (source.kind !== 'native') throw new Error('expected a native menu');

    const seen: unknown[] = [];
    const off = source.onCommand((command) => seen.push(command));
    await Promise.resolve();

    off();
    bus.emit('menu-command', 'find');
    expect(seen).toEqual([]);
  });
});

describe('the browser client has no menu bar', () => {
  it('answers none', () => {
    expect(new FetchApiClient().menuSource()).toEqual({ kind: 'none' });
  });
});
