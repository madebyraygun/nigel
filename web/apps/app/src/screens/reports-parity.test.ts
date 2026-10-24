import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import './reports.js';
import type { NigelReportsScreen } from './reports.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { ReportSlug } from '../api/types.js';

/**
 * The load-bearing test: the browser shows the figures the CLI prints.
 *
 * Both sides of the comparison come from one seeded database, captured by
 * `cargo test --features serve capture_web_report_fixtures -- --ignored` — the
 * JSON is what `/api/reports/<slug>` answered, and the text is what
 * `/api/exports/<slug>?format=text` produced, which is byte for byte what
 * `nigel report <slug> --mode export --format text` writes. So a mapper that
 * drops a subtotal or doubles a row fails here against the CLI's own bytes,
 * rather than at tax time.
 */
const fixtures = resolve(dirname(fileURLToPath(import.meta.url)), '../__fixtures__/reports');

interface ManifestEntry {
  report: string;
  route: string;
  exportRoute: string;
  params: { year?: number };
  json: string;
  text: string;
}

const manifest = JSON.parse(readFileSync(resolve(fixtures, 'manifest.json'), 'utf8')) as {
  company: string;
  reports: ManifestEntry[];
};

function entry(report: string): ManifestEntry {
  const found = manifest.reports.find((item) => item.report === report);
  if (!found) throw new Error(`no fixture for ${report}`);
  return found;
}

function reportJson(item: ManifestEntry): unknown {
  const parsed = JSON.parse(readFileSync(resolve(fixtures, item.json), 'utf8')) as {
    report: unknown;
  };
  return parsed.report;
}

function reportText(item: ManifestEntry): string {
  return readFileSync(resolve(fixtures, item.text), 'utf8');
}

/**
 * Reports whose text export prints magnitudes and lets a colour carry the
 * direction, while `wc-money` renders the sign. Every other report prints its
 * negatives as `-$x`, so the sign is part of what must match.
 */
const MAGNITUDE_ONLY_IN_TEXT = new Set<string>(['flagged', 'register']);

/**
 * Reports whose text export states the total in its heading, ahead of the
 * rows; the screen shows it after them.
 */
const TOTAL_IN_TEXT_HEADING = new Set<string>(['register']);

/**
 * Every money figure and percentage in a string, in the order it appears.
 *
 * Order is kept: a label-to-figure swap between two rows moves figures to
 * different positions, which a sorted comparison cannot see. A report whose
 * text prints magnitudes only has its sign dropped from both sides, because a
 * colour alone cannot be the only cue and the screen deliberately adds one.
 */
function figureTokens(source: string, signed: boolean): string[] {
  return [...source.matchAll(/-?\$[\d,]+\.\d{2}|-?\d+(?:\.\d+)?%/g)].map((match) =>
    signed ? match[0] : match[0].replace('-', ''),
  );
}

/**
 * Money and percentages rendered on the screen, skipping the cash-flow chart.
 *
 * The chart's accessible table restates the same months the table below it
 * already lists, so counting both would double every cash-flow figure. It is a
 * second view, not a second set of numbers.
 */
function screenFigures(el: NigelReportsScreen, signed: boolean): string[] {
  const collect = (node: ParentNode | null): string => {
    if (!node) return '';
    let out = '';
    for (const child of node.childNodes) {
      const element = child as Element & { shadowRoot?: ShadowRoot | null };
      const tag = element.tagName?.toLowerCase();
      if (tag === 'wc-bar-chart' || tag === 'style') continue;
      if (child.nodeType === Node.TEXT_NODE) out += ` ${child.textContent ?? ''}`;
      if (element.shadowRoot) out += collect(element.shadowRoot);
      if (element.childNodes?.length) out += collect(element as unknown as ParentNode);
    }
    return out;
  };
  return figureTokens(collect(el.shadowRoot), signed);
}

async function mountReport(slug: ReportSlug, item: ManifestEntry): Promise<NigelReportsScreen> {
  const client = new FakeApiClient();
  const report = reportJson(item);

  // Prime only the endpoint under test; every report has its own fixture.
  const fixtureFor: Record<string, () => void> = {
    pnl: () => (client.pnl = report as never),
    expenses: () => (client.expenses = report as never),
    tax: () => (client.tax = report as never),
    cashflow: () => (client.cashflow = report as never),
    balance: () => (client.balance = report as never),
    flagged: () => (client.flagged = report as never),
    register: () => (client.register = report as never),
    k1: () => (client.k1 = report as never),
  };
  fixtureFor[slug]?.();

  const store = initializeAppStore(client, { reload: () => {} });
  await store.refreshStatus();

  const el = document.createElement('nigel-reports-screen');
  el.client = client;
  const params = new URLSearchParams({ report: slug });
  if (item.params.year) params.set('year', String(item.params.year));
  el.params = params;
  document.body.appendChild(el);
  await el.updateComplete;
  await new Promise((r) => setTimeout(r, 0));
  await el.updateComplete;
  return el;
}

const SLUGS: ReportSlug[] = [
  'pnl',
  'expenses',
  'tax',
  'cashflow',
  'balance',
  'flagged',
  'register',
  'k1',
];

describe('figure parity with the CLI', () => {
  beforeEach(() => {
    resetAppStore();
  });

  afterEach(() => {
    document.body.innerHTML = '';
    resetAppStore();
  });

  it.each(SLUGS)('shows every figure the %s text export prints', async (slug) => {
    const item = entry(slug);
    const el = await mountReport(slug, item);

    const signed = !MAGNITUDE_ONLY_IN_TEXT.has(slug);
    const fromText = figureTokens(reportText(item), signed);
    if (TOTAL_IN_TEXT_HEADING.has(slug)) fromText.push(...fromText.splice(0, 1));
    const fromScreen = screenFigures(el, signed);

    expect(fromText.length, `${slug}: the fixture has no figures to compare`).toBeGreaterThan(
      0,
    );
    expect(fromScreen, `${slug} figures differ from the CLI text export`).toEqual(fromText);
  });

  it('shows the K-1 worksheet figures when categories need mapping', async () => {
    // The needs-mapping variant is the one whose totals deliberately exclude
    // some of the figures on the page, so it is worth its own comparison.
    const item = entry('k1-needs-mapping');
    const el = await mountReport('k1', item);
    expect(screenFigures(el, true)).toEqual(figureTokens(reportText(item), true));
  });

  it('reads the same company out of every capture', () => {
    // Guards the guard: fixtures captured from different databases would make
    // the comparisons above meaningless.
    expect(manifest.company).toBe('Bluepeak LLC');
    expect(manifest.reports).toHaveLength(SLUGS.length + 1);
  });
});
