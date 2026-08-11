/**
 * What the app looks like on paper.
 *
 * A printed report is the artifact an accountant keeps, so the page has to be
 * the report and nothing else: no sidebar, no toolbars, no export buttons, no
 * screen-sized colour.
 *
 * This sheet does the half of that a document-level stylesheet can actually
 * do: redefine the tokens at `:root`. Custom properties inherit through shadow
 * boundaries, which is the one thing that reaches inside every `wc-*` element
 * at once — a print sheet cannot select into a shadow root, but it can change
 * what the shadow root reads.
 *
 * Hiding the chrome is the other half and is *not* here, because a rule that
 * hides an element has to live in the tree that element is in. `wc-app-shell`
 * hides its own header, banner and sidebar slot; `wc-nav-sidebar`, `wc-toast`,
 * `wc-export-links`, `wc-period-nav` and `wc-register-toolbar` each hide
 * themselves; and `controlsCss` carries the `wa-button`/`wa-select` and table
 * rules into every root that hosts a control. What stays here is what applies
 * to content in the document itself.
 */
export declare const printCss: import("lit").CSSResult;
//# sourceMappingURL=print.d.ts.map