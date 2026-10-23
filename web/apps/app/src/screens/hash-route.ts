import { DEFAULT_SCREEN, isScreenId, type ScreenId } from './registry.js';
import { HOSTED_PARAM } from '../shell-preferences.js';

export interface Route {
  screen: ScreenId;
  params: URLSearchParams;
}

/**
 * The screen whose route takes one path segment, and the param it lands in:
 * `#/invoices/1254` is `#/invoices?number=1254` spelled the way the breadcrumb
 * reads.
 */
const SEGMENT_PARAM: Partial<Record<ScreenId, string>> = { invoices: 'number' };

/**
 * Parse `#/<screen>[/<segment>]?<params>`.
 *
 * Params are carried for deep links like `#/register?account=BofA%20Checking`.
 * A segment after the screen is accepted only where `SEGMENT_PARAM` names one,
 * and it lands as that param — so a screen reads `?number=1254` and
 * `/1254` the same way, and an older link in the query form still opens.
 * Anything unrecognized falls back to the default screen — a bad hash should
 * show the dashboard, not a blank page.
 */
export function parseHash(hash: string): Route {
  const raw = hash.replace(/^#\/?/, '');
  const [path, query = ''] = raw.split('?');
  const [head, segment, ...rest] = path.split('/');
  const params = new URLSearchParams(query);
  // `shell=native` says which host opened the page (`isHostedShell`), not
  // what the screen should show.
  params.delete(HOSTED_PARAM);

  if (!isScreenId(head)) return { screen: DEFAULT_SCREEN, params };
  const param = SEGMENT_PARAM[head];
  if (segment !== undefined && segment !== '') {
    if (!param || rest.length > 0) return { screen: DEFAULT_SCREEN, params };
    let value: string;
    try {
      value = decodeURIComponent(segment);
    } catch {
      return { screen: DEFAULT_SCREEN, params };
    }
    params.set(param, value);
  }
  return { screen: head, params };
}

export function routeToHash(route: Route): string {
  const params = new URLSearchParams(route.params);
  const param = SEGMENT_PARAM[route.screen];
  const segment = param ? params.get(param) : null;
  let path = `#/${route.screen}`;
  if (param && segment !== null && /^\d+$/.test(segment)) {
    params.delete(param);
    path += `/${segment}`;
  }
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}

export function screenToHash(screen: ScreenId): string {
  return `#/${screen}`;
}
