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
3. **Absent quietly.** An unconfigured or unpurchased capability does not nag, upsell
   inside workflows, or render broken controls. The pay button precedent (live, inert
   or absent) is the pattern for everything commercial.

## The ladder

Three rungs, each the same software with less friction. No rung gates a feature the
rung below has; each rung removes work.

| Rung | What you get | What you pay |
|---|---|---|
| **Build it yourself** | The full app from source; local delivery mode (epic 114) means invoicing and documents work with no cloud accounts at all | Nothing |
| **Nigel Desktop** | Signed, notarized, auto-updating builds for macOS, Windows and Linux, from nigel.works or the app stores | Perpetual license with 12 months of updates |
| **Nigel Cloud** | Hosted delivery: invoice and document pages served from nigel.works, mail sent from our infrastructure, acceptance recorded by us as a third party; later, zero-knowledge encrypted backup and sync | Subscription, which includes a Desktop license while active |

The rungs map onto the `delivery` setting: `local` (no infrastructure), `hosted` (your
own R2, Mailgun and Stripe keys — the bring-your-own-cloud path stays first-class and
free), and `nigel` (our infrastructure, authenticated by your license).

## Licensing: how it works

- **What is licensed is the build and its update channel**, not the software. MIT means
  anyone may compile, redistribute, even sell builds — the license key buys our signed
  artifacts and the updater feed that keeps them current.
- **Perpetual plus a year**: a purchased build works forever; the key entitles updates
  for 12 months from purchase, renewable. No expiring app, no phoning home to keep
  running. Bookkeeping has an annual rhythm (tax years, bank format drift); the renewal
  matches it honestly.
- **Sales run through a merchant of record** (checkout, VAT and sales tax, license key
  issuance, refunds). App store listings are a discovery channel with their own update
  path; the subscription is sold on nigel.works only, never by in-app purchase.
- **The key is a signed token** carried in config: the updater presents it for the feed,
  and `delivery = "nigel"` presents it to the Cloud API. Offline validation for the
  build, online validation for the service. No other phone-home.
- **The name is the enforcement.** Trademark policy: builds not produced by nigel.works
  do not use the Nigel name or icon. The policy is published in this repository; the
  code stays MIT.
- **Nothing commercial is compiled in**: no price, no key, no endpoint that cannot be
  overridden in config. The public repository carries no licensing server, signing key
  or store credential; that machinery lives in a private repository, which depends on
  this one and not the reverse (the lib+bin precedent).
- **The public repository does not build the artifacts it sells.** Its CI compiles and
  tests the desktop crate so the shell cannot rot, and publishes no installer and no
  update manifest. Producing them there would put the packaging and the update feed in
  public and make every fork a distributor of something indistinguishable from the paid
  build. `backlog/decisions/decision-3` records this and rewrites tasks 33.5 and 33.6,
  which were written before it and asked the public CI to publish installers.

## Nigel Cloud: scope by milestone

Cloud is convenience hosting, never data custody. Each phase ships when its client half
in this repository and its service half in the private repository are both real.

- **v1 — hosted email and invoice delivery.** Replaces the R2 + Mailgun + DNS onboarding
  wall with sign-in: invoice pages published to nigel.works, mail sent from a per-tenant
  subdomain with correct SPF/DKIM, payments still the operator's own Stripe. The client
  half is a third `AssetPublisher`/`Mailer` pair behind the existing traits.
- **v1.1 — hosted documents and acceptance.** Document pages and the accept endpoint
  served by nigel.works, acceptance pulled by `document sync`. When we host the
  acceptance record, Nigel is a third-party witness to assent — a stronger signing
  story than a record in the operator's own bucket, with the same no-legal-claim scope.
- **v1.2 — encrypted backup and sync.** Zero-knowledge snapshot backup and
  device-to-device sync of the SQLCipher database: ciphertext only, the key never
  leaves the operator. Conflicts are surfaced, not silently merged.

Explicitly **not** on this roadmap: multi-tenant hosted live books (a nigel.works web
app holding decryption keys and serving everyone's books). It would break promise 2,
and the trust model — `serve` binds localhost — is built on its absence. If a hosted
instance is ever offered it is a single-tenant deployment of the same standalone server
an operator could run themselves (the epic 32 / task 33.7 shape).

## Multi-user and the bookkeeper view

Launch requires that an operator can give their bookkeeper or accountant access that is
not "screen-share my laptop": a shared standalone instance with named users, an audit
trail, and admin / bookkeeper / read-only roles. That is epic 32 (multiuser level one),
and it is assigned to the v1 milestone — it ships before the public launch, because the
first question a working consultancy asks is "can my accountant see this?"

## Non-goals

- No feature-gated or crippled open-source build, ever.
- No custody of decryption keys or plaintext books on our infrastructure.
- No in-workflow upsells; commercial surfaces are the website and the docs.
- No payment processing of our own — payments remain the operator's Stripe relationship.
- No e-signature product claims; hosted acceptance is recorded assent, witnessed.
