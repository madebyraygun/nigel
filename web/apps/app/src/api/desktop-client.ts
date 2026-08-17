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

  constructor(options: DesktopApiClientOptions) {
    super(options);
    this.invoke = options.invoke;
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
