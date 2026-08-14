---
id: TASK-92
title: 'Web UI: a way to cancel an in-progress import'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-12 17:50'
updated_date: '2026-08-14 05:08'
labels:
  - web
  - ui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The import flow (choose file, preview, confirm) has no cancel affordance. A user who previews the wrong file or picks the wrong account should be able to abandon the import cleanly: clear the chosen file, preview state and any spooled upload, and return the screen to its initial state without confirming.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A cancel control abandons the current import at any pre-confirm stage and resets the screen
- [x] #2 Cancelling after a preview leaves no orphaned upload in the server spool beyond the existing purge
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Decide the spool question: cancel relies on the existing hourly purge rather than a new DELETE endpoint. The spool is documented non-authoritative state ("an upload is a file on disk with an mtime"), a browser-issued delete is best-effort and unobservable (a closed tab, a dead network and a crashed browser all abandon uploads without one), and the screen already discards an uploadId without telling the server when the dropzone is cleared or a new file chosen. The purge has to be correct on its own either way.
2. import-data.ts: add the two pure functions cancel needs — initialImportForm(accounts) (EMPTY_IMPORT_FORM plus the single-account preselect, so "initial state" has one definition) and sameImportForm(a, b) (field compare, so the screen can tell a touched form from an untouched one).
3. import.ts: handleCancel resets file, filename, filesize, uploadId, preview, result, errors, the toast set and the form; a `dirty` getter decides whether the control is offered at all; render Cancel beside whichever action would advance the import — Preview while there is no preview, Import once there is one, and on the duplicate-file panel, which has no advancing action.
4. Cancel is disabled while a request is in flight, like Preview and Import: the api client takes no AbortSignal, and an aborted upload would leave a spooled file the browser can no longer name.
5. Tests: import-data.test.ts for the two pure functions; import.test.ts for the control appearing/hiding, the reset at each pre-confirm stage, the account being cleared, and no upload being re-attached after a cancel.
6. wc-panel.preview.ts gains a two-action state (secondary + primary), which describePreviewA11y covers automatically.
7. Docs: CLAUDE.md SPA import bullet records the control and the purge decision.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Cancel renders beside whichever action would advance the import: the Preview button while there is no preview, the confirm once there is one, and the duplicate-file panel, which has no advancing action. One cancel on screen at a time.
- It is offered only when there is something to abandon. `initialImportForm(accounts)` is the single definition of the untouched screen (including the single-account preselect), so filling that account in cannot read as an import somebody started.
- The reset is fuller than the "Import another" one: the form goes back too, because "picked the wrong account" is half of what cancel is for. `discard()` is the shared half.
- Disabled while a request is in flight, like Preview and Import. `ApiClient` takes no `AbortSignal` and adding one would be a wide interface change; more to the point an aborted upload leaves a spooled file the browser can no longer name, which is strictly worse for AC #2 than waiting out a dry run.
- **Spool decision: no delete endpoint. Cancel tells the server nothing.** `uploads.rs` documents the spool as non-authoritative ("an upload is a file on disk with an mtime"), the sweep runs at startup and before every upload, and a closed tab, a dead network or a quit browser abandons an upload with no message at all — so the sweep has to be correct on its own and a DELETE would only ever cover the case where somebody stayed to click it. It would also be fire-and-forget (cancel must reset whatever the server answers), i.e. an unobservable request. The screen already discards a cached `uploadId` without telling the server when the dropzone is cleared or a new file is chosen; cancel is the same act. Confirm still deletes eagerly server-side, as a side effect of work the server was already doing.
- No Rust change, so the cargo matrix was not run. No new API surface, so docs/api.md is unchanged.

**Review round (PR #7, four findings) — all fixed.**
- *Correctness:* `load()` replaced the whole form with `initialImportForm()` whenever no account was chosen, wiping a format/mapping/profile name typed while the lists were still in flight. Only the account is patched now, and only while none has been named; the baseline moves with it, since a preselection is not work somebody did.
- *Correctness:* `dirty` compared against `initialImportForm()` rather than the screen's current state, so after "Import another" (which deliberately keeps account and format) the screen read as dirty on arrival and Cancel offered to wipe exactly what the reset preserved. A `baseline` form is now set at load and at reset, `dirty` measures against it, and cancel returns to it — an account nobody touched during this attempt survives the attempt being abandoned. Cancel still clears an account chosen *for* this attempt.
- *Cleanup:* `handleFileClear` was `discard()` written out again; it calls it.
- *Cleanup:* `sameImportForm` hand-enumerated seven fields. It now flattens through `FormLeaves`, a record type derived from `keyof ImportFormValue` and `keyof GenericCsvMapping`, so a new field is a missing-property error. Verified by adding a probe field to `ImportFormValue` and watching `tsc` fail on `leaves()` (TS2322), then reverting.
- Three tests written red first (all three failed against the old code), then green: keeps edits made while the account list was still loading; offers no cancel on the screen a finished import resets to; cancels back to what the reset kept, not to an empty form.
- Re-verified from web/: build, test (188 + 1059 + 778, no unhandled errors), lint, typecheck — all clean. Guardrail hook printed `OK: no identity strings found` on the commit.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The web import flow (choose → preview → confirm) gains a Cancel control, so a wrong file or a wrong account can be abandoned without confirming anything.

Changes:
- `screens/import.ts`: `handleCancel` clears the file, the cached `uploadId`, the preview, the errors and the form; a `dirty` getter decides whether the control is offered at all; `renderCancel()` places it beside whichever action would carry the import forward — Preview, then the confirm, and the duplicate-file panel, which has none. It is disabled mid-request, as Preview and Import already are.
- `screens/import-data.ts`: `initialImportForm(accounts)` (the untouched screen, single-account preselect included) and `sameImportForm(a, b)`, so "nothing to abandon" has one definition and the preselect is not mistaken for started work. `load()` now seeds the form through it.
- `wc-panel.preview.ts` gains a two-action state (secondary + primary), which `describePreviewA11y` covers.
- CLAUDE.md's SPA import bullet records the control and the spool decision.

Spool: cancel sends nothing and no endpoint was added. The spool is documented non-authoritative state — a file on disk with an mtime, swept at startup and before every upload — and a closed tab, a dead network or a quit browser abandons an upload with no message at all, so the sweep has to be right on its own; a delete call would only cover the case where somebody stayed to click, and it would have to be fire-and-forget because cancel must reset whatever the server answers. Confirm still deletes eagerly, as a side effect of work the server was already doing.

Tests: 12 new (10 screen, 2 pure-function groups). Cancel appears only when there is something to abandon, resets at each pre-confirm stage, restores the preselected account, clears a routed error, gets out of the duplicate-file dead end, refuses to fire mid-request, and — for AC #2 — issues no request of its own and never names the abandoned `uploadId` again, leaving it to the hourly purge.

Verification from web/: npm ci, npm run build, npm test (1059 + 775 + 188 passing, no unhandled errors), npm run lint, npm run typecheck — all clean. No Rust change, so no cargo run.
<!-- SECTION:FINAL_SUMMARY:END -->
