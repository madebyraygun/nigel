# Product foundation

What Nigel is, who it is for, how it makes money, and where the lines are. This is the
document the website, the pricing page and every monetization decision trace back to.
Prices themselves live on nigel.works, not here — this file carries the principles that
outlast any price change.

## What Nigel is

Nigel is local-first bookkeeping for freelancers and small consultancies: cash-basis,
single-entry, bank imports, rules-based categorization, invoicing and client documents —
in one binary, storing everything in one encrypted SQLite file the operator owns. A
personal profile serves household books with the same machinery.

**The wedge against QuickBooks** is ownership: no subscription hostage-taking, no cloud
custody of your books, no per-seat upsell. Your books are a file on your disk.
**The wedge against open-source ledgers** is polish: a real TUI, a real web UI, invoices
a client can pay online, documents a client can sign — without giving up the file.

## The promise

Three commitments, permanent:

1. **The source is MIT, forever.** Anyone can build the full app from this repository,
   and the build they make is not feature-gated, time-limited or watermarked. What is
   paid is convenience, never capability.
2. **The books are a local file, forever.** No Nigel feature may require cloud custody
   of the books. Hosted offerings are convenience around the file — delivery, backup —
   never a migration of the file into our hands.
3. **Never nagware.** An unconfigured or unpurchased capability does not nag, upsell inside workflows, or render broken controls. Small commercial notes about Nigel Cloud may appear during onboarding or in the settings app, which are for convenience and do not hold any features hostage. The App Store build carries none of them (see below). The pay button precedent (live, inert or absent) is the pattern for everything commercial.

## The ladder

Three rungs, each the same software with less friction. No rung gates a feature the
rung below has; each rung removes work. The middle rung, a built app someone else keeps
current, is sold through two channels.

| Rung | What you get | What you pay |
|---|---|---|
| **Build it yourself** | The full app from source; local delivery mode (epic 114) means invoicing and documents work with no cloud accounts at all | Nothing |
| **Nigel for Mac (App Store)** | The native Mac app from the Mac App Store, sandboxed, updated by the store | Paid up front; updates for the life of that App Store app |
| **Nigel Desktop (direct)** | Signed, notarized, auto-updating builds for macOS, Windows and Linux, from nigel.works | Perpetual license with 12 months of updates |
| **Nigel Cloud** | Hosted delivery: invoice and document pages served from nigel.works, mail sent from our infrastructure, acceptance recorded by us as a third party; later, zero-knowledge encrypted backup and sync | Subscription, sold on nigel.works, which includes a direct Desktop license while active |

The rungs map onto the `delivery` setting: `local` (no infrastructure), `hosted` (your
own R2, Mailgun and Stripe keys — the bring-your-own-cloud path stays first-class and
free), and `nigel` (our infrastructure, authenticated by your nigel.works account).

The Mac App Store is the first paid channel (2.0); direct downloads and licensing follow
(2.1). `backlog/decisions/decision-9` records why.

## Licensing: how it works

**The App Store build.** Apple is the merchant of record and the updater. The build is
paid up front with no in-app purchase, asks for no license key, and has no update
mechanism of its own (guideline 2.4.5). Store buyers update through the store for the
life of that App Store app; a future paid upgrade is a new App Store app. It signs in to
Nigel Cloud as an existing nigel.works account only (guideline 3.1.3(b)): no price, no
purchase or "subscribe" link and no call to action for Cloud appears inside it. If App
Review requires in-app purchase for that sign-in, Cloud is compiled out of the store build
rather than sold through the store. The Cloud subscription is sold on nigel.works only,
never by in-app purchase.

**Direct builds** carry a license key. The first four points below cover them; the last
four apply to every build.

- **What is licensed is the build and its update channel**, not the software. MIT means
  anyone may compile, redistribute, even sell builds — the license key buys our signed
  artifacts and the updater feed that keeps them current.
- **Perpetual plus a year**: a purchased direct build works forever; the key entitles
  updates for 12 months from purchase, renewable. No expiring app, no phoning home to keep
  running. Bookkeeping has an annual rhythm (tax years, bank format drift); the renewal
  matches it honestly.
- **Direct sales run through a merchant of record** (checkout, VAT and sales tax, license
  key issuance, refunds).
- **The key is a signed token** carried in config: the updater presents it for the feed,
  validated offline. No other phone-home.
- **Cloud authenticates with an account, not the key**: `delivery = "nigel"` signs in to
  the operator's nigel.works account, in every build.
- **Trademark remains ours** Trademark policy: builds not produced by nigel.works
  do not use the Nigel name or icon. The policy is published in this repository; the
  code stays MIT.
- **Nothing commercial is compiled in**: no price, no key, no endpoint that cannot be
  overridden in config. The public repository carries no licensing server, signing key
  or store credential; that machinery lives in a private repository, which depends on
  this one and not the reverse (the lib+bin precedent).
- **The public repository does not build the commercial artifacts.** The public CI compiles and
  tests the desktop crate, and publishes no installer and no
  update manifest. `backlog/decisions/decision-3` records this.

## Nigel Cloud: scope by release

Cloud is convenience hosting, never data custody. Each phase ships when its client half
in this repository and its service half in the private repository are both real.

- **2.0 — hosted email and invoice delivery.** Replaces the R2 + Mailgun + DNS onboarding
  wall with sign-in: invoice pages published to nigel.works, mail sent from a per-tenant
  subdomain with correct SPF/DKIM, payments still the operator's own Stripe. The client
  half is a third `AssetPublisher`/`Mailer` pair behind the existing traits.
- **2.2 — hosted documents and acceptance.** Document pages and the accept endpoint
  served by nigel.works, acceptance pulled by `document sync`. When we host the
  acceptance record, Nigel is a third-party witness to assent — a stronger signing
  story than a record in the operator's own bucket, with the same no-legal-claim scope.
- **2.3 — encrypted backup and sync.** Zero-knowledge snapshot backup and
  device-to-device sync of the SQLCipher database: ciphertext only, the key never
  leaves the operator. Conflicts are surfaced, not silently merged.

Explicitly **not** on this roadmap: multi-tenant hosted live books (a nigel.works web
app holding decryption keys and serving everyone's books). If a hosted
instance is ever offered it is a single-tenant deployment of the same standalone server
an operator could run themselves.

## Multi-user and the bookkeeper view

Launch requires that an operator can give their bookkeeper or accountant access that is
not "screen-share my laptop": a shared standalone instance with named users, an audit
trail, and admin / bookkeeper / read-only roles. That is epic 32 (multiuser level one),
and it is assigned to the 2.0 milestone — it ships before the public launch, because the
first question a working consultancy asks is "can my accountant see this?"

## Non-goals

- No feature-gated or crippled open-source build, ever.
- No custody of decryption keys or plaintext books on our infrastructure.
- In-app purchases. Minimal callouts to the website when appropriate, only offering convenience, never gating features.
- No in-app payment processing — payments remain the operator's Stripe relationship.
- No e-signature product claims; hosted acceptance is recorded assent, witnessed.
