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
cargo run -p nigel-desktop
```

Skip the first step and the window still opens — it just shows the "SPA not
built" placeholder `web/build.rs` seeds `web/dist` with, the same one a
sourceless `cargo build` shows for the CLI.

## The dev loop

The SPA is embedded at build time, so a change to `web/` needs `npm run
build` and a `cargo run -p nigel-desktop` rebuild before the shell shows it —
there is no live reload here. The Vite dev server that gives the browser loop
its speed proxies over HTTP, and a custom URI scheme is not something it can
proxy to, so that loop stays outside the shell entirely:

```bash
cargo run -- serve --no-open   # terminal 1
cd web && npm run dev          # terminal 2
```

Use the browser loop for UI work — it is the fast one — and reach for
`cargo run -p nigel-desktop` only to check the transport itself: the scheme
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

Invoice previews follow the same probe. WebKitGTK has no PDF viewer of its
own, so on Linux the bytes go to a private temp file
(`preview.rs::write_temp_pdf`) and `open_external` hands the path to the
system's own viewer. Navigation download only works on Windows and the blob
route only covers Linux and macOS, so everywhere else `openInvoicePreview`
runs the same save action `invoicePreviewTarget` offers: it fetches the bytes
and hands them to `save_export` through a native save dialog.

## Not a deep link

The scheme is an in-process transport, not a URL another application on the
machine may hand this one. Registering it as a deep link would let any
program open `nigel://` with a path of its own choosing and have this
router answer it, which is exactly the assumption the no-session-guard
section above depends on: that nothing outside this process can address the
scheme. `crates/nigel-desktop/tauri.conf.json` carries no `plugins.deep-link`
block and the manifest carries no `tauri-plugin-deep-link` dependency;
`tests/no_deep_link.rs` fails the build if either reappears.
