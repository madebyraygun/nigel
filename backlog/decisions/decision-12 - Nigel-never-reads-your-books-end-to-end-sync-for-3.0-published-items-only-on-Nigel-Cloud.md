---
id: decision-12
title: >-
  Nigel never reads your books: end-to-end sync for 3.0, published items only on
  Nigel Cloud
date: '2026-10-23 18:02'
status: accepted
---
## Context

An operator's bookkeeper should be able to log in from anywhere, the way a bookkeeper logs
in to a QuickBooks Online company today, without the operator emailing database copies or
running a public server. docs/product/foundation.md rules out custody of keys or readable
books on Nigel's infrastructure and puts multi-tenant hosted live books off the roadmap.

Two ways keep "Nigel can never read your books" true while making shared books reachable
from anywhere:

- **Confidential computing:** each customer's server runs in a hardware-sealed VM that
  attests to a published, reproducible build before any key is released. Server-side
  automations keep running, but trust moves to the CPU vendors, the engineering is heavy
  (attestation, key release, a transparency log) and operations are blind.
- **End-to-end encrypted sync:** the books live on their members' devices and travel
  through Nigel Cloud as ciphertext. Nigel Cloud never holds a key, so even a fully
  compromised Nigel Cloud learns nothing about the books beyond metadata such as sizes
  and timing.

End-to-end sync gives the stronger guarantee, is the simpler of the two, and grows out of
the encrypted backup and sync already planned (2.3). Its cost is that work which writes
to the books needs a member's device. The operator's requirements for a product most users
can run without their own server are that incoming payment events are handled and that
scheduled invoices go out on time with no device online.

## Decision

1. **Nigel never has access to an operator's books.** No Nigel service holds a key to the
   books or a readable copy of them, in storage, in memory or in transit. This holds for
   every release and every Nigel Cloud product.
2. **Shared books are end-to-end encrypted sync, targeted at 3.0.** Members' devices hold
   replicas; Nigel Cloud relays and stores only ciphertext; sharing wraps the books' data
   key to each member's key; revocation rotates it. It is pursued if Nigel gains traction
   after 2.0, under its own spec.
3. **Nigel Cloud may hold published items, and only those.** A published item is
   something the operator has chosen to send to a third party: a sent or scheduled
   invoice (its page, PDF, payment link, recipient and due date), a document out for
   signature, and their status. These are already readable by their recipient and by the
   delivery services involved; the cloud acting on them reads nothing of the books.
4. **With no device online, Nigel Cloud handles what touches only published items:**
   - Incoming events (a Stripe payment, a document response) update the published item —
     the invoice page reads Paid, reminders stop — and are sealed to the books' public key
     for the next member device to record.
   - Scheduled invoices are prepared ahead by a member device, a configurable window in
     advance (numbered, with payment link, page and PDF), and queued as delivery packages
     that Nigel Cloud sends on time. Editing or cancelling a schedule updates or withdraws
     them. If no device has come online to prepare the next ones before the window runs
     out, the operator is warned; a send is never skipped silently.
   - Overdue reminders run from the published items.
   Anything that writes to the books (recording a payment, categorizing, importing,
   preparing invoices beyond the window) waits for a member device.
5. **Bookkeeper access in 2.0 is self-hosted:** named users on the operator's own
   `nigel serve` (epic 32, TASK-164). Nigel Cloud plays no part in 2.0 bookkeeper access.

## Consequences

- docs/product/foundation.md states the guarantee and the published-items rule, replacing
  "single-tenant hosted instance" as the possible hosted future.
- 2.0 work builds toward the end-to-end model so 3.0 does not have to undo it:
  - the books package (TASK-163) gives each set of books a stable identity and room for a
    sync log beside the database;
  - recovery codes (TASK-161) use envelope encryption whose key file can hold more than
    two wrapped copies of the data key, one per future member;
  - write attribution and the audit log (TASK-32.4, 32.5) record writes as a stream of
    discrete operations at the data layer, the shape a sync log needs;
  - Nigel Cloud delivery (TASK-115.3) and local delivery (epic 114) are built around a
    delivery package containing only the published item, never books data;
  - recurring schedules (epic 140) and reminders (TASK-139) keep "what will be sent" as a
    value a device can prepare ahead of the send date.
- Automations that write to the books are not instantaneous under end-to-end sync; an
  operator who wants them always-on can run a headless member on their own hardware.
- Confidential computing stays a possible later addition for an always-on member that
  Nigel operates, only if it can be made verifiable; it is not planned.
