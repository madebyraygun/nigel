/**
 * The rules of Snake, ported from `crates/nigel/src/cli/snake.rs`.
 *
 * The terminal and the browser are meant to be playing the same game, not two
 * games that look alike, so every rule the TUI has — the board size, the tick
 * that shortens as the snake grows, food worth between a dollar and ten, the
 * three ways it ends — lives here in the same form and is pinned to the Rust
 * source by `snake-parity.test.ts`.
 *
 * Pure and rendering-free: the component owns the clock and the pixels, this
 * owns the rules, and a state can be written out by hand for a preview.
 */

/** Cells across. The TUI clamps to this and so, at most, does the board. */
export const BOARD_WIDTH = 40;
/** Cells down. */
export const BOARD_HEIGHT = 20;
/** Milliseconds a three-segment snake takes to move one cell. */
export const BASE_TICK_MS = 150;
/** However long the snake gets, it never moves faster than this. */
export const MIN_TICK_MS = 50;
/** Milliseconds shaved off the tick by each segment past the starting length. */
export const SPEEDUP_MS_PER_SEGMENT = 2;
/** Segments a new snake starts with. */
export const START_LENGTH = 3;
/** Food is worth a whole number of cents in this range. */
export const FOOD_MIN_CENTS = 100;
export const FOOD_MAX_CENTS = 999;

/**
 * The drifting specks behind the board, from `crates/nigel/src/effects.rs` — the same
 * effect the splash, goodbye and onboarding screens share with the TUI's
 * Snake. Decorative in both front ends, and the first thing a reduced-motion
 * preference switches off.
 */
export const MAX_PARTICLES = 20;
export const PARTICLE_CHARS = ['·', '∘', '•', '◦'] as const;

export type Direction = 'up' | 'down' | 'left' | 'right';

export interface Cell {
  x: number;
  y: number;
}

export interface SnakeState {
  /** Head first, tail last. */
  readonly body: readonly Cell[];
  /** The direction the last step was taken in. */
  readonly direction: Direction;
  /** The direction the next step will be taken in. */
  readonly nextDirection: Direction;
  readonly food: Cell;
  /** What the food on the board right now is worth, in dollars. */
  readonly foodValue: number;
  readonly score: number;
  readonly gameOver: boolean;
  readonly boardWidth: number;
  readonly boardHeight: number;
}

/** A `Math.random`-shaped source, injected so a test can make a fixed board. */
export type Rng = () => number;

export function opposite(direction: Direction): Direction {
  switch (direction) {
    case 'up':
      return 'down';
    case 'down':
      return 'up';
    case 'left':
      return 'right';
    case 'right':
      return 'left';
  }
}

const sameCell = (a: Cell, b: Cell) => a.x === b.x && a.y === b.y;

/** Dollars, to the cent — the TUI's `random_food_value`. */
export function randomFoodValue(rng: Rng): number {
  const span = FOOD_MAX_CENTS - FOOD_MIN_CENTS + 1;
  return (FOOD_MIN_CENTS + Math.floor(rng() * span)) / 100;
}

/**
 * A free cell for the next piece of food.
 *
 * Rejection sampling, as in the TUI: the board is 800 cells and a snake long
 * enough to make this slow has all but won. A full board answers the origin,
 * which the caller never renders because a full board is game over.
 */
export function randomFoodCell(
  body: readonly Cell[],
  width: number,
  height: number,
  rng: Rng,
): Cell {
  if (body.length >= width * height) return { x: 0, y: 0 };
  for (;;) {
    const cell = {
      x: Math.floor(rng() * width),
      y: Math.floor(rng() * height),
    };
    if (!body.some((segment) => sameCell(segment, cell))) return cell;
  }
}

export function createGame(rng: Rng = Math.random): SnakeState {
  const boardWidth = BOARD_WIDTH;
  const boardHeight = BOARD_HEIGHT;
  const cx = Math.floor(boardWidth / 2);
  const cy = Math.floor(boardHeight / 2);
  const body: Cell[] = [
    { x: cx, y: cy },
    { x: Math.max(0, cx - 1), y: cy },
    { x: Math.max(0, cx - 2), y: cy },
  ];

  return {
    body,
    direction: 'right',
    nextDirection: 'right',
    food: randomFoodCell(body, boardWidth, boardHeight, rng),
    foodValue: randomFoodValue(rng),
    score: 0,
    gameOver: false,
    boardWidth,
    boardHeight,
  };
}

/**
 * Aim the snake. A reversal is ignored rather than fatal — the TUI checks
 * against the direction of the last completed step, not against the pending
 * one, so two quick turns cannot fold the snake back into itself.
 */
export function turn(state: SnakeState, direction: Direction): SnakeState {
  if (state.gameOver) return state;
  if (state.direction === opposite(direction)) return state;
  if (state.nextDirection === direction) return state;
  return { ...state, nextDirection: direction };
}

function advance(head: Cell, direction: Direction): Cell {
  switch (direction) {
    case 'up':
      return { x: head.x, y: head.y - 1 };
    case 'down':
      return { x: head.x, y: head.y + 1 };
    case 'left':
      return { x: head.x - 1, y: head.y };
    case 'right':
      return { x: head.x + 1, y: head.y };
  }
}

/** One move. The three endings are the wall, the snake itself, and a full board. */
export function step(state: SnakeState, rng: Rng = Math.random): SnakeState {
  if (state.gameOver) return state;

  const direction = state.nextDirection;
  const head = advance(state.body[0], direction);

  const offBoard =
    head.x < 0 || head.y < 0 || head.x >= state.boardWidth || head.y >= state.boardHeight;
  if (offBoard || state.body.some((segment) => sameCell(segment, head))) {
    return { ...state, direction, gameOver: true };
  }

  const grown = [head, ...state.body];

  if (!sameCell(head, state.food)) {
    grown.pop();
    return { ...state, direction, body: grown };
  }

  const score = state.score + state.foodValue;

  // Filling the board is the win, and it is still game over: there is nowhere
  // left to put the next piece of food.
  if (grown.length >= state.boardWidth * state.boardHeight) {
    return { ...state, direction, body: grown, score, gameOver: true };
  }

  return {
    ...state,
    direction,
    body: grown,
    score,
    food: randomFoodCell(grown, state.boardWidth, state.boardHeight, rng),
    foodValue: randomFoodValue(rng),
  };
}

/** Milliseconds until the next step: shorter the longer the snake is. */
export function tickInterval(state: SnakeState): number {
  const extra = Math.max(0, state.body.length - START_LENGTH);
  return Math.max(MIN_TICK_MS, BASE_TICK_MS - extra * SPEEDUP_MS_PER_SEGMENT);
}
