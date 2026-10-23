import { describe, it, expect, afterEach } from 'vitest';
import { activeSection, scrolledToBottom, scrollingAncestor } from './settings-spy.js';

const tops = (...values: number[]) =>
  ['letterhead', 'security', 'location', 'appearance', 'updates'].map((id, i) => ({
    id,
    top: values[i] ?? 9999,
  }));

describe('activeSection', () => {
  it('is the first section before any has reached the threshold', () => {
    expect(activeSection(tops(120, 600, 900), false)).toBe('letterhead');
  });

  it('is the last section whose top has passed the threshold', () => {
    expect(activeSection(tops(-700, -40, 60, 400), false)).toBe('location');
  });

  it('is the last section once the area is scrolled to the bottom', () => {
    expect(activeSection(tops(-1400, -900, -500, 100, 420), true)).toBe('updates');
  });

  it('is nothing when there are no sections', () => {
    expect(activeSection([], false)).toBe('');
  });
});

describe('scrolledToBottom', () => {
  it('counts a fractional stop within a pixel of the end', () => {
    expect(scrolledToBottom({ scrollTop: 499.5, clientHeight: 500, scrollHeight: 1000 })).toBe(
      true,
    );
    expect(scrolledToBottom({ scrollTop: 400, clientHeight: 500, scrollHeight: 1000 })).toBe(
      false,
    );
  });

  it('is never at the bottom of an area that does not scroll', () => {
    expect(scrolledToBottom({ scrollTop: 0, clientHeight: 500, scrollHeight: 500 })).toBe(false);
  });
});

describe('scrollingAncestor', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('walks out of a slot and a shadow root to the area that scrolls', () => {
    const host = document.createElement('div');
    const root = host.attachShadow({ mode: 'open' });
    const main = document.createElement('main');
    main.style.overflowY = 'auto';
    main.appendChild(document.createElement('slot'));
    root.appendChild(main);
    const screen = document.createElement('section');
    host.appendChild(screen);
    document.body.appendChild(host);

    const inner = document.createElement('div');
    screen.attachShadow({ mode: 'open' }).appendChild(inner);

    expect(scrollingAncestor(inner)).toBe(main);
  });

  it('is null when nothing scrolls', () => {
    const el = document.createElement('div');
    document.body.appendChild(el);
    expect(scrollingAncestor(el)).toBeNull();
  });
});
