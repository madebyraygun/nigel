import { describe, it, expect } from 'vitest';
import {
  DEFAULT_SCREEN,
  SCREENS,
  isScreenId,
  navItems,
  screenDef,
  type ScreenId,
} from './registry.js';
import { ICON_TAGS } from '@nigel/ui';
import { FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { ScreenContext } from './context.js';

const ctx: ScreenContext = {
  client: new FakeApiClient(),
  params: new URLSearchParams(),
  navigate: () => {},
};

/** In the sidebar's order: the top group, Statements, Billing, Setup, the footer. */
const ALL: ScreenId[] = [
  'dashboard',
  'register',
  'review',
  'reports',
  'import',
  'reconcile',
  'undo',
  'clients',
  'invoices',
  'documents',
  'accounts',
  'categories',
  'rules',
  'settings',
  'setup',
  'unlock',
];

/** Screens the boot phase reaches, which the sidebar never offers. */
const GATES: ScreenId[] = ['setup', 'unlock'];

describe('screen registry', () => {
  it('covers every screen the epic plans', () => {
    expect([...SCREENS.keys()].sort()).toEqual([...ALL].sort());
  });

  it.each(ALL)('%s resolves to a definition with its own id', (id) => {
    const def = screenDef(id);
    expect(def.id).toBe(id);
    expect(def.title.length).toBeGreaterThan(0);
    expect(def.navLabel.length).toBeGreaterThan(0);
  });

  it('names only icons the library actually registers', () => {
    for (const def of SCREENS.values()) {
      expect(ICON_TAGS, `${def.id} icon`).toContain(def.icon);
    }
  });

  it('gives every screen a distinct title', () => {
    const titles = [...SCREENS.values()].map((d) => d.title);
    expect(new Set(titles).size).toBe(titles.length);
  });

  it('renders a template for each screen', () => {
    for (const def of SCREENS.values()) {
      expect(def.render(ctx), `${def.id} render`).toBeTruthy();
    }
  });

  it('defaults to the dashboard', () => {
    expect(DEFAULT_SCREEN).toBe('dashboard');
    expect(SCREENS.has(DEFAULT_SCREEN)).toBe(true);
  });

  describe('isScreenId', () => {
    it.each(ALL)('accepts %s', (id) => expect(isScreenId(id)).toBe(true));

    it.each(['', 'nope', 'DASHBOARD', '../etc/passwd'])(
      'rejects %s',
      (value) => expect(isScreenId(value)).toBe(false),
    );
  });

  describe('navItems', () => {
    it('keeps the gates out of the sidebar', () => {
      // Both are reached by boot phase, never by choice.
      const ids = navItems().map((i) => i.id);
      expect(ids).not.toContain('unlock');
      expect(ids).not.toContain('setup');
    });

    it('lists every screen but the gates, in registry order', () => {
      expect(navItems().map((i) => i.id)).toEqual(ALL.filter((id) => !GATES.includes(id)));
    });

    it('carries the label and icon from the registry', () => {
      const item = navItems().find((i) => i.id === 'register');
      expect(item).toEqual({
        id: 'register',
        label: 'Register',
        icon: 'wc-icon-register',
        disabled: false,
        group: undefined,
        pinned: false,
        count: undefined,
      });
    });

    it('groups the sidebar the way the design does', () => {
      const groups = new Map<string, string[]>();
      for (const item of navItems()) {
        const key = item.pinned ? 'footer' : (item.group ?? '');
        groups.set(key, [...(groups.get(key) ?? []), item.id]);
      }
      expect(Object.fromEntries(groups)).toEqual({
        '': ['dashboard', 'register', 'review', 'reports'],
        Statements: ['import', 'reconcile', 'undo'],
        Billing: ['clients', 'invoices', 'documents'],
        Setup: ['accounts', 'categories', 'rules'],
        footer: ['settings'],
      });
    });

    it('puts a count beside the screen it is given for', () => {
      const items = navItems({ counts: { review: 12 } });
      expect(items.find((i) => i.id === 'review')?.count).toBe(12);
      expect(items.find((i) => i.id === 'register')?.count).toBeUndefined();
    });

    it('can disable everything at once for the locked state', () => {
      expect(navItems({ disabled: true }).every((i) => i.disabled)).toBe(true);
    });
  });

  it('throws on an unknown screen rather than rendering nothing', () => {
    expect(() => screenDef('nope' as ScreenId)).toThrow(/unknown screen/);
  });
});
