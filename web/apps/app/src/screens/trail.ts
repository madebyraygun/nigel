import type { BreadcrumbItem } from '@nigel/ui';

import type { Route } from './hash-route.js';
import { screenDef } from './registry.js';
import { invoiceNumberOf } from './invoices.js';

/**
 * The breadcrumb after the book for a route: the screen, then any level below
 * it. An open invoice is the one level below a screen so far — `Invoices /
 * #1254`, with Invoices a way back to the list.
 */
export function trailFor(route: Route): BreadcrumbItem[] {
  const def = screenDef(route.screen);
  if (route.screen === 'invoices' && route.params.get('new') !== '1') {
    const number = invoiceNumberOf(route.params);
    if (number !== null) return [{ label: def.title, id: def.id }, { label: `#${number}` }];
  }
  return [{ label: def.title }];
}

/**
 * The window title: the trail innermost first, then the book. `Dashboard ·
 * Acme Consulting LLC`; `#1254 · Invoices · Acme Consulting LLC`.
 */
export function windowTitle(trail: BreadcrumbItem[], book: string): string {
  return [...trail.map((item) => item.label).reverse(), book].join(' · ');
}
