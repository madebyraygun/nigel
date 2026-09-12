import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { styleMap } from 'lit/directives/style-map.js';
import { classMap } from 'lit/directives/class-map.js';

import { prefersReducedMotion } from './particle-field.js';

/**
 * The ASCII wordmark, exactly as `crates/nigel/src/effects.rs` declares it.
 *
 * Pinned to the Rust source by `wordmark-parity.test.ts`, so the terminal and
 * the browser cannot end up drawing two different marks.
 */
export const WORDMARK_ART: readonly string[] = [
  '  /$$   /$$ /$$                     /$$',
  ' | $$$ | $$|__/                    | $$',
  ' | $$$$| $$ /$$  /$$$$$$   /$$$$$$ | $$',
  ' | $$ $$ $$| $$ /$$__  $$ /$$__  $$| $$',
  ' | $$  $$$$| $$| $$  \\ $$| $$$$$$$$| $$',
  ' | $$\\  $$$| $$| $$  | $$| $$_____/| $$',
  ' | $$ \\  $$| $$|  $$$$$$$|  $$$$$$$| $$',
  ' |__/  \\__/|__/ \\____  $$ \\_______/|__/',
  '                /$$  \\ $$',
  '               |  $$$$$$/',
  '                \\______/',
] as const;

/**
 * The art with every line padded to the width of the longest.
 *
 * The descender rows are barely a third the width of the wordmark, and a line
 * box shorter than its container is placed by the inherited `text-align` — so
 * on a centred surface the tail slides right and the `g` reads as a `q`. Equal
 * widths make the alignment moot, which is what the marketing page relies on.
 */
const ART_LINES: readonly string[] = (() => {
  const width = Math.max(...WORDMARK_ART.map((line) => line.length));
  return WORDMARK_ART.map((line) => line.padEnd(width));
})();

/** Every drawable position, row-major — spaces carry no colour and no reveal. */
function drawablePositions(): number[] {
  const positions: number[] = [];
  let index = 0;
  for (const line of ART_LINES) {
    for (const char of line) {
      if (char !== ' ') positions.push(index);
      index += 1;
    }
  }
  return positions;
}

/** The TUI's shuffled reveal order, so the mark assembles rather than wipes. */
function shuffled(positions: number[]): number[] {
  const order = [...positions];
  for (let i = order.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1));
    [order[i], order[j]] = [order[j], order[i]];
  }
  return order;
}

/**
 * Nigel's wordmark: the terminal's ASCII art, drawn as per-character spans
 * sharing one animated gradient with staggered delays.
 *
 * Every colour comes from `@nigel/theme` — `--nc-grad-brand-text` static,
 * `--nc-grad-brand-text-cycle` when it drifts, both of which are the ink ramp
 * on a light surface and the pastels on a dark one. Reduced motion renders the
 * static mark at full reveal: the wordmark is the identity, not the animation.
 */
@customElement('wc-wordmark')
export class WcWordmark extends LitElement {
  static styles = css`
    :host {
      display: inline-block;
    }

    .art {
      margin: 0;
      font-family: var(--nc-font-brand);
      font-size: var(--nc-wordmark-size, var(--wa-font-size-s, 13px));
      line-height: 1.05;
      white-space: pre;
      font-weight: var(--wa-font-weight-bold, 700);
    }

    .char {
      display: inline;
      background-image: var(--nc-grad-brand-text);
      background-size: 100% 100%;
      -webkit-background-clip: text;
      background-clip: text;
      -webkit-text-fill-color: transparent;
      color: transparent;
    }

    .hidden {
      visibility: hidden;
    }

    :host([animated]) .char {
      background-image: var(--nc-grad-brand-text-cycle);
      background-size: var(--nc-grad-brand-size);
      animation: nc-wordmark-cycle var(--nc-wordmark-duration, 3.5s) linear infinite;
    }

    @keyframes nc-wordmark-cycle {
      from {
        background-position: 0% 50%;
      }
      to {
        background-position: 100% 50%;
      }
    }

    :host([reduced-motion]) .char {
      background-image: var(--nc-grad-brand-text);
      background-size: 100% 100%;
      animation: none;
    }

    @media (prefers-reduced-motion: reduce) {
      :host([animated]) .char {
        animation: none;
      }
    }
  `;

  /** Whether the gradient drifts along the mark. */
  @property({ type: Boolean, reflect: true })
  animated = false;

  /** How much of the mark is drawn, 0 to 1. Characters appear in a shuffled order. */
  @property({ type: Number })
  reveal = 1;

  /** The accessible name. The ascii itself is never read aloud. */
  @property({ type: String })
  label = 'Nigel';

  @property({ type: Boolean, reflect: true, attribute: 'reduced-motion' })
  reducedMotion = prefersReducedMotion();

  /** Shuffled once per instance: a reshuffle per render un-draws characters. */
  private readonly order = shuffled(drawablePositions());

  private motionQuery: MediaQueryList | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
      this.motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
      this.motionQuery.addEventListener('change', this.handleMotionChange);
    }
  }

  disconnectedCallback(): void {
    this.motionQuery?.removeEventListener('change', this.handleMotionChange);
    this.motionQuery = null;
    super.disconnectedCallback();
  }

  private handleMotionChange = (event: MediaQueryListEvent): void => {
    this.reducedMotion = event.matches;
  };

  private visiblePositions(): Set<number> {
    // Motion is additive: somebody who asked for less of it gets the whole
    // mark rather than a partial one frozen mid-assembly.
    const fraction = this.reducedMotion ? 1 : Math.max(0, Math.min(this.reveal, 1));
    return new Set(this.order.slice(0, Math.round(this.order.length * fraction)));
  }

  /** Spaces are never hidden — they are blank either way, and hiding them
   * would make a full reveal still carry the hidden class. */
  private isHidden(char: string, index: number, visible: Set<number>): boolean {
    return char !== ' ' && !visible.has(index);
  }

  render() {
    const visible = this.visiblePositions();
    let index = -1;

    return html`
      <pre class="art" role="img" aria-label=${this.label}>${ART_LINES.map(
        (line, row) =>
          html`<span class="line"
              >${[...line].map((char, col) => {
                index += 1;
                return html`<span
                  class=${classMap({ char: true, hidden: this.isHidden(char, index, visible) })}
                  style=${styleMap({
                    animationDelay: `-${(row * 0.15 + col * 0.03).toFixed(2)}s`,
                  })}
                  >${char === ' ' ? '\u00a0' : char}</span
                >`;
              })}</span
            >${row < ART_LINES.length - 1 ? '\n' : ''}`,
      )}</pre>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-wordmark': WcWordmark;
  }
}
