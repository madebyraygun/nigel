---
id: TASK-33.3
title: Native file handling for imports
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-06 16:29'
updated_date: '2026-08-19 14:59'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Manual verification (desktop shell)

CI cannot open a native dialog or synthesize an OS drag, so the shell's half of this
task is proved by hand. On macOS, run through the following in order:

1. Build the web assets (`cd web && npm run build`), then launch the shell with
   `cargo run -p nigel-desktop` from the repository root.
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

Steps 4 through 7 have no automated coverage in this plan — native drag events, the
unreadable-file path through the shell, the end-to-end confirm in the packaged app,
and re-staging after expiry are all operator-verified only.
<!-- SECTION:NOTES:END -->
