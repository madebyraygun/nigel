# Desktop shell

`crates/nigel-desktop` is a Tauri 2 shell around the same SPA and JSON API
`nigel serve` exposes over a socket. Instead of a socket, it builds
`nigel_core::server::build_desktop_router` and drives it from
`register_asynchronous_uri_scheme_protocol` on a custom `nigel://` scheme: an
axum `Router` is a `tower::Service`, so `nigel_core::server::scheme::answer`
is the whole adapter — the request goes in, the router's response comes out,
one call to `oneshot`. There is no TCP port, no listener, and no CORS to
reason about, because the page and its API are served from the same origin.

The Swift macOS shell answers the same scheme through `crates/nigel-ffi`
(see [The Swift shell's bridge](#the-swift-shells-bridge-nigel-ffi)), over the
same router and the same `answer`.

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
`crates/nigel-core/src/server/scheme.rs`, which every shell uses so the origin
cannot drift between them, pinned to literals and checked by unit tests rather
than derived from each other, because a wrong value would move both sides
together and stay green while every real launch answered 403:

| Platform | `scheme_url()` | `trusted_host()` |
|---|---|---|
| macOS, Linux | `nigel://localhost/` | `localhost` |
| Windows | `http://nigel.localhost/` | `nigel.localhost` |

Each shell hands `trusted_origins()` to `build_desktop_router`, so the router
accepts requests bearing that one `Host` value, or the scheme's own origin, and
nothing else. A webview sends no `Host` for a custom scheme, so `answer`
carries the URL's authority into one before the guard reads it.

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
discipline. `crates/nigel-desktop` and `crates/nigel-ffi` are each their own
Cargo workspace, deliberately excluded from the root one — Cargo unifies
features across workspace members, so a member enabling `desktop` would switch
that feature on for every build in the tree, including the one that produces
the `nigel` binary. `crates/nigel/tests/layering.rs` guards this: it fails if
any root-workspace member's manifest asks `nigel-core` for the `desktop`
feature. CI builds and tests both crates from their own directories rather
than as part of the root workspace's run.

## The database

`src/db.rs::database_path()` opens the same database the CLI does —
`nigel_core::settings::try_data_dir()?.join("nigel.db")` — rather than a
database of its own, so a book edited from the terminal shows up in the
desktop shell and back. When settings.json cannot be parsed the shell shows
a native error dialog naming the file and the parse error and exits, as
`nigel serve` refuses to start; it never serves the default directory in
its place. Reveal-in-folder and import staging resolve the folder the same
way and report the same error.

## Window lifecycle

On macOS the app survives its last window, and **closing the window hides it
rather than destroying it**: hiding keeps the SPA's state (scroll positions,
a half-filled form), and the Dock's Reopen deminiaturizes and shows the same
window back instantly. A fullscreen window leaves fullscreen before it hides —
hiding it in place would strand an empty fullscreen Space — and hides once
the exit transition's resizes settle. The run loop prevents the windowless exit
(`ExitRequested` with no code) and answers `Reopen` by showing the hidden
window, rebuilding it only if the webview is genuinely gone — and exiting if
even the rebuild fails, rather than lingering windowless. Quit itself is
AppKit's `terminate:`, which raises no window event and no `ExitRequested` —
only the final `RunEvent::Exit` as the loop tears down. Windows and Linux
keep their own convention: closing the window exits the app.

Because quit gives the shell no warning, geometry is captured as it changes:
every move and resize feeds `window_state::GeometrySaver`, which writes
`config_dir()/window-state.json` (beside `settings.json`) once the window
holds still; close flushes synchronously, and so does loop teardown
(`RunEvent::Exit`), which every quit path reaches. Minimized and fullscreen
readings are never saved; a maximized window keeps the frame it will
unmaximize back to plus a flag. Restore plans the full frame rectangle —
decorations included — in the platform's one coherent coordinate space
(logical points on macOS, physical pixels elsewhere), clamps it to a visible
monitor and the 900×700 floor, and applies it with `set_position`, the
frame-top-left convention the reading used. With no monitor information the
saved size and maximized flag still apply but the position does not. The
window is built hidden and placed while hidden, so it never appears at the
default spot first; [Launch paint](#launch-paint) covers when it shows.
`src/window_state.rs` owns the arithmetic. The file is a convenience:
absent, corrupt, or unwritable all degrade silently to the 1200×820 default,
and the next clean close rewrites it.

## Launch paint

The window is built hidden and shows when the SPA reports itself ready
through the `frontend_ready` command, so first paint is the app rather than
a white sheet. Ready means the root component's first update settled — never
a `requestAnimationFrame` handshake, because a hidden webview gets no
rendering opportunities at all and would wait forever for the very show the
signal triggers. `main.ts` owns the wiring: the app component renders the
same everywhere, and only the entry point knows it is the page of a window
that starts hidden. A four-second fallback covers a wedged frontend, armed at
setup and again when Reopen rebuilds after a webview crash. Every show —
ready, fallback, Reopen — records itself in `chrome::Shown`, and ready and
the fallback only reveal a window nothing has shown yet, so whichever
arrives late cannot bring back a window the user has since closed.

The window's own background — what shows at the edges when a resize outruns
the webview — is set from the OS theme at build (`src/chrome.rs`, whose
canvas constants are pinned against `@nigel/theme`'s canvas token by
`tests/chrome.rs`) and then kept on the SPA's actually-resolved palette by
the `set_chrome_background` command, which
`web/apps/app/src/chrome-bridge.ts` drives from the theme package's
color-mode contract at boot and on every palette change. Each report carries
the resolved mode and its source: `explicit` when a color-mode class is an
in-app choice, `system` when the OS preference decides. On macOS the window
color does not reach the webview layer, and WebKit's default background
follows the window's appearance instead — so the command also pins the
window's appearance (`set_theme`) to an explicit in-app theme, and releases
it to the OS for `system`. With the appearance matching the palette, the
resize edges stay on theme even when the in-app theme and the OS disagree.
The appearance is app-wide on macOS and Linux, which a single-window app
does not notice, and native dialogs follow it too. On macOS the webview's
`prefers-color-scheme` follows the pinned appearance, which an explicit
class outranks anyway; on a switch back to `system` the release fires a
preference change and the bridge re-reports from the OS. Anything other than `light`/`dark` and `explicit`/`system`
is refused and leaves the chrome as it was.

## Window title

The SPA titles its document `{Screen} · {Book}` on every route
(`#1254 · Invoices · {Book}` on an open invoice), and the window's title bar
reads the same: `build_main_window` registers `on_document_title_changed`,
which hands the document's title to `set_title` through
`chrome::window_title`. Until the page has titled itself the window keeps
"Nigel". The hook needs no capability, since the page never calls into
Tauri for it; `tests/chrome.rs` holds the builder to it.

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

`crates/nigel-desktop/src/imports.rs` holds both commands; the `stage_file`
they share is `nigel_core::server::uploads::stage_file`, which the Swift
shell's `NigelHost::stage_file` calls too. The extension filter on the dialog is derived from
`uploads::UploadArea::Statement.allowed_extensions()`, so the dialog and the spool cannot disagree
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

## Showing the books folder

Settings > Location shows the folder the open books live in, and in the shell
a button opens it in the platform's file manager: `reveal_data_dir`
(`crates/nigel-desktop/src/reveal.rs`) runs `open` on macOS, `explorer` on
Windows and `xdg-open` elsewhere. The command takes no argument — it reads the
folder from settings itself — so the page cannot point it anywhere else.
A Windows path loses its `\\?\` verbatim prefix first (`\\?\UNC\` goes back to
`\\`), which Explorer does not understand, and the child is reaped on a thread
so `open` and `xdg-open` leave no zombies.
`DesktopApiClient.dataDirReveal()` answers the command and the platform's name
for it ("Reveal in Finder", "Show in Explorer", "Open folder"); the web client
answers `null` and the button is not drawn.

The logo field on the same screen says "Choose a PNG or JPEG" rather than
"Drop a PNG or JPEG here" when `importSource()` is `native`, because Tauri
intercepts the HTML5 drop that field listens for.

## Not a deep link

The scheme is an in-process transport, not a URL another application on the
machine may hand this one. Registering it as a deep link would let any
program open `nigel://` with a path of its own choosing and have this
router answer it, which is exactly the assumption the no-session-guard
section above depends on: that nothing outside this process can address the
scheme. `crates/nigel-desktop/tauri.conf.json` carries no `plugins.deep-link`
block and the manifest carries no `tauri-plugin-deep-link` dependency;
`tests/no_deep_link.rs` fails the build if either reappears.

## The Swift shell's bridge: `nigel-ffi`

The Swift macOS shell (design: `docs/superpowers/specs/2026-09-14-swift-shell-design.md`)
hosts the same SPA in a `WKWebView` and answers `nigel://` from a
`WKURLSchemeHandler`. `crates/nigel-ffi` is the Rust half: a `staticlib` built
with UniFFI's proc macros, exporting one object.

| `NigelHost` | What it does |
|---|---|
| `new(data_dir, config_dir)` | Opens `<data_dir>/nigel.db` behind `build_desktop_router` and `trusted_origins()`, on its own multi-threaded tokio runtime, and fixes the process's config directory (`settings.json`, the invoicing and documents config) at `config_dir` through `settings::set_config_dir`. Both folders must exist: a missing books folder is `BooksFolderMissing` and a missing config folder `ConfigFolderMissing`, and neither is created. The config folder is fixed for the life of the process; a second host naming another is `ConfigFolderConflict` |
| `async request(SchemeRequest)` | Method, whole URL, headers and body in; status, headers and body out, through `scheme::answer`. A request the `http` crate cannot represent is answered 400 without reaching the router |
| `stage_file(path)` | `uploads::stage_file` into the open books' spool, answering the same `StagedUpload` the Tauri shell's commands do |
| `data_dir()` | The folder the open books live in, following a switch made in Settings |

The FFI is one request method rather than a typed call per route, so every
route's validation stays in the router and a new route costs the shell
nothing. The SPA is embedded in the library the way it is in every build:
rust-embed over `web/dist`. `request` runs the router on the host's runtime,
so Swift may await it from any executor; releasing the host shuts the runtime
down without blocking.

`SchemeRequest` and `SchemeResponse` are a plain HTTP exchange, so a
transport that sends the same request to a remote `nigel serve` can reuse the
shapes. It is not a drop-in, though: `nigel serve`'s router has a session
guard and trusts its own host, not `nigel://localhost`. A remote transport
must rewrite `Host` and `Origin` to the server's before sending, and attach
the session credential itself, natively, so the page never holds it.

The host knows nothing about where its folders came from. In the sandboxed
app `$HOME` is the app container, so the shell owns both locations: it
resolves the security-scoped bookmark to the books folder, starts access, and
passes that folder and its config folder to `NigelHost::new`. `nigel-core`
sees only paths and derives neither from the home directory.

```bash
cd crates/nigel-ffi && cargo test -- --test-threads=1
```

`tests/host.rs` drives `request` over `testutil`-seeded databases (GET, JSON
POST, multipart, staging) from a non-tokio executor, as Swift calls it. On
`aarch64-apple-darwin`, SQLCipher's vendored OpenSSL links statically and
reqwest uses native-tls over Security.framework, so nothing in the library
asks for a system OpenSSL.

## Where builds come from

This repository's CI compiles and tests the desktop crate on Linux and macOS, and publishes
no installer and no update manifest. The signed, notarized, auto-updating build is sold, and
`backlog/decisions/decision-3` records why producing it here would put the packaging, the
signing identities and the update feed in a public repository.

The source is MIT, so building it yourself is supported rather than tolerated: a checkout,
`npm run build` in `web/`, and `cargo run` from `crates/nigel-desktop` gives the same
application without the packaging.
