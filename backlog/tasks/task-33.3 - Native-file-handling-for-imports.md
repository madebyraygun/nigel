---
id: TASK-33.3
title: Native file handling for imports
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-06 16:29'
updated_date: '2026-08-19 16:59'
labels:
  - tauri
  - frontend
dependencies:
  - TASK-33.2
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Replace the browser upload dance with native affordances when running in the desktop shell: file-open dialog scoped to CSV/XLSX and drag-and-drop onto the window, passing paths straight to the path-based import pipeline. The web upload flow remains for remote mode.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Import works via native file dialog and window drag-and-drop in the desktop app
- [ ] #2 Preview and confirm behavior matches the web import flow
- [ ] #3 Remote mode falls back to the upload flow
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Spec: docs/superpowers/specs/2026-08-19-native-import-handling-design.md. Plan: docs/superpowers/plans/2026-08-19-native-import-handling.md (branch feat/native-imports). Staging commands in nigel-desktop reuse uploads::store so preview/confirm are shared with the web flow; ImportSource seam in the API client; wc-dropzone native mode; Tauri drag-drop events through the client seam.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Manual verification (desktop shell)

CI cannot open a native dialog or synthesize an OS drag, so the shell's half of this
task is proved by hand. On macOS, run through the following in order:

1. Build the web assets (`cd web && npm run build`), then launch the shell with
   `cargo run` from `crates/nigel-desktop`. The desktop crate is excluded from the
   root workspace, so `-p nigel-desktop` does not resolve from the repository root.
2. On the import screen, click the drop well. The native file dialog opens, its
   filter offers only the spreadsheet extensions the importers accept, and the
   chosen statement comes back onto the screen labelled with its filename and size.
3. Reopen the dialog and cancel it. The screen keeps whatever it already had and
   surfaces no error.
4. Drag a statement out of Finder and over the window — anywhere on it, not only
   the well. The well highlights while the drag is over the window, drops the
   highlight when the drag leaves, and releasing the file names it on the screen.
5. Drag in a file the importers cannot read. The well says so and nothing is staged.
6. Preview and confirm the staged file, then open the import history and check the
   run is listed under the filename that was dropped.
7. Leave a preview sitting open past the spool's stale-sweep window, then confirm
   it. The file is re-staged from the retained path and the confirm succeeds instead
   of failing with an expired upload.

What is operator-only is the shell's own edge: the native dialog in steps 2 and 3,
a real OS drag reaching the window in steps 4 and 5, and the end-to-end confirm in
the packaged app in step 6.

Everything those steps drive once the event arrives is pinned automatically. The
reduction of Tauri's four drag events to over/leave/drop is covered in
`web/apps/app/src/api/desktop-client.test.ts`; the screen's highlight response and
its refusal of an unreadable drop are covered in
`web/apps/app/src/screens/import.test.ts`. Step 7 in particular has two pins — the
retained-path re-stage test in `import.test.ts` and
`a_staged_id_the_spool_has_forgotten_is_the_upload_expired_404` in
`crates/nigel-desktop/tests/desktop_imports.rs` — so the manual pass there is
confirming the timing in a real session, not the logic.
<!-- SECTION:NOTES:END -->
