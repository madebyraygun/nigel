import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  BASE_TICK_MS,
  BOARD_HEIGHT,
  BOARD_WIDTH,
  FOOD_MAX_CENTS,
  FOOD_MIN_CENTS,
  MIN_TICK_MS,
  SPEEDUP_MS_PER_SEGMENT,
  START_LENGTH,
} from './snake-engine.js';

/**
 * The web Snake is meant to *be* the terminal's Snake, not merely resemble it,
 * which is `palette-parity.test.ts`'s claim about the colours applied to the
 * rules. This reads `src/cli/snake.rs` and fails if a number that decides how
 * the game plays has drifted on either side.
 *
 * It reads the source rather than a port of it for the reason that test does:
 * a written-down copy of the Rust constants would agree with itself forever.
 */
const here = dirname(fileURLToPath(import.meta.url));
const snakeRs = resolve(here, '../../../../../src/cli/snake.rs');

function source(): string {
  const text = readFileSync(snakeRs, 'utf8');
  expect(text.length, `src/cli/snake.rs is empty at ${snakeRs}`).toBeGreaterThan(0);
  return text;
}

/** The value of a `const NAME: T = <int>;` declaration. */
function rustConst(name: string): number {
  const match = source().match(new RegExp(`const ${name}:[^=]*=\\s*(\\d+)`));
  expect(match, `${name} not found in src/cli/snake.rs`).not.toBeNull();
  return Number(match![1]);
}

describe('snake parity with src/cli/snake.rs', () => {
  it('ticks at the same base rate', () => {
    expect(BASE_TICK_MS).toBe(rustConst('BASE_TICK_MS'));
  });

  it('floors the tick at the same rate', () => {
    expect(MIN_TICK_MS).toBe(rustConst('MIN_TICK_MS'));
  });

  it('plays on the same board', () => {
    const text = source();
    expect(text).toContain(`let board_width: u16 = ${BOARD_WIDTH};`);
    expect(text).toContain(`let board_height: u16 = ${BOARD_HEIGHT};`);
  });

  it('starts at the same length', () => {
    // Three pushes into the body in `new()`, and the same three the speed
    // curve counts from in `tick_rate`.
    expect(START_LENGTH).toBe(3);
    expect(source()).toContain(
      `(self.body.len() as u64).saturating_sub(${START_LENGTH}) * ${SPEEDUP_MS_PER_SEGMENT}`,
    );
  });

  it('prices food over the same range of cents', () => {
    expect(source()).toContain(
      `rng.gen_range(${FOOD_MIN_CENTS}..=${FOOD_MAX_CENTS})`,
    );
  });

  it('still ends on a full board, which is what the win is', () => {
    const text = source();
    expect(text).toContain('let total = self.board_width as usize * self.board_height as usize');
    expect(text).toContain('if self.body.len() >= total');
  });
});
