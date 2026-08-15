---
id: decision-1
title: 'Desktop transport: custom URI scheme over the existing axum router'
date: '2026-08-15 23:13'
status: accepted
---
## Context

TASK-33.2 framed the desktop transport as a choice between an embedded axum server on a
random port and native Tauri IPC commands wrapping the core directly. Reading the api seam
changed the shape of that question.

`ApiClient` (`web/apps/app/src/api/client.ts`) is a typed interface of roughly sixty
methods, and `FetchApiClient` already takes both a `baseUrl` and a `fetchImpl`. Two of its
methods — `exportUrl` and `invoicePreviewUrl` — answer an **address rather than bytes**, on
purpose: the webview streams a download and names it from `Content-Disposition` better than
the app can, and an invoice preview is framed in an `<iframe src>`. Both are consumed as
raw URLs by `<a download>` and by the frame.

That makes the three options cost wildly different amounts:

- **Native IPC** is the only option with no address to give those two consumers, so it
  needs a second mechanism for downloads and framing on top of reimplementing all sixty
  methods as a second `ApiClient`. It also duplicates the whole `/api` surface (33 endpoints
  documented in `docs/api.md`) as a parallel command layer, leaving two definitions of every
  route's validation to keep in step.
- **A loopback port** costs almost nothing: point `baseUrl` at `http://127.0.0.1:<port>/api`
  and today's server, token and SPA work unchanged.
- **A custom URI scheme** costs almost nothing *and* opens no socket.

The security difference between the last two is narrow but real. `127.0.0.1` is not private
to one process: any process on the machine, including another user account, can connect,
and a web page can probe the port to learn Nigel is running. Nigel's three layers
(loopback bind, Host/Origin guard, per-run 32-byte session cookie) hold that to
fingerprinting rather than data loss. The louder cost is a first-run firewall prompt on
macOS and Windows for an app whose entire premise is "no terminal involved".

Tauri 2 offers `register_asynchronous_uri_scheme_protocol`, whose handler receives an
`http::Request` and a responder taking an `http::Response`. An axum `Router` is a
`tower::Service` over exactly those types, and `src/server/` already drives it that way in
its own tests (`.oneshot(get_request("/api/ping"))`).

## Decision

The desktop shell registers one custom URI scheme and serves **both the SPA and the JSON
API from the same `build_router()` that `nigel serve` builds**, driven by a protocol handler
instead of a `TcpListener`. `static_files.rs` already serves `web/dist` through rust-embed
with an SPA fallback, so one router covers the whole app.

The window loads `nigel://localhost/` (macOS, Linux) or `http://nigel.localhost/`
(Windows, Android — Tauri uses the http form there). The app and its API are therefore
same-origin: no CORS, no port, no token handed over in a URL. `FetchApiClient` changes by
one constructor argument, computed at runtime because the URL form is platform-dependent.

Native IPC is rejected. It is the most idiomatic Tauri answer and the wrong one here: it
would fork the API surface in a repository whose api seam exists precisely so the SPA has
one definition of every endpoint — `exportUrl`'s own comment says a screen that spells its
own endpoint is a screen a Tauri client cannot host.

Three consequences are decided here rather than left to implementation:

1. **The Host/Origin guard gains a trusted-origin mode.** Today's exact-match list refuses
   both forms: `nigel://localhost` fails `origin_is_local` because `strip_scheme` accepts
   only http and https, and `nigel.localhost` is not in `LOCAL_HOSTS`. `auth.rs` takes a
   configured trusted origin instead of a hardcoded list, with its own tests.
2. **Desktop mode runs with the session guard off, deliberately and explicitly.** With no
   listener the only possible caller is the app's own webview, so the token defends nothing
   a protocol handler exposes. Half-keeping it does not work in any case: `exportUrl` and
   `invoicePreviewUrl` are consumed as bare URLs, which cannot carry a header, and cookies
   under a custom scheme are fiddly. This is a constructed property of the desktop router,
   not a runtime flag, and it is tested.
3. **This scheme is never registered as a deep link.** The in-webview protocol has no
   OS-level reachability; `tauri-plugin-deep-link` would hand the same scheme back exactly
   the exposure this decision removes.

## Consequences

**What gets cheaper.** No second `ApiClient`, no parallel command layer, no duplicated
validation. Web mode and desktop mode run the same handlers, so a route added once is
served by both. Existing router tests cover the desktop transport, because production and
tests drive the router through the same `tower::Service` call.

**What gets harder.** The Host/Origin guard is security-sensitive code that now varies by
build, and the base URL is computed rather than written down. Custom protocols are a
less-trodden path than binding a port; if one goes wrong, the fallback is a loopback port,
and the api seam makes that swap a one-line base URL change.

**What is not yet verified.** Whether each platform's webview honors `<a download>` and
`Content-Disposition` under a custom scheme. WebKitGTK and WebView2 are the risks. This is
the *first* implementation step, before any shell is built around it: if it fails,
`exportUrl`'s contract changes to a Tauri save dialog and the change belongs in the seam
rather than in screens. Nothing else in this decision depends on the answer.

**What this does not decide.** Remote mode (TASK-33.7) still adds a third backend behind
the same interface, pointing at a real HTTP origin. That is what the seam was built for and
is unaffected either way.

**Response bodies are buffered.** `UriSchemeResponder::respond` takes an owned body, so a
response is assembled in memory rather than streamed. Reports and invoice PDFs are small;
a future export large enough to matter would need the save-dialog path from the probe above.
