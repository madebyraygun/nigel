# Native File Handling for Imports (TASK-33.3) — Design

**Goal:** In the desktop shell, importing a statement uses native affordances — a
file-open dialog scoped to the supported types, and drag-and-drop onto the window —
with the file's path handed to the existing import pipeline. In a browser, the
upload flow is unchanged.

## Facts the design rests on

- The web pipeline is upload → preview → confirm. `POST /api/imports/upload`
  spools the file into `uploads::uploads_dir(db_path)` via `uploads::store` and
  returns `{uploadId, filename, size}`; preview and confirm take the `uploadId`
  (`crates/nigel-core/src/server/routes/imports.rs`). `uploads::sanitize_filename`
  enforces `ALLOWED_EXTENSIONS = ["csv", "xlsx", "xls"]`; the upload route caps the
  body at `uploads::MAX_UPLOAD_BYTES` (25 MiB). All of these are `pub` and reachable
  from `nigel-desktop`.
- In the SPA, file selection lives in `wc-dropzone` (hidden `<input type="file">`
  plus HTML5 drag/drop), which emits `nc-file-select` with a `File`. The import
  screen holds that `File` and uploads lazily: `ensureUpload()` calls
  `client.uploadImport(file)` only when preview or confirm first needs an
  `uploadId`, and retries once on an upload-expired error
  (`web/apps/app/src/screens/import.ts`).
- Screens never detect the desktop shell themselves. The one detection point is
  `createApiClient()` (`web/apps/app/src/api/desktop-client.ts`): if
  `window.__TAURI__.core.invoke` exists it returns `DesktopApiClient`, which
  extends `FetchApiClient` and already reroutes exports through
  `invoke('save_export', …)`. Screens see only the `ApiClient` interface —
  `exportTarget()` returns a discriminated `{kind: 'href' | 'action'}` and the
  screen renders the affordance the client asked for.
- The shell (`crates/nigel-desktop/src/main.rs`) registers
  `tauri_plugin_dialog` and one invoke command, `save::save_export`, which opens
  the dialog Rust-side and does the file I/O in the command. Tauri's window-level
  drag-and-drop is enabled by default, which intercepts HTML5 drag events in the
  webview — the browser-style drop path cannot be assumed to work in the shell.
- TASK-33.2's implementation notes record that imports were never exercised in
  the shell at all.

## Design

### 1. Staging commands in `nigel-desktop`

New module `crates/nigel-desktop/src/imports.rs`, following `save.rs`'s shape
(async `#[tauri::command]`, dialog opened Rust-side, `Result<_, String>`):

- A plain function does the real work so it is testable without a Tauri app:
  `stage_file(path: &Path, uploads_dir: &Path) -> Result<StagedUpload, String>`.
  It takes the path's file name through `uploads::sanitize_filename` (which is
  where the extension allow-list lives), refuses files over
  `uploads::MAX_UPLOAD_BYTES` with a message naming the cap, reads the bytes, and
  calls `uploads::store`. `StagedUpload` serializes camelCase as
  `{uploadId, filename, size, path}` — the same first three fields the upload
  route answers, plus the source path so the SPA can re-stage after expiry.
- `#[tauri::command] stage_import(path: String)` wraps `stage_file` against
  `uploads::uploads_dir(&db::database_path())`. Used by drag-and-drop.
- `#[tauri::command] pick_import_file(app: AppHandle)` opens
  `app.dialog().file()` filtered to the `ALLOWED_EXTENSIONS` list (derive the
  filter from the constant; do not restate the extensions), returns `Ok(None)`
  on cancel, otherwise delegates to the same `stage_file`.
- Both are registered in `generate_handler!` alongside `save_export`.
- Like the upload route, staging calls `uploads::purge_stale` first, so the
  hour-old spool sweep happens on the desktop path too.

No new HTTP route: preview and confirm are reached through the existing API over
the custom scheme with the staged `uploadId`, which is what makes AC#2 (identical
preview/confirm behavior) a property of construction rather than a promise.

### 2. The `ImportSource` seam in the API client

`ApiClient` gains `importSource(): ImportSource`, mirroring `exportTarget()`:

- `FetchApiClient` returns `{kind: 'browser'}` — the screen renders `wc-dropzone`
  exactly as today.
- `DesktopApiClient` returns `{kind: 'native', pick, stagePath, onDragDrop}`:
  - `pick(): Promise<StagedUpload | null>` → `invoke('pick_import_file')`
    (null = cancelled dialog).
  - `stagePath(path: string): Promise<StagedUpload>` → `invoke('stage_import', {path})`.
  - `onDragDrop(handler): () => void` subscribes to Tauri's drag-drop events
    (`__TAURI__` event API, injected the way `invoke` already is so tests can
    fake it) and reports `{type: 'over' | 'leave'}` and
    `{type: 'drop', paths: string[]}`; returns an unsubscribe function.

Screens still never touch `__TAURI__`; everything desktop-shaped enters through
the client, and the constructor keeps taking its externals as injectable options
so vitest never needs a global.

### 3. Import screen behavior in native mode

When `importSource().kind === 'native'`:

- The dropzone's browse action calls `pick()`. A non-null result replaces the
  screen's file state with the staged upload (`uploadId` known immediately;
  `filename`/`size` shown as today).
- While the import screen is connected, it subscribes via `onDragDrop` and
  unsubscribes on disconnect. `over`/`leave` drive the dropzone's highlight;
  `drop` takes the first path whose extension is allowed and stages it via
  `stagePath`; a drop containing no usable file surfaces the same message the
  dropzone's own validation produces. Because the Tauri events are window-level,
  a drop anywhere on the window works while the import screen is showing; a drop
  on other screens does nothing.
- `ensureUpload()` in native mode returns the staged `uploadId`. On an
  upload-expired error the screen re-stages from the retained `path` and retries
  once — the same shape as the web flow's re-upload retry.
- Everything downstream of the `uploadId` — form fields, preview rendering,
  confirm, error routing — is untouched and shared.

In browser mode (`kind: 'browser'`) nothing changes; that is AC#3, since remote
mode is the SPA served from `nigel serve` into a real browser. Known seam for
TASK-33.7/33.8: a future desktop client attached to a *remote* server must not
stage by path (the server cannot see the local disk); `importSource()` is the
single point where that mode will answer `browser` instead of `native`.

### 4. `wc-dropzone` native mode (Component-First)

`wc-dropzone` gains a `native` boolean property:

- When set, the browse button emits `nc-pick-request` instead of clicking the
  hidden input, and the component's own HTML5 drag/drop handlers stand down; a
  public `highlight` property drives the drag-over treatment so the screen can
  reflect Tauri's events.
- The staged filename/size display reuses the existing selected-state rendering.
- The preview (`wc-dropzone.preview.ts`) gains the native states (idle, highlight,
  staged) and `describePreviewA11y` covers them with zero violations, per the
  Component-First workflow. No brand values inline; tokens only.

## Testing

- **Rust (`nigel-desktop`):** unit tests for `stage_file` — happy path stores and
  returns the sanitized name; wrong extension refused with the allow-list message;
  over-cap file refused naming the cap; missing file surfaces the OS error.
  An integration test in `crates/nigel-desktop/tests/` drives the desktop router:
  stage a fixture CSV via `stage_file`, then POST `/api/imports/preview` and
  `/confirm` with the returned `uploadId` — proving the staged id is
  indistinguishable from an uploaded one.
- **Web (vitest):** import screen tests with an injected fake `ImportSource`
  (native pick, drag-drop staging, expired-upload re-stage, drops with no usable
  file); `DesktopApiClient` tests for the two new invokes and the event
  subscription with injected fakes; `wc-dropzone` tests for native mode
  (`nc-pick-request`, highlight, HTML5 handlers inert) plus the preview a11y
  suite.
- **Docs:** `docs/desktop.md` gains an imports section (how staging works, where
  files spool); `docs/api.md` is untouched because no HTTP surface changed.
- Manual verification on macOS by the operator stays on the task as an
  implementation note, as it did for 33.2 — CI cannot drive a native dialog.

## Out of scope

- Remote mode inside the desktop shell (TASK-33.7/33.8).
- Windows/Linux in-person QA (TASK-33.12).
- Any change to the import pipeline itself — formats, mapping, categorization.
