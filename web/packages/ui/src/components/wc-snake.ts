import { LitElement, html, css, nothing, type PropertyValues } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { styleMap } from 'lit/directives/style-map.js';
import { gradientColor } from '@nigel/theme';

// The score is money, and there is one component in this package that renders
// money. `plain` drops the income/expense colouring, which belongs to a ledger
// rather than to a scoreboard on a dark board.
import './wc-money.js';

import {
  createGame,
  step,
  tickInterval,
  turn,
  type Direction,
  type SnakeState,
} from './snake-engine.js';
import { seedParticleField, prefersReducedMotion } from './particle-field.js';

/** How far the gradient phase advances per move — `snake.rs`'s `1.0 / 70.0`. */
const PHASE_PER_TICK = 1 / 70;
/** How far along the ramp consecutive segments sit — `snake.rs`'s `0.05`. */
const PHASE_PER_SEGMENT = 0.05;

const ARROWS: Record<string, Direction> = {
  ArrowUp: 'up',
  ArrowDown: 'down',
  ArrowLeft: 'left',
  ArrowRight: 'right',
};

/**
 * Snake, as the TUI plays it.
 *
 * `snake-engine.ts` owns the rules and this owns the clock and the pixels: the
 * board is absolutely positioned cells rather than a canvas, so the snake is
 * inspectable in the DOM and each segment's colour can be asserted rather than
 * eyeballed off a bitmap. A board is 800 cells but only the snake, the food
 * and twenty specks are ever rendered, so a frame is a handful of nodes.
 *
 * Arrow keys steer, `R` restarts a finished game, and Escape asks the host to
 * close by dispatching `nc-snake-exit` — the component never removes itself,
 * because whatever put it on screen is what has to restore the focus.
 *
 * Reduced motion is answered honestly rather than by turning the game off: the
 * snake still moves, because a Snake that does not move is not the game, while
 * the two decorative animations — the specks drifting up the board and the
 * gradient cycling along the snake — stop. The specks stop in CSS as well as
 * in the property, so the preference holds even where nothing sets the
 * attribute.
 */
@customElement('wc-snake')
export class WcSnake extends LitElement {
  static styles = css`
    :host {
      display: block;
      font-family: var(--wa-font-family-mono, monospace);
      color: var(--nc-color-arcade-ink);
      --cols: 40;
      --rows: 20;
    }

    /* The app's overlay. The component is a plain block otherwise, so the
       preview harness can show four boards side by side. */
    :host([fullscreen]) {
      position: fixed;
      inset: 0;
      z-index: 900;
      display: grid;
      place-items: center;
      padding: var(--wa-space-l, 16px);
      background: var(--nc-color-arcade-bg);
    }

    :host(:focus) {
      outline: none;
    }

    .frame {
      display: grid;
      gap: var(--wa-space-xs, 6px);
      width: 100%;
      max-width: 56rem;
      padding: var(--wa-space-m, 12px);
      border: 1px solid var(--nc-color-arcade-ink);
      border-radius: var(--wa-radius-m, 6px);
      background: var(--nc-color-arcade-bg);
    }

    .chrome {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      gap: var(--wa-space-s, 8px);
      font-size: var(--wa-font-size-s, 13px);
    }

    .title {
      font-weight: var(--wa-font-weight-bold, 700);
      background: var(--nc-grad-brand);
      background-clip: text;
      -webkit-background-clip: text;
      color: transparent;
    }

    .score {
      font-weight: var(--wa-font-weight-bold, 700);
    }

    .board {
      position: relative;
      aspect-ratio: var(--cols) / var(--rows);
      overflow: hidden;
      border: 1px solid var(--nc-color-arcade-ink);
      border-radius: var(--wa-radius-s, 4px);
      background: var(--nc-color-arcade-bg);
    }

    .cell {
      position: absolute;
      width: calc(100% / var(--cols));
      height: calc(100% / var(--rows));
      left: calc(var(--x) * 100% / var(--cols));
      top: calc(var(--y) * 100% / var(--rows));
    }

    .segment {
      background: var(--tint);
    }

    .food {
      background: var(--nc-color-arcade-food);
      border-radius: 50%;
    }

    .particle {
      position: absolute;
      top: 0;
      height: 100%;
      line-height: 1;
      font-size: var(--wa-font-size-s, 13px);
      color: var(--tint);
      opacity: var(--brightness);
      transform: translateY(var(--rest));
      animation: rise var(--duration) linear var(--delay) infinite;
      pointer-events: none;
    }

    /* The glyph sits at the top of a board-height box, so one board height of
       travel in each direction carries it across and off. */
    @keyframes rise {
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

    /* A panel over the board rather than instead of it: the TUI leaves the
       snake that just died on screen underneath. */
    .over {
      position: absolute;
      top: 50%;
      left: 50%;
      transform: translate(-50%, -50%);
      display: grid;
      justify-items: center;
      gap: var(--wa-space-xs, 6px);
      text-align: center;
      padding: var(--wa-space-m, 12px);
      background: var(--nc-color-arcade-bg);
      border: 1px solid var(--nc-color-arcade-ink);
      border-radius: var(--wa-radius-s, 4px);
    }

    .over h2 {
      margin: 0;
      font-size: var(--wa-font-size-m, 15px);
      font-weight: var(--wa-font-weight-bold, 700);
    }

    .over p {
      margin: 0;
      font-size: var(--wa-font-size-s, 13px);
    }

    .hint {
      margin: 0;
      font-size: var(--wa-font-size-xs, 11px);
      text-align: center;
    }

    kbd {
      font: inherit;
      font-weight: var(--wa-font-weight-bold, 700);
    }

    .visually-hidden {
      position: absolute;
      width: 1px;
      height: 1px;
      padding: 0;
      margin: -1px;
      overflow: hidden;
      clip: rect(0 0 0 0);
      white-space: nowrap;
      border: 0;
    }

    /* An easter egg is not part of a printed report. */
    @media print {
      :host {
        display: none;
      }
    }
  `;

  /** Cover the viewport. The app's overlay; a preview leaves it off. */
  @property({ type: Boolean, reflect: true })
  fullscreen = false;

  /** Hold the board still. Previews are paused so their states are the states. */
  @property({ type: Boolean, reflect: true })
  paused = false;

  /**
   * Stop the decorative animation. Seeded from the media query and kept in
   * step with it, so an OS setting changed mid-game is honoured.
   */
  @property({ type: Boolean, reflect: true, attribute: 'reduced-motion' })
  reducedMotion = prefersReducedMotion();

  /** The board. Settable, so a preview can pose a mid-game or finished state. */
  @property({ attribute: false })
  game: SnakeState = createGame();

  @state() private phase = 0;

  /**
   * Whether the page is in a background tab.
   *
   * Its own flag rather than a write to `paused`, which belongs to whoever
   * mounted the component: a preview poses a paused board, and a tab coming
   * back to the front must not start a game on it.
   */
  @state() private backgrounded = false;

  private particles = seedParticleField(Math.random);
  private timer: ReturnType<typeof setTimeout> | null = null;
  private motionQuery: MediaQueryList | null = null;

  connectedCallback(): void {
    super.connectedCallback();
    // A game with no focusable children still has to hold the keyboard.
    if (!this.hasAttribute('tabindex')) this.setAttribute('tabindex', '-1');
    this.setAttribute('role', 'dialog');
    if (!this.hasAttribute('aria-label')) this.setAttribute('aria-label', 'Snake');

    this.addEventListener('keydown', this.handleKeydown);

    if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
      this.motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
      this.motionQuery.addEventListener?.('change', this.handleMotionChange);
    }

    document.addEventListener('visibilitychange', this.handleVisibilityChange);
    this.backgrounded = document.hidden;

    this.sync();
  }

  disconnectedCallback(): void {
    this.removeEventListener('keydown', this.handleKeydown);
    this.motionQuery?.removeEventListener?.('change', this.handleMotionChange);
    this.motionQuery = null;
    document.removeEventListener('visibilitychange', this.handleVisibilityChange);
    this.stop();
    super.disconnectedCallback();
  }

  protected willUpdate(changed: PropertyValues<this>): void {
    // Only the overlay is modal. Inline, in the preview harness, it is one
    // component among several and claiming otherwise would be a lie to a
    // screen reader.
    if (!changed.has('fullscreen')) return;
    if (this.fullscreen) this.setAttribute('aria-modal', 'true');
    else this.removeAttribute('aria-modal');
  }

  protected firstUpdated(): void {
    // The overlay owns the keyboard from the moment it opens.
    if (this.fullscreen) this.focus();
  }

  protected updated(): void {
    this.sync();
  }

  /** Throw the board away and start over, as `R` does in the TUI. */
  restart(): void {
    this.game = createGame();
    this.phase = 0;
  }

  private handleMotionChange = (event: MediaQueryListEvent): void => {
    this.reducedMotion = event.matches;
  };

  private handleVisibilityChange = (): void => {
    this.backgrounded = document.hidden;
  };

  private stop(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }

  /**
   * Keep the loop matching the state.
   *
   * A pending timer is left alone rather than replaced, which is what stops a
   * player from holding the snake still by drumming on the arrow keys: a turn
   * re-renders, and a re-render that rescheduled would push the next move back
   * every keystroke.
   *
   * A background tab counts as paused. Browsers throttle its timers to about
   * once a minute, so a game left running there takes single blind steps into
   * a wall — the player comes back to a Game Over they never had a chance at.
   */
  private sync(): void {
    if (!this.isConnected || this.paused || this.backgrounded || this.game.gameOver) {
      this.stop();
      return;
    }
    if (this.timer !== null) return;
    this.timer = setTimeout(this.advance, tickInterval(this.game));
  }

  private advance = (): void => {
    this.timer = null;
    this.game = step(this.game);
    if (!this.reducedMotion) this.phase += PHASE_PER_TICK;
  };

  private handleKeydown = (event: KeyboardEvent): void => {
    // A chord belongs to whoever bound it — the reload, the tab switch, the
    // word-wise cursor move. The game holds the keyboard, not the browser, and
    // a Cmd+R that restarted the snake would have stolen the reload.
    if (event.ctrlKey || event.metaKey || event.altKey) return;

    if (event.key === 'Escape') {
      event.preventDefault();
      this.dispatchEvent(
        new CustomEvent('nc-snake-exit', { bubbles: true, composed: true }),
      );
      return;
    }

    // Nothing inside the overlay is focusable, so Tab would drop the keyboard
    // into the screen underneath while the game is still covering it.
    if (event.key === 'Tab' && this.fullscreen) {
      event.preventDefault();
      return;
    }

    if (this.game.gameOver) {
      if (event.key === 'r' || event.key === 'R') {
        event.preventDefault();
        this.restart();
      }
      return;
    }

    const direction = ARROWS[event.key];
    if (!direction) return;
    // Arrows scroll the page otherwise, which moves the board under the snake.
    event.preventDefault();
    this.game = turn(this.game, direction);
  };

  /**
   * The specks. Every one of them is drawn in every state — still specks, in
   * the same palette, when motion is unwelcome, so the board keeps its texture
   * and nothing on it moves but the snake.
   *
   * Whether they drift is entirely the stylesheet's: `reduced-motion` is a
   * reflected attribute and `:host([reduced-motion]) .particle` turns the
   * animation off, with the media query behind it for a preference nothing
   * here was told about. Withholding the timing from the inline style as well
   * would be a second mechanism saying the same thing, and the one that goes
   * stale first.
   */
  private renderParticles() {
    return this.particles.map(
      (p) => html`<span
        class="particle"
        style=${styleMap({
          left: p.left,
          '--rest': p.rest,
          '--tint': p.tint,
          '--brightness': p.brightness,
          '--duration': p.duration,
          '--delay': p.delay,
        })}
        >${p.glyph}</span
      >`,
    );
  }

  private renderSnake() {
    const phase = this.reducedMotion ? 0 : this.phase;

    return this.game.body.map((segment, index) => {
      // The head is the bright mark the TUI gives it; the body runs along the
      // ramp from wherever the phase currently sits.
      const color =
        index === 0
          ? 'var(--nc-color-arcade-ink)'
          : gradientColor(phase + index * PHASE_PER_SEGMENT);

      return html`<div
        class="cell segment"
        data-segment=${index}
        style=${styleMap({
          '--x': String(segment.x),
          '--y': String(segment.y),
          '--tint': color,
        })}
      ></div>`;
    });
  }

  render() {
    const { food, score, gameOver, boardWidth, boardHeight } = this.game;

    return html`
      <div
        class="frame"
        part="frame"
        style=${styleMap({ '--cols': String(boardWidth), '--rows': String(boardHeight) })}
      >
        <div class="chrome">
          <span class="title">$ Snake $</span>
          <!-- The one live region. The end of the game is announced through
               it rather than by the panel below, which is inside the board
               and so behind aria-hidden. -->
          <span class="score" role="status">
            Score <wc-money variant="plain" .amount=${score}></wc-money>
            ${gameOver
              ? html`<span class="visually-hidden"
                  >— game over. Press R to play again, or Escape to close.</span
                >`
              : nothing}
          </span>
        </div>

        <div class="board" part="board" aria-hidden="true">
          ${this.renderParticles()}
          <div
            class="cell food"
            style=${styleMap({ '--x': String(food.x), '--y': String(food.y) })}
          ></div>
          ${this.renderSnake()}
          ${gameOver
            ? html`
                <div class="over" part="game-over">
                  <h2>Game over</h2>
                  <p>
                    Final score <wc-money variant="plain" .amount=${score}></wc-money>
                  </p>
                  <p><kbd>R</kbd> play again · <kbd>Esc</kbd> close</p>
                </div>
              `
            : nothing}
        </div>

        <p class="hint">
          <kbd>↑</kbd> <kbd>↓</kbd> <kbd>←</kbd> <kbd>→</kbd> move ·
          <kbd>R</kbd> restart · <kbd>Esc</kbd> close
        </p>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'wc-snake': WcSnake;
  }
}
