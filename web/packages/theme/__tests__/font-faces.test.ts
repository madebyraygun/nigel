import { describe, it, expect } from 'vitest';
import { fontFacesCss, BUNDLED_FONT_WEIGHTS, FONT_FAMILY } from '../src/tokens/font-faces.js';
import { typographyCss } from '../src/tokens/typography.js';
import { nigelTheme } from '../src/themes/nigel.js';

const text = fontFacesCss.cssText;
const composed = nigelTheme.cssText;

function faces(): string[] {
  return text.split('@font-face').slice(1);
}

describe('fontFacesCss', () => {
  it('declares one face per bundled weight', () => {
    expect(faces()).toHaveLength(BUNDLED_FONT_WEIGHTS.length);
    for (const weight of BUNDLED_FONT_WEIGHTS) {
      expect(text).toMatch(new RegExp(`font-weight:\\s*${weight}`));
    }
  });

  it('bundles exactly the weights the typography tokens ask for', () => {
    // 400/500/600 are --wa-font-weight-normal/medium/bold. A weight the tokens
    // name but nothing bundles is a browser-synthesised fake; a weight bundled
    // and never named is dead bytes in the binary.
    expect([...BUNDLED_FONT_WEIGHTS]).toEqual([400, 500, 600]);
    for (const weight of BUNDLED_FONT_WEIGHTS) {
      expect(typographyCss.cssText).toMatch(new RegExp(`font-weight-\\w+:\\s*${weight}`));
    }
  });

  it('names the family the typography token asks for', () => {
    expect(text).toContain(`font-family: '${FONT_FAMILY}'`);
    expect(typographyCss.cssText).toContain(`'${FONT_FAMILY}'`);
  });

  it('sets font-display: swap on every face', () => {
    // Not `optional`: on a slow first paint that can settle on the fallback
    // permanently, so the brand face would sometimes simply not appear. Not
    // `block` either — the bytes come from the same binary over loopback.
    for (const face of faces()) {
      expect(face).toMatch(/font-display:\s*swap/);
    }
  });

  it('sources every face from a relative woff2 — no CDN, no absolute path', () => {
    const srcs = [...text.matchAll(/src:\s*url\((['"])(.*?)\1\)/g)].map((m) => m[2]);
    expect(srcs).toHaveLength(BUNDLED_FONT_WEIGHTS.length);
    for (const src of srcs) {
      expect(src).toMatch(/^\.\.\/fonts\/[\w-]+\.woff2$/);
    }
    expect(text).toContain("format('woff2')");
  });

  it('leaves no remote reference anywhere in the composed sheet', () => {
    // The binary serves everything over loopback. A single absolute URL here
    // would make an offline app render in a system face, silently.
    expect(composed).not.toMatch(/url\(\s*['"]?(https?:)?\/\//);
    expect(composed).not.toContain('@import');
    expect(composed).not.toContain('fonts.googleapis');
    expect(composed).not.toContain('fonts.gstatic');
  });

  it('composes into nigelTheme, ahead of the tokens that use the family', () => {
    expect(composed).toContain(text);
    expect(composed.indexOf('@font-face')).toBeLessThan(
      composed.indexOf('--wa-font-family-sans'),
    );
  });
});
