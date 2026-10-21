import { describe, it, expect, afterEach } from 'vitest';
import './wc-logo-field.js';
import type { WcLogoField, NcLogoFileDetail } from './wc-logo-field.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-logo-field.preview.js';

async function mount(props: Partial<WcLogoField> = {}): Promise<WcLogoField> {
  const el = document.createElement('wc-logo-field');
  Object.assign(el, { name: 'Acme Consulting LLC' }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function files(el: WcLogoField): File[] {
  const seen: File[] = [];
  el.addEventListener('nc-logo-file', (e) =>
    seen.push((e as CustomEvent<NcLogoFileDetail>).detail.file),
  );
  return seen;
}

const png = () => new File([new Uint8Array([1, 2, 3])], 'logo.png', { type: 'image/png' });

describe('wc-logo-field', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it("stands the book's initials in for a missing logo", async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('img')).toBeNull();
    expect(el.shadowRoot?.querySelector('.preview')?.textContent?.trim()).toBe('AC');
  });

  it('previews a stored logo and offers to remove it', async () => {
    const el = await mount({ logo: 'data:image/png;base64,AAAA' });
    expect(el.shadowRoot?.querySelector('img')?.getAttribute('src')).toBe(
      'data:image/png;base64,AAAA',
    );
    let cleared = 0;
    el.addEventListener('nc-logo-clear', () => (cleared += 1));
    const remove = [...(el.shadowRoot?.querySelectorAll('wa-button') ?? [])].find((b) =>
      b.textContent?.includes('Remove'),
    ) as HTMLElement;
    remove.click();
    expect(cleared).toBe(1);
  });

  it('hands a picked file up and resets the picker', async () => {
    const el = await mount();
    const seen = files(el);
    const picker = el.shadowRoot?.querySelector('input[type="file"]') as HTMLInputElement;
    const file = png();
    Object.defineProperty(picker, 'files', { value: [file], configurable: true });
    picker.dispatchEvent(new Event('change'));
    expect(seen).toEqual([file]);
    expect(picker.value).toBe('');
  });

  it('takes a dropped file the same way', async () => {
    const el = await mount();
    const seen = files(el);
    const file = png();
    const drop = new Event('drop', { cancelable: true }) as DragEvent;
    Object.defineProperty(drop, 'dataTransfer', { value: { files: [file] } });
    el.shadowRoot?.querySelector('.zone')?.dispatchEvent(drop);
    expect(seen).toEqual([file]);
  });

  it('takes nothing while disabled', async () => {
    const el = await mount({ disabled: true });
    const seen = files(el);
    const drop = new Event('drop', { cancelable: true }) as DragEvent;
    Object.defineProperty(drop, 'dataTransfer', { value: { files: [png()] } });
    el.shadowRoot?.querySelector('.zone')?.dispatchEvent(drop);
    expect(seen).toEqual([]);
  });
});

describePreviewA11y(preview);
