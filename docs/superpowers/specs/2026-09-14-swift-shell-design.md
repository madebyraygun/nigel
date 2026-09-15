# A Swift shell for the Mac app — design

A native macOS shell, written in Swift, that hosts the existing SPA in a
`WKWebView` and owns everything around it: the menu bar, the sidebar, the
toolbar, the command palette, the menu bar extra, notifications, file dialogs,
window lifecycle, unlock, and updates. The screens stay web. On macOS it
replaces the Tauri shell; Tauri stays for Windows and Linux.

## The gap

The Tauri shell works, and decision-1's transport (the SPA and the JSON API
served over one in-process custom scheme) is the right one. What remains on the
desktop epic is almost entirely chrome: TASK-33.14 (settings window), 33.19
(directory chooser), 33.20 (unified title bar), 33.22 (menu bar), 33.23 (window
lifecycle), 33.24 (context menu, keystroke beep), 33.25 (white flash), 33.26
(sidebar vibrancy), 33.27 (window menu control), 33.4 (keychain unlock), 33.5
(updater). Every one of those is a fight with Tauri's abstraction over AppKit,
and 33.26 needs a private-API flag that forecloses the App Store. In Swift each
one is a first-class, public API.

A full SwiftUI port was sized separately and rejected: roughly 2.5 to 3 times
the wren app, and, unlike wren, it would leave a permanent second frontend,
because the SPA is also the browser UI, the Windows and Linux desktop UI, and
the base for Nigel Cloud. The shell captures most of what a port would deliver
for a fraction of the build and near-zero ongoing overhead.

## Decisions

| Question | Decision | Rejected |
|---|---|---|
| Screens | The SPA in `WKWebView`, unchanged | SwiftUI port (second frontend forever) |
| Transport | `WKURLSchemeHandler` for `nigel://localhost`, answered by the in-process axum router through `nigel-ffi`; same origin, same host guard, no session, exactly as Tauri | Loopback port (decision-1's reasons); `URLProtocol` |
| Core access | One UniFFI async `request()` plus file staging, over `build_desktop_router` | A typed 70-method FFI (duplicates every route's validation; decision-1 rejected the same shape) |
| Shell to SPA | The `invoke`/`listen` seam `DesktopApiClient` already takes, carried by `WKScriptMessageHandlerWithReply` and `evaluateJavaScript`; menu commands through 33.22's `MenuSource` | A second api client; host detection inside any `wc-*` |
| Sidebar | Native `NavigationSplitView`, rendering the SPA's own registry, which the page posts at boot | A hand-kept Swift copy of `registry.ts` (drifts) |
| Where `nigel://` resolves | A `Transport` protocol from day one: in-process router now, a remote `nigel serve` later | Baking the in-process case in |
| Tauri on macOS | Retired once the Swift shell reaches parity; Tauri stays for Windows and Linux | Two Mac shells |
| Distribution | Unchanged under decision-3: CI compiles and tests the shell on a macOS runner, signs and publishes nothing | |

## Layout

```
apps/Nigel/                          Xcode project (XcodeGen), macOS 15+, Swift 6
  Nigel/App/          NigelApp, AppDelegate, Commands, Sidebar, Toolbar, Palette,
                      MenuBarExtra, Notifications, SettingsWindow, Unlock
  Nigel/Web/          WebHost (WKWebView), SchemeHandler, Bridge (script messages)
  Packages/NigelKit/  platform-neutral SwiftPM: Transport, NigelClient (the few
                      typed calls the shell makes), Codable types, palette model,
                      Keychain
  Packages/NigelCore/ xcframework + generated Swift, produced by build-ffi.sh
crates/nigel-ffi/                    UniFFI staticlib over nigel-core (feature desktop)
scripts/build-ffi.sh                 npm run build → cargo → bindgen → xcframework
```

## The bridge — `crates/nigel-ffi`

A `staticlib` crate exporting one object:

```rust
#[uniffi::Object]
struct NigelHost { router: Router, runtime: Runtime, uploads_dir: PathBuf }

impl NigelHost {
    fn new(data_dir: Option<String>) -> Self;        // AppState::new + build_desktop_router
    async fn request(&self, req: Request) -> Response; // method, path, headers, body → status, headers, body
    fn stage_file(&self, path: String) -> Result<StagedUpload, FfiError>;
    fn data_dir(&self) -> String;
}
```

`request` is `crates/nigel-desktop/src/transport.rs::answer` with the Tauri
types swapped for plain records: carry the authority as `Host`, `oneshot` the
router, collect the body under the same 64 MB cap. `stage_file` is
`imports.rs::stage_file` unchanged. The `trusted_origins` and `scheme_url`
helpers move to a small shared module both shells use, so the origin cannot
drift between them. The SPA is embedded the way it is today: `rust-embed` over
`web/dist` inside the staticlib.

Linking: SQLCipher's vendored OpenSSL builds for `aarch64-apple-darwin`
statically and just costs minutes on the first build; reqwest lands on
Security.framework through native-tls. The `desktop` feature keeps the router
session-free, as it must (see `build_desktop_router`'s doc comment).

## The shell

**Web host.** One `WKWebView` per window, `nigel://` registered on its
configuration, navigated to `nigel://localhost/?shell=native`. The scheme
handler hands every request to `Transport.request` and streams the response
back. No cookies, no session, no CORS: the origin is the process, as with Tauri.

**Bridge.** A `WKUserScript` at document start defines
`window.__NIGEL_SHELL__ = { invoke, listen }` with the same shapes as
`window.__TAURI__.core.invoke` and `.event.listen`. `invoke` posts to a
`WKScriptMessageHandlerWithReply` and resolves with its reply; `listen`
subscribes to an in-page emitter the shell drives with `evaluateJavaScript`.
`createApiClient` learns to look for `__NIGEL_SHELL__` beside `__TAURI__`, and
`DesktopApiClient` is reused as is. Commands the shell answers: `save_export`,
`stage_import`, `pick_import_file`, `pick_directory`. Events the shell emits:
`menu-command {id}`, the four drag events, and `navigate {hash}`.

**Hosted mode.** With `?shell=native` the SPA hides its sidebar rail and
nav toggle, and posts its nav registry (`id`, label, order, profile-filtered)
through `invoke('nav', items)` on boot and whenever it changes. It also posts
`navigated {screen, title}` on every hash change. That is the whole SPA
change: one query flag, two informational messages, five lines in
`createApiClient`. No `wc-*` component learns it is hosted.

**Sidebar and toolbar.** `NavigationSplitView` renders the posted registry;
selecting an item sets `location.hash` in the page. The sidebar gets system
vibrancy and Reduce Transparency handling for free, with no private API, which
un-forecloses the App Store that TASK-33.26 would have closed. The toolbar
carries the sidebar toggle and the screen title from `navigated`; the web
header stays in the page for now.

**Menu bar.** The bar TASK-33.22 specifies, verbatim: app menu with About and
Settings (⌘,), File with Import Statement… (⌘O) and New Invoice (⌘N), Edit
built from the standard items (load-bearing for clipboard chords in
`WKWebView`) plus Find (⌘F), View with ⌘1–⌘9 over the posted registry and
Toggle Sidebar, Window as the NSApp windows menu, Help. Custom items emit
`menu-command` with the same ids 33.22's `MenuSource` defines, so that seam is
built once and serves both shells.

**Settings window.** A second `WKWebView` at `#/settings?shell=native`, fixed
size, ⌘W to close, reused rather than stacked (TASK-33.14). Data Directory
uses `NSOpenPanel` in directory mode (TASK-33.19).

**Window lifecycle.** Frame autosave, ⌘W closes, Dock click reopens, no white
flash (the window paints the page background before load), `WKWebView`'s
context menu and beep silenced (TASK-33.23, 33.24, 33.25).

**Command palette (⌘K).** A native panel over three sources: screens from the
registry, actions from the menu command table, and records searched through
the in-process router (`/api/clients`, `/api/invoices`, the register with `q`).
Selection navigates by hash with params, which `hash-route.ts` already parses.

**Menu bar extra.** Review queue count and overdue invoice count, read through
the router; Open Nigel, Import Statement…, New Invoice, Review. Refreshed on
activation and after any 2xx non-GET response passes through the scheme
handler, which needs no SPA cooperation.

**Notifications.** `UNUserNotificationCenter`, only for things that happen
while the user is not looking: an update available (`/api/status`), and a sync
finding a paid invoice when the shell runs `POST /api/invoices/sync` on a timer
with the window closed. Clicking opens the relevant screen by hash.

**Files and drops.** `NSOpenPanel` for imports, `NSSavePanel` for exports
(replacing `save_export`), file drops on the window routed through
`stage_file` and the existing `ImportSource` seam, and `CFBundleDocumentTypes`
for CSV and XLSX so a drop on the Dock icon or Finder's Open With lands in
the import flow.

**Unlock.** The shell calls `POST /api/status` and, when the database is
encrypted, `POST /api/unlock` itself before the page loads, with the password
from an opt-in Keychain item released by Touch ID through `LocalAuthentication`.
The SPA's unlock screen remains the fallback and the browser path (TASK-33.4).

**Updates.** A Sparkle hook pointed at the licensed feed TASK-115.2 owns;
nothing in this repository signs or publishes (decision-3, TASK-33.5).

## The contract, and why the overhead stays near zero

Everything the shell depends on is listed here. A change outside this list
costs the shell nothing.

| Surface | Owner | Changes when |
|---|---|---|
| `nigel://localhost/*` and the host guard | `nigel-core` | Never for a feature |
| `invoke` commands: `save_export`, `stage_import`, `pick_import_file`, `pick_directory` | shell | A new native affordance |
| `listen` events: `menu-command`, drag events, `navigate` | shell | A new menu item |
| `nav` and `navigated` messages | SPA | Never for a feature; a new screen flows through `registry.ts` as today |
| `?shell=native` hosted mode | SPA | Never |

A new screen: add it to `registry.ts`, and the sidebar, View menu and palette
pick it up. A new API route or report: zero shell work. A new export or import
kind: the `ExportTarget`/`ImportSource` seam, as today. Only a new native
affordance is shell work, and it is shell-only work.

## iOS

Yes, with the transport abstraction in place from day one. The reusable parts
are `NigelKit` (transport, client, types, keychain, palette model) and the web
host and bridge, which are `WKWebView` on both platforms. What differs is the
chrome (`NavigationSplitView` on iPad and a tab bar on iPhone instead of a
menu bar; share sheet and document picker instead of panels) and the transport:

- **`InProcessTransport`** resolves `nigel://localhost/*` against the FFI
  router. The Mac default. Compiles for iOS, but TASK-33.18 rules local books
  out on the device, and this design keeps that.
- **`RemoteTransport`** resolves the same scheme against a `nigel serve` on
  another machine, attaching the credential natively so the page never holds
  it, and proxies the SPA from the server too, so page and API can never
  disagree on version. The page does not know it is remote.

The server half is TASK-33.8's open question (how a remote instance is
addressed and authenticated across restarts, since `nigel serve` binds
loopback only and mints a per-run token) and, for shared servers, TASK-32's
authentication. Neither is shell work, and both are prerequisites for any
remote client, Swift or Tauri. Once 33.8 lands, remote mode on the Mac is a
`Transport` swap with a mode indicator, and the iOS app is that same shell
with iOS chrome. This is a better route to 33.18 than Tauri's iOS target,
which is Tauri's least mature platform wrapping the very `WKWebView` the Swift
shell uses directly.

## Effort

Calibrated against wren PR #8, where the shell-shaped tasks (37.4, 37.5,
37.16, 37.17, 37.19, 37.20) were about six of twenty-five tasks and roughly a
quarter of the Swift.

| Phase | Scope | Absorbs |
|---|---|---|
| 1. Host | `nigel-ffi`, `build-ffi.sh`, Xcode project, web host, scheme handler, bridge, hosted mode, imports, exports, drops; parity with Tauri | |
| 2. Chrome | Menu bar, settings window, native sidebar with vibrancy, unified title bar, window lifecycle, directory chooser, context menu and beep, white flash | 33.14, 33.19, 33.20, 33.22–33.27 |
| 3. Beyond Tauri | ⌘K, menu bar extra, notifications, Keychain and Touch ID unlock, Dock and Finder open, Sparkle hook | 33.4, 33.5 |
| 4. Retire | Tauri gated to Windows and Linux; docs (`desktop.md`, `native-feel.md`, `architecture.md`, README) | |
| Later | `RemoteTransport`, then the iOS target | 33.8, 33.18 |

Estimated at 12 to 15 subtasks, 4 to 6k lines of Swift, 300 to 500 lines of
Rust and about 150 lines of SPA change. About 60 percent of wren's epic, best
shipped as three PRs (phases 1, 2, 3 and 4 together). Two constraints wren did
not have: the shell cannot be built or run on the Linux development machine,
so phase 1 starts on a Mac and CI compiles it on a macOS runner, and the
device-only checks wren's handoff flagged (menu chords, Dock reopen,
notification click, drops) need a person at a Mac at the end of each phase.

## Testing

- **Rust.** `nigel-ffi` drives `request()` against `testutil`-seeded
  databases, mirroring `crates/nigel-desktop/tests/desktop_router.rs`; origin
  helpers keep their existing drift tests.
- **Swift.** swift-testing over the bridge codec, the scheme handler with an
  in-memory transport, palette ranking, registry decoding, and the keychain
  wrapper with a fake store. No UI snapshot tests.
- **Web.** `createApiClient` detects `__NIGEL_SHELL__`; hosted mode hides the
  rail and posts the registry; a fake shell exercises `nav` and `navigated`.
- **Manual, on device, per phase.** Clipboard chords in text fields, ⌘1–⌘9,
  Dock reopen, notification click, window and Dock drops, Touch ID unlock.

## Scope

Out: any `wc-*` port; moving the web header into the toolbar (a follow-up
once the shell is in); Windows and Linux (Tauri stays); local books on iOS;
App Store submission and the paywall (TASK-115); background sync beyond the
one timer notifications need.
