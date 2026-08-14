import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-register-table.js';
import type {
  CategoryOption,
  NcEditCommitDetail,
  NcFlagToggleDetail,
  NcRowEventDetail,
  RegisterTableRow,
  WcRegisterTable,
} from './wc-register-table.js';
import {
  REGISTER_SHORTCUTS,
  WcRegisterTable as RegisterTableCtor,
} from './wc-register-table.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import preview from './wc-register-table.preview.js';

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
  { id: 21, name: 'Bank fees', categoryType: 'expense' },
];

function fixture(count = 4): RegisterTableRow[] {
  return Array.from({ length: count }, (_, index) => ({
    id: 100 + index,
    date: `2025-03-${String(index + 1).padStart(2, '0')}`,
    description: `TRANSACTION ${index}`,
    amount: index % 2 === 0 ? -10 * (index + 1) : 100 * (index + 1),
    category: index === 0 ? null : 'Bank fees',
    categoryId: index === 0 ? null : 21,
    vendor: index === 0 ? null : 'Someone',
    accountName: 'BofA Checking',
    isFlagged: index === 2,
  }));
}

async function mount(props: Partial<WcRegisterTable> = {}): Promise<WcRegisterTable> {
  const el = document.createElement('wc-register-table');
  Object.assign(el, { rows: fixture(), categories, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function rowEls(el: WcRegisterTable): HTMLElement[] {
  return [...(el.shadowRoot?.querySelectorAll<HTMLElement>('tbody tr') ?? [])];
}

function selectedId(el: WcRegisterTable): number | null {
  const row = rowEls(el).find((tr) => tr.getAttribute('aria-selected') === 'true');
  return row ? Number(row.dataset.id) : null;
}

/** `composed` because a real key event crosses the shadow boundary; the
    table listens on its host so a key from any part of it is heard. */
async function press(el: WcRegisterTable, key: string): Promise<void> {
  el.shadowRoot
    ?.querySelector('.scroller')
    ?.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, composed: true }));
  await el.updateComplete;
}

function listen<T>(el: WcRegisterTable, type: string): T[] {
  const seen: T[] = [];
  el.addEventListener(type, (event) => seen.push((event as CustomEvent<T>).detail));
  return seen;
}

describe('wc-register-table', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one row per transaction, in the order given', async () => {
    const el = await mount();
    expect(rowEls(el).map((tr) => Number(tr.dataset.id))).toEqual([100, 101, 102, 103]);
  });

  it('shows an em dash for an uncategorized row and nothing for a missing vendor', async () => {
    const el = await mount();
    const cells = [...(rowEls(el)[0]?.querySelectorAll('td') ?? [])];
    expect(cells[3]?.textContent?.trim()).toBe('—');
    expect(cells[4]?.textContent?.trim()).toBe('');
  });

  it('marks a flagged row for more than color alone', async () => {
    const el = await mount();
    const flagged = rowEls(el)[2];
    expect(flagged?.dataset.flagged).toBe('true');
    expect(
      flagged?.querySelector('button')?.getAttribute('aria-pressed'),
    ).toBe('true');
  });

  it('drops the account column on request and keeps every row the same width', async () => {
    const el = await mount({ showAccount: false, total: 12 });
    const headers = el.shadowRoot?.querySelectorAll('thead th').length;
    const cells = rowEls(el)[0]?.querySelectorAll('td').length;
    expect(headers).toBe(6);
    expect(cells).toBe(6);
  });

  it('renders no wa-* component while nothing is being edited', async () => {
    const el = await mount();
    const tags = [...(el.shadowRoot?.querySelectorAll('*') ?? [])].map((node) =>
      node.tagName.toLowerCase(),
    );
    expect(tags.filter((tag) => tag.startsWith('wa-'))).toEqual([]);
  });

  it('renders the empty state instead of a table', async () => {
    const el = await mount({ rows: [] });
    expect(el.shadowRoot?.querySelector('table')).toBeNull();
    expect(el.shadowRoot?.querySelector('wc-empty-state')).not.toBeNull();
  });

  // -- selection and keyboard ------------------------------------------------

  it('keeps exactly one row in the tab order', async () => {
    const el = await mount({ selectedId: 101 });
    expect(rowEls(el).filter((tr) => tr.tabIndex === 0).map((tr) => tr.dataset.id)).toEqual(
      ['101'],
    );
  });

  it('moves selection with the arrow keys and stops at the ends', async () => {
    const el = await mount({ selectedId: 100 });

    await press(el, 'ArrowDown');
    expect(selectedId(el)).toBe(101);

    await press(el, 'ArrowUp');
    await press(el, 'ArrowUp');
    expect(selectedId(el)).toBe(100);

    await press(el, 'End');
    expect(selectedId(el)).toBe(103);

    await press(el, 'ArrowDown');
    expect(selectedId(el)).toBe(103);

    await press(el, 'Home');
    expect(selectedId(el)).toBe(100);
  });

  it('pages by a screenful, falling back to the TUI page size without layout', async () => {
    const rows = fixture(50);
    const el = await mount({ rows, selectedId: rows[0]?.id });

    await press(el, 'PageDown');
    expect(selectedId(el)).toBe(rows[20]?.id);

    await press(el, 'PageUp');
    expect(selectedId(el)).toBe(rows[0]?.id);
  });

  it('clamps a page jump to the last row', async () => {
    const el = await mount({ selectedId: 100 });
    await press(el, 'PageDown');
    expect(selectedId(el)).toBe(103);
  });

  it('announces selection changes to the host', async () => {
    const el = await mount({ selectedId: 100 });
    const seen = listen<NcRowEventDetail>(el, 'nc-row-select');
    await press(el, 'ArrowDown');
    expect(seen).toEqual([{ id: 101 }]);
  });

  it('selects a row when it takes focus', async () => {
    const el = await mount({ selectedId: 100 });
    const seen = listen<NcRowEventDetail>(el, 'nc-row-select');
    rowEls(el)[2]?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await el.updateComplete;
    expect(seen).toEqual([{ id: 102 }]);
  });

  it('asks the host to edit on Enter and on double click', async () => {
    const el = await mount({ selectedId: 101 });
    const seen = listen<NcRowEventDetail>(el, 'nc-row-activate');

    await press(el, 'Enter');
    rowEls(el)[3]?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    await el.updateComplete;

    expect(seen).toEqual([{ id: 101 }, { id: 103 }]);
  });

  it('asks the toolbar for focus on slash', async () => {
    const el = await mount();
    let asked = 0;
    el.addEventListener('nc-search-focus', () => (asked += 1));
    await press(el, '/');
    expect(asked).toBe(1);
  });

  it('leaves the keyboard alone while a row is being edited', async () => {
    const el = await mount({ selectedId: 100, editingId: 100 });
    await press(el, 'ArrowDown');
    expect(selectedId(el)).toBe(100);
  });

  // -- flagging --------------------------------------------------------------

  it('sends the desired flag state rather than a toggle', async () => {
    const el = await mount({ selectedId: 100 });
    const seen = listen<NcFlagToggleDetail>(el, 'nc-flag-toggle');

    await press(el, 'f');
    rowEls(el)[2]?.querySelector('button')?.click();

    expect(seen).toEqual([
      { id: 100, flag: true },
      { id: 102, flag: false },
    ]);
  });

  it('disables the flag button on a row with a write in flight', async () => {
    const el = await mount({ busyId: 100 });
    const button = rowEls(el)[0]?.querySelector('button');
    expect(button?.hasAttribute('disabled')).toBe(true);
    expect(rowEls(el)[0]?.getAttribute('aria-busy')).toBe('true');
  });

  // -- inline editing --------------------------------------------------------

  it('swaps only the category and vendor cells into editors', async () => {
    const el = await mount({ editingId: 101 });
    const editing = rowEls(el)[1];
    expect(editing?.querySelector('input[role="combobox"]')).not.toBeNull();
    expect(editing?.querySelector('wa-input')).not.toBeNull();
    expect(rowEls(el)[0]?.querySelector('input')).toBeNull();
  });

  it('seeds the editors from the row', async () => {
    const el = await mount({ editingId: 101 });
    const input = el.shadowRoot?.querySelector<HTMLInputElement>('.category-input');
    const vendor = el.shadowRoot?.querySelector('wa-input');
    expect(input?.value).toBe('Bank fees');
    expect(vendor?.getAttribute('value')).toBe('Someone');
  });

  it('filters categories case-insensitively over the labelled name', async () => {
    const el = await mount({ editingId: 101 });
    const input = el.shadowRoot?.querySelector<HTMLInputElement>('.category-input');
    if (!input) throw new Error('no category input');

    input.value = 'consult';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await el.updateComplete;

    const options = [...(el.shadowRoot?.querySelectorAll('[role="option"]') ?? [])];
    expect(options.map((o) => o.textContent?.trim())).toEqual([
      'Consulting income (inc)',
    ]);
  });

  it('commits the chosen category and typed vendor', async () => {
    const el = await mount({ editingId: 101 });
    const seen = listen<NcEditCommitDetail>(el, 'nc-edit-commit');
    const input = el.shadowRoot?.querySelector<HTMLInputElement>('.category-input');
    if (!input) throw new Error('no category input');

    input.value = 'consult';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await el.updateComplete;
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await el.updateComplete;

    const vendor = el.shadowRoot?.querySelector<HTMLElement & { value: string }>(
      '.vendor-input',
    );
    if (!vendor) throw new Error('no vendor input');
    vendor.value = 'Northwind';
    vendor.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await el.updateComplete;

    expect(seen).toEqual([{ id: 101, categoryId: 3, vendor: 'Northwind' }]);
  });

  it('reports an emptied vendor as a clear, not as an untouched field', async () => {
    const el = await mount({ editingId: 101 });
    const seen = listen<NcEditCommitDetail>(el, 'nc-edit-commit');
    const vendor = el.shadowRoot?.querySelector<HTMLElement & { value: string }>(
      '.vendor-input',
    );
    if (!vendor) throw new Error('no vendor input');

    vendor.value = '   ';
    vendor.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await el.updateComplete;

    expect(seen).toEqual([{ id: 101, categoryId: 21, vendor: null }]);
  });

  it('cancels from either editor without committing', async () => {
    const el = await mount({ editingId: 101 });
    const commits = listen<NcEditCommitDetail>(el, 'nc-edit-commit');
    const cancels = listen<NcRowEventDetail>(el, 'nc-edit-cancel');

    el.shadowRoot
      ?.querySelector('.category-input')
      ?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await el.updateComplete;

    expect(cancels).toEqual([{ id: 101 }]);
    expect(commits).toEqual([]);
  });

  it('offers Save and Cancel for a pointer', async () => {
    const el = await mount({ editingId: 101 });
    const commits = listen<NcEditCommitDetail>(el, 'nc-edit-commit');
    const cancels = listen<NcRowEventDetail>(el, 'nc-edit-cancel');
    const buttons = [
      ...(rowEls(el)[1]?.querySelectorAll<HTMLButtonElement>('.edit-actions button') ??
        []),
    ];

    buttons[0]?.click();
    buttons[1]?.click();
    await el.updateComplete;

    expect(commits.length).toBe(1);
    expect(cancels.length).toBe(1);
  });

  // -- scrolling -------------------------------------------------------------

  it('scrolls a row into view by index and selects it', async () => {
    const el = await mount();
    const scrollIntoView = vi.fn();
    rowEls(el).forEach((row) => {
      row.scrollIntoView = scrollIntoView;
    });

    expect(el.scrollToIndex(2)).toBe(true);
    await el.updateComplete;
    await Promise.resolve();

    expect(selectedId(el)).toBe(102);
    expect(scrollIntoView).toHaveBeenCalledWith({ block: 'center' });
  });

  it('scrolls by transaction id, and reports an id it does not have', async () => {
    const el = await mount();
    expect(el.scrollToRow(103)).toBe(true);
    expect(el.scrollToRow(9999)).toBe(false);
    expect(el.scrollToIndex(99)).toBe(false);
  });

  it('moves selection off a row that filtering removed', async () => {
    const el = await mount({ selectedId: 103 });
    el.rows = fixture(2);
    await el.updateComplete;
    expect(selectedId(el)).toBe(100);
  });

  describe('readonly', () => {
    it('offers no flag button', async () => {
      const el = await mount({ readonly: true });
      expect(el.shadowRoot?.querySelector('.flag button')).toBeNull();
    });

    it('still shows which rows are flagged, with a label rather than colour', async () => {
      const el = await mount({ readonly: true });
      const marks = el.shadowRoot?.querySelectorAll('.flag wc-icon-flag') ?? [];
      // Exactly the one flagged row the fixture carries.
      expect(marks).toHaveLength(1);
      expect(marks[0]?.getAttribute('aria-label')).toBe('Flagged');
    });

    it('emits no flag toggle from the f key', async () => {
      const el = await mount({ readonly: true, selectedId: 102 });
      const seen = listen<NcFlagToggleDetail>(el, 'nc-flag-toggle');
      await press(el, 'f');
      expect(seen).toEqual([]);
    });

    it('emits no row activation from Enter', async () => {
      const el = await mount({ readonly: true, selectedId: 102 });
      const seen = listen<NcRowEventDetail>(el, 'nc-row-activate');
      await press(el, 'Enter');
      expect(seen).toEqual([]);
    });

    it('emits no row activation from a double click', async () => {
      const el = await mount({ readonly: true });
      const seen = listen<NcRowEventDetail>(el, 'nc-row-activate');
      rowEls(el)[1]?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
      await el.updateComplete;
      expect(seen).toEqual([]);
    });

    it('keeps arrow-key selection, because reading still wants a cursor', async () => {
      const el = await mount({ readonly: true, selectedId: 100 });
      await press(el, 'ArrowDown');
      expect(selectedId(el)).toBe(101);
    });
  });
});

/**
 * Every key the legend prints, proved against the table.
 *
 * The register's shortcuts were all dead: the keydown listener sat on the
 * scroller, and the roving tabindex had no home until something selected a
 * row, so on a dated register no element inside the table was in the tab order
 * at all and none of these keys could ever reach a handler. The list below is
 * driven off `REGISTER_SHORTCUTS`, so a line added to the legend without a
 * proof fails the suite rather than shipping as prose.
 */
describe('the documented shortcuts', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  /** Keydown from the focused row, which is where a real keystroke starts. */
  async function pressOnRow(el: WcRegisterTable, key: string): Promise<void> {
    const row = rowEls(el).find((tr) => tr.tabIndex === 0) ?? rowEls(el)[0];
    row?.dispatchEvent(
      new KeyboardEvent('keydown', { key, bubbles: true, composed: true }),
    );
    await el.updateComplete;
  }

  const proofs: Record<string, () => Promise<void>> = {
    ArrowDown: async () => {
      const el = await mount({ selectedId: 100 });
      await pressOnRow(el, 'ArrowDown');
      expect(selectedId(el)).toBe(101);
    },
    ArrowUp: async () => {
      const el = await mount({ selectedId: 101 });
      await pressOnRow(el, 'ArrowUp');
      expect(selectedId(el)).toBe(100);
    },
    PageDown: async () => {
      const el = await mount({ rows: fixture(50), selectedId: 100 });
      await pressOnRow(el, 'PageDown');
      expect(selectedId(el)).toBe(120);
    },
    PageUp: async () => {
      const el = await mount({ rows: fixture(50), selectedId: 130 });
      await pressOnRow(el, 'PageUp');
      expect(selectedId(el)).toBe(110);
    },
    Home: async () => {
      const el = await mount({ selectedId: 103 });
      await pressOnRow(el, 'Home');
      expect(selectedId(el)).toBe(100);
    },
    End: async () => {
      const el = await mount({ selectedId: 100 });
      await pressOnRow(el, 'End');
      expect(selectedId(el)).toBe(103);
    },
    Enter: async () => {
      const el = await mount({ selectedId: 101 });
      const seen = listen<NcRowEventDetail>(el, 'nc-row-activate');
      await pressOnRow(el, 'Enter');
      expect(seen).toEqual([{ id: 101 }]);
    },
    // Escape belongs to the editors, which is where the legend says it is:
    // "cancel the edit". The table-level handler must leave it alone.
    Escape: async () => {
      const el = await mount({ selectedId: 101, editingId: 101 });
      const seen = listen<NcRowEventDetail>(el, 'nc-edit-cancel');
      el.shadowRoot
        ?.querySelector('.category-input')
        ?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      await el.updateComplete;
      expect(seen).toEqual([{ id: 101 }]);
    },
    f: async () => {
      const el = await mount({ selectedId: 100 });
      const seen = listen<NcFlagToggleDetail>(el, 'nc-flag-toggle');
      await pressOnRow(el, 'f');
      expect(seen).toEqual([{ id: 100, flag: true }]);
    },
    '/': async () => {
      const el = await mount({ selectedId: 100 });
      let asked = 0;
      el.addEventListener('nc-search-focus', () => (asked += 1));
      await pressOnRow(el, '/');
      expect(asked).toBe(1);
    },
  };

  it('is a list every key on which has a proof below', () => {
    const documented = REGISTER_SHORTCUTS.flatMap((hint) => hint.keys);
    expect(documented.filter((key) => !(key in proofs))).toEqual([]);
    expect(Object.keys(proofs).filter((key) => !documented.includes(key))).toEqual([]);
  });

  it.each(REGISTER_SHORTCUTS.flatMap((hint) => hint.keys))('%s works', async (key) => {
    await proofs[key]?.();
  });
});

describe('reaching the register from the keyboard', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('is in the tab order before anything has selected a row', async () => {
    // A register opened with a date filter selects nothing on load. With the
    // tab stop tied to the selection, that table had no tabbable element and
    // was unreachable by keyboard, which is why none of its keys worked.
    const el = await mount();
    const stops = rowEls(el).filter((tr) => tr.tabIndex === 0);
    expect(stops.map((tr) => tr.dataset.id)).toEqual(['100']);
  });

  it('keeps exactly one tab stop once a row is selected', async () => {
    const el = await mount({ selectedId: 102 });
    expect(rowEls(el).filter((tr) => tr.tabIndex === 0).map((tr) => tr.dataset.id)).toEqual(
      ['102'],
    );
  });

  it('still has a tab stop after Escape', async () => {
    const el = await mount({ selectedId: 102 });
    rowEls(el)[2]?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, composed: true }),
    );
    await el.updateComplete;
    expect(rowEls(el).filter((tr) => tr.tabIndex === 0)).toHaveLength(1);
  });

  it('moves its tab stop with the selection, never leaving two', async () => {
    const el = await mount({ selectedId: 100 });
    await press(el, 'ArrowDown');
    expect(rowEls(el).filter((tr) => tr.tabIndex === 0).map((tr) => tr.dataset.id)).toEqual(
      ['101'],
    );
  });

  it('answers a key pressed on the flag button, not only on the row', async () => {
    const el = await mount({ selectedId: 100 });
    rowEls(el)[0]
      ?.querySelector('button')
      ?.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, composed: true }),
      );
    await el.updateComplete;
    expect(selectedId(el)).toBe(101);
  });

  it('focuses the row the cursor is on when asked', async () => {
    const el = await mount({ selectedId: 102 });
    expect(el.focusSelectedRow()).toBe(true);
    expect(el.shadowRoot?.activeElement?.getAttribute('data-id')).toBe('102');
  });

  it('focuses the first row when nothing has been selected yet', async () => {
    const el = await mount();
    expect(el.focusSelectedRow()).toBe(true);
    expect(el.shadowRoot?.activeElement?.getAttribute('data-id')).toBe('100');
  });

  it('reports that an empty register had nothing to focus', async () => {
    const el = await mount({ rows: [] });
    expect(el.focusSelectedRow()).toBe(false);
  });
});

describe('a register too big to put in the DOM', () => {
  const BIG = 1872;

  afterEach(() => {
    document.body.innerHTML = '';
  });

  function dataRows(el: WcRegisterTable): HTMLElement[] {
    return [...(el.shadowRoot?.querySelectorAll<HTMLElement>('tbody tr[data-id]') ?? [])];
  }

  function renderedIds(el: WcRegisterTable): number[] {
    return dataRows(el).map((tr) => Number(tr.dataset.id));
  }

  function spacerHeights(el: WcRegisterTable): number[] {
    return [...(el.shadowRoot?.querySelectorAll<HTMLElement>('tr.spacer td') ?? [])].map(
      (td) => Number.parseFloat(td.style.height),
    );
  }

  /** jsdom measures nothing, so the scroller is given a viewport and rows a height. */
  function stubLayout(el: WcRegisterTable, rowHeight = 33, viewport = 660): void {
    const scroller = el.shadowRoot?.querySelector('.scroller');
    if (!scroller) throw new Error('no scroller');
    Object.defineProperty(scroller, 'clientHeight', {
      value: viewport,
      configurable: true,
    });
    let top = 0;
    Object.defineProperty(scroller, 'scrollTop', {
      get: () => top,
      set: (value: number) => {
        top = value;
      },
      configurable: true,
    });
    for (const row of dataRows(el)) {
      row.getBoundingClientRect = () => ({ height: rowHeight }) as DOMRect;
    }
  }

  async function scrollTo(el: WcRegisterTable, top: number): Promise<void> {
    const scroller = el.shadowRoot?.querySelector('.scroller');
    if (!scroller) throw new Error('no scroller');
    (scroller as HTMLElement).scrollTop = top;
    scroller.dispatchEvent(new Event('scroll'));
    await el.updateComplete;
  }

  it('puts a bounded slice in the DOM, not 1,872 rows', async () => {
    const el = await mount({ rows: fixture(BIG) });
    expect(el.rows.length).toBe(BIG);
    expect(dataRows(el).length).toBeLessThan(60);
    expect(el.shadowRoot?.querySelectorAll('*').length ?? 0).toBeLessThan(700);
  });

  it('keeps the slice the same size however many rows there are', async () => {
    const small = await mount({ rows: fixture(400) });
    const large = await mount({ rows: fixture(20000) });
    expect(dataRows(large).length).toBe(dataRows(small).length);
  });

  it('renders a short register whole, because windowing is not free', async () => {
    const el = await mount({ rows: fixture(40) });
    expect(dataRows(el).length).toBe(40);
    expect(el.shadowRoot?.querySelector('tr.spacer')).toBeNull();
  });

  it('stands in for the rows it left out, so the scrollbar is the whole register', async () => {
    const el = await mount({ rows: fixture(BIG) });
    const shown = dataRows(el).length;
    const total = spacerHeights(el).reduce((sum, height) => sum + height, 0);
    // Every row is one line tall, so the drawn rows plus the spacers are the
    // height the register would have had.
    expect(total / 33 + shown).toBe(BIG);
  });

  it('still tells assistive technology how many rows there really are', async () => {
    const el = await mount({ rows: fixture(BIG) });
    const table = el.shadowRoot?.querySelector('table');
    expect(table?.getAttribute('aria-rowcount')).toBe(String(BIG + 1));
    // The header is row 1, so the first data row is row 2 wherever it sits.
    expect(dataRows(el)[0]?.getAttribute('aria-rowindex')).toBe('2');
  });

  it('numbers a row by its place in the register, not in the window', async () => {
    const el = await mount({ rows: fixture(BIG), selectedId: 100 });
    await press(el, 'End');
    const last = dataRows(el).at(-1);
    expect(Number(last?.dataset.id)).toBe(100 + BIG - 1);
    expect(last?.getAttribute('aria-rowindex')).toBe(String(BIG + 1));
  });

  // -- the four behaviours that have to survive a boundary -------------------

  it('walks the arrow keys past the end of the window', async () => {
    const el = await mount({ rows: fixture(BIG), selectedId: 100 });
    stubLayout(el);

    const before = renderedIds(el);
    for (let i = 0; i < 40; i += 1) await press(el, 'ArrowDown');

    expect(selectedId(el)).toBe(140);
    expect(renderedIds(el)).not.toEqual(before);
    // The selected row is in the DOM and holds the tab stop, which is what
    // makes the next keystroke work.
    const selected = dataRows(el).find((tr) => Number(tr.dataset.id) === 140);
    expect(selected?.tabIndex).toBe(0);
  });

  it('jumps to the last row and back to the first', async () => {
    const el = await mount({ rows: fixture(BIG), selectedId: 100 });
    stubLayout(el);

    await press(el, 'End');
    expect(selectedId(el)).toBe(100 + BIG - 1);
    expect(renderedIds(el)).toContain(100 + BIG - 1);

    await press(el, 'Home');
    expect(selectedId(el)).toBe(100);
    expect(renderedIds(el)).toContain(100);
  });

  it('pages by a screenful across the boundary', async () => {
    const el = await mount({ rows: fixture(BIG), selectedId: 100 });
    stubLayout(el, 33, 33 * 20);

    await press(el, 'PageDown');
    await press(el, 'PageDown');

    expect(selectedId(el)).toBe(140);
    expect(renderedIds(el)).toContain(140);
  });

  it('scrolls to today across the boundary and selects that row', async () => {
    const el = await mount({ rows: fixture(BIG) });
    stubLayout(el);

    expect(el.scrollToIndex(1500)).toBe(true);
    await el.updateComplete;

    expect(selectedId(el)).toBe(1600);
    expect(renderedIds(el)).toContain(1600);
  });

  it('opens the editors on a row the window had left out', async () => {
    const el = await mount({ rows: fixture(BIG), categories });
    el.editingId = 1700;
    await el.updateComplete;

    const editing = dataRows(el).find((tr) => Number(tr.dataset.id) === 1700);
    expect(editing).toBeDefined();
    expect(editing?.querySelector('input[role="combobox"]')).not.toBeNull();
    expect(editing?.querySelector('wa-input')).not.toBeNull();
  });

  it('flags a row that scrolled into the window', async () => {
    const el = await mount({ rows: fixture(BIG) });
    stubLayout(el);
    await scrollTo(el, 33 * 900);

    const seen = listen<NcFlagToggleDetail>(el, 'nc-flag-toggle');
    const row = dataRows(el).find((tr) => Number(tr.dataset.id) === 1000);
    expect(row).toBeDefined();
    row?.querySelector('button')?.click();

    expect(seen).toEqual([{ id: 1000, flag: true }]);
  });

  it('moves the window when the scroller is scrolled', async () => {
    const el = await mount({ rows: fixture(BIG) });
    stubLayout(el);
    expect(renderedIds(el)[0]).toBe(100);

    await scrollTo(el, 33 * 500);
    expect(renderedIds(el)[0]).toBe(100 + 500 - 8);

    await scrollTo(el, 0);
    expect(renderedIds(el)[0]).toBe(100);
  });

  it('takes a search back to the top of its results', async () => {
    const el = await mount({ rows: fixture(BIG) });
    stubLayout(el);
    await scrollTo(el, 33 * 900);
    expect(renderedIds(el)[0]).not.toBe(100);

    el.rows = fixture(BIG).slice(0, 300);
    await el.updateComplete;

    expect(renderedIds(el)[0]).toBe(100);
  });

  it('stays where it is when the host hands back the same rows', async () => {
    // The screen rebuilds its filtered array on every render, so array
    // identity changes constantly; only a different row set is a new list.
    const el = await mount({ rows: fixture(BIG) });
    stubLayout(el);
    await scrollTo(el, 33 * 900);
    const before = renderedIds(el);

    el.rows = fixture(BIG);
    await el.updateComplete;

    expect(renderedIds(el)).toEqual(before);
  });

  it('draws more rows into a taller viewport', async () => {
    const short = await mount({ rows: fixture(BIG) });
    stubLayout(short, 33, 33 * 10);
    // Far enough to move the window, which is what makes the table re-read
    // the viewport it was given.
    await scrollTo(short, 33 * 100);

    const tall = await mount({ rows: fixture(BIG) });
    stubLayout(tall, 33, 33 * 40);
    await scrollTo(tall, 33 * 100);

    expect(dataRows(tall).length).toBeGreaterThan(dataRows(short).length);
    // Still bounded: what grew is the viewport, not the register.
    expect(dataRows(tall).length).toBeLessThan(80);
  });

  it('can be told to render everything anyway', async () => {
    const el = await mount({ rows: fixture(200), virtualizeAbove: Infinity });
    expect(dataRows(el).length).toBe(200);
  });
});

describe('wc-register-table height', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  /** jsdom measures every box as zero, so the two boxes paging reads are given
      sizes here: one row, and the viewport the scroller was granted. */
  function stubLayout(el: WcRegisterTable, rowHeight: number, viewport: number): void {
    const scroller = el.shadowRoot?.querySelector('.scroller');
    if (!scroller) throw new Error('no scroller');
    Object.defineProperty(scroller, 'clientHeight', {
      value: viewport,
      configurable: true,
    });
    for (const row of rowEls(el)) {
      row.getBoundingClientRect = () => ({ height: rowHeight }) as DOMRect;
    }
  }

  it('pages by the rows its own scroller shows, not by a fixed count', async () => {
    const rows = fixture(60);
    const el = await mount({ rows, selectedId: rows[0]?.id, fill: true });

    // A short window: seven rows fit.
    stubLayout(el, 24, 24 * 7 + 8);
    await press(el, 'PageDown');
    expect(selectedId(el)).toBe(rows[7]?.id);

    // A tall one: thirty-one do, and the same key moves that much further.
    stubLayout(el, 24, 24 * 31 + 8);
    await press(el, 'PageDown');
    expect(selectedId(el)).toBe(rows[38]?.id);
  });

  it('reflects fill, which is what the height rules select on', async () => {
    const el = await mount({ fill: true });
    expect(el.hasAttribute('fill')).toBe(true);
  });

  const text = styleText(RegisterTableCtor);

  it('grows into the space a flex-column parent has left, and scrolls inside it', () => {
    // jsdom has no layout engine, so the rules themselves are the evidence.
    // Filling takes three things and breaks without any one of them: the host
    // has to be a growing flex item, the scroller has to be the part that
    // grows, and `min-height: 0` has to override the automatic minimum that
    // would otherwise size the scroller to all 1,800 rows.
    expect(text).toMatch(/:host\(\[fill\]\)\s*{[^}]*flex:\s*1 1 auto/);
    expect(text).toMatch(/:host\(\[fill\]\)\s+\.scroller\s*{[^}]*flex:\s*1 1 auto/);
    expect(text).toMatch(/:host\(\[fill\]\)\s+\.scroller\s*{[^}]*min-height:\s*0/);
    expect(text).toMatch(/:host\(\[fill\]\)\s+\.scroller\s*{[^}]*max-height:\s*none/);
  });

  it('keeps the capped, content-sized shape when it is not filling', () => {
    // The reports screen puts this table inside a page that scrolls as a
    // whole; a table that grew there would push the note below it off-screen.
    expect(text).toMatch(/\.scroller\s*{[^}]*max-height:\s*var\(--nc-register-height, 60vh\)/);
  });

  it('keeps the Net row against the bottom of the scroller', () => {
    expect(text).toMatch(/tfoot td\s*{[^}]*position:\s*sticky/);
    expect(text).toMatch(/tfoot td\s*{[^}]*bottom:\s*0/);
  });
});

describePreviewA11y(preview);
