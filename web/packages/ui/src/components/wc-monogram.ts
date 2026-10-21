import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { PASTEL_COUNT } from '@nigel/theme';

/** Company-form suffixes that say nothing about which book this is. */
const SUFFIXES = new Set(['llc', 'inc', 'ltd', 'co', 'corp', 'plc', 'llp', 'lp', 'gmbh']);

/**
 * Up to two initials for a name: the first letter of its first two words, or
 * the first two letters of a single word. "Acme Consulting LLC" is "AC",
 * "Globex" is "GL". Legal suffixes are skipped, so "Initech, Inc." is "IN"
 * rather than "II".
 */
export function monogramInitials(name: string): string {
  const words = name
    .split(/[^\p{L}\p{N}]+/u)
    .filter((word) => word !== '' && !SUFFIXES.has(word.toLowerCase()));
  if (words.length === 0) return '';
  const letters =
    words.length === 1 ? [...words[0]].slice(0, 2) : [[...words[0]][0], [...words[1]][0]];
  return letters.join('').toUpperCase();
}

/**
 * Which of the pastels a seed lands on, 1 to `PASTEL_COUNT`.
 *
 * FNV-1a over the seed's UTF-16 code units: stable across sessions and
 * machines, so a book keeps its colour wherever it is opened, and spread well
 * enough that two books in one list rarely share one.
 */
export function monogramTone(seed: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < seed.length; i += 1) {
    hash ^= seed.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return ((hash >>> 0) % PASTEL_COUNT) + 1;
}

/**
 * A book's monogram: its initials on a pastel tile.
 *
 * The colour comes from `seed` rather than from the name, so renaming a book
 * keeps its colour; a caller passes whatever identifies the book for good
 * (its folder). Decorative — the name is always printed beside it — so it is
 * hidden from assistive tech.
 */
@customElement('wc-monogram')
export class WcMonogram extends LitElement {
  static styles = css`
    :host {
      --nc-monogram-size: 20px;
      display: inline-flex;
      flex: none;
    }

    :host([size='l']) {
      --nc-monogram-size: 32px;
    }

    .tile {
      display: inline-grid;
      place-items: center;
      width: var(--nc-monogram-size);
      height: var(--nc-monogram-size);
      border-radius: var(--wa-radius-sm);
      background: var(--nc-color-pastel-1);
      color: var(--nc-color-on-gradient);
      font-family: var(--wa-font-family-sans);
      font-size: calc(var(--nc-monogram-size) * 0.45);
      font-weight: var(--wa-font-weight-bold);
      line-height: 1;
      letter-spacing: 0;
    }

    :host([size='l']) .tile {
      border-radius: var(--wa-radius-md);
      font-size: var(--nc-font-size-caption);
    }

    .tone-2 {
      background: var(--nc-color-pastel-2);
    }
    .tone-3 {
      background: var(--nc-color-pastel-3);
    }
    .tone-4 {
      background: var(--nc-color-pastel-4);
    }
    .tone-5 {
      background: var(--nc-color-pastel-5);
    }
    .tone-6 {
      background: var(--nc-color-pastel-6);
    }
    .tone-7 {
      background: var(--nc-color-pastel-7);
    }
  `;

  /** The book's name; the initials come from it. */
  @property({ type: String })
  name = '';

  /** What picks the colour. Falls back to the name when empty. */
  @property({ type: String })
  seed = '';

  /** `s` is the 20px breadcrumb tile, `l` the 32px card tile. */
  @property({ type: String, reflect: true })
  size: 's' | 'l' = 's';

  render() {
    const tone = monogramTone(this.seed || this.name);
    return html`<span class="tile tone-${tone}" part="tile" aria-hidden="true"
      >${monogramInitials(this.name)}</span
    >`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-monogram': WcMonogram;
  }
}
