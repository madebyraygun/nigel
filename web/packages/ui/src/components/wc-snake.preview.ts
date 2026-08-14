import { html } from 'lit';
import './wc-snake.js';
import { BOARD_HEIGHT, BOARD_WIDTH, type SnakeState } from './snake-engine.js';
import type { Preview } from '../../preview/types.js';

/**
 * Boards written out rather than played, so a state is the same state every
 * time it is reviewed — and so axe runs over a board that is not moving under
 * it. Every state is `paused` for the same reason.
 */
function board(partial: Partial<SnakeState> = {}): SnakeState {
  return {
    body: [
      { x: 20, y: 10 },
      { x: 19, y: 10 },
      { x: 18, y: 10 },
    ],
    direction: 'right',
    nextDirection: 'right',
    food: { x: 28, y: 6 },
    foodValue: 4.25,
    score: 0,
    gameOver: false,
    boardWidth: BOARD_WIDTH,
    boardHeight: BOARD_HEIGHT,
    ...partial,
  };
}

/** A snake long enough to show the whole gradient running down its length. */
const coiled = board({
  body: [
    { x: 26, y: 8 },
    { x: 25, y: 8 },
    { x: 24, y: 8 },
    { x: 23, y: 8 },
    { x: 22, y: 8 },
    { x: 22, y: 9 },
    { x: 22, y: 10 },
    { x: 22, y: 11 },
    { x: 23, y: 11 },
    { x: 24, y: 11 },
    { x: 25, y: 11 },
    { x: 26, y: 11 },
    { x: 27, y: 11 },
    { x: 27, y: 10 },
  ],
  food: { x: 12, y: 4 },
  foodValue: 7.5,
  score: 38.75,
});

const preview: Preview = {
  id: 'wc-snake',
  title: 'Snake',
  group: 'Overlays',
  description:
    'The easter egg the TUI hides behind s, playing by the same rules: 40×20, food worth $1.00–$9.99, and a tick that shortens as the snake grows. Arrow keys steer, R restarts, Escape asks the host to close.',
  layout: 'stack',
  states: [
    {
      name: 'new game',
      render: () => html`<wc-snake paused .game=${board()}></wc-snake>`,
    },
    {
      name: 'in play',
      render: () => html`<wc-snake paused .game=${coiled}></wc-snake>`,
    },
    {
      name: 'game over',
      render: () =>
        html`<wc-snake
          paused
          .game=${{ ...coiled, gameOver: true, score: 61.4 }}
        ></wc-snake>`,
    },
    {
      name: 'reduced motion',
      render: () =>
        html`<wc-snake paused reduced-motion .game=${coiled}></wc-snake>`,
    },
  ],
};

export default preview;
