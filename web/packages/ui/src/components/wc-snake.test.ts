import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-snake.js';
import { WcSnake } from './wc-snake.js';
import type { WcMoney } from './wc-money.js';
import { NIGEL_PALETTE, gradientColor } from '@nigel/theme';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import { styleText } from '../../preview/controls-suite.js';
import preview, { board } from './wc-snake.preview.js';

async function mount(props: Partial<WcSnake> = {}): Promise<WcSnake> {
  const el = document.createElement('wc-snake');
  Object.assign(el, { paused: true, game: board() }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const press = (el: WcSnake, key: string, init: KeyboardEventInit = {}) => {
  const event = new KeyboardEvent('keydown', {
    key,
    bubbles: true,
    cancelable: true,
    ...init,
  });
  el.dispatchEvent(event);
  return event;
};

const segments = (el: WcSnake) =>
  [...(el.shadowRoot?.querySelectorAll<HTMLElement>('.segment') ?? [])];

/** What a money figure inside the board actually reads, one shadow root down. */
const moneyIn = (el: WcSnake, selector: string) =>
  el.shadowRoot?.querySelector<WcMoney>(`${selector} wc-money`)?.formatted;

/** Send the page to a background tab and back. jsdom has no page lifecycle. */
function hide(hidden: boolean): void {
  Object.defineProperty(document, 'hidden', { value: hidden, configurable: true });
  document.dispatchEvent(new Event('visibilitychange'));
}

describe('wc-snake', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.useRealTimers();
  });

  it('announces itself as a labelled dialog that can hold focus', async () => {
    const el = await mount();
    expect(el.getAttribute('role')).toBe('dialog');
    expect(el.getAttribute('aria-label')).toBe('Snake');
    expect(el.getAttribute('tabindex')).toBe('-1');
  });

  it('claims modality only as the overlay', async () => {
    const inline = await mount();
    expect(inline.hasAttribute('aria-modal')).toBe(false);

    const overlay = await mount({ fullscreen: true });
    expect(overlay.getAttribute('aria-modal')).toBe('true');
  });

  it('draws one cell per segment plus the food', async () => {
    const el = await mount();
    expect(segments(el)).toHaveLength(3);
    expect(el.shadowRoot?.querySelector('.food')).toBeTruthy();
  });

  it('places each segment on its own cell', async () => {
    const el = await mount();
    const [head] = segments(el);
    expect(head.style.getPropertyValue('--x')).toBe('20');
    expect(head.style.getPropertyValue('--y')).toBe('10');
  });

  it('draws the body in the brand gradient and the head in the board ink', async () => {
    const el = await mount({ reducedMotion: true });
    const [head, ...body] = segments(el);

    expect(head.style.getPropertyValue('--tint')).toBe('var(--nc-color-arcade-ink)');
    body.forEach((segment, i) => {
      // Each segment sits one step further along the ramp than the one in
      // front of it, which is the trail `snake.rs` draws.
      expect(segment.style.getPropertyValue('--tint')).toBe(gradientColor((i + 1) * 0.05));
    });
  });

  it('shows the score through the money component the rest of the app uses', async () => {
    const el = await mount({ game: board({ score: 12.5 }) });
    expect(moneyIn(el, '.score')).toBe('$12.50');
  });

  it('steers with the arrow keys and swallows the page scroll', async () => {
    const el = await mount();
    const event = press(el, 'ArrowUp');
    await el.updateComplete;

    expect(el.game.nextDirection).toBe('up');
    expect(event.defaultPrevented).toBe(true);
  });

  it('ignores a reversal, as the TUI does', async () => {
    const el = await mount();
    press(el, 'ArrowLeft');
    await el.updateComplete;
    expect(el.game.nextDirection).toBe('right');
  });

  it('asks the host to close on Escape rather than closing itself', async () => {
    const el = await mount();
    const spy = vi.fn();
    el.addEventListener('nc-snake-exit', spy);

    press(el, 'Escape');

    expect(spy).toHaveBeenCalledOnce();
    expect(el.isConnected).toBe(true);
  });

  it('keeps Tab inside the overlay', async () => {
    const el = await mount({ fullscreen: true });
    expect(press(el, 'Tab').defaultPrevented).toBe(true);
  });

  it('lets Tab through when it is not the overlay', async () => {
    const el = await mount();
    expect(press(el, 'Tab').defaultPrevented).toBe(false);
  });

  /**
   * The game holds the keyboard, not the browser. A chord belongs to whoever
   * bound it — the reload, the tab switch, the word-wise cursor move — and a
   * game that swallows Cmd+R to restart itself has stolen the reload.
   */
  describe('browser chords', () => {
    it.each(['ctrlKey', 'metaKey', 'altKey'] as const)(
      'lets %s+R reload rather than restarting the game',
      async (modifier) => {
        const el = await mount({ game: board({ gameOver: true, score: 9.25 }) });
        const event = press(el, 'r', { [modifier]: true });

        expect(event.defaultPrevented).toBe(false);
        expect(el.game.gameOver).toBe(true);
      },
    );

    it.each(['ctrlKey', 'metaKey', 'altKey'] as const)(
      'lets a %s+arrow chord through instead of steering with it',
      async (modifier) => {
        const el = await mount();
        const event = press(el, 'ArrowUp', { [modifier]: true });
        await el.updateComplete;

        expect(event.defaultPrevented).toBe(false);
        expect(el.game.nextDirection).toBe('right');
      },
    );

    it('does not exit on a modified Escape', async () => {
      const el = await mount();
      const spy = vi.fn();
      el.addEventListener('nc-snake-exit', spy);

      press(el, 'Escape', { ctrlKey: true });

      expect(spy).not.toHaveBeenCalled();
    });
  });

  it('shows the game-over panel with the final score', async () => {
    const el = await mount({ game: board({ gameOver: true, score: 9.25 }) });
    expect(el.shadowRoot?.querySelector('.over')?.textContent).toContain('Game over');
    expect(moneyIn(el, '.over')).toBe('$9.25');
  });

  it('restarts a finished game on R and nothing else', async () => {
    const el = await mount({ game: board({ gameOver: true, score: 9.25 }) });

    press(el, 'ArrowUp');
    await el.updateComplete;
    expect(el.game.gameOver).toBe(true);

    press(el, 'r');
    await el.updateComplete;

    expect(el.game.gameOver).toBe(false);
    expect(el.game.score).toBe(0);
    expect(el.game.body).toHaveLength(3);
    expect(el.shadowRoot?.querySelector('.over')).toBeNull();
  });

  it('takes focus when it opens as the overlay', async () => {
    const el = await mount({ fullscreen: true });
    expect(document.activeElement).toBe(el);
  });

  describe('the clock', () => {
    it('moves the snake on its own while it is playing', async () => {
      vi.useFakeTimers();
      const el = await mount({ paused: false });
      const startX = el.game.body[0].x;

      await vi.advanceTimersByTimeAsync(400);
      await el.updateComplete;

      expect(el.game.body[0].x).toBeGreaterThan(startX);
    });

    it('holds still while paused', async () => {
      vi.useFakeTimers();
      const el = await mount();
      const before = el.game;

      await vi.advanceTimersByTimeAsync(1000);

      expect(el.game).toBe(before);
    });

    it('stops when the game ends', async () => {
      vi.useFakeTimers();
      const el = await mount({ paused: false, game: board({ gameOver: true }) });
      const before = el.game;

      await vi.advanceTimersByTimeAsync(1000);

      expect(el.game).toBe(before);
    });

    /**
     * A background tab throttles timers to about once a minute, so a game left
     * running there is a snake taking single steps into a wall it cannot be
     * steered away from. The player comes back to a Game Over they never had a
     * chance at.
     */
    it('holds still while the tab is hidden, and picks up when it comes back', async () => {
      vi.useFakeTimers();
      const el = await mount({ paused: false });

      hide(true);
      const hidden = el.game;
      await vi.advanceTimersByTimeAsync(1000);
      expect(el.game).toBe(hidden);

      hide(false);
      await el.updateComplete;
      await vi.advanceTimersByTimeAsync(400);
      await el.updateComplete;

      expect(el.game).not.toBe(hidden);
    });

    it('stops when it is removed from the page', async () => {
      vi.useFakeTimers();
      const el = await mount({ paused: false });
      el.remove();
      const before = el.game;

      await vi.advanceTimersByTimeAsync(1000);

      expect(el.game).toBe(before);
    });
  });

  describe('reduced motion', () => {
    it('reflects the preference to an attribute the stylesheet can select', async () => {
      const still = await mount({ reducedMotion: true });
      expect(still.hasAttribute('reduced-motion')).toBe(true);

      const moving = await mount({ reducedMotion: false });
      expect(moving.hasAttribute('reduced-motion')).toBe(false);
    });

    /**
     * The drift is stopped in CSS, in both of the two places that can know
     * about the preference — the attribute this component sets and the media
     * query it may never be told about. jsdom applies neither, so what is
     * assertable is that the rules are shipped.
     */
    it('stops the drift by attribute and by media query', () => {
      const sheet = styleText(WcSnake).replace(/\s+/g, ' ');
      expect(sheet).toContain(':host([reduced-motion]) .particle { animation: none; }');
      expect(sheet).toMatch(
        /@media \(prefers-reduced-motion: reduce\) \{ \.particle \{ animation: none; \}/,
      );
    });

    it('keeps the specks on the board rather than removing them', async () => {
      const el = await mount({ reducedMotion: true });
      expect(el.shadowRoot?.querySelectorAll('.particle').length).toBeGreaterThan(0);
    });

    it('stops the gradient cycling along the snake', async () => {
      vi.useFakeTimers();
      const el = await mount({ paused: false, reducedMotion: true });
      const before = segments(el)[1].style.getPropertyValue('--tint');

      await vi.advanceTimersByTimeAsync(600);
      await el.updateComplete;

      expect(segments(el)[1].style.getPropertyValue('--tint')).toBe(before);
    });

    it('answers the media query when nothing sets the attribute', async () => {
      const matchMedia = vi.fn().mockReturnValue({
        matches: true,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      });
      vi.stubGlobal('matchMedia', matchMedia);

      const el = document.createElement('wc-snake');
      el.paused = true;
      document.body.appendChild(el);
      await el.updateComplete;

      expect(el.reducedMotion).toBe(true);
      vi.unstubAllGlobals();
    });
  });

  it('draws the specks from the shared palette', async () => {
    const el = await mount();
    const specks = [...(el.shadowRoot?.querySelectorAll<HTMLElement>('.particle') ?? [])];
    expect(specks.length).toBeGreaterThan(0);

    for (const speck of specks) {
      expect(NIGEL_PALETTE).toContain(speck.style.getPropertyValue('--tint'));
    }
  });
});

describePreviewA11y(preview);
