import { describe, it, expect, afterEach } from 'vitest';
import './wc-particle-field.js';
import type { WcParticleField } from './wc-particle-field.js';
import { MAX_PARTICLES } from './snake-engine.js';
import { seedParticleField } from './particle-field.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-particle-field.preview.js';

async function mount(props: Partial<WcParticleField> = {}): Promise<WcParticleField> {
  const el = document.createElement('wc-particle-field');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const specks = (el: WcParticleField) => [...(el.shadowRoot?.querySelectorAll('.particle') ?? [])];

describe('seedParticleField', () => {
  it('never exceeds the TUI cap however many are asked for', () => {
    expect(seedParticleField(() => 0.5, 500)).toHaveLength(MAX_PARTICLES);
  });

  it('draws every field from the shared glyph and palette sets', () => {
    const field = seedParticleField(() => 0.5, 4);
    expect(field).toHaveLength(4);
    for (const speck of field) {
      expect(speck.glyph).toMatch(/[·∘•◦]/);
      expect(speck.tint).toMatch(/^#[0-9a-f]{6}$/i);
    }
  });
});

describe('wc-particle-field', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the default density', async () => {
    const el = await mount();
    expect(specks(el)).toHaveLength(MAX_PARTICLES);
  });

  it('honours a lower density', async () => {
    const el = await mount({ density: 6 });
    expect(specks(el)).toHaveLength(6);
  });

  it("caps density at the TUI's own limit", async () => {
    const el = await mount({ density: 500 });
    expect(specks(el)).toHaveLength(MAX_PARTICLES);
  });

  it('is decoration and says so', async () => {
    // Drifting punctuation read aloud one glyph at a time is not information.
    const el = await mount();
    expect(el.getAttribute('aria-hidden')).toBe('true');
  });

  it('still renders the specks when motion is unwelcome, and stops them', async () => {
    const el = await mount({ reducedMotion: true });
    expect(specks(el)).toHaveLength(MAX_PARTICLES);
    expect(el.hasAttribute('reduced-motion')).toBe(true);
  });
});

describePreviewA11y(preview);
