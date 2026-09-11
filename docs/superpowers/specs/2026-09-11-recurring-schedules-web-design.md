# Recurring invoice schedules on the web

## The gap

Recurring schedules shipped a data layer and a CLI and nothing else. There is no
route file, no endpoint, no SPA screen and no TUI subscreen — `grep -rn schedule`
over `server/routes/` returns nothing, and `cli/invoice_schedule.rs` is the only
surface in the workspace. A retainer can only be set up by someone at a terminal.

Two consequences, and the second is worse than the first. An operator working in
the browser or the desktop shell cannot create a schedule. And even if they could,
**it would never fire**: generation happens in `nigel invoice schedule run`, which
is built for cron, and nobody who lives in a desktop app edits a crontab.

## What runs it

At launch, best-effort, silent when unconfigured, and never failing the thing the
operator actually asked for. `sync_invoice_payments()` in
`crates/nigel/src/main.rs:13` is the established precedent for background-ish work
in an app with no daemon.

The launch hook **drafts only, and skips autosend schedules entirely**. Opening
your books must never send email to a client.

### Where it goes — not beside the sync hook

`sync_invoice_payments()` is guarded by
`if !matches!(command, … | Commands::Serve { .. } | …)`: **`serve` is excluded from
it.** Placing the schedules hook beside it would put it in the one dispatch path a
web or desktop operator never takes, which is the entire population this work
exists for.

So there are two call sites, not one:

- **`serve` startup**, in `server/mod.rs`, before the listener binds — this is the
  one that matters, and the one the sync hook is missing.
- **The CLI dispatch guard**, under the same condition as the sync hook, so a
  terminal operator who never runs `schedule run` still gets drafts.

Whether `sync_invoice_payments()` should follow the schedules hook into `serve`
startup is TASK-37's question, not this one. Do not widen that guard here; it has
a task of its own and the failure modes are different.

### The trap this avoids

`draft_due_schedules` (`invoicing/schedules.rs:492`) looks like the right entry
point and is not. It drafts every due schedule *including* autosend ones,
reporting them as unsent — honest behaviour for an installation with no sending
configured, which is what it was written for. But generating a period **consumes**
it: the run row and the advanced `next_period` commit in the same transaction as
the invoice.

So a launch hook calling it would draft an autosend retainer, discharge the
period, and leave a later cron run with nothing to do. The invoice would never be
sent, and nothing would report an error. A convenience feature would have
introduced a silent billing failure.

The hook therefore needs a variant that skips autosend schedules without
generating or advancing them, reporting them as due and awaiting a run. Autosend
remains the property of cron and of the explicit Run button.

```
2 invoices drafted from 2 schedules
1 schedule (Acme retainer, autosend) is due — run it to send
```

## API

`server/routes/invoice_schedules.rs`, mirroring the data layer one-for-one
because `schedules.rs` already exposes everything the screen needs:

```
GET    /api/invoice-schedules?all=        list_schedules
POST   /api/invoice-schedules             add_schedule
GET    /api/invoice-schedules/{id}        get_schedule + schedule_items + schedule_runs
PATCH  /api/invoice-schedules/{id}        update_schedule
POST   /api/invoice-schedules/{id}/pause  pause_schedule
POST   /api/invoice-schedules/{id}/resume resume_schedule
POST   /api/invoice-schedules/{id}/end    end_schedule, ending today as the CLI does
POST   /api/invoice-schedules/run         run_due_schedules, autosend honoured
```

## The screen

A tab on the existing Invoices screen rather than a fourteenth sidebar entry. A
schedule is a thing that produces invoices and belongs where invoices are, and a
small shop keeps three of them.

Components land in `@nigel/ui` with co-located previews and
`describePreviewA11y`, read theme tokens, and adopt `controlsCss` wherever they
render a `wa-*` primitive:

- `wc-schedule-list` — client, cadence, next period, drafts-or-sends, paused state.
- `wc-schedule-form` — reusing `wc-line-items` for the items.

The detail shows the schedule's line items and its run history: which invoice came
out of which period.

The form carries the CLI's `--from` as an optional "start from invoice" prefill,
seeding items, currency, notes, terms and the issue-to-due term. Creating the
first schedule for a new client still works without one.

It also states what the CLI help states, because it is not guessable from the
form: **edits apply to future invoices, never past ones.**

## Two things the UI must not hide

**Running from the browser sends real email.** A schedule with autosend set will
mail its client the moment Run completes. The button takes a confirmation naming
which invoices go to whom — the same discipline as the CLI refusing a non-TTY send
without `--yes`.

**Catch-up generates one invoice per missed cycle**, each dated its own period's
issue date rather than the run day. This is deliberately unlike the reminder rule,
where a gap collapses to a single message: a missed reminder is a message nobody
needed twice, while a missed billing period is money genuinely owed. Pressing Run
on a schedule six cycles stale produces six invoices, so the screen states the
count before the run, not after.

## Blocked on TASK-125

The screen needs an End button and a sentence saying what End does. TASK-125 asks
exactly that question — whether ending a schedule with unbilled periods before its
end date forgives them or bills them — and it is undecided. Building the button
first means writing that sentence twice and shipping the wrong one in between.

## Scope

The TUI has no schedules surface either. It stays out: decision-8 puts web and
desktop ahead of the TUI, and the CLI already serves anyone in a terminal. Filing
a child nobody intends to take is how TASK-81's web gap went unnoticed in the
first place.

## Testing

- The launch hook drafts non-autosend schedules and leaves autosend ones
  ungenerated, with `next_period` unmoved and the schedule reported as due.
- A cron run after a launch hook still sends the autosend schedule — the period
  was not consumed.
- The launch hook is best-effort: a failure prints a notice, does not fail the
  command, and does not stop `serve` from binding.
- The hook runs on `serve` startup, which is what the sync hook's guard excludes.
- Every endpoint round-trips against the real router with a real session, as the
  existing route tests do.
- Schedules join the `invoicing-parity` manifest, so the figures the screen shows
  are the figures `nigel invoice schedule` prints.
- Component previews cover: no schedules, an active one, a paused one, an ended
  one, an autosend one, and a form with validation errors.
