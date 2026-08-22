# Native File Handling for Imports (TASK-33.3) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In the desktop shell, choosing a statement to import uses native affordances — a file-open dialog filtered to the types the importers read, and drag-and-drop onto the window — with the file's path handed straight to the existing import pipeline. In a browser nothing changes.

**Architecture:** A new `crates/nigel-desktop/src/imports.rs` spools a file the user already has on disk into the same `uploads::` directory a browser upload lands in, and answers with the `uploadId` the API already understands, so preview and confirm are reached over the existing routes with no new HTTP surface. The SPA learns about this through one new seam on `ApiClient` — `importSource()`, mirroring `exportTarget()` — which answers `{kind: 'browser'}` from `FetchApiClient` and `{kind: 'native', pick, stagePath, onDragDrop}` from `DesktopApiClient`; the import screen branches on that discriminant and nothing else in the app ever asks which shell it is running in. `wc-dropzone` gains a `native` mode in which it asks its owner for a pick instead of opening one and stands its own HTML5 handlers down, because Tauri intercepts drag events in the webview before the page sees them.

**Tech Stack:** Rust, Tauri 2 (`tauri` 2.11, `tauri-plugin-dialog` 2.7), axum, tower; TypeScript, Lit 3, vitest, axe.

**Spec:** `docs/superpowers/specs/2026-08-19-native-import-handling-design.md`. That document is the binding authority; this plan implements it.

## Findings that add to the spec

Three things the spec does not mention that the code forces. None of them contradict the design; they are additions the design needs in order to run.

1. **An ACL capability is required for the drag-drop subscription.** `crates/nigel-desktop/` has no `capabilities/` directory, so `tauri-build`'s `parse_capabilities("./capabilities/**/*")` resolves to an empty set. `save_export` works anyway because `Webview::on_message` only ACL-checks a command when it is a plugin command, when the app ships its own ACL manifest, or when the origin is remote — an app command from a local origin is exempt. `__TAURI__.event.listen` is *not* an app command: it dispatches `plugin:event|listen`, which is always checked. So the subscription in §2 of the spec cannot work until `crates/nigel-desktop/capabilities/default.json` grants `core:event:allow-listen` and `core:event:allow-unlisten`. Task 2 adds that file and a test that pins its contents. Adding it does not pull the app's own commands into the ACL: `has_app_acl` comes from an `__app-acl__` entry in the *manifest* map, which is produced by a `permissions/` directory in the crate, not by a capability file. The custom scheme counts as a local origin (`Webview::is_local_url` treats any user-registered `uri_scheme_protocol` as local), so the default local capability context is the right one.
2. **`nigel-desktop` has no `serde` or `tempfile` dependency yet.** `StagedUpload` needs `serde::Serialize` for a `#[tauri::command]` return value, and the `stage_file` unit tests need a temporary directory. Both crates are already in `crates/nigel-desktop/Cargo.lock` transitively, so this is a manifest edit and a lock refresh, not a new download.
3. **The "same message the dropzone's own validation produces"** (spec §3) is currently built inline inside `WcDropzone.reject()` from `this.extensions`. Task 4 lifts it into an exported `unsupportedFileMessage(extensions)` alongside an exported `DEFAULT_EXTENSIONS`, and `reject()` calls it. Without that the screen would restate the sentence and the two copies would drift.

Task decomposition follows the spec's suggested six, with the capability file folded into Task 2 (it is a shell-configuration change and belongs with the commands it enables) and the shared extension list folded into Task 4 (it is a `@nigel/ui` change and belongs with the component that owns the list).

## Verified Tauri facts

Read out of `tauri-2.11.5` and `tauri-plugin-dialog-2.7.2` in the local registry, cross-checked against the Tauri 2 docs.

**Drag-and-drop events reaching the page.** `crates/tauri/src/manager/window.rs` names them and emits them; the window is a `WebviewWindow`, so the emit goes through `manager().emit_to(EventTarget::labeled(label), …)`, and a JS listener registered with the default `EventTarget::Any` matches it (`match_any_or_filter` in `crates/tauri/src/event/listener.rs` returns true for `Any` before consulting the filter). A plain `listen(name, handler)` therefore receives them; `getCurrentWebview().onDragDropEvent()` is a wrapper over the same four.

| Event name | Payload as serialized to the page |
|---|---|
| `tauri://drag-enter` | `{ "paths": string[], "position": { "x": number, "y": number } }` |
| `tauri://drag-over` | `{ "position": { "x": number, "y": number } }` — `paths` is `Option::None` and carries `skip_serializing_if`, so the key is absent, not null |
| `tauri://drag-drop` | `{ "paths": string[], "position": { "x": number, "y": number } }` |
| `tauri://drag-leave` | `null` — the Rust side emits `&()` |

The handler `listen` calls receives the envelope `{ event: string, id: number, payload: T }`, and `listen` resolves to an `UnlistenFn` (`() => void`). `withGlobalTauri: true` is already set in `crates/nigel-desktop/tauri.conf.json`, which exposes `window.__TAURI__.event.listen` next to the `window.__TAURI__.core.invoke` the client already uses. Window-level drag-and-drop is on by default (no `dragDropEnabled: false` in the config), which is exactly why the page's own HTML5 `drop` never fires in the shell.

**The dialog.** `tauri_plugin_dialog::FileDialogBuilder` (from `app.dialog().file()`):

```rust
pub fn add_filter(mut self, name: impl Into<String>, extensions: &[&str]) -> Self
pub fn blocking_pick_file(self) -> Option<FilePath>
```

`extensions` are bare extensions without a leading dot, which is exactly the shape of `uploads::ALLOWED_EXTENSIONS` (`["csv", "xlsx", "xls"]`), so the filter is derived from the constant rather than restated. `FilePath::into_path()` yields `Result<PathBuf, _>`; on desktop the variant is always `FilePath::Path`, because the desktop implementation constructs it through `From<PathBuf>`. `blocking_pick_file` must not run on the main thread, which is why the command is `async` — the same reason `save::save_export` is.

## Global Constraints

- **Public repository — no real book data**, in any file, test fixture, doc or commit message. The fictional cast only — Acme, Cedar Systems, Juniper Labs, Harbor & Vale, Globex, Initech — with invented amounts. `./scripts/check-no-real-data.sh --staged` runs from the pre-commit hook; judge it by its **exit status**, never by grepping its output.
- **Rust tests run serially:** `cargo test -- --test-threads=1`. The DB password is a process global. `crates/nigel-desktop` is its own Cargo workspace, so its tests run from `crates/nigel-desktop/`, never from the root.
- **`cargo fmt --check` must pass**, in the desktop crate's own directory as well as at the root. CI runs it first and a failure there fails the build. `cargo clippy --all-targets -- -D warnings` runs alongside it.
- **Web tests are `npm test` from `web/`**, with `npm run lint` and `npm run typecheck` beside them.
- **Component-First:** a component change ships with its `.preview.ts` states and `describePreviewA11y(preview)` passing with **zero violations**. Tokens from `@nigel/theme` only — no inline brand values, and no styling logic for primitives in `web/apps/app/`.
- **Web Awesome primitives are cherry-picked** (`@awesome.me/webawesome/dist/components/<x>/<x>.js`), never the autoloader; any file importing a `wa-*` module adopts `controlsCss` in its `static styles`, which `controls-adoption.test.ts` enforces.
- **Screens never spell an endpoint and never touch `__TAURI__`.** `web/apps/app/src/api/` is the only place either appears; `__tests__/api-seam.test.ts` fails the build otherwise.
- **No provenance comments.** No "added because", "changed in", "was formerly", or edit-justifying notes, in code or in docs. Comments exist for constraints the code cannot show. Rationale goes in the commit message.

---

### Task 1: `stage_file` and `StagedUpload`

**Files:**
- Create: `crates/nigel-desktop/src/imports.rs`
- Modify: `crates/nigel-desktop/src/lib.rs`
- Modify: `crates/nigel-desktop/Cargo.toml`

**Interfaces:**
- Consumes: `nigel_core::server::uploads::{sanitize_filename, store, purge_stale, uploads_dir, ALLOWED_EXTENSIONS, MAX_AGE, MAX_UPLOAD_BYTES}`.
- Produces:
  ```rust
  pub struct StagedUpload {
      pub upload_id: String,   // serializes as uploadId
      pub filename: String,
      pub size: u64,
      pub path: String,        // the source on disk
  }
  pub fn stage_file(path: &Path, uploads_dir: &Path) -> Result<StagedUpload, String>
  ```
  `StagedUpload` derives `Serialize` with `rename_all = "camelCase"`, so the JSON is `{uploadId, filename, size, path}` — the upload route's three fields plus the source path.

The first three fields are the upload route's answer verbatim, which is what lets the SPA treat a staged file and an uploaded one as one type. `path` is the extra a native client has and a browser does not: it is what an expired spool is re-staged from.

Order of checks matters. `sanitize_filename` comes first because it is the cheap one and because the extension allow-list lives there; the size check reads only `metadata`, so a 400 MB file is refused without ever being read; the bytes are read last.

- [ ] **Step 1: Add the two manifest entries**

`crates/nigel-desktop/Cargo.toml` — add to `[dependencies]`:

```toml
serde = { version = "1", features = ["derive"] }
```

and to `[dev-dependencies]`:

```toml
tempfile = "3"
```

- [ ] **Step 2: Write the failing tests**

Create `crates/nigel-desktop/src/imports.rs` containing only the test module and the two items it names, so the file compiles as far as "function not found":

```rust
//! Spooling a file the user already has on disk into the same place a browser
//! upload lands, so the rest of the import pipeline cannot tell them apart.

use std::path::Path;

use serde::Serialize;

use nigel_core::server::uploads;

/// A file the desktop shell has spooled.
///
/// `uploadId`, `filename` and `size` are `POST /api/imports/upload`'s answer,
/// field for field. `path` is the one thing a native client knows and a
/// browser cannot: where the file came from, so an expired spool is re-staged
/// without asking the user for it again.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedUpload {
    pub upload_id: String,
    pub filename: String,
    pub size: u64,
    pub path: String,
}

pub fn stage_file(_path: &Path, _uploads_dir: &Path) -> Result<StagedUpload, String> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A temp directory plus the spool inside it, laid out the way
    /// `uploads_dir` lays it out beside a real database.
    fn workspace() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let spool = dir.path().join("tmp").join("uploads");
        (dir, spool)
    }

    fn statement(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(
            &path,
            "Date,Description,Amount,Running Bal.\n\
             04/01/2025,CEDAR SYSTEMS INVOICE 002,1250.00,0.00\n\
             04/03/2025,JUNIPER LABS HOSTING,-84.50,0.00\n",
        )
        .expect("write statement");
        path
    }

    #[test]
    fn a_statement_is_spooled_under_a_sanitized_name() {
        let (dir, spool) = workspace();
        let source = statement(dir.path(), "Cedar Systems April 2025.csv");

        let staged = stage_file(&source, &spool).expect("stage");

        // The spaces are what `sanitize_filename` reduces; the extension is
        // what the importers dispatch on and must survive untouched.
        assert_eq!(staged.filename, "Cedar_Systems_April_2025.csv");
        assert_eq!(staged.size, std::fs::metadata(&source).unwrap().len());
        assert_eq!(staged.path, source.display().to_string());
        assert_eq!(staged.upload_id.len(), 32);

        let spooled = spool.join(&staged.upload_id).join(&staged.filename);
        assert!(spooled.is_file(), "nothing at {}", spooled.display());
        assert_eq!(
            std::fs::read(&spooled).unwrap(),
            std::fs::read(&source).unwrap()
        );
    }

    #[test]
    fn a_file_type_no_importer_reads_is_refused_by_name() {
        let (dir, spool) = workspace();
        let source = dir.path().join("notes.txt");
        std::fs::write(&source, "not a statement").expect("write");

        let error = stage_file(&source, &spool).expect_err("refused");

        assert!(error.contains("notes.txt"), "{error}");
        for extension in uploads::ALLOWED_EXTENSIONS {
            assert!(error.contains(extension), "{error} omits {extension}");
        }
        assert!(!spool.exists(), "a refused file still made a spool directory");
    }

    #[test]
    fn a_file_over_the_cap_is_refused_before_it_is_read() {
        let (dir, spool) = workspace();
        let source = dir.path().join("huge.csv");
        // Sparse rather than 25 MiB of real bytes: the check reads the length
        // from the metadata, which is the point of doing it before the read.
        let file = std::fs::File::create(&source).expect("create");
        file.set_len(uploads::MAX_UPLOAD_BYTES as u64 + 1)
            .expect("set_len");
        drop(file);

        let error = stage_file(&source, &spool).expect_err("refused");

        assert!(error.contains("25 MB"), "{error}");
        assert!(!spool.exists(), "an oversized file still made a spool directory");
    }

    #[test]
    fn a_path_that_is_not_there_reports_the_os_error() {
        let (dir, spool) = workspace();
        let source = dir.path().join("gone.csv");

        let error = stage_file(&source, &spool).expect_err("refused");

        assert!(error.contains("gone.csv"), "{error}");
        assert!(
            error.to_lowercase().contains("no such file"),
            "the OS error did not survive: {error}"
        );
    }
}
```

Register the module in `crates/nigel-desktop/src/lib.rs`, alphabetically among the existing three:

```rust
pub mod db;
pub mod imports;
pub mod save;
pub mod transport;
```

- [ ] **Step 3: Run them and watch them fail**

```bash
cd /home/dalton/Dev/nigel/wt-imports/crates/nigel-desktop && cargo test --lib imports -- --test-threads=1
```

Expected: four failures, each `panicked at 'not implemented'`, e.g.

```
failures:
    imports::tests::a_file_over_the_cap_is_refused_before_it_is_read
    imports::tests::a_file_type_no_importer_reads_is_refused_by_name
    imports::tests::a_path_that_is_not_there_reports_the_os_error
    imports::tests::a_statement_is_spooled_under_a_sanitized_name

test result: FAILED. 0 passed; 4 failed
```

- [ ] **Step 4: Implement**

Replace the `unimplemented!()` body in `crates/nigel-desktop/src/imports.rs`:

```rust
/// Spool a file that is already on this machine, and answer with the same
/// `uploadId` a browser upload would have produced.
///
/// The checks are ordered by what they cost: the name is decided without
/// touching the file, the size is decided from the metadata, and the bytes are
/// read only once both have passed.
pub fn stage_file(path: &Path, uploads_dir: &Path) -> Result<StagedUpload, String> {
    let raw_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
    let filename = uploads::sanitize_filename(raw_name)?;

    let metadata = std::fs::metadata(path).map_err(|e| read_error(path, &e))?;
    if metadata.len() > uploads::MAX_UPLOAD_BYTES as u64 {
        let limit = uploads::MAX_UPLOAD_BYTES / (1024 * 1024);
        return Err(format!("That file is over the {limit} MB limit."));
    }

    let bytes = std::fs::read(path).map_err(|e| read_error(path, &e))?;

    // The upload route sweeps before every store; the desktop path is the same
    // spool, so it owes the same sweep.
    uploads::purge_stale(uploads_dir, uploads::MAX_AGE);
    let stored = uploads::store(uploads_dir, &filename, &bytes).map_err(|e| e.to_string())?;

    Ok(StagedUpload {
        upload_id: stored.id,
        filename: stored.filename,
        size: stored.size,
        path: path.display().to_string(),
    })
}

fn read_error(path: &Path, error: &std::io::Error) -> String {
    format!("Couldn't read {}: {error}", path.display())
}
```

- [ ] **Step 5: Run them and watch them pass**

```bash
cd /home/dalton/Dev/nigel/wt-imports/crates/nigel-desktop && cargo test --lib imports -- --test-threads=1 && cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

Expected: `test result: ok. 4 passed; 0 failed`, then no output from `fmt` and no warnings from `clippy`.

- [ ] **Step 6: Commit**

```bash
cd /home/dalton/Dev/nigel/wt-imports && git add crates/nigel-desktop/src/imports.rs crates/nigel-desktop/src/lib.rs crates/nigel-desktop/Cargo.toml crates/nigel-desktop/Cargo.lock && git commit -m "Spool a local file into the upload directory from the desktop shell"
```

---

### Task 2: The two Tauri commands, their registration, and the event capability

**Files:**
- Modify: `crates/nigel-desktop/src/imports.rs`
- Modify: `crates/nigel-desktop/src/main.rs`
- Create: `crates/nigel-desktop/capabilities/default.json`
- Create: `crates/nigel-desktop/tests/desktop_imports.rs`

**Interfaces:**
- Consumes: `stage_file`, `StagedUpload` from Task 1; `nigel_desktop::db::database_path`; `tauri_plugin_dialog::DialogExt`.
- Produces: two invoke commands, which are the names the TypeScript in Task 3 calls.
  - `stage_import(path: String) -> Result<StagedUpload, String>` — argument key `path`.
  - `pick_import_file() -> Result<Option<StagedUpload>, String>` — no arguments from the page; `null` means the dialog was cancelled.
  - A capability granting `core:event:allow-listen` and `core:event:allow-unlisten` to the `main` window.

- [ ] **Step 1: Write the failing integration test**

Create `crates/nigel-desktop/tests/desktop_imports.rs`:

```rust
//! A file staged from disk and a file uploaded through the API are the same
//! thing downstream. This drives the desktop router over the staged id and
//! asserts preview and confirm behave exactly as the browser pipeline's do.

use nigel_core::server::state::AppState;
use nigel_core::server::{build_desktop_router, testutil, uploads};

use nigel_desktop::imports::stage_file;

fn router_over(db_path: &std::path::Path) -> axum::Router {
    let token = nigel_core::server::auth::generate_token();
    let state = AppState::new(db_path.to_path_buf(), token);
    build_desktop_router(state, nigel_desktop::trusted_origins())
}

fn post_json(path: &str, body: &str) -> tauri::http::Request<Vec<u8>> {
    tauri::http::Request::builder()
        .method("POST")
        .uri(format!(
            "{}{}",
            nigel_desktop::scheme_url(),
            path.trim_start_matches('/')
        ))
        .header(tauri::http::header::HOST, nigel_desktop::trusted_host())
        .header(tauri::http::header::CONTENT_TYPE, "application/json")
        .body(body.as_bytes().to_vec())
        .expect("build scheme request")
}

/// Three rows a built-in importer can read, in Bank of America's checking
/// layout — the format `seeded_db`'s account is set up for.
fn statement(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("cedar-april-2025.csv");
    std::fs::write(
        &path,
        "Date,Description,Amount,Running Bal.\n\
         04/01/2025,CEDAR SYSTEMS INVOICE 002,1250.00,0.00\n\
         04/03/2025,JUNIPER LABS HOSTING,-84.50,0.00\n\
         04/09/2025,HARBOR AND VALE RETAINER,-119.00,0.00\n",
    )
    .expect("write statement");
    path
}

#[tokio::test]
async fn a_staged_file_previews_and_confirms_like_an_uploaded_one() {
    let (dir, db_path) = testutil::seeded_db();
    let source = statement(dir.path());

    let staged = stage_file(&source, &uploads::uploads_dir(&db_path)).expect("stage");
    assert_eq!(staged.filename, "cedar-april-2025.csv");
    assert_eq!(staged.path, source.display().to_string());

    let router = router_over(&db_path);
    let body = format!(
        r#"{{"uploadId":"{}","account":"BofA Checking"}}"#,
        staged.upload_id
    );

    let response =
        nigel_desktop::transport::answer(router.clone(), post_json("/api/imports/preview", &body))
            .await;
    assert_eq!(
        response.status(),
        200,
        "{}",
        String::from_utf8_lossy(response.body())
    );
    let preview: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(preview["format"], "bofa_checking");
    assert_eq!(preview["imported"], 3);
    assert_eq!(preview["duplicateFile"], false);
    assert_eq!(preview["importId"], serde_json::Value::Null);

    let response =
        nigel_desktop::transport::answer(router, post_json("/api/imports/confirm", &body)).await;
    assert_eq!(
        response.status(),
        200,
        "{}",
        String::from_utf8_lossy(response.body())
    );
    let confirmed: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(confirmed["imported"], 3);
    assert!(confirmed["importId"].is_i64(), "{confirmed}");

    // Confirm consumes the spooled file whichever way it got there.
    assert!(
        uploads::resolve(&uploads::uploads_dir(&db_path), &staged.upload_id).is_none(),
        "the staged file outlived its confirm"
    );
}

#[tokio::test]
async fn a_staged_id_the_spool_has_forgotten_is_the_upload_expired_404() {
    // The screen's re-stage-and-retry hangs off this exact answer, so it is
    // worth pinning that the staged path produces it too.
    let (dir, db_path) = testutil::seeded_db();
    let source = statement(dir.path());

    let staged = stage_file(&source, &uploads::uploads_dir(&db_path)).expect("stage");
    uploads::delete(&uploads::uploads_dir(&db_path), &staged.upload_id);

    let body = format!(
        r#"{{"uploadId":"{}","account":"BofA Checking"}}"#,
        staged.upload_id
    );
    let response = nigel_desktop::transport::answer(
        router_over(&db_path),
        post_json("/api/imports/preview", &body),
    )
    .await;

    assert_eq!(response.status(), 404);
    let error: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(error["error"]["details"]["reason"], "upload_not_found");
}

#[test]
fn the_shell_grants_itself_only_the_event_permissions_drag_and_drop_needs() {
    // `plugin:event|listen` is ACL-checked on every call, unlike an app
    // command, so without a capability the page's drag-drop subscription is
    // rejected and a drop goes nowhere. The list is pinned rather than merely
    // non-empty: `core:default` or a filesystem permission here would hand the
    // page far more than the four events the import screen listens for.
    let source = std::fs::read_to_string("capabilities/default.json")
        .expect("read the capability");
    let capability: serde_json::Value = serde_json::from_str(&source).expect("json");

    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    assert_eq!(
        capability["permissions"],
        serde_json::json!(["core:event:allow-listen", "core:event:allow-unlisten"])
    );
}

#[test]
fn both_staging_commands_are_reachable_from_the_page() {
    // `generate_handler!` is a macro over a literal list, so a command written
    // and never registered compiles, ships, and answers "not allowed" the
    // first time anyone drops a file on the window.
    let main = std::fs::read_to_string("src/main.rs").expect("read main.rs");

    for command in ["imports::stage_import", "imports::pick_import_file"] {
        assert!(
            main.contains(command),
            "{command} is not in the invoke handler"
        );
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

```bash
cd /home/dalton/Dev/nigel/wt-imports/crates/nigel-desktop && cargo test --test desktop_imports -- --test-threads=1
```

Expected: two failures and two passes. The two router tests pass immediately — they exercise Task 1's `stage_file`, which already works, and they exist to prove the staged id is indistinguishable from an uploaded one rather than to drive new code. The two shell tests are the red ones:

```
---- the_shell_grants_itself_only_the_event_permissions_drag_and_drop_needs stdout ----
panicked at 'read the capability: Os { code: 2, kind: NotFound, message: "No such file or directory" }'

---- both_staging_commands_are_reachable_from_the_page stdout ----
panicked at 'imports::stage_import is not in the invoke handler'

test result: FAILED. 2 passed; 2 failed
```

If the first two also fail, the router or the fixture is wrong — fix that before writing any of Step 3.

- [ ] **Step 3: Implement the commands**

Append to `crates/nigel-desktop/src/imports.rs`, above the test module:

```rust
use tauri_plugin_dialog::DialogExt;

use crate::db;

/// Spool a file the user dropped onto the window.
///
/// Async for `stage_file`'s benefit rather than the dialog's: reading and
/// writing a statement is blocking work, and an async `#[tauri::command]` runs
/// it on the async runtime instead of the main thread.
#[tauri::command]
pub async fn stage_import(path: String) -> Result<StagedUpload, String> {
    stage_file(
        Path::new(&path),
        &uploads::uploads_dir(&db::database_path()),
    )
}

/// Open a native file dialog filtered to what the importers read, and spool
/// whatever comes back.
///
/// `Ok(None)` is a cancelled dialog, which is a normal outcome and not an
/// error: the user changed their mind.
#[tauri::command]
pub async fn pick_import_file(app: tauri::AppHandle) -> Result<Option<StagedUpload>, String> {
    let Some(picked) = app
        .dialog()
        .file()
        .add_filter("Statements", &uploads::ALLOWED_EXTENSIONS)
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    stage_file(&path, &uploads::uploads_dir(&db::database_path())).map(Some)
}
```

Register both in `crates/nigel-desktop/src/main.rs` — extend the `use` line and the handler list:

```rust
use nigel_desktop::{db, imports, save, scheme_url, transport, SCHEME};
```

```rust
        .invoke_handler(tauri::generate_handler![
            save::save_export,
            imports::stage_import,
            imports::pick_import_file
        ])
```

Create `crates/nigel-desktop/capabilities/default.json`:

```json
{
  "identifier": "default",
  "description": "The window subscribes to Tauri's drag-and-drop events so a statement dropped anywhere on it reaches the import screen.",
  "windows": ["main"],
  "permissions": ["core:event:allow-listen", "core:event:allow-unlisten"]
}
```

- [ ] **Step 4: Run it and watch it pass**

```bash
cd /home/dalton/Dev/nigel/wt-imports/crates/nigel-desktop && cargo test -- --test-threads=1 && cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

Expected: every existing suite still green plus

```
test a_staged_file_previews_and_confirms_like_an_uploaded_one ... ok
test a_staged_id_the_spool_has_forgotten_is_the_upload_expired_404 ... ok
test both_staging_commands_are_reachable_from_the_page ... ok
test the_shell_grants_itself_only_the_event_permissions_drag_and_drop_needs ... ok

test result: ok. 4 passed; 0 failed
```

- [ ] **Step 5: Commit**

```bash
cd /home/dalton/Dev/nigel/wt-imports && git add crates/nigel-desktop/src/imports.rs crates/nigel-desktop/src/main.rs crates/nigel-desktop/capabilities/default.json crates/nigel-desktop/tests/desktop_imports.rs && git commit -m "Stage imports from a path or a native dialog in the desktop shell"
```

---

### Task 3: The `ImportSource` seam

**Files:**
- Modify: `web/apps/app/src/api/types.ts`
- Modify: `web/apps/app/src/api/client.ts`
- Modify: `web/apps/app/src/api/desktop-client.ts`
- Modify: `web/apps/app/src/api/index.ts`
- Modify: `web/apps/app/src/__mocks__/fake-api-client.ts`
- Test: `web/apps/app/src/api/client.test.ts`, `web/apps/app/src/api/desktop-client.test.ts`

**Interfaces:**
- Consumes: the two command names from Task 2 and the four event names above.
- Produces, exported from `web/apps/app/src/api/index.ts`:
  ```ts
  export interface StagedUpload extends UploadResponse {
    path: string;
  }

  export type DragDropEvent =
    | { type: 'over' }
    | { type: 'leave' }
    | { type: 'drop'; paths: string[] };

  export type ImportSource =
    | { kind: 'browser' }
    | {
        kind: 'native';
        pick(): Promise<StagedUpload | null>;
        stagePath(path: string): Promise<StagedUpload>;
        onDragDrop(handler: (event: DragDropEvent) => void): () => void;
      };
  ```
  plus `importSource(): ImportSource` on `ApiClient`, `FetchApiClient`, `DesktopApiClient` and `FakeApiClient`, and `type ListenFn` on `DesktopApiClientOptions`.

`onDragDrop` returns its unsubscribe synchronously even though `listen` is async, because the caller is a Lit `disconnectedCallback` that cannot await. A subscription that resolves after the caller has already unsubscribed is torn down immediately.

- [ ] **Step 1: Write the failing tests**

Add to `web/apps/app/src/api/client.test.ts`:

```ts
describe('importSource', () => {
  it('answers browser, because a browser has no path to hand over', () => {
    const client = new FetchApiClient({ fetchImpl: vi.fn() });
    expect(client.importSource()).toEqual({ kind: 'browser' });
  });
});
```

Replace the import line at the head of `web/apps/app/src/api/desktop-client.test.ts`:

```ts
import { describe, it, expect, vi } from 'vitest';
import { FetchApiClient, type DragDropEvent } from './client.js';
import { DesktopApiClient, createApiClient } from './desktop-client.js';
```

then append:

```ts
/** A fake `__TAURI__.event.listen`, with the handlers reachable per event. */
function eventBus() {
  const handlers = new Map<string, Array<(event: { payload: unknown }) => void>>();
  const unlistened: string[] = [];

  const listen = async (
    name: string,
    handler: (event: { payload: unknown }) => void,
  ) => {
    const forName = handlers.get(name) ?? [];
    forName.push(handler);
    handlers.set(name, forName);
    return () => unlistened.push(name);
  };

  const emit = (name: string, payload: unknown) => {
    for (const handler of handlers.get(name) ?? []) handler({ payload });
  };

  return { listen, emit, unlistened, names: () => [...handlers.keys()] };
}

describe('DesktopApiClient importSource', () => {
  it('picks through the native dialog and answers the staged upload', async () => {
    const invoked: Array<[string, Record<string, unknown>]> = [];
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: eventBus().listen,
      invoke: async (cmd, args) => {
        invoked.push([cmd, args]);
        return {
          uploadId: 'a1b2',
          filename: 'cedar-april-2025.csv',
          size: 8214,
          path: '/home/books/cedar-april-2025.csv',
        };
      },
    });

    const source = client.importSource();
    if (source.kind !== 'native') throw new Error('expected a native source');
    const staged = await source.pick();

    expect(invoked[0][0]).toBe('pick_import_file');
    expect(staged).toEqual({
      uploadId: 'a1b2',
      filename: 'cedar-april-2025.csv',
      size: 8214,
      path: '/home/books/cedar-april-2025.csv',
    });
  });

  it('reports a cancelled dialog as null rather than as a failure', async () => {
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: eventBus().listen,
      invoke: async () => null,
    });

    const source = client.importSource();
    if (source.kind !== 'native') throw new Error('expected a native source');
    await expect(source.pick()).resolves.toBeNull();
  });

  it('stages a dropped path by its path', async () => {
    const invoked: Array<[string, Record<string, unknown>]> = [];
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: eventBus().listen,
      invoke: async (cmd, args) => {
        invoked.push([cmd, args]);
        return {
          uploadId: 'c3d4',
          filename: 'juniper-may-2025.xlsx',
          size: 41000,
          path: '/home/books/juniper-may-2025.xlsx',
        };
      },
    });

    const source = client.importSource();
    if (source.kind !== 'native') throw new Error('expected a native source');
    const staged = await source.stagePath('/home/books/juniper-may-2025.xlsx');

    expect(invoked[0]).toEqual([
      'stage_import',
      { path: '/home/books/juniper-may-2025.xlsx' },
    ]);
    expect(staged.uploadId).toBe('c3d4');
  });

  it('reduces the four Tauri drag events to over, leave and drop', async () => {
    const bus = eventBus();
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: bus.listen,
      invoke: async () => null,
    });

    const source = client.importSource();
    if (source.kind !== 'native') throw new Error('expected a native source');
    const seen: DragDropEvent[] = [];
    const off = source.onDragDrop((event) => seen.push(event));
    await Promise.resolve();

    expect(bus.names()).toEqual([
      'tauri://drag-enter',
      'tauri://drag-over',
      'tauri://drag-drop',
      'tauri://drag-leave',
    ]);

    bus.emit('tauri://drag-enter', {
      paths: ['/home/books/cedar-april-2025.csv'],
      position: { x: 10, y: 20 },
    });
    bus.emit('tauri://drag-over', { position: { x: 12, y: 24 } });
    bus.emit('tauri://drag-drop', {
      paths: ['/home/books/cedar-april-2025.csv'],
      position: { x: 12, y: 24 },
    });
    bus.emit('tauri://drag-leave', null);

    expect(seen).toEqual([
      { type: 'over' },
      { type: 'over' },
      { type: 'drop', paths: ['/home/books/cedar-april-2025.csv'] },
      { type: 'leave' },
    ]);

    off();
    expect(bus.unlistened).toHaveLength(4);
  });

  it('reports a drop carrying no paths as an empty drop rather than throwing', async () => {
    const bus = eventBus();
    const client = new DesktopApiClient({
      fetchImpl: vi.fn(),
      listen: bus.listen,
      invoke: async () => null,
    });

    const source = client.importSource();
    if (source.kind !== 'native') throw new Error('expected a native source');
    const seen: DragDropEvent[] = [];
    source.onDragDrop((event) => seen.push(event));
    await Promise.resolve();

    bus.emit('tauri://drag-drop', { position: { x: 1, y: 1 } });

    expect(seen).toEqual([{ type: 'drop', paths: [] }]);
  });
});

describe('createApiClient', () => {
  it('answers a browser client when there is no Tauri global', () => {
    expect(createApiClient()).toBeInstanceOf(FetchApiClient);
    expect(createApiClient().importSource()).toEqual({ kind: 'browser' });
  });

  it('answers a native client when the shell exposes invoke and listen', () => {
    const globals = globalThis as Record<string, unknown>;
    globals.__TAURI__ = {
      core: { invoke: async () => null },
      event: { listen: async () => () => {} },
    };
    try {
      expect(createApiClient().importSource().kind).toBe('native');
    } finally {
      delete globals.__TAURI__;
    }
  });
});
```

- [ ] **Step 2: Run them and watch them fail**

```bash
cd /home/dalton/Dev/nigel/web && npx vitest run apps/app/src/api
```

Expected: type errors resolved at runtime by vitest but the assertions fail —

```
TypeError: client.importSource is not a function
```

on every new case.

- [ ] **Step 3: Implement**

`web/apps/app/src/api/types.ts` — beneath `UploadResponse`:

```ts
/**
 * A file the native shell has already spooled.
 *
 * The first three fields are `UploadResponse`'s, because downstream of the
 * `uploadId` there is no difference between a staged file and an uploaded one.
 * `path` is the difference: it is where the file still lives, so a spool that
 * expired can be refilled without asking the user to choose again.
 */
export interface StagedUpload extends UploadResponse {
  path: string;
}
```

`web/apps/app/src/api/client.ts` — beside `ExportTarget`:

```ts
/**
 * Tauri's window-level drag-and-drop, reduced to the three moments a screen
 * has a use for. `enter` and `over` both mean "a file is over the window",
 * and one highlight is all a screen shows, so both arrive as `over`.
 */
export type DragDropEvent =
  | { type: 'over' }
  | { type: 'leave' }
  | { type: 'drop'; paths: string[] };

/**
 * Where a statement comes from in this client, in the shape of
 * `ExportTarget`: a discriminant plus whatever the running platform can
 * actually do.
 *
 * A browser has bytes and no path; a native shell has a path and no reason to
 * put the bytes through the webview. This is also the seam a desktop client
 * attached to a *remote* server will use to answer `browser` — a server on
 * another machine cannot see this disk, so path staging must not be offered
 * there.
 */
export type ImportSource =
  | { kind: 'browser' }
  | {
      kind: 'native';
      /** `null` is a cancelled dialog, which is not a failure. */
      pick(): Promise<StagedUpload | null>;
      stagePath(path: string): Promise<StagedUpload>;
      /** Returns its unsubscribe synchronously; a screen disconnects synchronously. */
      onDragDrop(handler: (event: DragDropEvent) => void): () => void;
    };
```

Add `StagedUpload` to the type import from `./types.js`, and to the `ApiClient` interface, beside `uploadImport`:

```ts
  /**
   * How this client obtains a statement. Screens branch on the discriminant
   * and never ask which shell they are in.
   */
  importSource(): ImportSource;
```

On `FetchApiClient`:

```ts
  importSource(): ImportSource {
    return { kind: 'browser' };
  }
```

`web/apps/app/src/api/desktop-client.ts` — the injectable listener, the four event names, and the override:

```ts
/** The Tauri event bridge, injectable for the same reason `invoke` is. */
export type ListenFn = (
  event: string,
  handler: (event: { payload: unknown }) => void,
) => Promise<() => void>;

export interface DesktopApiClientOptions extends FetchApiClientOptions {
  invoke: InvokeFn;
  listen: ListenFn;
}

/**
 * The window-level drag-and-drop events Tauri 2 emits to the page.
 *
 * They are window-level rather than element-level: Tauri intercepts drag
 * events in the webview, so the page's own HTML5 handlers never see a drop.
 */
const DRAG_EVENTS = {
  enter: 'tauri://drag-enter',
  over: 'tauri://drag-over',
  drop: 'tauri://drag-drop',
  leave: 'tauri://drag-leave',
} as const;
```

On the class:

```ts
  private readonly listen: ListenFn;
```

assigned in the constructor as `this.listen = options.listen;`, then:

```ts
  override importSource(): ImportSource {
    return {
      kind: 'native',
      pick: async () =>
        ((await this.invoke('pick_import_file', {})) as StagedUpload | null) ?? null,
      stagePath: async (path: string) =>
        (await this.invoke('stage_import', { path })) as StagedUpload,
      onDragDrop: (handler) => this.subscribeDragDrop(handler),
    };
  }

  private subscribeDragDrop(handler: (event: DragDropEvent) => void): () => void {
    const off: Array<() => void> = [];
    let cancelled = false;

    const subscribe = (name: string, toEvent: (payload: unknown) => DragDropEvent) => {
      void this.listen(name, (event) => {
        if (!cancelled) handler(toEvent(event.payload));
      }).then((unlisten) => {
        if (cancelled) unlisten();
        else off.push(unlisten);
      });
    };

    subscribe(DRAG_EVENTS.enter, () => ({ type: 'over' }));
    subscribe(DRAG_EVENTS.over, () => ({ type: 'over' }));
    subscribe(DRAG_EVENTS.drop, (payload) => ({ type: 'drop', paths: pathsOf(payload) }));
    subscribe(DRAG_EVENTS.leave, () => ({ type: 'leave' }));

    return () => {
      cancelled = true;
      for (const unlisten of off.splice(0)) unlisten();
    };
  }
```

and, beside `filenameFrom`:

```ts
/** `tauri://drag-drop` carries `{paths, position}`; a drag of nothing carries no paths. */
function pathsOf(payload: unknown): string[] {
  const paths = (payload as { paths?: unknown } | null)?.paths;
  if (!Array.isArray(paths)) return [];
  return paths.filter((path): path is string => typeof path === 'string');
}
```

`createApiClient` reads both halves of the global:

```ts
export function createApiClient(): ApiClient {
  const tauri = (
    globalThis as {
      __TAURI__?: { core?: { invoke?: InvokeFn }; event?: { listen?: ListenFn } };
    }
  ).__TAURI__;
  const invoke = tauri?.core?.invoke;
  const listen = tauri?.event?.listen;
  // Both or neither: `withGlobalTauri` publishes the whole api namespace, so a
  // shell offering one and not the other is not a shell this app runs in.
  return invoke && listen ? new DesktopApiClient({ invoke, listen }) : new FetchApiClient();
}
```

`web/apps/app/src/api/index.ts` — add the two new type names to the existing `./client.js` export block:

```ts
  type DragDropEvent,
  type ImportSource,
```

(`StagedUpload` reaches the app through the existing `export * from './types.js'`.)

`web/apps/app/src/__mocks__/fake-api-client.ts` — add `ImportSource` to the type import from `../api/client.js`, then beside `exportTarget`:

```ts
  /** A browser by default; the native-mode screen tests swap this. */
  importSourceValue: ImportSource = { kind: 'browser' };

  importSource(): ImportSource {
    return this.importSourceValue;
  }
```

- [ ] **Step 4: Run them and watch them pass**

```bash
cd /home/dalton/Dev/nigel/web && npx vitest run apps/app/src/api && npm run typecheck && npm run lint
```

Expected: `Test Files 2 passed`, with the eight new cases named in the output — one on `FetchApiClient`, five on `DesktopApiClient.importSource`, two on `createApiClient` — then a clean typecheck and lint.

- [ ] **Step 5: Commit**

```bash
cd /home/dalton/Dev/nigel/wt-imports && git add web/apps/app/src/api web/apps/app/src/__mocks__/fake-api-client.ts && git commit -m "Add the importSource seam and its native implementation"
```

---

### Task 4: `wc-dropzone` native mode

**Files:**
- Modify: `web/packages/ui/src/components/wc-dropzone.ts`
- Modify: `web/packages/ui/src/components/wc-dropzone.preview.ts`
- Modify: `web/packages/ui/src/components/index.ts`
- Test: `web/packages/ui/src/components/wc-dropzone.test.ts`

**Interfaces:**
- Produces, on `WcDropzone`: `native: boolean` (attribute `native`, reflected) and `highlight: boolean` (attribute `highlight`, reflected); the event `nc-pick-request` (`CustomEvent<void>`, bubbles and composed, no detail).
- Produces, exported from `@nigel/ui`: `DEFAULT_EXTENSIONS: readonly string[]` (`['.csv', '.xlsx', '.xls']`) and `unsupportedFileMessage(extensions?: readonly string[]): string`.

In native mode the component keeps its whole appearance and loses two behaviours: the browse button asks its owner to open the dialog instead of clicking a hidden input, and the HTML5 drag handlers stand down. Standing them down is not a tidiness choice — Tauri consumes the drag before the page sees it, so a live handler would be a second source of truth that never fires, and `highlight` is how the real one gets in.

- [ ] **Step 1: Write the failing tests**

Add to `web/packages/ui/src/components/wc-dropzone.test.ts`:

```ts
describe('wc-dropzone in native mode', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('asks its owner for a pick instead of opening the browser picker', async () => {
    const el = await mount({ native: true });
    const input = el.shadowRoot?.querySelector('input[type="file"]');
    const asked = vi.fn();
    el.addEventListener('nc-pick-request', asked);

    (el.shadowRoot?.querySelector('.well') as HTMLButtonElement).click();

    expect(asked).toHaveBeenCalledOnce();
    // There is nothing to click: the shell owns the dialog.
    expect(input).toBeNull();
  });

  it('asks for a pick from the replace button too', async () => {
    const el = await mount({
      native: true,
      filename: 'cedar-april-2025.csv',
      size: 8214,
    });
    const asked = vi.fn();
    el.addEventListener('nc-pick-request', asked);

    const buttons = [...(el.shadowRoot?.querySelectorAll('.replace') ?? [])];
    (buttons[0] as HTMLButtonElement).click();

    expect(asked).toHaveBeenCalledOnce();
  });

  it('ignores an HTML5 drop, which the shell never lets through anyway', async () => {
    const el = await mount({ native: true });
    const selected = vi.fn();
    const failed = vi.fn();
    el.addEventListener('nc-file-select', selected);
    el.addEventListener('nc-file-error', failed);

    drop(el, [file('cedar-april-2025.csv')]);

    expect(selected).not.toHaveBeenCalled();
    expect(failed).not.toHaveBeenCalled();
  });

  it('takes its drag treatment from highlight rather than from dragover', async () => {
    const el = await mount({ native: true });

    dragOver(el);
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.zone')?.classList.contains('dragover')).toBe(
      false,
    );

    el.highlight = true;
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.zone')?.classList.contains('dragover')).toBe(
      true,
    );
  });

  it('still emits nc-file-clear from Remove', async () => {
    const el = await mount({
      native: true,
      filename: 'cedar-april-2025.csv',
      size: 8214,
    });
    const cleared = vi.fn();
    el.addEventListener('nc-file-clear', cleared);

    const buttons = [...(el.shadowRoot?.querySelectorAll('.replace') ?? [])];
    (buttons.at(-1) as HTMLButtonElement).click();

    expect(cleared).toHaveBeenCalledOnce();
  });
});

describe('unsupportedFileMessage', () => {
  it('is the message the well itself produces for a file it cannot read', async () => {
    const el = await mount();
    const failed = vi.fn();
    el.addEventListener('nc-file-error', (e) =>
      failed((e as CustomEvent).detail.message),
    );

    drop(el, [file('notes.txt')]);

    expect(failed.mock.calls[0][0]).toBe(unsupportedFileMessage());
  });
});
```

Extend the file's imports — the existing bare `import './wc-dropzone.js';` becomes:

```ts
import './wc-dropzone.js';
import { unsupportedFileMessage } from './wc-dropzone.js';
```

- [ ] **Step 2: Run them and watch them fail**

```bash
cd /home/dalton/Dev/nigel/web && npx vitest run packages/ui/src/components/wc-dropzone.test.ts
```

Expected: the missing export fails the whole module before a single case runs —

```
SyntaxError: The requested module './wc-dropzone.js' does not provide an export named 'unsupportedFileMessage'
```

Add the export alone and re-run to see the six native cases fail on their own terms (`asked` never called, `selected` called once, `highlight` not reflected).

- [ ] **Step 3: Implement**

`web/packages/ui/src/components/wc-dropzone.ts` — beside `DEFAULT_MAX_BYTES`:

```ts
/** What the importers read, which is also what the picker offers. */
export const DEFAULT_EXTENSIONS = ['.csv', '.xlsx', '.xls'] as const;

/**
 * The one sentence a file nigel cannot read produces.
 *
 * Exported because a native shell decides this before the component ever sees
 * a file — it is handed a path, not a `File` — and two copies of a sentence
 * drift.
 */
export function unsupportedFileMessage(
  extensions: readonly string[] = DEFAULT_EXTENSIONS,
): string {
  return `nigel reads ${extensions.join(', ')} statements. That one is something else.`;
}
```

Default `accept` from the same list:

```ts
  @property({ type: String })
  accept = DEFAULT_EXTENSIONS.join(',');
```

New properties, beside `disabled`:

```ts
  /**
   * The shell owns choosing and dragging.
   *
   * The browse action asks for a pick rather than opening one, and the HTML5
   * handlers stand down: Tauri intercepts drag events in the webview, so a
   * handler here would never fire and `highlight` is how the real drag gets in.
   */
  @property({ type: Boolean, reflect: true })
  native = false;

  /** The drag-over treatment, driven by the owner in native mode. */
  @property({ type: Boolean, reflect: true })
  highlight = false;
```

`reject()` uses the shared sentence:

```ts
    if (extensions.length > 0 && !extensions.some((ext) => name.endsWith(ext))) {
      return unsupportedFileMessage(extensions);
    }
```

The three behaviour changes:

```ts
  private handleBrowse = (): void => {
    if (this.blocked) return;
    if (this.native) {
      this.dispatchEvent(
        new CustomEvent('nc-pick-request', { bubbles: true, composed: true }),
      );
      return;
    }
    this.input?.click();
  };

  private handleDragOver = (event: DragEvent): void => {
    if (this.native || this.blocked) return;
    event.preventDefault();
    this.dragover = true;
  };

  private handleDragLeave = (): void => {
    if (this.native) return;
    this.dragover = false;
  };

  private handleDrop = (event: DragEvent): void => {
    if (this.native) return;
    event.preventDefault();
    this.dragover = false;
    if (this.blocked) return;
    this.offer(event.dataTransfer?.files?.[0]);
  };
```

`render()` picks its mark and drops the input in native mode:

```ts
  render() {
    const marked = this.native ? this.highlight : this.dragover;

    return html`
      <div
        class="zone ${marked ? 'dragover' : ''}"
        @dragover=${this.handleDragOver}
        @dragleave=${this.handleDragLeave}
        @drop=${this.handleDrop}
      >
        ${this.filename ? this.renderSelected() : this.renderWell()}
      </div>
      ${this.native
        ? nothing
        : html`<input
            type="file"
            accept=${this.accept}
            tabindex="-1"
            aria-hidden="true"
            @change=${this.handleInputChange}
          />`}
      ${this.error ? html`<p class="error" role="alert">${this.error}</p>` : nothing}
    `;
  }
```

Add to the `HTMLElementEventMap` block at the foot of the file:

```ts
    'nc-pick-request': CustomEvent<void>;
```

`web/packages/ui/src/components/index.ts` — extend the dropzone export block:

```ts
export {
  WcDropzone,
  DEFAULT_EXTENSIONS,
  DEFAULT_MAX_BYTES,
  unsupportedFileMessage,
  type NcFileErrorDetail,
  type NcFileSelectDetail,
} from './wc-dropzone.js';
```

`web/packages/ui/src/components/wc-dropzone.preview.ts` — three states appended to the existing six:

```ts
    {
      name: 'native-idle',
      render: () => html`<wc-dropzone native></wc-dropzone>`,
    },
    {
      name: 'native-highlight',
      render: () => html`<wc-dropzone native highlight></wc-dropzone>`,
    },
    {
      name: 'native-staged',
      render: () =>
        html`<wc-dropzone
          native
          filename="cedar-april-2025.csv"
          .size=${8214}
        ></wc-dropzone>`,
    },
```

- [ ] **Step 4: Run them and watch them pass**

```bash
cd /home/dalton/Dev/nigel/web && npx vitest run packages/ui/src/components/wc-dropzone.test.ts
```

Expected: every existing case still green plus the six new ones, and `describePreviewA11y` reporting nine states with zero violations:

```
 ✓ wc-dropzone preview a11y > native-idle has no axe violations
 ✓ wc-dropzone preview a11y > native-highlight has no axe violations
 ✓ wc-dropzone preview a11y > native-staged has no axe violations
```

Then the package's guards:

```bash
cd /home/dalton/Dev/nigel/web && npm test && npm run typecheck && npm run lint
```

Expected: all suites pass, `controls-adoption.test.ts` included.

- [ ] **Step 5: Commit**

```bash
cd /home/dalton/Dev/nigel/wt-imports && git add web/packages/ui/src/components && git commit -m "Give wc-dropzone a native mode that defers picking and dragging to the shell"
```

---

### Task 5: The import screen's native branch

**Files:**
- Modify: `web/apps/app/src/screens/import.ts`
- Modify: `web/apps/app/src/screens/import-data.ts`
- Test: `web/apps/app/src/screens/import.test.ts`, `web/apps/app/src/screens/import-data.test.ts`

**Interfaces:**
- Consumes: `ImportSource`, `DragDropEvent`, `StagedUpload` from `../api/index.js`; `DEFAULT_EXTENSIONS`, `unsupportedFileMessage` from `@nigel/ui`; `wc-dropzone`'s `native`, `highlight` and `nc-pick-request`.
- Produces, from `web/apps/app/src/screens/import-data.ts`:
  ```ts
  export function supportedDrop(
    paths: string[],
    extensions?: readonly string[],
  ): string | null;
  ```
  the first dropped path an importer could read, or `null`.

Everything downstream of the `uploadId` is untouched. `ensureUpload()` grows one branch, and `withUpload`'s existing retry then does the re-stage for free: it clears `uploadId` and calls back into `ensureUpload`, which in native mode stages the retained `path` again.

The subscribe lives in `firstUpdated` rather than `connectedCallback` because Lit inserts a template-created element before applying its property parts — `client` is not set yet on the first connect. A reconnect gets its subscription from `connectedCallback`, guarded on `hasUpdated`.

- [ ] **Step 1: Write the failing tests**

Add to `web/apps/app/src/screens/import-data.test.ts`:

```ts
describe('supportedDrop', () => {
  it('takes the first path an importer could read', () => {
    expect(
      supportedDrop([
        '/home/books/notes.txt',
        '/home/books/cedar-april-2025.csv',
        '/home/books/juniper-may-2025.xlsx',
      ]),
    ).toBe('/home/books/cedar-april-2025.csv');
  });

  it('matches the extension regardless of case', () => {
    expect(supportedDrop(['/home/books/JUNIPER-MAY-2025.XLSX'])).toBe(
      '/home/books/JUNIPER-MAY-2025.XLSX',
    );
  });

  it('answers null when nothing dropped is readable', () => {
    expect(supportedDrop(['/home/books/receipt.pdf', '/home/books'])).toBeNull();
  });

  it('answers null for a drop of nothing', () => {
    expect(supportedDrop([])).toBeNull();
  });
});
```

Add to `web/apps/app/src/screens/import.test.ts` — a native source built on the fake client, then the cases:

```ts
/**
 * Point the fake client at a native source, and hand the test the two things
 * only the shell can do: answer the dialog, and drop a file on the window.
 */
function nativeSource(fake: FakeApiClient) {
  const staged: StagedUpload[] = [];
  let nextId = 1;
  let picked: string | null = null;
  let handler: ((event: DragDropEvent) => void) | null = null;
  let subscribes = 0;
  let unsubscribes = 0;

  const stage = (path: string): StagedUpload => {
    const uploadId = `staged-${nextId++}`;
    fake.liveUploads.add(uploadId);
    const upload = {
      uploadId,
      filename: path.slice(path.lastIndexOf('/') + 1),
      size: 8214,
      path,
    };
    staged.push(upload);
    return upload;
  };

  fake.importSourceValue = {
    kind: 'native',
    pick: async () => (picked === null ? null : stage(picked)),
    stagePath: async (path) => stage(path),
    onDragDrop: (fn) => {
      subscribes += 1;
      handler = fn;
      return () => {
        unsubscribes += 1;
        handler = null;
      };
    },
  };

  return {
    /** What the next dialog answers; null is a cancel, which is the default. */
    willPick: (path: string | null) => {
      picked = path;
    },
    staged,
    emit: (event: DragDropEvent) => handler?.(event),
    counts: () => ({ subscribes, unsubscribes }),
    isSubscribed: () => handler !== null,
  };
}

describe('the import screen in native mode', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('puts the dropzone in native mode', async () => {
    const fake = client();
    nativeSource(fake);
    const { el } = await mount(fake);

    expect(dropzone(el).native).toBe(true);
  });

  it('stages what the native dialog returns and previews it', async () => {
    const fake = client();
    const native = nativeSource(fake);
    native.willPick('/home/books/cedar-april-2025.csv');
    const { el } = await mount(fake);

    dropzone(el).dispatchEvent(
      new CustomEvent('nc-pick-request', { bubbles: true, composed: true }),
    );
    await settle(el);

    expect(dropzone(el).filename).toBe('cedar-april-2025.csv');
    await setForm(el, { account: 'BofA Checking' });
    await click(el, 'Preview');

    expect(panelHeadings(el)).toContain('Preview');
    // Nothing was uploaded: the file never crossed the wire.
    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(0);
    const previewCall = fake.calls.find((c) => c.startsWith('previewImport'));
    if (previewCall === undefined) throw new Error('no previewImport call');
    expect(bodyOf(previewCall).uploadId).toBe('staged-1');
  });

  it('leaves the screen alone when the dialog is cancelled', async () => {
    const fake = client();
    nativeSource(fake);
    const { el } = await mount(fake);

    dropzone(el).dispatchEvent(
      new CustomEvent('nc-pick-request', { bubbles: true, composed: true }),
    );
    await settle(el);

    expect(dropzone(el).filename).toBe('');
    expect(dropzone(el).error).toBe('');
  });

  it('highlights the dropzone while a file is over the window', async () => {
    const fake = client();
    const native = nativeSource(fake);
    const { el } = await mount(fake);

    native.emit({ type: 'over' });
    await settle(el);
    expect(dropzone(el).highlight).toBe(true);

    native.emit({ type: 'leave' });
    await settle(el);
    expect(dropzone(el).highlight).toBe(false);
  });

  it('stages the first usable path in a drop', async () => {
    const fake = client();
    const native = nativeSource(fake);
    const { el } = await mount(fake);

    native.emit({
      type: 'drop',
      paths: ['/home/books/receipt.pdf', '/home/books/juniper-may-2025.xlsx'],
    });
    await settle(el);

    expect(native.staged.map((s) => s.path)).toEqual([
      '/home/books/juniper-may-2025.xlsx',
    ]);
    expect(dropzone(el).filename).toBe('juniper-may-2025.xlsx');
    expect(dropzone(el).highlight).toBe(false);
  });

  it('says the same thing the dropzone would about a drop it cannot read', async () => {
    const fake = client();
    const native = nativeSource(fake);
    const { el } = await mount(fake);

    native.emit({ type: 'drop', paths: ['/home/books/receipt.pdf'] });
    await settle(el);

    expect(dropzone(el).error).toBe(unsupportedFileMessage());
    expect(native.staged).toHaveLength(0);
  });

  it('re-stages from the retained path when the spool has forgotten the file', async () => {
    const fake = client();
    const native = nativeSource(fake);
    fake.previewErrorOnce = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'gone',
      status: 404,
      details: { reason: UPLOAD_NOT_FOUND },
    });
    const { el } = await mount(fake);

    native.emit({ type: 'drop', paths: ['/home/books/cedar-april-2025.csv'] });
    await settle(el);
    await setForm(el, { account: 'BofA Checking' });
    await click(el, 'Preview');

    // Recovered without saying anything: the file is still on disk.
    expect(panelHeadings(el)).toContain('Preview');
    expect(native.staged.map((s) => s.path)).toEqual([
      '/home/books/cedar-april-2025.csv',
      '/home/books/cedar-april-2025.csv',
    ]);
    expect(dropzone(el).error).toBe('');
  });

  it('unsubscribes from the window on disconnect and resubscribes on reconnect', async () => {
    const fake = client();
    const native = nativeSource(fake);
    const { el } = await mount(fake);

    expect(native.counts()).toEqual({ subscribes: 1, unsubscribes: 0 });

    el.remove();
    expect(native.counts()).toEqual({ subscribes: 1, unsubscribes: 1 });
    expect(native.isSubscribed()).toBe(false);

    document.body.appendChild(el);
    await settle(el);
    expect(native.counts()).toEqual({ subscribes: 2, unsubscribes: 1 });
  });

  it('leaves the dropzone alone when the client is a browser', async () => {
    const { el } = await mount();

    expect(dropzone(el).native).toBe(false);
    expect(dropzone(el).highlight).toBe(false);
  });
});
```

Extend the test file's imports: `unsupportedFileMessage` joins the `@nigel/ui` block, and the api barrel import becomes

```ts
import { ApiError, type DragDropEvent, type StagedUpload } from '../api/index.js';
```

`supportedDrop` joins the `./import-data.js` import in `import-data.test.ts`.

- [ ] **Step 2: Run them and watch them fail**

```bash
cd /home/dalton/Dev/nigel/web && npx vitest run apps/app/src/screens/import.test.ts apps/app/src/screens/import-data.test.ts
```

Expected: `supportedDrop is not a function` on the four data cases, and on the screen cases `expect(dropzone(el).native).toBe(true)` receiving `undefined`.

- [ ] **Step 3: Implement**

`web/apps/app/src/screens/import-data.ts` — add `DEFAULT_EXTENSIONS` to the `@nigel/ui` import, then:

```ts
/**
 * The first dropped path an importer could read, or null.
 *
 * A native drop is a list — a window drop can carry several files and a
 * directory — and the screen imports one statement at a time.
 */
export function supportedDrop(
  paths: string[],
  extensions: readonly string[] = DEFAULT_EXTENSIONS,
): string | null {
  const found = paths.find((path) => {
    const lower = path.toLowerCase();
    return extensions.some((extension) => lower.endsWith(extension));
  });
  return found ?? null;
}
```

`web/apps/app/src/screens/import.ts` — extend the three existing import statements. `unsupportedFileMessage` joins the `@nigel/ui` block:

```ts
import {
  dispatchNcToast,
  EMPTY_IMPORT_FORM,
  unsupportedFileMessage,
  type ImportAccountOption,
  type ImportFormValue,
  type NcFileErrorDetail,
  type NcFileSelectDetail,
  type NcImportChangeDetail,
} from '@nigel/ui';
```

the two new seam types join the api barrel import:

```ts
import {
  ApiError,
  type ApiClient,
  type DragDropEvent,
  type ImportSource,
} from '../api/index.js';
```

`StagedUpload` joins the `../api/types.js` block:

```ts
import type {
  CsvProfile,
  ImportConfirmation,
  ImporterFormat,
  ImportPreview,
  StagedUpload,
} from '../api/types.js';
```

and `supportedDrop` joins the `./import-data.js` block, alphabetically after `routeImportError`.

New state and fields, beside `filesize`:

```ts
  /** Driven by the shell's drag events; the dropzone renders the treatment. */
  @state() private highlight = false;

  /** The file the shell has spooled, in native mode. */
  private staged: StagedUpload | null = null;

  /** Fixed for the screen's life: one client, one answer. */
  private source: ImportSource = { kind: 'browser' };

  private unsubscribeDragDrop: (() => void) | null = null;
```

Lifecycle:

```ts
  firstUpdated(): void {
    this.source = this.client.importSource();
    this.listenForDrops();
    void this.load();
  }

  connectedCallback(): void {
    super.connectedCallback();
    // The first connect happens before `client` is set, so `firstUpdated` owns
    // the first subscription; this one is for a screen that comes back.
    if (this.hasUpdated) this.listenForDrops();
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this.unsubscribeDragDrop?.();
    this.unsubscribeDragDrop = null;
    this.highlight = false;
  }

  private listenForDrops(): void {
    if (this.source.kind !== 'native' || this.unsubscribeDragDrop !== null) return;
    const source = this.source;
    this.unsubscribeDragDrop = source.onDragDrop((event) => {
      void this.handleDragDrop(event);
    });
  }
```

The three native handlers:

```ts
  private handlePickRequest = async (): Promise<void> => {
    if (this.source.kind !== 'native' || this.busy !== null) return;
    try {
      const staged = await this.source.pick();
      if (staged !== null) this.adopt(staged);
    } catch (error) {
      this.surface(error);
    }
  };

  /**
   * Tauri's drag events are window-level, so a drop anywhere on the window
   * lands here while this screen is the one showing.
   */
  private async handleDragDrop(event: DragDropEvent): Promise<void> {
    if (event.type !== 'drop') {
      this.highlight = event.type === 'over';
      return;
    }

    this.highlight = false;
    const path = supportedDrop(event.paths);
    if (path === null) {
      this.dropzoneError = unsupportedFileMessage();
      return;
    }
    await this.stage(path);
  }

  private async stage(path: string): Promise<void> {
    if (this.source.kind !== 'native') return;
    this.busy = 'upload';
    try {
      this.adopt(await this.source.stagePath(path));
    } catch (error) {
      this.surface(error);
    } finally {
      this.busy = null;
    }
  }

  /** A staged file replaces everything the previous choice implied. */
  private adopt(staged: StagedUpload): void {
    this.staged = staged;
    this.file = null;
    this.filename = staged.filename;
    this.filesize = staged.size;
    this.uploadId = staged.uploadId;
    this.preview = null;
    this.result = null;
    this.clearErrors();
  }
```

`handleFileSelect` gains `this.staged = null;` beside `this.uploadId = null;`.

`ready` and `dirty` measure the choice rather than the `File`:

```ts
  /** A file has been chosen, however this client chooses one. */
  private get chosen(): boolean {
    return this.file !== null || this.staged !== null;
  }

  private get ready(): boolean {
    return this.chosen && this.form.account !== '' && this.busy === null;
  }
```

and in `dirty`, `this.file !== null ||` becomes `this.chosen ||`.

`ensureUpload` grows the native branch:

```ts
  private async ensureUpload(): Promise<string> {
    if (this.uploadId !== null) return this.uploadId;

    if (this.source.kind === 'native') {
      if (this.staged === null) throw new Error('no file chosen');
      this.busy = 'upload';
      // The retained path is why an expired spool costs nothing here: the file
      // never left the disk.
      const staged = await this.source.stagePath(this.staged.path);
      this.staged = staged;
      this.uploadId = staged.uploadId;
      return staged.uploadId;
    }

    if (this.file === null) throw new Error('no file chosen');
    this.busy = 'upload';
    const upload = await this.client.uploadImport(this.file);
    this.uploadId = upload.uploadId;
    return upload.uploadId;
  }
```

`discard()` gains `this.staged = null;` and `this.highlight = false;`.

The dropzone binding in `renderChoose`:

```ts
          <wc-dropzone
            ?native=${this.source.kind === 'native'}
            ?highlight=${this.highlight}
            filename=${this.filename}
            .size=${this.filesize}
            error=${this.dropzoneError}
            ?busy=${busyNow}
            @nc-pick-request=${this.handlePickRequest}
            @nc-file-select=${this.handleFileSelect}
            @nc-file-error=${this.handleFileError}
            @nc-file-clear=${this.handleFileClear}
          ></wc-dropzone>
```

- [ ] **Step 4: Run them and watch them pass**

```bash
cd /home/dalton/Dev/nigel/web && npx vitest run apps/app/src/screens/import.test.ts apps/app/src/screens/import-data.test.ts
```

Expected: every existing case in both files still green plus the thirteen new ones.

Then the whole suite, which includes the seam guards this change is most likely to trip:

```bash
cd /home/dalton/Dev/nigel/web && npm test && npm run typecheck && npm run lint
```

Expected: all suites pass — `__tests__/api-seam.test.ts` included, which is the one that would fail if `__TAURI__` or an endpoint reached the screen.

- [ ] **Step 5: Commit**

```bash
cd /home/dalton/Dev/nigel/wt-imports && git add web/apps/app/src/screens && git commit -m "Import a statement by path when the client offers a native source"
```

---

### Task 6: Document the desktop import path

**Files:**
- Modify: `docs/desktop.md`

**Interfaces:** none. Nothing consumes this task.

`docs/api.md` is deliberately untouched: no HTTP surface changed, which is the point of staging into the existing spool.

- [ ] **Step 1: Write the section**

Add to `docs/desktop.md`, between "Exports" and "Not a deep link":

```markdown
## Imports

A browser has no path to hand over, so the web flow uploads the bytes and gets
an `uploadId` back. The shell has the path and no reason to put a statement
through the webview at all, so it spools the file itself and answers with the
same `uploadId`:

| | Choosing | Getting the file to the spool |
|---|---|---|
| Browser | `wc-dropzone`'s hidden input and HTML5 drop | `POST /api/imports/upload` |
| Shell | `pick_import_file`, or a drop on the window | `stage_import`, or the same call inside the pick |

Both land in `<data_dir>/tmp/uploads/<id>/<filename>`, both take the name
through `uploads::sanitize_filename`, both refuse anything over 25 MB, and both
sweep spools older than an hour first. Preview and confirm are then the
existing routes over the custom scheme with the staged id — there is no
desktop-only import endpoint, which is what makes the two paths identical
downstream rather than merely similar.

`crates/nigel-desktop/src/imports.rs` holds both commands and the `stage_file`
they share. The extension filter on the dialog is derived from
`uploads::ALLOWED_EXTENSIONS`, so the dialog and the spool cannot disagree
about what nigel reads.

Two things are the shell's rather than the page's. Tauri intercepts drag events
in the webview, so `wc-dropzone`'s own HTML5 handlers never fire there: in
`native` mode it stands them down and takes its drag treatment from a
`highlight` property the screen drives from `tauri://drag-enter`,
`tauri://drag-over`, `tauri://drag-drop` and `tauri://drag-leave`. Those
subscriptions are why `capabilities/default.json` exists — `plugin:event|listen`
is ACL-checked on every call, unlike an app command, so without the capability a
drop would go nowhere and say nothing.

The events are window-level, so a statement dropped anywhere on the window is
imported while the import screen is showing, and dropped on any other screen
does nothing.

`DesktopApiClient.importSource()` is the single point where all of this is
decided. It answers `{kind: 'native', …}`; the web client answers
`{kind: 'browser'}`; screens branch on the discriminant and never ask which
shell they are in. A desktop client attached to a remote server will answer
`browser` from here too, since a server on another machine cannot see this disk.
```

- [ ] **Step 2: Verify the docs and the no-real-data sweep**

```bash
cd /home/dalton/Dev/nigel/wt-imports && ./scripts/check-no-real-data.sh; echo "exit: $?"
```

Expected: `exit: 0`. Judge it by that number, not by anything in its output.

- [ ] **Step 3: Commit**

```bash
cd /home/dalton/Dev/nigel/wt-imports && git add docs/desktop.md && git commit -m "Document how the desktop shell stages a statement for import"
```

- [ ] **Step 4: Record the manual verification the task still owes**

CI cannot drive a native dialog or synthesize an OS drag, so the shell's half of this is only proved by hand — as it was for 33.2. Add an implementation note to the task recording what the operator must do, without recording anything read off their books:

```bash
cd /home/dalton/Dev/nigel/wt-imports && backlog task 33.3 --plain
```

then append a note covering, in this order:

1. `cd web && npm run build && cd .. && cargo run -p nigel-desktop`.
2. On the import screen, click the well — the dialog opens, offers only `.csv`, `.xlsx` and `.xls`, and a chosen statement appears with its name and size.
3. Cancel the dialog — the screen is unchanged and shows no error.
4. Drag a statement from Finder anywhere onto the window — the well highlights on the way in, un-highlights on the way out, and a drop names the file.
5. Drop something the importers cannot read — the well says so and stages nothing.
6. Preview and confirm the staged file, then check the import shows in the history with the filename it was dropped under.
7. Leave a preview open for over an hour, then confirm — it re-stages and succeeds rather than reporting an expired upload.

Steps 4 through 7 are the ones no test in this plan covers. State the steps, never the figures.

---

## Review

Checked against the spec before this plan was committed.

- **Spec §1, staging commands** — Task 1 (`stage_file`, `StagedUpload`, the four unit tests the spec names) and Task 2 (`stage_import`, `pick_import_file`, `generate_handler!`, the filter derived from `ALLOWED_EXTENSIONS`, `purge_stale` before every store). Covered.
- **Spec §1, "no new HTTP route"** — Task 2's integration test drives `/api/imports/preview` and `/confirm` over the desktop router with a staged id and asserts the same fields the core route tests assert. Covered.
- **Spec §2, the `ImportSource` seam** — Task 3, including `pick` answering `null` on cancel, `stagePath`, `onDragDrop` returning an unsubscribe, and externals injected so no test touches a global. Covered.
- **Spec §3, screen behaviour** — Task 5: pick replaces the file state; subscribe on connect and unsubscribe on disconnect; `over`/`leave` drive the highlight; a drop takes the first allowed path; a drop with nothing usable produces the dropzone's own sentence; `ensureUpload` re-stages from the retained path on expiry through the existing retry. Covered.
- **Spec §4, `wc-dropzone` native mode** — Task 4: `nc-pick-request`, inert HTML5 handlers, public `highlight`, three preview states, `describePreviewA11y` over all nine. Covered.
- **Spec, Testing** — every listed test has a step. The one item that cannot be automated, macOS verification by the operator, is Task 6 step 4.
- **Spec, Out of scope** — nothing here touches remote mode, Windows/Linux QA, or the import pipeline's formats, mapping or categorization.
- **Type consistency** — `StagedUpload` is `{uploadId, filename, size, path}` in Rust (camelCase serialization) and `UploadResponse & {path: string}` in TypeScript; `stage_import` takes `{path}` in both places; `pick_import_file` answers `StagedUpload | null` in both; `DragDropEvent`'s three variants are produced only in `DesktopApiClient.subscribeDragDrop` and consumed only in `NigelImportScreen.handleDragDrop`.
- **Fixtures** — every test file, path and description uses the fictional cast (Cedar Systems, Juniper Labs, Harbor & Vale) with invented amounts. The account name `BofA Checking` and the `bofa_checking` importer are the repository's existing fixture and built-in format.
