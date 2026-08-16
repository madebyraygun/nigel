import { describe, it, expect, beforeAll } from 'vitest';
import { render } from 'lit';
import './icons.js';
import { ICON_TAGS } from './icons.js';
import { WcIconBase } from './icon-base.js';
import '../components/wc-empty-state.js';
import { WcEmptyState } from '../components/wc-empty-state.js';
import { WcDropzone } from '../components/wc-dropzone.js';
import { WcUnlockCard } from '../components/wc-unlock-card.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './icons.preview.js';

function sheet(component: { styles: unknown }): string {
  return [component.styles].flat().map(String).join('\n');
}

const tagOf = (el: Element) => el.tagName.toLowerCase();

/** Every icon one preview state draws, rendered rather than read off source. */
function marksIn(name: string): Element[] {
  const state = preview.states.find((candidate) => candidate.name === name);
  if (!state) throw new Error(`the icons preview has no "${name}" state`);

  const host = document.createElement('div');
  document.body.appendChild(host);
  render(state.render(), host);
  const marks = [...host.querySelectorAll('*')].filter((el) =>
    tagOf(el).startsWith('wc-icon-'),
  );
  host.remove();
  return marks;
}

async function mount(tag: string, label?: string): Promise<HTMLElement> {
  const el = document.createElement(tag);
  if (label !== undefined) el.setAttribute('label', label);
  document.body.appendChild(el);
  await (el as HTMLElement & { updateComplete: Promise<unknown> }).updateComplete;
  return el;
}

describe('icons', () => {
  beforeAll(() => {
    document.body.innerHTML = '';
  });

  it.each(ICON_TAGS)('%s is registered', (tag) => {
    expect(customElements.get(tag)).toBeDefined();
  });

  it.each(ICON_TAGS)('%s renders an svg with geometry', async (tag) => {
    const el = await mount(tag);
    const svg = el.shadowRoot?.querySelector('svg');
    expect(svg).toBeTruthy();
    expect(svg?.querySelectorAll('path').length).toBeGreaterThan(0);
    el.remove();
  });

  it('is hidden from assistive tech when unlabelled', async () => {
    const el = await mount('wc-icon-flag');
    const svg = el.shadowRoot?.querySelector('svg');
    expect(svg?.getAttribute('role')).toBe('presentation');
    expect(svg?.getAttribute('aria-hidden')).toBe('true');
    el.remove();
  });

  it('becomes an img with an accessible name when labelled', async () => {
    const el = await mount('wc-icon-flag', 'Flagged');
    const svg = el.shadowRoot?.querySelector('svg');
    expect(svg?.getAttribute('role')).toBe('img');
    expect(svg?.getAttribute('aria-hidden')).toBe('false');
    expect(svg?.getAttribute('aria-label')).toBe('Flagged');
    el.remove();
  });
});

/**
 * One mechanism for the marks that sit in text, rather than a `--nc-icon-size:
 * 1em` line repeated in every component that draws one.
 */
describe('inline sizing', () => {
  it('is a mode of its own, leaving the token to size everything else', () => {
    const css = sheet(WcIconBase);
    expect(css).toContain('width: var(--nc-icon-size, 20px)');
    expect(css).toContain('height: var(--nc-icon-size, 20px)');
    // A separate rule behind the attribute, so nothing gets 1em by default.
    expect(css).toMatch(/:host\(\[inline\]\)\s*{\s*width:\s*1em;\s*height:\s*1em;\s*}/);
  });

  it('is off unless it is asked for', async () => {
    const el = await mount('wc-icon-flag');
    expect(el.hasAttribute('inline')).toBe(false);
    expect((el as HTMLElement & { inline: boolean }).inline).toBe(false);
    el.remove();
  });

  it('leaves the standalone uses sizing by the token, as they were', async () => {
    // The three components that size icons in px must not have picked up 1em:
    // an empty state's 32px mark is not text, and neither is a dropzone's.
    expect(sheet(WcEmptyState)).toContain('--nc-icon-size: 32px');
    expect(sheet(WcDropzone)).toContain('--nc-icon-size: 28px');
    expect(sheet(WcUnlockCard)).toContain('--nc-icon-size: 24px');

    const empty = document.createElement('wc-empty-state');
    empty.icon = 'wc-icon-register';
    document.body.appendChild(empty);
    await empty.updateComplete;

    const icon = empty.shadowRoot?.querySelector('.icon');
    expect(icon?.tagName.toLowerCase()).toBe('wc-icon-register');
    expect(icon?.hasAttribute('inline')).toBe(false);
    empty.remove();
  });

  it('is what the gallery leaves alone and the inline states ask for', () => {
    // Both halves matter: the sized grids must stay standalone, and the two
    // states that stand in for text characters must be the ones using it.
    for (const name of ['default', 'large', 'small', 'colored']) {
      const marks = marksIn(name);
      expect(marks.length, `${name} rendered no icons`).toBeGreaterThan(0);
      expect(
        marks.filter((mark) => mark.hasAttribute('inline')).map(tagOf),
        `${name} sizes by the token and must not be inline`,
      ).toEqual([]);
    }

    for (const name of ['inline-with-text', 'inline-follows-the-type-size']) {
      const marks = marksIn(name);
      expect(marks.length, `${name} rendered no icons`).toBeGreaterThan(0);
      expect(
        marks.filter((mark) => !mark.hasAttribute('inline')).map(tagOf),
        `${name} stands in for text characters and must be inline`,
      ).toEqual([]);
    }
  });
});

describePreviewA11y(preview);
