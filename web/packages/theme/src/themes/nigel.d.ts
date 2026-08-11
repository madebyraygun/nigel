/**
 * The composed token sheet — the one `scripts/build-css.js` emits as
 * `dist/css/nigel.css` and the document loads.
 *
 * Order is load bearing: light defaults first, then the dark overrides (whose
 * higher-specificity selectors have to come later to win), then the print
 * sheet last of all — it has to win over everything, including dark mode.
 *
 * There are deliberately no `::part()` rules here. A document-level rule
 * reaches one shadow boundary down, and every `wa-*` primitive in this app is
 * several boundaries deep, so those rules ship in `controlsCss` and are
 * adopted by the components that host the primitives. What this sheet delivers
 * into a component is custom properties, which inherit through boundaries on
 * their own.
 */
export declare const nigelTheme: import("lit").CSSResult;
//# sourceMappingURL=nigel.d.ts.map