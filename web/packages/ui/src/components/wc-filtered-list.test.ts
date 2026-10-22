import { describe, it, expect, afterEach } from 'vitest';
import { controlsCss } from '@nigel/theme';
import './wc-filtered-list.js';
import { WcFilteredList } from './wc-filtered-list.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import preview from './wc-filtered-list.preview.js';

async function mount(markup: string): Promise<WcFilteredList> {
  document.body.innerHTML = `<wc-filtered-list>${markup}</wc-filtered-list>`;
  const el = document.querySelector('wc-filtered-list')!;
  await el.updateComplete;
  // slotchange is async: let it land, then the re-render.
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
  return el;
}

describe('wc-filtered-list', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('spaces the filter row from the list with the shared toolbar gap', () => {
    const text = styleText(WcFilteredList).replace(/\s+/g, ' ');
    expect(text).toMatch(/:host \{[^}]*gap: var\(--nc-toolbar-gap\)/);
  });

  it('lays the filters on one wrapping, vertically centred row', () => {
    const text = styleText(WcFilteredList).replace(/\s+/g, ' ');
    const row = text.match(/\.filters \{([^}]*)\}/)?.[1] ?? '';
    expect(row).toContain('display: flex');
    expect(row).toContain('flex-wrap: wrap');
    expect(row).toContain('align-items: center');
    expect(row).toContain('gap: var(--nc-filter-row-gap)');
  });

  it('names the filter row as a group', async () => {
    const el = await mount('<a slot="filters" href="#a">All</a><table></table>');
    const row = el.shadowRoot?.querySelector('.filters');
    expect(row?.getAttribute('role')).toBe('group');
    expect(row?.getAttribute('aria-label')).toBe('Filters');
    expect(row?.hasAttribute('hidden')).toBe(false);
  });

  it('drops the row, and its gap, when nothing filters the list', async () => {
    const el = await mount('<table></table>');
    expect(el.shadowRoot?.querySelector('.filters')?.hasAttribute('hidden')).toBe(true);
  });
});

describe('the inline select label on a filter row', () => {
  const rules = controlsCss.cssText.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s+/g, ' ');

  it('turns the select into a row with its label beside it', () => {
    expect(rules).toMatch(
      /:is\(wc-filtered-list, wc-manager-layout\) > wa-select:is\(\[slot='filters'\], \[slot='toolbar'\]\) \{ flex-direction: row; align-items: center;/,
    );
  });

  it('takes the stacked label’s bottom margin away, so it centres', () => {
    expect(rules).toMatch(/::part\(form-control-label\) \{ margin: 0;/);
  });
});

describePreviewA11y(preview);
