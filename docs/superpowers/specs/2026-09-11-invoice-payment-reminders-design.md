# Invoice payment reminders

## The gap

TASK-5 "Invoicing: Automatic Reminders" was migrated from an archived GitHub
issue with no acceptance criteria and marked Done in a bulk sweep
(`chore: mark invoicing tasks 1-6 done`) alongside five tasks that had actually
shipped. Nothing was built. There is no `remind` subcommand, and `grep -i remind`
over `crates/` returns nothing. An invoice that goes unpaid is chased by hand or
not at all.

`overdue` is derived on read, never stored (`invoicing/invoices.rs:422`), so there
is no existing state to hang "already reminded" off. That is the one thing this
design has to add.

## Shape

The established pattern, set by recurring schedules: no daemon, a command that
does whatever is due, invoked by cron or launchd, idempotent on a natural key,
never prompting, running on an encrypted database via `NIGEL_DB_PASSWORD`.

Reminders send **unattended**. This is the point at which they differ from
schedule runs, which draft by default. A generated invoice states new figures
nobody has checked; a reminder states nothing new — it points at an invoice the
client already received, quoting a balance the database already knows. There is
no wrong number for an unattended run to mail out, so there is no queue, no
release step and no review surface.

## Cadence

One global cadence in `Settings`:

```
reminder_offsets  = [-3, 1, 7, 14, 30]   # days relative to the due date
reminders_enabled = false                # off until the operator turns it on
```

**`reminders_enabled` defaults to off, and an upgrade must not turn it on.** The
feature mails clients unattended; an existing install that gains it silently would
start chasing every open invoice on its books the next time cron fired, with
nobody having asked for it. Turning it on is a deliberate act, and the offsets
above are only the default cadence it adopts when switched on.

Negative offsets are a heads-up before the due date; positive ones chase after it.
The list is also the cap — there is no separate maximum count, and reminders stop
when the last offset is discharged.

Per-client and named policies were considered and rejected as machinery a
one-person shop will configure once. The escape hatch is per invoice, not per
client: `invoices` gains a `reminders_enabled` column, so the invoice you are
negotiating over can be taken out of the cadence without disabling the feature.

## State

One table:

```sql
CREATE TABLE invoice_reminders (
    id          INTEGER PRIMARY KEY,
    invoice_id  INTEGER NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    offset_days INTEGER NOT NULL,
    decided_on  TEXT    NOT NULL,   -- the run date that discharged it
    outcome     TEXT    NOT NULL,   -- 'sent' | 'superseded'
    UNIQUE (invoice_id, offset_days)
);
```

A row means an offset is **discharged**, whether or not an email left the
building. The unique key is the whole idempotency story: a run that fires twice
writes nothing the second time, exactly as schedule runs dedupe on
(schedule, period).

## Selection

An invoice is in scope when all of these hold:

- money is owed — effective status is `sent`, `partial` or `overdue`
- it has a due date
- its own `reminders_enabled` is set
- the global `reminders_enabled` is set

For each in-scope invoice the run computes `due_date + offset` for every
configured offset, takes those now passed with no row, **sends the largest**, and
writes `superseded` rows for the rest.

Sending only the largest is what keeps a gap from turning into a pile-up. A
machine shut for three weeks passes `+1`, `+7` and `+14`; three emails landing in
one minute reads as a malfunction, and a `+1` nudge is absurd a month late. One
current message goes out and the rest are marked passed.

Two consequences of the rule are load-bearing and must be tested:

- **A partially paid invoice keeps reminding**, and the figure it quotes is the
  remainder, not the original total.
- **A retroactively added offset cannot send a stale message.** Drop `+3` into the
  cadence after `+7` has gone out and the naive rule fires a gentle nudge behind
  an escalation. So: never send an offset earlier than the latest already sent for
  that invoice — mark it superseded.

## The run

```
nigel invoice remind run [--today <date>] [--dry-run]
```

Beside `nigel invoice schedule run`, not folded into it. Two cron lines rather
than one, bought deliberately: the jobs fail independently, so an outage that
breaks invoice generation does not also swallow the day's reminders, and
reminders can run daily while generation runs monthly.

It reports per invoice the way `ScheduleRunReport` does, and it sends email only
— no republish, no payment-gateway call, no PDF attachment, because nothing about
the invoice has changed.

A client with no billing address is **reported, not recorded**. `require_email`
refuses, no row is written, and the reminder still goes out once the address is
fixed. It re-reports on every run until then, which is correct: an unsendable
client with an open invoice is a hole in the books, not noise to suppress.

## The email

Its own renderer beside `render_email_text`, sharing the `MoneySummary` and
`money` seams so the figure cannot disagree with the page:

```
Subject: Reminder: invoice 1249

Invoice 1249 was due 2026-08-14 — 28 days ago.

Amount outstanding: $2,400.00

Pay now: https://billing.example.test/i/ab12cd34/index.html

If you've already paid, please ignore this.
```

No line items and no attachment: both are one click away on the published page,
and repeating them makes a reminder read as a second invoice. A before-due offset
renders the same body with the tense corrected — `is due in 3 days`.

Escalating copy per offset was rejected. It is three to five bodies to write, keep
in step and test, and tone is the first thing an operator will want to override
anyway — which is a template-override question, not a reminders question.

## Stop conditions

Paid in full, void, no due date, the per-invoice switch off, or the global switch
off. Nothing else.

## Surfaces

Recurring schedules shipped their engine and CLI with no web UI, and the gap was
found from the outside months later. Reminders are decomposed so the same thing
cannot happen quietly: the web and TUI work are named children of the epic, not
an implied follow-up.

- **CLI** — `remind run`, `remind on|off <number>` for the per-invoice switch, and
  whatever turns the global switch on, since nothing works until it is.
- **Web** — reminder history and the toggle on the invoice detail, through
  `@nigel/ui` components with previews and `describePreviewA11y`.
- **TUI** — the same two affordances on the invoice detail subscreen.
- **API** — reminder state on the invoice detail payload, and an endpoint for the
  toggle.

## Testing

`--today` makes every case deterministic, and the `Senders` trait already allows a
mock mailer. The cases that must be pinned:

- a second run the same day sends nothing
- three passed offsets send one email and write two `superseded` rows
- an offset added behind one already sent is superseded, never sent
- a partial payment keeps the cadence running and quotes the remainder
- paid, void, no due date and both switches each stop it
- an unsendable client is reported and leaves no row, then sends once fixed
- `--dry-run` writes nothing and sends nothing
- a database upgraded from before the feature has reminders off, and a run on it
  sends nothing
