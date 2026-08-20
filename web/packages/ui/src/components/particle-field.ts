import { NIGEL_PALETTE } from '@nigel/theme';
import { MAX_PARTICLES, PARTICLE_CHARS, type Rng } from './snake-engine.js';

/**
 * The drifting specks the TUI puts behind its splash, goodbye, onboarding and
 * Snake screens, as CSS a component can hand straight to `styleMap`.
 *
 * The constants are `snake-engine.ts`'s, which are `crates/nigel/src/effects.rs`'s: the cap
 * and the glyph set are the same in the terminal and the browser, and there is
 * one seeding function rather than one per screen that wants specks.
 */
export interface FieldParticle {
  /** Horizontal position, as a percentage of the field. */
  left: string;
  /** Where in its rise the speck starts, as a percentage. */
  rest: string;
  duration: string;
  delay: string;
  tint: string;
  brightness: string;
  glyph: string;
}

/** Seed a field. `density` is clamped to the TUI's own cap. */
export function seedParticleField(
  rng: Rng = Math.random,
  density: number = MAX_PARTICLES,
): FieldParticle[] {
  const count = Math.max(0, Math.min(Math.floor(density), MAX_PARTICLES));
  return Array.from({ length: count }, () => ({
    left: `${(rng() * 100).toFixed(2)}%`,
    rest: `${(rng() * 100).toFixed(2)}%`,
    duration: `${(9 + rng() * 12).toFixed(2)}s`,
    delay: `${(-rng() * 12).toFixed(2)}s`,
    tint: NIGEL_PALETTE[Math.floor(rng() * NIGEL_PALETTE.length)],
    brightness: (0.2 + rng() * 0.4).toFixed(2),
    glyph: PARTICLE_CHARS[Math.floor(rng() * PARTICLE_CHARS.length)],
  }));
}

/** Whether the viewer has asked for less movement. False where nobody asked. */
export function prefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  );
}
