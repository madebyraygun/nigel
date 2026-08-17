# Desktop Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri 2 desktop app that boots nigel's SPA against a local database over a custom URI scheme, with no TCP port, and saves exports through the native side because no webview will download from that scheme portably.

**Architecture:** `crates/nigel-desktop` builds `nigel_core::server::build_desktop_router` — the session-guard-free router already behind the `desktop` feature — and drives it from `register_asynchronous_uri_scheme_protocol`. The axum `Router` is a `tower::Service`, so the scheme handler is an adapter: Tauri's request in, the router's response out. No listener, no port, no CORS, because the page and its API are the same origin. Exports use the `ExportTarget` action arm the api seam already has: the page fetches the bytes and hands them to a Rust command, which opens a native save dialog and writes the file.

**Tech Stack:** Rust, Tauri 2, axum, tower; TypeScript, Lit 3, vitest.

**Spec:** `backlog/tasks/task-33.2 - Tauri-2-app-shell-and-backend-transport-decision.md` and `backlog/decisions/decision-1 - Desktop-transport-custom-URI-scheme-over-the-existing-axum-router.md`. The probe results that decide the export path are in 33.2's notes and on branch `probe/33.2-download-scheme`.

This plan covers TASK-33.2 acceptance criteria **#1, #4, #8** and the `nigel-desktop` scaffold TASK-33.1 also names. Criteria #2, #3, #5, #6, #7 are already met — see PR #24 and PR #25.

## What already exists, and must not be rebuilt

- `nigel_core::server::build_desktop_router(state: AppState, trusted: auth::TrustedOrigins) -> Router`, behind the crate's `desktop` feature (not in `default`). It carries no session guard by construction.
- `nigel_core::server::auth::TrustedOrigins::exactly(hosts: Vec<String>)`, which does **not** implicitly trust loopback.
- `AppState::new(db_path: PathBuf, session_token: String)`.
- The router already serves the SPA: `finish_router` sets `static_files::static_handler` as its fallback, nests `/api`, and layers the host guard and security headers. **The shell serves no files itself.**
- `POST /api/unlock` takes `{ password }` and is answerable before the database is open — it rides the shared api router, so unlock works over this transport with no new route.
- The api seam's `ExportTarget` union: `{ kind: 'href'; href: string } | { kind: 'action'; run: () => Promise<void> }`. `FetchApiClient` answers `href`; a desktop client answers `action`. `wc-export-links` and `wc-send-dialog` already render both.

## Global Constraints

- **The web build must not change.** `nigel serve` and the browser SPA behave exactly as they do today. The desktop client is a subclass and a runtime branch, never an edit to `FetchApiClient`'s behaviour.
- **Origin forms differ by platform**, and the code must not assume one: `nigel://localhost` on macOS and Linux, `http://nigel.localhost` on Windows. Compute at runtime.
- **The scheme is not registered as a deep link** (AC #8). It is an in-process transport, not a URL other applications may hand us.
- **`ExportTarget` is declared twice on purpose** — `web/apps/app/src/api/client.ts` and `web/packages/ui/src/components/wc-export-links.ts` — because `@nigel/ui` must not depend on the app. `web/apps/app/src/__tests__/export-target-seam.test.ts` fails on any drift. Keep them identical.
- **No provenance or edit-justification comments** ("added because", "renamed from", "previously"). Describe the current state. Hard repo rule.
- **No real book data.** Fixture cast only: Acme, Cedar Systems, Juniper Labs, Harbor & Vale, Globex, Initech.
- **Rust tests run serially:** `cargo test -- --test-threads=1`.
- **Web tests:** `cd web && npm test`, or `npm run test --workspace=@nigel/app`. `npx vitest run <path>` does **not** work — there is no root vitest config and it fails over a thousand tests on a clean tree.
- **CI runs, in order:** `./scripts/check-no-real-data.sh`, `npm run lint`, `npm run typecheck`, `npm test`, `npm run build`, `cargo fmt --check`, `cargo clippy -- -D warnings`, then four `cargo test` variants. A task is not done until the ones it can affect pass locally.

---

### Task 1: The crate, and a window on the custom scheme

**Files:**
- Create: `crates/nigel-desktop/Cargo.toml`, `crates/nigel-desktop/build.rs`, `crates/nigel-desktop/tauri.conf.json`, `crates/nigel-desktop/src/main.rs`, `crates/nigel-desktop/icons/icon.png`, `crates/nigel-desktop/icons/icon.ico`
- Modify: `Cargo.toml` (workspace members)

**Interfaces:**
- Consumes: nothing.
- Produces: `fn scheme_url() -> String`, returning `"http://nigel.localhost/"` on Windows and `"nigel://localhost/"` elsewhere; `const SCHEME: &str = "nigel"`.

This task proves the transport before any of nigel is behind it. The scheme handler answers one hard-coded HTML string; the next task replaces it with the router.

- [ ] **Step 1: Write the failing test**

`crates/nigel-desktop/src/main.rs` gets a test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scheme_url_matches_the_platform_origin_form() {
        // Tauri serves a custom scheme from a different origin per platform.
        // Getting this wrong means the host guard refuses every request.
        let url = scheme_url();
        if cfg!(windows) {
            assert_eq!(url, "http://nigel.localhost/");
        } else {
            assert_eq!(url, "nigel://localhost/");
        }
        assert!(url.starts_with(SCHEME) || url.contains(SCHEME));
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p nigel-desktop -- --test-threads=1`
Expected: FAIL to compile — the crate does not exist yet.

- [ ] **Step 3: Create the crate**

`crates/nigel-desktop/Cargo.toml`:

```toml
[package]
name = "nigel-desktop"
version = "0.0.0"
edition = "2021"
publish = false

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
nigel-core = { path = "../nigel-core", default-features = false, features = ["desktop"] }
```

`crates/nigel-desktop/build.rs`:

```rust
fn main() {
    tauri_build::build()
}
```

`crates/nigel-desktop/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Nigel",
  "version": "0.0.0",
  "identifier": "books.nigel.desktop",
  "build": { "frontendDist": "../../web/dist" },
  "app": {
    "windows": [],
    "security": { "csp": null }
  },
  "bundle": { "active": false, "icon": ["icons/icon.png", "icons/icon.ico"] }
}
```

`frontendDist` points at the built SPA so `tauri-build` is satisfied; the shell does not serve from it — the router's rust-embed fallback does.

Add `"crates/nigel-desktop"` to the workspace `members` in the root `Cargo.toml`.

Generate the two icons (tauri-build requires an `.ico` on Windows):

```bash
cd crates/nigel-desktop && mkdir -p icons
python3 -c "
import zlib,struct
w=h=32
raw=b''.join(b'\x00'+bytes([80,90,120,255]*w) for _ in range(h))
def chunk(t,d):
    c=t+d
    return struct.pack('>I',len(d))+c+struct.pack('>I',zlib.crc32(c)&0xffffffff)
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b'')
open('icons/icon.png','wb').write(png)
ico=struct.pack('<HHH',0,1,1)+struct.pack('<BBBBHHII',32,32,0,0,1,32,len(png),22)+png
open('icons/icon.ico','wb').write(ico)
"
```

- [ ] **Step 4: Write `main.rs`**

```rust
//! The desktop shell: nigel's SPA and JSON API over one custom URI scheme.

use tauri::{WebviewUrl, WebviewWindowBuilder};

/// The scheme the SPA and the API are both served from.
const SCHEME: &str = "nigel";

/// The origin form Tauri gives a custom scheme, which differs by platform.
fn scheme_url() -> String {
    if cfg!(windows) {
        format!("http://{SCHEME}.localhost/")
    } else {
        format!("{SCHEME}://localhost/")
    }
}

fn main() {
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol(SCHEME, move |_ctx, _request, responder| {
            responder.respond(
                tauri::http::Response::builder()
                    .header(tauri::http::header::CONTENT_TYPE, "text/html")
                    .body(b"<title>nigel</title><p>scheme reached".to_vec())
                    .expect("build response"),
            );
        })
        .setup(|app| {
            WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::CustomProtocol(scheme_url().parse().expect("scheme url")),
            )
            .title("Nigel")
            .inner_size(1200.0, 820.0)
            .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run nigel desktop");
}
```

- [ ] **Step 5: Run the test and the build**

Run: `cargo test -p nigel-desktop -- --test-threads=1` — expect PASS.
Run: `cargo build -p nigel-desktop` — expect success.
Run: `cargo build` and confirm the workspace still builds, and `cargo build --release` still produces `target/release/nigel` unchanged.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/nigel-desktop
git commit -m "Open a window on nigel's own URI scheme"
```

---

### Task 2: Drive the axum router from the scheme handler

**Files:**
- Create: `crates/nigel-desktop/src/transport.rs`
- Modify: `crates/nigel-desktop/src/main.rs`, `crates/nigel-desktop/Cargo.toml`

**Interfaces:**
- Consumes: `scheme_url()`, `SCHEME` from Task 1.
- Produces: `pub async fn answer(router: axum::Router, request: tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>>`.

An axum `Router` is a `tower::Service<http::Request<Body>>`. The scheme handler owns a clone of the router per request and calls it. This is the whole transport — there is no listener and no port.

Add to `Cargo.toml`, matching what `crates/nigel-core/Cargo.toml` already pins so cargo unifies rather than building two copies:

```toml
axum = { version = "0.8.9", default-features = false, features = ["http1", "json", "tokio"] }
tower = { version = "0.5", features = ["util"] }
tokio = { version = "1.53", features = ["rt-multi-thread"] }
```

`axum::body::to_bytes` collects the response body, so no `http-body-util` dependency is needed.

- [ ] **Step 1: Write the failing test**

`crates/nigel-desktop/src/transport.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;

    #[tokio::test]
    async fn it_answers_from_the_router_with_status_headers_and_body() {
        let router = axum::Router::new().route(
            "/hello",
            get(|| async {
                ([(axum::http::header::CONTENT_TYPE, "text/plain")], "hi there")
            }),
        );

        let request = tauri::http::Request::builder()
            .uri("nigel://localhost/hello")
            .method("GET")
            .body(Vec::new())
            .unwrap();

        let response = answer(router, request).await;

        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/plain"
        );
        assert_eq!(response.body(), b"hi there");
    }

    #[tokio::test]
    async fn it_carries_the_request_method_body_and_headers_through() {
        let router = axum::Router::new().route(
            "/echo",
            axum::routing::post(|headers: axum::http::HeaderMap, body: String| async move {
                format!("{} {}", headers.get("x-probe").unwrap().to_str().unwrap(), body)
            }),
        );

        let request = tauri::http::Request::builder()
            .uri("nigel://localhost/echo")
            .method("POST")
            .header("x-probe", "seen")
            .body(b"payload".to_vec())
            .unwrap();

        let response = answer(router, request).await;

        assert_eq!(response.body(), b"seen payload");
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-desktop -- --test-threads=1`
Expected: FAIL to compile — `answer` does not exist.

- [ ] **Step 3: Implement the adapter**

```rust
//! The custom scheme's request path: Tauri's request in, the router's out.

use axum::body::Body;
use tower::ServiceExt;

/// No response this router builds is larger than a PDF export.
const MAX_RESPONSE: usize = 64 * 1024 * 1024;

/// Answer one scheme request from the router.
///
/// The router is a `tower::Service`, so serving it needs no listener and no
/// port — which is the point: the page and its API are the same origin, so
/// there is nothing for another process on the machine to connect to.
pub async fn answer(
    router: axum::Router,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let (parts, body) = request.into_parts();
    let request = axum::http::Request::from_parts(parts, Body::from(body));

    let response = match router.oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    };

    let (parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, MAX_RESPONSE).await {
        Ok(collected) => collected.to_vec(),
        Err(e) => {
            return tauri::http::Response::builder()
                .status(500)
                .body(format!("response body: {e}").into_bytes())
                .expect("build error response");
        }
    };

    tauri::http::Response::from_parts(parts, bytes)
}
```

If `axum::Router::oneshot`'s error type is not `Infallible` on the pinned axum version, match the real error and answer 500 with its text rather than forcing the `match never {}` shape.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nigel-desktop -- --test-threads=1` — expect PASS.

- [ ] **Step 5: Wire it into the handler**

In `main.rs`, replace the hard-coded response. Build the router once in `setup` and clone it per request:

```rust
let router = nigel_core::server::build_desktop_router(
    state.clone(),
    nigel_core::server::auth::TrustedOrigins::exactly(vec![trusted_host()]),
);
```

with

```rust
/// The `Host` header Tauri sends for this scheme, which the router's host
/// guard must be given and nothing else.
fn trusted_host() -> String {
    if cfg!(windows) {
        format!("{SCHEME}.localhost")
    } else {
        "localhost".to_string()
    }
}
```

and the handler spawning onto a tokio runtime the app owns:

```rust
.register_asynchronous_uri_scheme_protocol(SCHEME, move |_ctx, request, responder| {
    let router = router.clone();
    let runtime = runtime.clone();
    runtime.spawn(async move {
        responder.respond(transport::answer(router, request).await);
    });
})
```

Hold the `tokio::runtime::Runtime` in an `Arc` created before the builder, since the scheme handler is not already on a runtime.

- [ ] **Step 6: Verify by hand, then commit**

Run `cargo run -p nigel-desktop` and confirm the window shows nigel's SPA shell rather than the placeholder string. If it shows "SPA not built", run `cd web && npm run build` first — the router serves the embedded `web/dist`.

```bash
git add crates/nigel-desktop
git commit -m "Serve the router over the scheme instead of a port"
```

---

### Task 3: Boot against a real database (AC #1)

**Files:**
- Modify: `crates/nigel-desktop/src/main.rs`
- Create: `crates/nigel-desktop/src/db.rs`

**Interfaces:**
- Consumes: Task 2's transport.
- Produces: `fn database_path() -> PathBuf`, resolving the same database `nigel` itself uses.

`AppState::new` takes the database path and a session token. The desktop router reads no session, but `AppState` still wants the field — pass `nigel_core::server::auth::generate_token()` rather than an empty string, so nothing downstream can mistake a blank token for a match.

The CLI resolves its database at `crates/nigel/src/main.rs:113` as
`nigel_core::settings::get_data_dir().join("nigel.db")`. Use that exact
expression. Do **not** invent a new location: a desktop app that opens a
different database from the CLI is the worst possible outcome of this task.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn the_desktop_opens_the_same_database_the_cli_does() {
    // A desktop app pointing at its own database would silently show the user
    // an empty set of books.
    assert_eq!(
        database_path(),
        nigel_core::settings::get_data_dir().join("nigel.db")
    );
}
```

`crates/nigel/src/main.rs:113` resolves the CLI's database as
`nigel_core::settings::get_data_dir().join("nigel.db")`. `database_path()` is
that same expression and nothing else — the test exists so it stays that way.

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p nigel-desktop -- --test-threads=1`
Expected: FAIL — `database_path` does not exist.

- [ ] **Step 3: Implement, then boot end to end**

Build the state in `setup`, before the router:

```rust
let state = nigel_core::server::AppState::new(
    db::database_path(),
    nigel_core::server::auth::generate_token(),
);
```

- [ ] **Step 4: Verify end to end by hand**

This is AC #1 and it is a manual check. With a database that has demo data:

1. `cargo run -p nigel-desktop`
2. The window shows the dashboard, or the unlock form if the database is encrypted.
3. If encrypted: enter the password. `POST /api/unlock` rides the shared api router, so this must work with no new route. A wrong password must be refused and a right one must open the books.
4. Navigate to Register, Reports and Invoices. Each must load data.

Record what you saw in the report. If any screen 403s, the host guard is being given the wrong `trusted_host()` for this platform — that is the first thing to check.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-desktop
git commit -m "Boot the desktop shell against the CLI's database"
```

---

### Task 4: Exports save through the native side

**Files:**
- Create: `web/apps/app/src/api/desktop-client.ts`, `web/apps/app/src/api/desktop-client.test.ts`
- Modify: `web/apps/app/src/api/client.ts` (export a factory), `crates/nigel-desktop/src/main.rs`, `crates/nigel-desktop/Cargo.toml`
- Create: `crates/nigel-desktop/src/save.rs`

**Interfaces:**
- Consumes: `ExportTarget`, `FetchApiClient`, `exportTarget`, `invoicePreviewTarget` from the api seam.
- Produces: `class DesktopApiClient extends FetchApiClient` overriding `exportTarget` and `invoicePreviewTarget` to answer `{ kind: 'action', run }`; `export function createApiClient(): ApiClient` choosing by `'__TAURI__' in window`; the Rust command `save_export(name: String, bytes: Vec<u8>) -> Result<Option<String>, String>`.

The probe settled why this exists: navigation downloads work only on Windows, client-side blob saves work everywhere except Windows, and only handing the bytes to the native side works on all three.

The user chose a **save dialog** rather than a silent write, so `save_export` opens one and answers `Ok(None)` when the user cancels. A cancelled save is not an error.

Add `tauri-plugin-dialog = "2"` to the crate and `.plugin(tauri_plugin_dialog::init())` to the builder.

- [ ] **Step 1: Write the failing test**

`web/apps/app/src/api/desktop-client.test.ts`:

```ts
describe('DesktopApiClient', () => {
  it('answers an action target that fetches the bytes and hands them to the native side', async () => {
    const saved: Array<{ name: string; bytes: number[] }> = [];
    const fetchImpl = vi.fn(async () =>
      new Response('date,amount\n2026-01-05,10.00\n', {
        status: 200,
        headers: {
          'content-type': 'text/csv',
          'content-disposition': 'attachment; filename="pnl.csv"',
        },
      }),
    );
    const client = new DesktopApiClient({
      fetchImpl,
      invoke: async (cmd, args) => {
        saved.push(args as { name: string; bytes: number[] });
        return null;
      },
    });

    const target = client.exportTarget('pnl', 'text', { year: 2026 });
    expect(target.kind).toBe('action');

    await (target as { run: () => Promise<void> }).run();

    expect(saved).toHaveLength(1);
    expect(saved[0].name).toBe('pnl.csv');
    expect(saved[0].bytes.length).toBeGreaterThan(0);
  });

  it('names the file from Content-Disposition rather than guessing', async () => {
    const saved: Array<{ name: string }> = [];
    const fetchImpl = vi.fn(async () =>
      new Response('%PDF-1.4', {
        status: 200,
        headers: {
          'content-type': 'application/pdf',
          'content-disposition': 'attachment; filename="invoice-1251.pdf"',
        },
      }),
    );
    const client = new DesktopApiClient({
      fetchImpl,
      invoke: async (_cmd, args) => {
        saved.push(args as { name: string });
        return null;
      },
    });

    await (client.invoicePreviewTarget(1251) as { run: () => Promise<void> }).run();

    expect(saved[0].name).toBe('invoice-1251.pdf');
  });

  it('raises a failed export rather than swallowing it', async () => {
    const fetchImpl = vi.fn(async () => new Response('nope', { status: 500 }));
    const client = new DesktopApiClient({ fetchImpl, invoke: async () => null });

    await expect(
      (client.exportTarget('pnl', 'pdf') as { run: () => Promise<void> }).run(),
    ).rejects.toThrow();
  });
});
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cd web && npm run test --workspace=@nigel/app -- desktop-client`
Expected: FAIL — the module does not exist.

- [ ] **Step 3: Implement the desktop client**

```ts
import { FetchApiClient, type FetchApiClientOptions, type ExportTarget } from './client.js';

/** The Tauri command bridge, injectable so tests never touch a global. */
export type InvokeFn = (cmd: string, args: Record<string, unknown>) => Promise<unknown>;

export interface DesktopApiClientOptions extends FetchApiClientOptions {
  invoke: InvokeFn;
}

/**
 * The api client the desktop shell runs.
 *
 * A webview serving this app from a custom URI scheme will not download from a
 * navigation, and the blob route that covers macOS and Linux does not work on
 * Windows. So the bytes come back through `fetch` and the native side writes
 * them, which is the one route all three platforms share.
 */
export class DesktopApiClient extends FetchApiClient {
  private readonly invoke: InvokeFn;
  // `FetchApiClient` keeps its own `fetchImpl` private, so this class holds the
  // reference it was given rather than reaching into the parent.
  private readonly fetchBytes: typeof fetch;

  constructor(options: DesktopApiClientOptions) {
    super(options);
    this.invoke = options.invoke;
    this.fetchBytes = options.fetchImpl ?? globalThis.fetch.bind(globalThis);
  }

  private save(url: string, fallbackName: string): ExportTarget {
    return {
      kind: 'action',
      run: async () => {
        const response = await this.fetchBytes(url);
        if (!response.ok) {
          throw new Error(`Export failed: ${response.status}`);
        }
        const bytes = [...new Uint8Array(await response.arrayBuffer())];
        await this.invoke('save_export', {
          name: filenameFrom(response.headers.get('content-disposition'), fallbackName),
          bytes,
        });
      },
    };
  }
}

/** The name the server chose, or the caller's fallback. */
function filenameFrom(disposition: string | null, fallback: string): string {
  const match = /filename\*?=(?:UTF-8'')?"?([^";]+)"?/i.exec(disposition ?? '');
  return match ? decodeURIComponent(match[1]) : fallback;
}
```

Override `exportTarget` and `invoicePreviewTarget` to return
`this.save(this.exportUrl(report, format, params), \`${report}.${format === 'pdf' ? 'pdf' : 'txt'}\`)`
and `this.save(this.invoicePreviewUrl(number, 'pdf'), \`invoice-${number}.pdf\`)`.
Both `exportUrl` and `invoicePreviewUrl` are public on the parent, so the
subclass builds no URL of its own — the seam rule holds.

Note `baseUrl` and `fetchImpl` are `private readonly` on `FetchApiClient`
(`client.ts:459-460`). A subclass cannot read either, which is why this class
keeps its own `fetchBytes` and goes through the parent's public URL builders.

- [ ] **Step 4: Add the factory and use it**

In `client.ts`:

In `desktop-client.ts` — not `client.ts`, so the browser bundle carries no
reference to the desktop class:

```ts
/**
 * The client for the environment this build is running in.
 *
 * The desktop shell exposes `window.__TAURI__`; a browser does not. Screens
 * never ask which one they got.
 */
export function createApiClient(): ApiClient {
  const tauri = (globalThis as { __TAURI__?: { core?: { invoke?: InvokeFn } } }).__TAURI__;
  const invoke = tauri?.core?.invoke;
  return invoke ? new DesktopApiClient({ invoke }) : new FetchApiClient();
}
```

The only production construction is `web/apps/app/src/components/nigel-app.ts:64`:

```ts
  client: ApiClient = new FetchApiClient();
```

Change that one line to `createApiClient()` and leave every test construction
alone — the fake client and the direct `new FetchApiClient(...)` in tests are
how those tests stay hermetic.

- [ ] **Step 5: Implement the Rust side**

`crates/nigel-desktop/src/save.rs`:

```rust
use tauri_plugin_dialog::DialogExt;

/// Write exported bytes wherever the user chooses.
///
/// `Ok(None)` is a cancelled dialog, which is a normal outcome and not an
/// error: the user changed their mind.
#[tauri::command]
pub async fn save_export(
    app: tauri::AppHandle,
    name: String,
    bytes: Vec<u8>,
) -> Result<Option<String>, String> {
    let Some(path) = app.dialog().file().set_file_name(&name).blocking_save_file() else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, &bytes).map_err(|e| format!("Couldn't save {name}: {e}"))?;
    Ok(Some(path.display().to_string()))
}
```

Register it with `.invoke_handler(tauri::generate_handler![save::save_export])`, and set `"withGlobalTauri": true` under `app` in `tauri.conf.json` so the page can reach `window.__TAURI__.core.invoke`.

- [ ] **Step 6: Run every gate**

Run: `cd web && npm test && npm run typecheck && npm run lint && npm run build`, then `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -- --test-threads=1`.

- [ ] **Step 7: Verify by hand**

`cargo run -p nigel-desktop`, go to Reports, click **Text** and then **PDF**. A save dialog must appear for each, the chosen file must contain the right bytes, and cancelling must leave no file and raise no error dialog.

- [ ] **Step 8: Commit**

```bash
git add web crates/nigel-desktop
git commit -m "Save exports through a native dialog on the desktop"
```

---

### Task 5: Inline PDFs open in an external viewer on Linux

**Files:**
- Modify: `crates/nigel-desktop/src/main.rs`, `crates/nigel-desktop/Cargo.toml`, `web/apps/app/src/api/desktop-client.ts`

**Interfaces:**
- Consumes: Task 4's `invoke` bridge.
- Produces: the Rust command `open_external(path: String) -> Result<(), String>`.

The probe found macOS renders an inline PDF under the custom scheme. WebKitGTK has no built-in PDF viewer, so Linux is where the invoice preview's PDF form has nowhere to render. The user's decision: **open it in the system viewer** rather than falling back to the HTML preview.

Add `tauri-plugin-opener = "2"` and `.plugin(tauri_plugin_opener::init())`.

```rust
use tauri_plugin_opener::OpenerExt;

/// Hand a file to whatever the system uses for its type.
#[tauri::command]
pub async fn open_external(app: tauri::AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}
```

On the page side, the desktop client's PDF *preview* path (not the export path) writes the bytes to a temporary file through a Rust command and then opens it. Only do this on Linux — `navigator.userAgent` is not a reliable platform signal inside a webview, so pass the platform from Rust instead: add a `platform()` command returning `std::env::consts::OS` and have the client ask once at construction.

- [ ] **Step 1: Write the failing test**

```ts
it('opens an inline PDF externally on linux and frames it elsewhere', async () => {
  const calls: string[] = [];
  const linux = new DesktopApiClient({
    fetchImpl: async () => new Response('%PDF-1.4', { status: 200 }),
    invoke: async (cmd) => { calls.push(cmd); return cmd === 'platform' ? 'linux' : null; },
    platform: 'linux',
  });

  await linux.openInvoicePreview(1251);

  expect(calls).toContain('open_external');
});
```

Name the method whatever the invoice screen will actually call; read `web/apps/app/src/screens/invoices.ts` around the preview frame first, and keep the browser path untouched.

- [ ] **Step 2-5: fail, implement, pass, commit**

Follow the same cycle as Task 4. Verify by hand on Linux that the invoice PDF opens in the system viewer, and on macOS that it still renders in-app.

```bash
git commit -m "Open inline PDFs in the system viewer where the webview cannot"
```

---

### Task 6: No deep link, and the dev workflow (AC #8, AC #4)

**Files:**
- Modify: `crates/nigel-desktop/tauri.conf.json`, `docs/architecture.md`, `README.md`
- Create: `docs/desktop.md`
- Test: `crates/nigel-desktop/tests/no_deep_link.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: nothing code depends on.

AC #8: the scheme is an in-process transport, not a URL other applications may hand us. Registering it as a deep link would let any program on the machine open `nigel://` with a path of its choosing and have our router answer it.

- [ ] **Step 1: Write the failing test**

`crates/nigel-desktop/tests/no_deep_link.rs`:

```rust
//! The custom scheme is a transport, not something other applications may open.

use std::fs;

#[test]
fn the_scheme_is_not_registered_as_a_deep_link() {
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string("tauri.conf.json").expect("read config"))
            .expect("parse config");

    // Tauri registers deep links through the plugin's config block. Any
    // presence of it means another program could hand us a nigel:// URL.
    assert!(
        config["plugins"]["deep-link"].is_null(),
        "tauri.conf.json registers a deep link: {}",
        config["plugins"]["deep-link"]
    );

    let manifest = fs::read_to_string("Cargo.toml").expect("read manifest");
    assert!(
        !manifest.contains("tauri-plugin-deep-link"),
        "the deep-link plugin is a dependency"
    );
}
```

Add `serde_json` as a dev-dependency.

- [ ] **Step 2: Run it**

Run: `cargo test -p nigel-desktop --test no_deep_link -- --test-threads=1`
Expected: PASS immediately, since nothing registers one. **Then prove it works**: temporarily add a `"plugins": {"deep-link": {...}}` block, watch the test fail, and remove it. A guard never seen to fail is not a guard. Say so in your report.

- [ ] **Step 3: Write `docs/desktop.md`**

Cover, in the house style — full sentences that explain why, not bullet fragments:

- What the shell is: `crates/nigel-desktop`, serving `build_desktop_router` over the `nigel` scheme with no TCP port.
- Running it: `cd web && npm run build` first, because the router serves the embedded `web/dist`; then `cargo run -p nigel-desktop`.
- The dev loop, and its one sharp edge: the SPA is embedded at build time, so a web change needs `npm run build` and a rebuild. The Vite dev server cannot proxy to a custom scheme, so the fast browser loop is still `cargo run -- serve --no-open` plus `npm run dev`; use that for UI work and the desktop shell to check the transport.
- The origin forms per platform and why `TrustedOrigins::exactly` is given one of them.
- Why there is no session guard here, and why the router is behind the `desktop` feature.
- How exports save, and why: the probe's platform matrix, in a table.
- That the scheme is deliberately not a deep link.

- [ ] **Step 4: Point the other docs at it**

Add a `docs/desktop.md` row to the table in `docs/architecture.md`, and a line to `README.md` only if the desktop app is something a user can run — if it is not yet packaged, leave `README.md` alone rather than promising a binary that does not ship. CLAUDE.md changes only if a command, rule or pointer changes; adding the `docs/desktop.md` pointer to its table is such a change, so add that one row and nothing else.

- [ ] **Step 5: Check and commit**

Run `./scripts/check-no-real-data.sh` and judge it by its **exit status**, never by grepping its output.

```bash
git add crates/nigel-desktop docs README.md CLAUDE.md
git commit -m "Keep the scheme out of the deep-link registry and document the shell"
```

---

## Self-Review

**Spec coverage.** AC #1 — Tasks 2 and 3, with a named manual end-to-end check because booting a GUI against a real database is not something the suite can assert. AC #4 — Task 6. AC #8 — Task 6, with the guard probed to failure. The `nigel-desktop` scaffold TASK-33.1 names — Task 1. The export seam's consumer, which the probe forced — Task 4. The Linux PDF decision — Task 5.

**Type consistency.** `scheme_url()` and `trusted_host()` are the two platform-dependent values and they are defined once, in Task 1 and Task 2 respectively; every later task takes them rather than re-deriving. `save_export(name, bytes)` is the command name in Task 4's TypeScript and its Rust; `open_external(path)` likewise in Task 5. `DesktopApiClient` takes an injected `invoke` in every task that touches it, so no test reaches a global.

**Every symbol named here was checked against the tree before this plan shipped.** Two earlier plans in this epic invented helpers that did not exist — `HttpApiClient`, `test_state()`, `@open-wc/testing`'s `fixture` — costing a detour each. The verification pass on this plan found four more of the same kind and fixed them here rather than in execution: axum is 0.8.9 and not 0.7 (and `axum::body::to_bytes` replaces an `http-body-util` dependency that was not needed); the settings call is `get_data_dir()`, not `load()`; the api client is constructed at `nigel-app.ts:64` and nowhere else; and `FetchApiClient` keeps `baseUrl` and `fetchImpl` **private**, so the subclass in Task 4 cannot reach them — which would have failed to compile as first written.

That leaves the Tauri API surface as the one unverified area: `blocking_save_file`'s return type, `FilePath::into_path`, and the exact `on_download`/opener signatures come from the plugin documentation rather than from this tree, because no Tauri code exists here yet. Treat those three as claims to check on first compile.

**The manual checks are real acceptance, not ceremony.** Tasks 3, 4 and 5 each end with something only a person at a screen can confirm: that the books load, that a save dialog appears and writes the right bytes, and that a PDF opens where the platform can show it. AC #1 is not dischargeable any other way.
