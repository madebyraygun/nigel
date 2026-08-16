import { describe, it, expect } from 'vitest';
import {
  BOARD_HEIGHT,
  BOARD_WIDTH,
  BASE_TICK_MS,
  MIN_TICK_MS,
  createGame,
  opposite,
  randomFoodValue,
  randomFoodCell,
  step,
  tickInterval,
  turn,
  type Cell,
  type SnakeState,
} from './snake-engine.js';

/**
 * These are `mod tests` in `src/cli/snake.rs`, case for case and in its order.
 * Keeping the names recognisable is the point: a rule changed on one side and
 * not the other should fail a test whose Rust twin still passes.
 */

/**
 * Deterministic stand-in for `Math.random`, cycling a fixed sequence.
 *
 * The default draws the top-left cell, which no test's snake occupies, so
 * placing food never has to reject a draw. A constant that *did* name an
 * occupied cell would spin `randomFoodCell` forever — rejection sampling
 * against a source that never varies.
 */
function fixedRng(...values: number[]) {
  const sequence = values.length > 0 ? values : [0.01];
  let i = 0;
  return () => sequence[i++ % sequence.length];
}

const cells = (...pairs: [number, number][]): Cell[] =>
  pairs.map(([x, y]) => ({ x, y }));

const at = (state: SnakeState, index: number) => state.body[index];

describe('snake engine', () => {
  it('new game starts correctly', () => {
    const game = createGame(fixedRng());
    expect(game.body).toHaveLength(3);
    expect(game.score).toBe(0);
    expect(game.gameOver).toBe(false);
    expect(game.direction).toBe('right');
    expect(game.boardWidth).toBe(BOARD_WIDTH);
    expect(game.boardHeight).toBe(BOARD_HEIGHT);
  });

  it('snake moves right', () => {
    const game = createGame(fixedRng());
    const before = at(game, 0);
    const after = step(game, fixedRng());
    expect(at(after, 0)).toEqual({ x: before.x + 1, y: before.y });
    expect(after.body).toHaveLength(3);
  });

  it('snake changes direction', () => {
    const game = step(turn(createGame(fixedRng()), 'down'), fixedRng(0.01));
    expect(at(game, 0).y).toBe(at(game, 1).y + 1);
  });

  it('cannot reverse direction', () => {
    const game = step(turn(createGame(fixedRng()), 'left'), fixedRng(0.01));
    expect(at(game, 0).x).toBeGreaterThan(at(game, 1).x);
  });

  it('wall collision ends game', () => {
    let game = createGame(fixedRng(0.99));
    for (let i = 0; i < 100 && !game.gameOver; i += 1) game = step(game, fixedRng(0.99));
    expect(game.gameOver).toBe(true);
  });

  it('eating food grows snake and scores', () => {
    const start = createGame(fixedRng());
    const head = at(start, 0);
    const primed: SnakeState = {
      ...start,
      food: { x: head.x + 1, y: head.y },
      foodValue: 5,
    };

    const game = step(primed, fixedRng(0.01));
    expect(game.body).toHaveLength(start.body.length + 1);
    expect(game.score).toBe(5);
  });

  it('food value in range', () => {
    for (let i = 0; i < 1000; i += 1) {
      const value = randomFoodValue(() => i / 1000);
      expect(value).toBeGreaterThanOrEqual(1);
      expect(value).toBeLessThanOrEqual(9.99);
      // A whole number of cents, which is what makes it read as money.
      expect(value * 100).toBeCloseTo(Math.round(value * 100), 6);
    }
  });

  it('self collision ends game', () => {
    const base = createGame(fixedRng());
    const looped: SnakeState = {
      ...base,
      body: cells([5, 5], [4, 5], [4, 6], [5, 6], [6, 6], [6, 5]),
      direction: 'right',
      nextDirection: 'right',
    };
    expect(step(looped, fixedRng(0.01)).gameOver).toBe(true);
  });

  it('board full ends game', () => {
    const base = createGame(fixedRng());
    const nearlyFull: SnakeState = {
      ...base,
      boardWidth: 3,
      boardHeight: 1,
      body: cells([1, 0], [0, 0]),
      direction: 'right',
      nextDirection: 'right',
      food: { x: 2, y: 0 },
      foodValue: 1,
    };

    const game = step(nearlyFull, fixedRng(0.01));
    expect(game.body).toHaveLength(3);
    expect(game.gameOver).toBe(true);
    expect(game.score).toBe(1);
  });

  it('direction opposite', () => {
    expect(opposite('up')).toBe('down');
    expect(opposite('down')).toBe('up');
    expect(opposite('left')).toBe('right');
    expect(opposite('right')).toBe('left');
  });
});

describe('food placement', () => {
  it('never lands on the snake', () => {
    const body = cells([0, 0], [1, 0], [2, 0]);
    // The first two draws name occupied cells; the third is the free one.
    const cell = randomFoodCell(body, 3, 2, fixedRng(0, 0, 0.5, 0, 0, 0.9));
    expect(body.some((s) => s.x === cell.x && s.y === cell.y)).toBe(false);
  });

  it('answers the origin rather than looping forever on a full board', () => {
    const body = cells([0, 0], [1, 0]);
    expect(randomFoodCell(body, 2, 1, fixedRng(0))).toEqual({ x: 0, y: 0 });
  });
});

describe('tick interval', () => {
  it('starts at the base rate', () => {
    expect(tickInterval(createGame(fixedRng()))).toBe(BASE_TICK_MS);
  });

  it('sheds two milliseconds per segment past the starting length', () => {
    const base = createGame(fixedRng());
    const grown: SnakeState = {
      ...base,
      body: [...base.body, { x: 0, y: 0 }, { x: 0, y: 1 }],
    };
    expect(tickInterval(grown)).toBe(BASE_TICK_MS - 4);
  });

  it('floors, so a very long snake stays playable', () => {
    const base = createGame(fixedRng());
    const long: SnakeState = {
      ...base,
      body: Array.from({ length: 300 }, (_, i) => ({ x: i, y: 0 })),
    };
    expect(tickInterval(long)).toBe(MIN_TICK_MS);
  });
});

describe('a finished game', () => {
  const over = (): SnakeState => ({ ...createGame(fixedRng()), gameOver: true });

  it('does not move', () => {
    const game = over();
    expect(step(game, fixedRng())).toBe(game);
  });

  it('does not turn', () => {
    const game = over();
    expect(turn(game, 'up')).toBe(game);
  });
});
