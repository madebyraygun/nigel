/**
 * Light surfaces reuse the values the pre-SPA placeholder page used, so the
 * built application is visually continuous with the shell it replaced.
 *
 * The brand, danger, success, warning and info entries are darkened
 * derivations of the `effects.rs` pastels (lavender, pink, mint, yellow,
 * cyan). `__tests__/contrast.test.ts` holds every pairing to WCAG AA.
 */
export declare const colorCss: import("lit").CSSResult;
/**
 * Dark mode by system preference, with `.light-mode` able to opt back out and
 * `.dark-mode` able to force it on regardless of the system setting.
 */
export declare const colorDarkCss: import("lit").CSSResult;
//# sourceMappingURL=color.d.ts.map