# Desktop Seams Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the SPA and the HTTP layer the two seams a desktop client needs — an export path that does not depend on the webview downloading, and a router whose trusted origin and session guard are chosen at construction — without changing anything the web build does today.

**Architecture:** Exports stop being a bare URL the screen puts in an `href` and become an `ExportTarget`, a discriminated union the api client returns: the web client always answers `{kind: 'href'}`, so the browser keeps today's anchors byte for byte, and a desktop client can later answer `{kind: 'action'}` with a handler that fetches the bytes and saves them natively. On the Rust side, `build_router` gains a sibling that takes the trusted host list as a parameter and omits the session layer entirely, so "no session guard" is a router that was never built with one rather than a flag checked at request time.

**Tech Stack:** TypeScript, Lit 3, vitest, axe; Rust, axum, tower.

**Spec:** `backlog/tasks/task-33.2 - Tauri-2-app-shell-and-backend-transport-decision.md`, plus `backlog/decisions/decision-1 - Desktop-transport-custom-URI-scheme-over-the-existing-axum-router.md`. The probe results that force the export seam are recorded in the task's notes and on branch `probe/33.2-download-scheme`.

This plan covers acceptance criteria **#3, #6, #7** and the api-seam consequence of **#5**. Criteria **#1, #4, #8** — the Tauri crate, the dev workflow docs, and the deep-link exclusion — belong to the follow-on plan that builds the shell on these seams.

## Global Constraints

- **The web build's behaviour does not change.** Same anchors, same `download` attributes, same URLs. A user of `nigel serve` must not be able to tell this plan landed. Any diff to rendered web output is a defect.
- **Screens never spell an endpoint.** `web/apps/app/src/api/client.ts` is the only file that builds a URL. This is the existing rule; the seam exists because of it.
- **Every visual change ships through `@nigel/ui`** with a co-located `.preview.ts` covering the visible states and `describePreviewA11y` passing with zero violations. See CLAUDE.md, Component-First UI Workflow.
- **Tests run serially:** `cargo test -- --test-threads=1`. The DB password is a process global.
- **CI runs, in order:** `./scripts/check-no-real-data.sh`, `npm run lint`, `npm run typecheck`, `npm test`, `npm run build`, `cargo fmt --check`, `cargo clippy -- -D warnings`, then four `cargo test` variants (default, `--no-default-features`, `--no-default-features --features serve`, `-p nigel-core`). A task is not done until the ones it can affect pass locally.
- **No real book data**, in any file or commit message. Fixture cast only: Acme, Cedar Systems, Juniper Labs, Harbor & Vale, Globex, Initech.
- **No provenance comments.** Describe the current state; `git log` carries history.

---

### Task 1: The `ExportTarget` type and the client that answers it

**Files:**
- Modify: `web/apps/app/src/api/client.ts`
- Modify: `web/apps/app/src/__mocks__/fake-api-client.ts`
- Test: `web/apps/app/src/api/client.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `export type ExportTarget = { kind: 'href'; href: string } | { kind: 'action'; run: () => Promise<void>; filename: string }`, exported from `web/apps/app/src/api/client.ts`; `exportTarget(report: ReportSlug, format: ExportFormat, params?: ExportParams): ExportTarget` and `invoicePreviewTarget(number: number): ExportTarget` on the `ApiClient` interface and on `FakeApiClient`.

Why a union rather than replacing `exportUrl` with a handler: the browser downloads better than we can — it streams, it names the file from `Content-Disposition`, and it never holds a PDF in memory. The probe found that a webview under a custom scheme cannot do any of that, but that is the desktop's problem, not the web's. A union lets each client answer with what its platform can actually do, and lets the component keep rendering a real anchor wherever one works.

`exportUrl` and `invoicePreviewUrl` stay. The iframe in the send flow needs a URL and always will, and `invoicePreviewUrl(number, 'html')` is that URL.

- [ ] **Step 1: Write the failing test**

Add to `web/apps/app/src/api/client.test.ts`:

```ts
describe('exportTarget', () => {
  it('answers an href target pointing at the same address exportUrl builds', () => {
    const client = new FetchApiClient({ fetchImpl: vi.fn() });
    const target = client.exportTarget('pnl', 'pdf', { year: 2026 });

    expect(target).toEqual({
      kind: 'href',
      href: client.exportUrl('pnl', 'pdf', { year: 2026 }),
    });
  });

  it('answers an href target for an invoice preview pdf', () => {
    const client = new FetchApiClient({ fetchImpl: vi.fn() });
    const target = client.invoicePreviewTarget(41);

    expect(target).toEqual({
      kind: 'href',
      href: client.invoicePreviewUrl(41, 'pdf'),
    });
  });
});
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cd web && npm run test --workspace=@nigel/app -- client -t exportTarget`
Expected: FAIL — `client.exportTarget is not a function`.

- [ ] **Step 3: Add the type and the two methods**

In `web/apps/app/src/api/client.ts`, above the `ApiClient` interface:

```ts
/**
 * How a screen reaches an export: an address it can put in an anchor, or an
 * action it must run.
 *
 * The browser downloads better than the app can, so `href` is what every
 * client answers where a plain link works, and the component renders a real
 * anchor. A webview serving the app from a custom URI scheme cannot download
 * from a navigation at all, so a desktop client answers `action` instead and
 * carries the saving itself. `filename` is what the file should be called,
 * which only the `action` form has to say — an `href` download takes its name
 * from `Content-Disposition`.
 */
export type ExportTarget =
  | { kind: 'href'; href: string }
  | { kind: 'action'; run: () => Promise<void>; filename: string };
```

On the `ApiClient` interface, directly below `exportUrl`'s declaration:

```ts
  /**
   * The same export as `exportUrl`, in the form the running client can use.
   * Screens bind this rather than the URL, so a client that cannot download
   * from a link is a different answer here rather than a different screen.
   */
  exportTarget(
    report: ReportSlug,
    format: ExportFormat,
    params?: ExportParams,
  ): ExportTarget;
```

And below `invoicePreviewUrl`'s declaration:

```ts
  /** The invoice's PDF, in the form the running client can use. */
  invoicePreviewTarget(number: number): ExportTarget;
```

In `class FetchApiClient` (the concrete client, at `client.ts:434`), directly below the `exportUrl` implementation:

```ts
  exportTarget(
    report: ReportSlug,
    format: ExportFormat,
    params: ExportParams = {},
  ): ExportTarget {
    return { kind: 'href', href: this.exportUrl(report, format, params) };
  }
```

and below `invoicePreviewUrl`'s implementation:

```ts
  invoicePreviewTarget(number: number): ExportTarget {
    return { kind: 'href', href: this.invoicePreviewUrl(number, 'pdf') };
  }
```

- [ ] **Step 4: Mirror both on the fake client**

In `web/apps/app/src/__mocks__/fake-api-client.ts`, below its `exportUrl`:

```ts
  exportTarget(
    report: ReportSlug,
    format: ExportFormat,
    params: ExportParams = {},
  ): ExportTarget {
    return { kind: 'href', href: this.exportUrl(report, format, params) };
  }

  invoicePreviewTarget(number: number): ExportTarget {
    return { kind: 'href', href: this.invoicePreviewUrl(number, 'pdf') };
  }
```

Import `ExportTarget` from `../api/client` alongside the types it already imports from there.

- [ ] **Step 5: Run the test and the typechecker**

Run: `cd web && npm run test --workspace=@nigel/app -- client -t exportTarget && npm run typecheck`
Expected: both PASS. The typecheck matters most — it proves `FakeApiClient` still satisfies `ApiClient`.

- [ ] **Step 6: Commit**

```bash
git add web/apps/app/src/api/client.ts web/apps/app/src/api/client.test.ts web/apps/app/src/__mocks__/fake-api-client.ts
git commit -m "Answer exports as a target the client can actually use"
```

---

### Task 2: `wc-export-links` renders either form

**Files:**
- Modify: `web/packages/ui/src/components/wc-export-links.ts`
- Modify: `web/packages/ui/src/components/wc-export-links.preview.ts`
- Test: `web/packages/ui/src/components/wc-export-links.test.ts`

**Interfaces:**
- Consumes: the shape of `ExportTarget` from Task 1. The component must **not** import from `web/apps/app` — `@nigel/ui` does not depend on the app. Declare the same shape locally and export it:
  `export type ExportTarget = { kind: 'href'; href: string } | { kind: 'action'; run: () => Promise<void>; filename: string }`
- Produces: `textTarget` and `pdfTarget` properties on `wc-export-links`, both `ExportTarget | null`, defaulting to `null`.

The existing `textHref` / `pdfHref` string properties stay and keep working. A target, when set, wins. Keeping both is what lets Task 3 move one screen at a time and lets the preview harness show both forms side by side.

- [ ] **Step 1: Write the failing tests**

Add to `web/packages/ui/src/components/wc-export-links.test.ts`:

```ts
describe('export targets', () => {
  it('renders an anchor for an href target, exactly as a plain href does', async () => {
    const el = mount();   // the helper this test file already uses
    el.textTarget = { kind: 'href', href: '/api/exports/pnl?format=text' };
    await el.updateComplete;

    const anchor = el.shadowRoot!.querySelector('a')!;
    expect(anchor.getAttribute('href')).toBe('/api/exports/pnl?format=text');
    expect(anchor.hasAttribute('download')).toBe(true);
    expect(el.shadowRoot!.querySelector('button[data-export="text"]')).toBeNull();
  });

  it('renders a button for an action target and runs it on click', async () => {
    let ran = 0;
    const el = mount();   // the helper this test file already uses
    el.textTarget = {
      kind: 'action',
      filename: 'pnl.txt',
      run: async () => { ran += 1; },
    };
    await el.updateComplete;

    const button = el.shadowRoot!.querySelector<HTMLButtonElement>('button[data-export="text"]')!;
    expect(button).not.toBeNull();
    button.click();
    await el.updateComplete;

    expect(ran).toBe(1);
  });

  it('does not run an action while busy', async () => {
    let ran = 0;
    const el = mount({ busy: true });   // match how neighbouring tests set busy
    el.textTarget = {
      kind: 'action',
      filename: 'pnl.txt',
      run: async () => { ran += 1; },
    };
    await el.updateComplete;

    el.shadowRoot!.querySelector<HTMLButtonElement>('button[data-export="text"]')!.click();
    await el.updateComplete;

    expect(ran).toBe(0);
  });
});
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cd web && npm run test --workspace=@nigel/ui -- wc-export-links -t "export targets"`
Expected: FAIL — the first on `textTarget` not being a property that renders, the second on the button not existing.

- [ ] **Step 3: Implement**

In `web/packages/ui/src/components/wc-export-links.ts`, add the type above the class:

```ts
/**
 * How this component reaches an export.
 *
 * Declared here rather than imported: `@nigel/ui` does not depend on the app.
 * The api client produces the same shape.
 */
export type ExportTarget =
  | { kind: 'href'; href: string }
  | { kind: 'action'; run: () => Promise<void>; filename: string };
```

Add the two properties beside the existing `textHref` / `pdfHref`:

```ts
  /** Wins over `textHref` when set. */
  @property({ attribute: false })
  textTarget: ExportTarget | null = null;

  /** Wins over `pdfHref` when set. */
  @property({ attribute: false })
  pdfTarget: ExportTarget | null = null;
```

Add one renderer both labels go through, and a click handler:

```ts
  private runAction = async (event: Event, target: ExportTarget): Promise<void> => {
    event.preventDefault();
    if (this.busy || target.kind !== 'action') return;
    await target.run();
  };

  /**
   * An anchor where the platform can download from a link, a button where it
   * cannot. Both carry the same label, so the accessible name does not depend
   * on which platform is running.
   */
  private renderTarget(
    slot: 'text' | 'pdf',
    label: string,
    target: ExportTarget | null,
    href: string,
  ) {
    if (target?.kind === 'action') {
      return html`
        <button
          type="button"
          data-export=${slot}
          ?disabled=${this.busy}
          @click=${(event: Event) => this.runAction(event, target)}
        >
          <wc-icon-download></wc-icon-download>
          ${label}
        </button>
      `;
    }

    return html`
      <a
        href=${target?.kind === 'href' ? target.href : href}
        download
        aria-disabled=${this.busy ? 'true' : nothing}
        @click=${this.blockWhileBusy}
      >
        <wc-icon-download></wc-icon-download>
        ${label}
      </a>
    `;
  }
```

Replace the anchor inside `renderPdf`'s success branch with `return this.renderTarget('pdf', 'PDF', this.pdfTarget, this.pdfHref);`, and replace the text anchor in `render()` with `${this.renderTarget('text', 'Text', this.textTarget, this.textHref)}`. Leave the `pdfAvailable === false` branch exactly as it is.

- [ ] **Step 4: Run the tests**

Run: `cd web && npm run test --workspace=@nigel/ui -- wc-export-links`
Expected: PASS, including every test that existed before — those are what prove the href path is unchanged.

- [ ] **Step 5: Add the action states to the preview**

In `web/packages/ui/src/components/wc-export-links.preview.ts`, add two states beside the existing ones:

```ts
  {
    name: 'action targets',
    render: () => html`
      <wc-export-links
        .textTarget=${{ kind: 'action', filename: 'pnl.txt', run: async () => {} }}
        .pdfTarget=${{ kind: 'action', filename: 'pnl.pdf', run: async () => {} }}
      ></wc-export-links>
    `,
  },
  {
    name: 'action targets, busy',
    render: () => html`
      <wc-export-links
        busy
        .textTarget=${{ kind: 'action', filename: 'pnl.txt', run: async () => {} }}
        .pdfTarget=${{ kind: 'action', filename: 'pnl.pdf', run: async () => {} }}
      ></wc-export-links>
    `,
  },
```

`describePreviewA11y` picks these up automatically — do not restate them in the test file.

- [ ] **Step 6: Run the package's tests and lint**

Run: `cd web && npm run test --workspace=@nigel/ui && npm run lint && npm run typecheck`
Expected: all PASS, a11y included, zero violations.

- [ ] **Step 7: Commit**

```bash
git add web/packages/ui/src/components/wc-export-links.ts web/packages/ui/src/components/wc-export-links.preview.ts web/packages/ui/src/components/wc-export-links.test.ts
git commit -m "Let export links render an action where a link cannot download"
```

---

### Task 3: The reports screen and the send dialog pass targets

**Files:**
- Modify: `web/apps/app/src/screens/reports.ts:633-634`
- Modify: `web/packages/ui/src/components/wc-send-dialog.ts:312,448`
- Modify: `web/packages/ui/src/components/wc-send-dialog.preview.ts`
- Test: `web/apps/app/src/screens/reports.test.ts`, `web/packages/ui/src/components/wc-send-dialog.test.ts`

**Interfaces:**
- Consumes: `exportTarget` / `invoicePreviewTarget` from Task 1; `textTarget` / `pdfTarget` from Task 2.
- Produces: nothing new. After this task no screen passes an export href.

- [ ] **Step 1: Write the failing test**

Add to `web/apps/app/src/screens/reports.test.ts`:

```ts
it('hands the export links a target rather than a bare href', async () => {
  const client = new FakeApiClient();
  const screen = await mount(client);   // `mount` at reports.test.ts:51

  const links = screen.shadowRoot!.querySelector('wc-export-links') as HTMLElement & {
    textTarget: { kind: string; href?: string } | null;
    pdfTarget: { kind: string; href?: string } | null;
  };

  expect(links.textTarget).toEqual({ kind: 'href', href: client.exportUrl('pnl', 'text', {}) });
  expect(links.pdfTarget).toEqual({ kind: 'href', href: client.exportUrl('pnl', 'pdf', {}) });
});
```

`mount` at `reports.test.ts:51` and `seeded()` at line 26 are the helpers this file uses; match how the neighbouring tests call them rather than introducing a new one.

- [ ] **Step 2: Run it and watch it fail**

Run: `cd web && npm run test --workspace=@nigel/app -- reports -t "target rather than a bare href"`
Expected: FAIL — `textTarget` is `null`.

- [ ] **Step 3: Switch the reports screen**

In `web/apps/app/src/screens/reports.ts`, replace lines 633-634:

```ts
              .textTarget=${this.client.exportTarget(slug, 'text', request)}
              .pdfTarget=${this.client.exportTarget(slug, 'pdf', request)}
```

- [ ] **Step 4: Switch the send dialog's PDF link**

`wc-send-dialog` renders one PDF link with `download` at line 448. Give it the same treatment: add

```ts
  /** Wins over `pdfHref` when set. */
  @property({ attribute: false })
  pdfTarget: ExportTarget | null = null;
```

importing `ExportTarget` from `./wc-export-links`. Add a renderer above `render()`:

```ts
  /** The same treatment `wc-export-links` gives its links, for the one here. */
  private renderPdfLink() {
    const target = this.pdfTarget;
    if (target?.kind === 'action') {
      return html`<button
        class="caveat"
        type="button"
        data-pdf-link
        @click=${() => void target.run()}
      >
        Download the PDF
      </button>`;
    }
    const href = target?.kind === 'href' ? target.href : this.pdfHref;
    return html`<a class="caveat" href=${href} data-pdf-link download
      >Download the PDF</a
    >`;
  }
```

and replace lines 447-450 with:

```ts
      ${this.pdfAvailable
        ? this.renderPdfLink()
```

leaving the `: html\`<p class="caveat" data-pdf-unavailable>\`` branch exactly as it stands. `data-pdf-link` is on both forms, and the label is the same string in both, so the existing tests keep selecting it and the accessible name does not depend on the platform.

`web/apps/app/src/screens/invoices.ts:1051` constructs the dialog. Add `.pdfTarget=${this.client.invoicePreviewTarget(this.sending.number)}` to that element, using whatever the surrounding lines already call the invoice being sent rather than assuming `this.sending`.

- [ ] **Step 5: Add an action state to the send dialog preview**

```ts
  {
    name: 'pdf as an action',
    render: () => html`
      <wc-send-dialog
        open
        .pdfTarget=${{ kind: 'action', filename: 'invoice-41.pdf', run: async () => {} }}
      ></wc-send-dialog>
    `,
  },
```

- [ ] **Step 6: Run every web test**

Run: `cd web && npm test && npm run lint && npm run typecheck && npm run build`
Expected: all PASS. Any existing test that asserts on the rendered anchors is the check that the web build did not change.

- [ ] **Step 7: Commit**

```bash
git add web/
git commit -m "Pass export targets from the screens that own them"
```

---

### Task 4: The host guard takes its trusted hosts as a parameter

**Files:**
- Modify: `crates/nigel-core/src/server/auth.rs:27,53-100,~120` (the `LOCAL_HOSTS` const, `host_is_local`, `origin_is_local`, `host_guard`)
- Modify: `crates/nigel-core/src/server/mod.rs:180`
- Test: `crates/nigel-core/src/server/auth.rs` (its own `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct TrustedOrigins { hosts: Vec<String> }` with `TrustedOrigins::loopback() -> Self` and `TrustedOrigins::exactly(hosts: Vec<String>) -> Self`; `pub fn host_is_trusted(&self, host: &str) -> bool` and `pub fn origin_is_trusted(&self, origin: &str) -> bool` on it; `pub async fn host_guard(State(trusted): State<TrustedOrigins>, req: Request, next: Next) -> Response`.

`TrustedOrigins::loopback()` holds exactly today's `["localhost", "127.0.0.1", "::1"]`, so `nigel serve` is unchanged. The desktop shell will construct `TrustedOrigins::exactly(vec!["nigel.localhost".into()])` — its Windows origin form — and the `nigel://localhost` form on the other two platforms.

Exact matching stays exactly as strict as it is now: `127.0.0.1.evil.com` and `localhost.evil.com` must still be refused, because they resolve wherever their owner points them.

- [ ] **Step 1: Write the failing tests**

In `crates/nigel-core/src/server/auth.rs`'s test module:

```rust
#[test]
fn loopback_trust_is_what_serve_has_always_allowed() {
    let trusted = TrustedOrigins::loopback();
    assert!(trusted.host_is_trusted("localhost"));
    assert!(trusted.host_is_trusted("127.0.0.1:5731"));
    assert!(trusted.host_is_trusted("[::1]:5731"));
    assert!(!trusted.host_is_trusted("127.0.0.1.evil.com"));
    assert!(!trusted.host_is_trusted("localhost.evil.com"));
    assert!(!trusted.host_is_trusted("nigel.localhost"));
}

#[test]
fn a_configured_origin_is_trusted_and_loopback_then_is_not() {
    let trusted = TrustedOrigins::exactly(vec!["nigel.localhost".to_string()]);
    assert!(trusted.host_is_trusted("nigel.localhost"));
    assert!(trusted.origin_is_trusted("http://nigel.localhost"));
    // A desktop router has no reason to answer the loopback interface, and
    // trusting it anyway would hand the guard back the hole it exists to close.
    assert!(!trusted.host_is_trusted("127.0.0.1:5731"));
    assert!(!trusted.host_is_trusted("localhost"));
    assert!(!trusted.host_is_trusted("evil.nigel.localhost"));
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core --features serve auth:: -- --test-threads=1`
Expected: FAIL to compile — `TrustedOrigins` does not exist.

- [ ] **Step 3: Implement**

Replace the `LOCAL_HOSTS` const with:

```rust
/// The hosts that may address a router.
///
/// A parameter rather than a constant because the desktop client is served from
/// a custom URI scheme whose origin is neither loopback nor the same string on
/// every platform. Matching stays exact — `127.0.0.1.evil.com` and
/// `localhost.evil.com` resolve wherever their owner points them.
#[derive(Clone, Debug)]
pub struct TrustedOrigins {
    hosts: Vec<String>,
}

impl TrustedOrigins {
    /// What `nigel serve` allows: the loopback interface, by any of its names.
    pub fn loopback() -> Self {
        Self {
            hosts: ["localhost", "127.0.0.1", "::1"]
                .iter()
                .map(|h| (*h).to_string())
                .collect(),
        }
    }

    /// Exactly these hosts and nothing else — not loopback unless it is listed.
    pub fn exactly(hosts: Vec<String>) -> Self {
        Self { hosts }
    }

    /// True when a `Host` header names a trusted host, with any port.
    pub fn host_is_trusted(&self, host: &str) -> bool {
        let host = host.trim();
        if host.is_empty() {
            return false;
        }

        let bare = if let Some(rest) = host.strip_prefix('[') {
            let Some((inner, after)) = rest.split_once(']') else {
                return false;
            };
            match after.strip_prefix(':') {
                Some(port) if is_port(port) => {}
                None if after.is_empty() => {}
                _ => return false,
            }
            inner
        } else {
            match host.split_once(':') {
                Some((h, port)) if is_port(port) => h,
                Some(_) => return false,
                None => host,
            }
        };

        self.hosts.iter().any(|h| bare.eq_ignore_ascii_case(h))
    }

    /// True when an `Origin` header names an http(s) trusted origin. A literal
    /// `null` origin (sandboxed iframes, `file://` documents) is never trusted.
    pub fn origin_is_trusted(&self, origin: &str) -> bool {
        let Some(rest) = strip_scheme(origin.trim()) else {
            return false;
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let authority = match authority.rsplit_once('@') {
            Some((_, host)) => host,
            None => authority,
        };
        self.host_is_trusted(authority)
    }
}
```

These bodies are the existing `host_is_local` and `origin_is_local` moved across with one line changed — `LOCAL_HOSTS.iter()` becomes `self.hosts.iter()`. The free functions `is_port` and `strip_scheme` stay where they are. Do not rewrite the parsing: it is what the existing tests cover, and it is the part that refuses `127.0.0.1.evil.com`.

**Keep `host_is_local` and `origin_is_local`**, rewritten as one-line delegations:

```rust
/// True when a `Host` header names the loopback interface, with any port.
pub fn host_is_local(host: &str) -> bool {
    TrustedOrigins::loopback().host_is_trusted(host)
}

/// True when an `Origin` header names an http(s) loopback origin. A literal
/// `null` origin (sandboxed iframes, `file://` documents) is never local.
pub fn origin_is_local(origin: &str) -> bool {
    TrustedOrigins::loopback().origin_is_trusted(origin)
}
```

They have four existing test callers in this module's `mod tests` — the table-driven
cases at roughly lines 197-246 that assert `127.0.0.1.evil.com` and
`localhost.evil.com` are refused. Those tests are the regression net for the
parsing this task moves, so they must keep compiling and passing **unchanged**. A
green run of them after the move is the proof the move was faithful.

Change `host_guard` to take `State<TrustedOrigins>` and call the methods, and in `crates/nigel-core/src/server/mod.rs:180` change `.layer(middleware::from_fn(auth::host_guard))` to `.layer(middleware::from_fn_with_state(auth::TrustedOrigins::loopback(), auth::host_guard))`.

- [ ] **Step 4: Run the server tests**

Run: `cargo test -p nigel-core --features serve -- --test-threads=1`
Expected: PASS, including `cross_origin_request_is_forbidden` at `mod.rs:244` — that test is the proof `nigel serve` still refuses what it refused before.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/server/auth.rs crates/nigel-core/src/server/mod.rs
git commit -m "Take the trusted hosts as a parameter rather than a constant"
```

---

### Task 5: A router built without a session guard

**Files:**
- Modify: `crates/nigel-core/src/server/mod.rs:165-185`
- Test: `crates/nigel-core/src/server/mod.rs` (its own `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `TrustedOrigins` from Task 4.
- Produces: `pub fn build_router(state: AppState) -> Router` — unchanged signature and behaviour — and `pub fn build_desktop_router(state: AppState, trusted: TrustedOrigins) -> Router`, which layers the same routes and the same host guard but never attaches `auth::session_guard`.

AC #7 requires this be a property of construction, not a runtime flag. So there is no `desktop: bool` reaching a guard: the two functions build two routers, and the session layer exists in only one of them. A reader can see which router is which by reading the constructor, and a test can prove it without setting any state.

Why no session guard is right rather than lax: the browser needs one because anything on the machine can reach a loopback port. The desktop webview reaches the router through a custom URI scheme registered inside the process, which nothing else on the machine can address, so a session cookie would be a token the app hands to itself.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn the_desktop_router_answers_an_api_route_without_a_session() {
    let state = test_state().await;          // the helper this module's tests already use
    let router = build_desktop_router(
        state,
        auth::TrustedOrigins::exactly(vec!["nigel.localhost".to_string()]),
    );

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/accounts")
                .header("host", "nigel.localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn the_served_router_still_refuses_the_same_request() {
    let state = test_state().await;
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/accounts")
                .header("host", "127.0.0.1:5731")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
```

Use whatever this module's tests already call to build state and to send a request — copy the surrounding tests' style rather than introducing a new helper.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p nigel-core --features serve desktop_router -- --test-threads=1`
Expected: FAIL to compile — `build_desktop_router` does not exist.

- [ ] **Step 3: Implement**

Factor the shared part of the current `build_router` into a private function that returns the routes plus the non-session layers, then:

```rust
/// The router `nigel serve` runs: loopback-only, and every api route behind a
/// session cookie.
pub fn build_router(state: AppState) -> Router {
    let api = routes::api_router(&state).layer(middleware::from_fn_with_state(
        state.clone(),
        auth::session_guard,
    ));
    finish_router(state, api, auth::TrustedOrigins::loopback())
}

/// The router a desktop shell serves over its custom URI scheme.
///
/// No session guard, because the scheme is registered inside this process and
/// nothing else on the machine can address it — a cookie here would be a token
/// the app issues to itself. The absence is structural: this router is never
/// built with the layer, rather than built with one that is asked to stand down.
pub fn build_desktop_router(state: AppState, trusted: auth::TrustedOrigins) -> Router {
    let api = routes::api_router(&state);
    finish_router(state, api, trusted)
}
```

with `finish_router` carrying the `.nest("/api", api)`, the static-file routes, `host_guard` with the passed `TrustedOrigins`, and `security_headers` exactly as `build_router` layers them today.

- [ ] **Step 4: Run the whole core suite**

Run: `cargo test -p nigel-core -- --test-threads=1`
Expected: PASS, all 962-plus. Then `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.

- [ ] **Step 5: Commit**

```bash
git add crates/nigel-core/src/server/mod.rs
git commit -m "Build the desktop router without a session guard"
```

---

### Task 6: Record the seams

**Files:**
- Modify: `docs/api.md`
- Modify: `docs/design-constraints.md`

**Interfaces:**
- Consumes: everything above.
- Produces: nothing code depends on.

- [ ] **Step 1: Document the two routers in `docs/api.md`**

Add to the security-model section:

```markdown
Two routers are built from the same routes. `build_router` is what `nigel serve`
runs: it trusts the loopback interface by its three names and puts every `/api`
route behind a session cookie, because anything on the machine can reach a
loopback port. `build_desktop_router` takes its trusted origin as a parameter and
carries no session layer at all — a desktop shell serves it over a custom URI
scheme registered inside its own process, which nothing else on the machine can
address. The absence is structural rather than conditional: the layer is not
attached, so there is no flag that could be set wrongly.
```

- [ ] **Step 2: Document the export seam in `docs/design-constraints.md`**

Add one bullet:

```markdown
- **An export is a target, not a URL.** `exportTarget` and `invoicePreviewTarget`
  answer either an address to put in an anchor or an action to run, and screens
  bind whichever they are given. The web client always answers an address,
  because the browser streams the download, names the file from
  `Content-Disposition`, and never holds a PDF in memory. A webview serving the
  app from a custom URI scheme can do none of that — a probe across WebKitGTK,
  WKWebView and WebView2 found that navigation downloads work on Windows only,
  and client-side blob saves work everywhere except Windows — so a desktop client
  answers an action that fetches the bytes and saves them natively. Screens do
  not branch on which is running.
```

- [ ] **Step 3: Check the docs against the rules and commit**

Run: `./scripts/check-no-real-data.sh` and judge it by its exit status.

```bash
git add docs/api.md docs/design-constraints.md
git commit -m "Document the export target and the two routers"
```

---

## Self-Review

**Spec coverage.** AC #3 — Tasks 1-3 keep one SPA working in both modes, and the web tests that already assert on the rendered anchors are what prove the web half. AC #6 — Task 4. AC #7 — Task 5, as construction rather than a flag, with a test per router. AC #5's consequence — Tasks 1-3. AC #1, #4, #8 are explicitly out of scope and named as the follow-on plan's, so the gap is deliberate rather than missed.

**Type consistency.** `ExportTarget` is declared twice on purpose — once in `web/apps/app/src/api/client.ts` and once in `web/packages/ui/src/components/wc-export-links.ts` — because `@nigel/ui` must not depend on the app. The two declarations are character-identical and Task 3 is where a drift between them would surface as a type error. `textTarget` / `pdfTarget` are the property names in Tasks 2 and 3 alike; `exportTarget` / `invoicePreviewTarget` are the method names in Tasks 1 and 3 alike.

**Known soft spots, for the executor rather than the reviewer.** Three places name a line number or a helper that may have moved: `reports.ts:633-634`, `wc-send-dialog.ts:312,448`, and the test helpers in `reports.test.ts` and the server test modules. Each step says to follow the surrounding code rather than invent a name. If a helper does not exist under the name given, that is the plan being stale, not a licence to add one.


## Addendum: running the web tests

`npx vitest run <path>` from `web/` does **not** work. There is no root vitest
config, so the jsdom environment is never applied and the run fails wholesale
with `document is not defined` — over a thousand failures on a clean tree. Use
the workspace scripts, which are also what CI runs:

```bash
cd web && npm run test --workspace=@nigel/ui      # one package
cd web && npm run test --workspace=@nigel/app
cd web && npm test                                # all three, as CI does
```

`@open-wc/testing` is not a dependency of this repo. Its `fixture`/`html`
helpers are unavailable; each test file has its own `mount()` DOM helper, and a
new test uses the one beside it.
