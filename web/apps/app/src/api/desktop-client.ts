import {
  FetchApiClient,
  type FetchApiClientOptions,
  type ApiClient,
  type DragDropEvent,
  type ExportTarget,
  type ImportSource,
  type ShellChrome,
} from './client.js';
import type { ExportFormat, ExportParams, ReportSlug, StagedUpload } from './types.js';

/** The Tauri command bridge, injectable so tests never touch a global. */
export type InvokeFn = (cmd: string, args: Record<string, unknown>) => Promise<unknown>;

/** The Tauri event bridge, injectable for the same reason `invoke` is. */
export type ListenFn = (
  event: string,
  handler: (event: { payload: unknown }) => void,
) => Promise<() => void>;

export interface DesktopApiClientOptions extends FetchApiClientOptions {
  invoke: InvokeFn;
  listen: ListenFn;
}

/**
 * The window-level drag-and-drop events Tauri 2 emits to the page.
 *
 * They are window-level rather than element-level: Tauri intercepts drag
 * events in the webview, so the page's own HTML5 handlers never see a drop.
 */
const DRAG_EVENTS = {
  enter: 'tauri://drag-enter',
  over: 'tauri://drag-over',
  drop: 'tauri://drag-drop',
  leave: 'tauri://drag-leave',
} as const;

/**
 * The api client the desktop shell runs.
 *
 * A webview serving this app from a custom URI scheme will not download from a
 * navigation, and the blob route that covers macOS and Linux does not work on
 * Windows. So the bytes come back through `fetch` and the native side writes
 * them, which is the one route all three platforms share.
 */
export class DesktopApiClient extends FetchApiClient {
  private readonly invoke: InvokeFn;
  private readonly listen: ListenFn;
  // `FetchApiClient` keeps its own `fetchImpl` private, so this class holds the
  // reference it was given rather than reaching into the parent.
  private readonly fetchBytes: typeof fetch;

  constructor(options: DesktopApiClientOptions) {
    super(options);
    this.invoke = options.invoke;
    this.listen = options.listen;
    this.fetchBytes = options.fetchImpl ?? globalThis.fetch.bind(globalThis);
  }

  override exportTarget(
    report: ReportSlug,
    format: ExportFormat,
    params: ExportParams = {},
  ): ExportTarget {
    return this.save(
      this.exportUrl(report, format, params),
      `${report}.${format === 'pdf' ? 'pdf' : 'txt'}`,
    );
  }

  override invoicePreviewTarget(number: number): ExportTarget {
    return this.save(this.invoicePreviewUrl(number, 'pdf'), `invoice-${number}.pdf`);
  }

  override shellChrome(): ShellChrome | null {
    return {
      // Fire-and-forget like the drag subscription: a refused invoke leaves
      // the fallback timer to show the window, which works.
      ready: () => void Promise.resolve(this.invoke('frontend_ready', {})).catch(() => {}),
      background: (mode) =>
        void Promise.resolve(this.invoke('set_chrome_background', { mode })).catch(() => {}),
    };
  }

  override importSource(): ImportSource {
    return {
      kind: 'native',
      pick: async () =>
        ((await this.invoke('pick_import_file', {})) as StagedUpload | null) ?? null,
      stagePath: async (path: string) =>
        (await this.invoke('stage_import', { path })) as StagedUpload,
      onDragDrop: (handler) => this.subscribeDragDrop(handler),
    };
  }

  private subscribeDragDrop(handler: (event: DragDropEvent) => void): () => void {
    const off: Array<() => void> = [];
    let cancelled = false;

    const subscribe = (name: string, toEvent: (payload: unknown) => DragDropEvent) => {
      void this.listen(name, (event) => {
        if (!cancelled) handler(toEvent(event.payload));
      })
        .then((unlisten) => {
          if (cancelled) unlisten();
          else off.push(unlisten);
        })
        // An ACL-denied `listen` leaves the picker as the only way in, which
        // works; a rejection thrown from here would only be unhandled.
        .catch(() => {});
    };

    subscribe(DRAG_EVENTS.enter, () => ({ type: 'over' }));
    subscribe(DRAG_EVENTS.over, () => ({ type: 'over' }));
    subscribe(DRAG_EVENTS.drop, (payload) => ({ type: 'drop', paths: pathsOf(payload) }));
    subscribe(DRAG_EVENTS.leave, () => ({ type: 'leave' }));

    return () => {
      cancelled = true;
      for (const unlisten of off.splice(0)) unlisten();
    };
  }

  private save(url: string, fallbackName: string): Extract<ExportTarget, { kind: 'action' }> {
    return {
      kind: 'action',
      run: async () => {
        const response = await this.fetchBytes(url);
        if (!response.ok) {
          throw new Error(`Export failed: ${response.status}`);
        }
        const bytes = [...new Uint8Array(await response.arrayBuffer())];
        await this.invoke('save_export', {
          name: filenameFrom(response.headers.get('content-disposition'), fallbackName),
          bytes,
        });
      },
    };
  }
}

/** The name the server chose, or the caller's fallback. */
function filenameFrom(disposition: string | null, fallback: string): string {
  const match = /filename\*?=(?:UTF-8'')?"?([^";]+)"?/i.exec(disposition ?? '');
  return match ? decodeURIComponent(match[1]) : fallback;
}

/** `tauri://drag-drop` carries `{paths, position}`; a drag of nothing carries no paths. */
function pathsOf(payload: unknown): string[] {
  const paths = (payload as { paths?: unknown } | null)?.paths;
  if (!Array.isArray(paths)) return [];
  return paths.filter((path): path is string => typeof path === 'string');
}

/**
 * The client for the environment this build is running in.
 *
 * The desktop shell exposes `window.__TAURI__`; a browser does not. Screens
 * never ask which one they got.
 */
export function createApiClient(): ApiClient {
  const tauri = (
    globalThis as {
      __TAURI__?: { core?: { invoke?: InvokeFn }; event?: { listen?: ListenFn } };
    }
  ).__TAURI__;
  const invoke = tauri?.core?.invoke;
  const listen = tauri?.event?.listen;
  // Both or neither: `withGlobalTauri` publishes the whole api namespace, so a
  // shell offering one and not the other is not a shell this app runs in.
  return invoke && listen ? new DesktopApiClient({ invoke, listen }) : new FetchApiClient();
}
