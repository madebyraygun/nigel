---
id: TASK-95
title: 'Web UI: reorganize the database password panel in settings'
status: Done
assignee:
  - '@claude'
created_date: '2026-08-12 17:51'
updated_date: '2026-08-14 18:29'
labels:
  - web
  - ui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Database password panel runs the change-password form and the remove-password form together with no separation: the 'Change password' button is followed immediately by the remove form's 'Current password' label (screenshot), so the two forms read as one broken form with two Current-password fields. Separate the operations visually (sub-sections, dividers, or tabs), make each form's scope unambiguous, and give Remove password the destructive treatment consistent with the rest of the app.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Change password and Remove password are visually distinct operations with unambiguous field ownership
- [x] #2 Remove password reads as a destructive action and confirms accordingly
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. wc-password-form gains the operation frame: a fieldset+legend naming the operation (h3 inside the legend so it is both a form group and a heading), a per-mode description, and a destructive treatment for mode=remove. Field ownership becomes native rather than positional.
2. Per-mode heading/description defaults live in the component; `heading`/`description` properties override.
3. Preview gains an `encrypted-panel` state — change and remove stacked as the settings screen stacks them, which is the state the bug is about — plus a `remove` state check; describePreviewA11y covers all of them.
4. settings.ts stacks the two forms in a gapped wrapper and routes `passwordError` to the form whose submit failed (today a failed remove renders under the change form, a second ownership bug).
5. Tests: component tests for the legend, the destructive marking and the overrides; screen tests for error routing and for a confirmed remove.
6. Verify: npm ci, build, test, lint, typecheck from web/.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `wc-password-form` now renders one operation as a fieldset/legend group: the legend carries an `h3` with the operation name, a per-mode sentence sits under it, and `mode=remove` gets a danger border, danger heading and the danger submit it already had. The two Current-password fields now belong to named groups rather than to positions.
- Per-mode heading/description defaults live in the component; `heading`/`description` properties override.
- Settings stacks the forms in a gapped `.operations` grid and routes `passwordError` through `passwordErrorMode`, so a failed remove no longer renders under the change form.
- Remove was already behind `confirmDialog({ variant: danger })`; that is now pinned by a test, as is change needing no confirmation.

Review round on PR #6 — six findings, all fixed:
1. The non-ApiError fallback was one hardcoded change-worded sentence, so a failed remove read as a failed change. Now `PASSWORD_FAILURE`, one sentence per mode.
2/3. `passwordError` + `passwordErrorMode` collapsed into one `passwordFailure { mode, message }`, and the render binding no longer drops a failure whose operation left the screen: `failureSlot` falls back to the first rendered form, since a stale-but-visible failure beats a vanished one.
4. Dropped the unused `heading`/`description` override props with their preview state and test — the per-mode defaults cover the one consumer.
5. The `encrypted database` preview state now spaces its two forms with `var(--wa-space-l, 16px)`, the token the settings screen uses.
6. Deleted the unreachable `.description { max-width: 68ch }` — the form's 24rem cap binds first.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The database password panel ran two forms as one flat stack, so the Change password button was followed straight by the remove form's Current password label and the panel read as a single broken form with two Current-password fields.

Structure: `wc-password-form` is now explicitly *one* operation rather than a field group. Each instance renders a `fieldset` under a `legend` naming the operation, with the name as an `h3` inside the legend (which `legend`'s content model allows), a per-mode sentence saying what the operation does, and its own message line and submit. That puts the boundary in the component instead of in whichever screen stacks it, and it makes field ownership native: two "Current password" inputs on one screen are told apart by their group, not by their position, which is the part a screen reader could not convey before.

Destructive treatment: `mode="remove"` carries a danger border, a danger heading and description, and the danger submit it already had; the settings screen keeps it behind `confirmDialog({ variant: "danger" })`. Change is deliberately not confirmed — it is reversible by changing it back.

Also fixed while in there: the screen bound `passwordError` to the first form only, so a failed *remove* rendered its error under the *change* form — a second ownership bug and the one most likely to make someone retype the wrong password. Errors are now filed against the operation that produced them (`passwordErrorMode`).

Tests: four new component tests (legend/ownership, per-mode descriptions, heading and description overrides, destructive presentation) and four new screen tests (danger confirmation on remove, no confirmation on change, error routing, plus the existing confirm test kept). The preview gains an `encrypted database` state showing change and remove stacked — the exact arrangement the bug was about — and an `overridden wording` state; `describePreviewA11y` covers all seven with zero violations.

Verification from web/: npm ci, npm run build, npm test (theme 188, ui 1064, app 763 — all passing, no Unhandled Errors block), npm run lint, npm run typecheck all clean.

Review round (PR #6): the non-ApiError fallback now names the operation that failed, rather than saying "Could not change the password." under the Remove form — the very confusion this task removes. The two password-error fields became one `passwordFailure { mode, message }`, so the message and its operation can only be written together, and the render binding falls back to the first form when the failure names an operation no longer on screen (another session encrypting or decrypting the books swaps the forms out from under it): a stale-but-visible failure beats a vanished one. The unused heading/description overrides, their preview state and their test are gone; the stacked preview state now uses the same spacing token the screen does; and an unreachable max-width was deleted.
<!-- SECTION:FINAL_SUMMARY:END -->
