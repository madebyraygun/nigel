import {
  FetchApiClient,
  type FetchApiClientOptions,
  type ApiClient,
  type ExportTarget,
} from './client.js';
import type { ExportFormat, ExportParams, ReportSlug } from './types.js';

/** The Tauri command bridge, injectable so tests never touch a global. */
export type InvokeFn = (cmd: string, args: Record<string, unknown>) => Promise<unknown>;

export interface DesktopApiClientOptions extends FetchApiClientOptions {
  invoke: InvokeFn;
  /**
   * `std::env::consts::OS`, for tests to inject directly.
   *
   * Left unset in production, where the client asks the native side for it,
   * once, via the `platform` command — `navigator.userAgent` is not a
   * reliable platform signal inside a webview. Left lazy rather than fetched
   * eagerly in the constructor: most `DesktopApiClient`s never call
   * `openInvoicePreview`, the one method that needs it.
   */
  platform?: string;
}

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
  // `FetchApiClient` keeps its own `fetchImpl` private, so this class holds the
  // reference it was given rather than reaching into the parent.
  private readonly fetchBytes: typeof fetch;
  private readonly injectedPlatform?: string;
  private platformPromise: Promise<string> | null = null;

  constructor(options: DesktopApiClientOptions) {
    super(options);
    this.invoke = options.invoke;
    this.fetchBytes = options.fetchImpl ?? globalThis.fetch.bind(globalThis);
    this.injectedPlatform = options.platform;
  }

  /**
   * Memoized so the native side is asked at most once per client — but the
   * memo is cleared on a rejection, so an IPC bridge that is not ready yet
   * gets a retry on the next call rather than poisoning every later call
   * with the same failure forever.
   */
  private platform(): Promise<string> {
    if (this.injectedPlatform !== undefined) return Promise.resolve(this.injectedPlatform);
    this.platformPromise ??= this.invoke('platform', {})
      .then(String)
      .catch((error: unknown) => {
        this.platformPromise = null;
        throw error;
      });
    return this.platformPromise;
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

  /**
   * Show an invoice's PDF the way the running platform can actually show it.
   *
   * WebKitGTK has no built-in PDF viewer, so a Linux desktop writes the bytes
   * to a private temp file and hands the path to the system's own viewer.
   * Navigation download only works on Windows and the blob route only covers
   * Linux and macOS, so everywhere else this runs the same save action
   * `invoicePreviewTarget` offers: fetch the bytes and hand them to
   * `save_export` through a native save dialog, matching the link's own
   * label, "Download the PDF".
   */
  async openInvoicePreview(number: number): Promise<void> {
    const platform = await this.platform();
    const url = this.invoicePreviewUrl(number, 'pdf');
    if (platform !== 'linux') {
      await this.save(url, `invoice-${number}.pdf`).run();
      return;
    }

    const response = await this.fetchBytes(url);
    if (!response.ok) {
      throw new Error(`Preview failed: ${response.status}`);
    }
    const bytes = [...new Uint8Array(await response.arrayBuffer())];
    const path = await this.invoke('write_temp_pdf', {
      name: filenameFrom(response.headers.get('content-disposition'), `invoice-${number}.pdf`),
      bytes,
    });
    await this.invoke('open_external', { path });
  }

  private save(url: string, fallbackName: string): ExportTarget {
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

/**
 * The client for the environment this build is running in.
 *
 * The desktop shell exposes `window.__TAURI__`; a browser does not. Screens
 * never ask which one they got.
 */
export function createApiClient(): ApiClient {
  const tauri = (globalThis as { __TAURI__?: { core?: { invoke?: InvokeFn } } }).__TAURI__;
  const invoke = tauri?.core?.invoke;
  return invoke ? new DesktopApiClient({ invoke }) : new FetchApiClient();
}
