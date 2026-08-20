import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { styleMap } from 'lit/directives/style-map.js';

import { MAX_PARTICLES } from './snake-engine.js';
import {
  prefersReducedMotion,
  seedParticleField,
  type FieldParticle,
} from './particle-field.js';

/**
 * The ambient drift: pastel punctuation rising slowly through whatever it is
 * placed behind.
 *
 * Decoration and nothing else, so the host is `aria-hidden` and the specks are
 * never in the tab order or the accessibility tree. Under reduced motion they
 * are still drawn and simply stop — the field is part of the composition, and
 * removing it would move everything laid out over it.
 */
@customElement('wc-particle-field')
export class WcParticleField extends LitElement {
  static styles = css`
    :host {
      display: block;
      position: relative;
      overflow: hidden;
      pointer-events: none;
    }

    .particle {
      position: absolute;
      top: 0;
      height: 100%;
      line-height: 1;
      font-family: var(--wa-font-family-mono);
      font-size: var(--wa-font-size-s, 13px);
      color: var(--tint);
      opacity: var(--brightness);
      transform: translateY(var(--rest));
      animation: nc-particle-rise var(--duration) linear var(--delay) infinite;
    }

    /* The glyph sits at the top of a field-height box, so one field height of
       travel in each direction carries it across and off. */
    @keyframes nc-particle-rise {
      from {
        transform: translateY(100%);
      }
      to {
        transform: translateY(-100%);
      }
    }

    :host([reduced-motion]) .particle {
      animation: none;
    }

    @media (prefers-reduced-motion: reduce) {
      .particle {
        animation: none;
      }
    }
  `;

  /** How many specks. Clamped to the cap the TUI draws under. */
  @property({ type: Number })
  density = MAX_PARTICLES;

  @property({ type: Boolean, reflect: true, attribute: 'reduced-motion' })
  reducedMotion = prefersReducedMotion();

  private motionQuery: MediaQueryList | null = null;
  private field: FieldParticle[] = [];
  private seededFor = -1;

  connectedCallback(): void {
    super.connectedCallback();
    this.setAttribute('aria-hidden', 'true');
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

  render() {
    // Reseeded only when the count changes: a fresh field on every render
    // would teleport every speck each time the host updates.
    if (this.seededFor !== this.density) {
      this.field = seedParticleField(Math.random, this.density);
      this.seededFor = this.density;
    }

    return this.field.map(
      (speck) => html`<span
        class="particle"
        style=${styleMap({
          left: speck.left,
          '--rest': speck.rest,
          '--tint': speck.tint,
          '--brightness': speck.brightness,
          '--duration': speck.duration,
          '--delay': speck.delay,
        })}
        >${speck.glyph}</span
      >`,
    );
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-particle-field': WcParticleField;
  }
}
