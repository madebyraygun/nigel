# Desktop shell

`crates/nigel-desktop` is a Tauri 2 shell around the same SPA and JSON API
`nigel serve` exposes over a socket. Instead of a socket, it builds
`nigel_core::server::build_desktop_router` and drives it from
`register_asynchronous_uri_scheme_protocol` on a custom `nigel://` scheme: an
axum `Router` is a `tower::Service`, so `src/transport.rs::answer` is the
whole adapter — Tauri's request goes in, the router's response comes out, one
call to `oneshot`. There is no TCP port, no listener, and no CORS to reason
about, because the page and its API are served from the same origin.

## Running it

The router serves `web/dist` through the same rust-embed fallback `nigel
serve` uses, so building the SPA comes first:

```bash
cd web && npm run build
cd crates/nigel-desktop && cargo run
```

Skip the first step and the window still opens — it just shows the "SPA not
built" placeholder `web/build.rs` seeds `web/dist` with, the same one a
sourceless `cargo build` shows for the CLI.

## The dev loop

The SPA is embedded at build time, so a change to `web/` needs `npm run
build` and a `cargo run` rebuild before the shell shows it —
there is no live reload here. The Vite dev server that gives the browser loop
its speed proxies over HTTP, and a custom URI scheme is not something it can
proxy to, so that loop stays outside the shell entirely:

```bash
cargo run -- serve --no-open   # terminal 1
cd web && npm run dev          # terminal 2
```

Use the browser loop for UI work — it is the fast one — and reach for
`cargo run` in `crates/nigel-desktop` only to check the transport itself: the scheme
protocol, the save dialog, the PDF handling, anything a browser tab can't
stand in for.

## Origin and trust

Tauri gives a custom scheme a different origin form per platform, and the
shell has to know it two ways: as the URL the window navigates to, and as the
`Host` header the router's guard must accept. Both live in
`crates/nigel-desktop/src/lib.rs`, pinned to literals and checked by unit
tests rather than derived from each other, because a wrong value would move
both sides together and stay green while every real launch answered 403:

| Platform | `scheme_url()` | `trusted_host()` |
|---|---|---|
| macOS, Linux | `nigel://localhost/` | `localhost` |
| Windows | `http://nigel.localhost/` | `nigel.localhost` |

`main.rs` hands `trusted_host()` to `TrustedOrigins::exactly`, so the router
accepts requests bearing that one `Host` value and nothing else.

## No session guard

`build_desktop_router` never attaches the session-cookie layer `nigel
serve`'s router carries. The scheme is registered inside this process, and
nothing else on the machine can address it, so a cookie here would only be a
token the app issues to itself. The router is behind `nigel-core`'s `desktop`
feature, which is not in `default`: bound to a TCP port instead of a custom
scheme, `Host`/`Origin` checking alone would not stop another host on the
same LAN from reaching the whole API with no session, since `Host` and
`Origin` are headers a curl invocation sets freely.

That absence has to stay structural rather than a bet on one crate's
discipline. `crates/nigel-desktop` is its own Cargo workspace, deliberately
excluded from the root one — Cargo unifies features across workspace
members, so a member enabling `desktop` would switch that feature on for
every build in the tree, including the one that produces the `nigel` binary.
`crates/nigel/tests/layering.rs` guards this: it fails if any root-workspace
member's manifest asks `nigel-core` for the `desktop` feature. CI builds and
tests `nigel-desktop` from its own directory rather than as part of the root
workspace's run.

## The database

`src/db.rs::database_path()` opens the same database the CLI does —
`nigel_core::settings::get_data_dir().join("nigel.db")` — rather than a
database of its own, so a book edited from the terminal shows up in the
desktop shell and back.

## Window lifecycle

On macOS the app survives its last window, and **closing the window hides it
rather than destroying it** — a deliberate choice, not an accident to fix:
hiding keeps the SPA's state (scroll positions, a half-filled form), and the
Dock's Reopen shows the same window back instantly. The run loop prevents the
windowless exit (`ExitRequested` with no code) and answers `Reopen` by showing
the hidden window, rebuilding it only if the webview is genuinely gone. An
explicit quit carries an exit code and is never prevented. Windows and Linux
keep their own convention: closing the window exits the app.

The window's logical size and position are written to
`config_dir()/window-state.json` (beside `settings.json`) on close and on
exit, and restored by the builder clamped to a visible monitor and the
900×700 floor — `src/window_state.rs::clamp_restore` owns the arithmetic. The
file is a convenience: absent, corrupt, or unwritable all degrade silently to
the 1200×820 default, and the next clean close rewrites it.

## Exports

A webview serving the app from a custom URI scheme cannot download the way a
browser tab does. A probe across the three engines found only one route that
works everywhere:

| Engine | Platform | Navigation download | Blob save |
|---|---|---|---|
| WebKitGTK | Linux | no | yes |
| WKWebView | macOS | no | yes |
| WebView2 | Windows | yes | no |

No single client-side route covers all three, so the desktop client hands
the bytes to the native side instead of triggering either kind of download:
`DesktopApiClient` answers an export as `{kind: 'action'}` rather than a URL,
fetches the export's bytes over `fetch`, and `save_export` writes them
through a native save dialog (`crates/nigel-desktop/src/save.rs`). The web
client, which has no native side to hand bytes to, keeps answering an
address the browser downloads directly; screens bind whichever kind of
target they are given and never branch on which one it is.

An invoice's PDF is never rendered inline — WebKitGTK has no built-in PDF
viewer, and nothing in this app tries to frame a PDF — so it follows the same
route as an export: `openInvoicePreview` fetches the bytes and hands them to
`save_export` through a native save dialog, on all three platforms.

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

## Not a deep link

The scheme is an in-process transport, not a URL another application on the
machine may hand this one. Registering it as a deep link would let any
program open `nigel://` with a path of its own choosing and have this
router answer it, which is exactly the assumption the no-session-guard
section above depends on: that nothing outside this process can address the
scheme. `crates/nigel-desktop/tauri.conf.json` carries no `plugins.deep-link`
block and the manifest carries no `tauri-plugin-deep-link` dependency;
`tests/no_deep_link.rs` fails the build if either reappears.

## Where builds come from

This repository's CI compiles and tests the desktop crate on Linux and macOS, and publishes
no installer and no update manifest. The signed, notarized, auto-updating build is sold, and
`backlog/decisions/decision-3` records why producing it here would put the packaging, the
signing identities and the update feed in a public repository.

The source is MIT, so building it yourself is supported rather than tolerated: a checkout,
`npm run build` in `web/`, and `cargo run` from `crates/nigel-desktop` gives the same
application without the packaging.
