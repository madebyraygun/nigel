# Testing duplication and recurring schedules

Two features, one runbook. **Duplication** copies an invoice into a fresh
draft. **Schedules** bill a client on a cadence, generating an invoice per
period without being asked twice.

Every command below was run end to end against this branch, and the output
shown is what it actually printed. Work through it in a scratch set of books
first — Step 0 sets one up — because a schedule with a start date in the past
generates real invoices the moment you run it, and because an unconfigured lab
cannot email a client by accident.

Budget twenty minutes. Steps 3 and 4 are the ones that would cost you money if
they were wrong.

Steps 3, 4, 5, 7 and 8 also run in CI, in `crates/nigel/tests/cli_dispatch.rs`,
against the same commands through the real binary — so hand-run them when you
want to *see* the behaviour, not to establish that it holds. Steps 1, 2 and 6
are covered by the schedule and duplication tests beside them. What no test can
do for you is step 10: deciding that what it billed is what you meant.

## 0. A lab that cannot touch your books or your clients

```bash
LAB=$(mktemp -d)
export HOME="$LAB"

nigel init --data-dir "$LAB/books" --profile business
nigel client add "Cedar Systems" --email cedar@example.test
nigel invoice new --client 1 --issue 2026-01-15 --due 2026-02-14 \
  --item "Retainer:1:2400" --item "Hosting:2:45" --notes "Thanks"
```

Two things make this safe. The `HOME` override puts the settings file and the
database inside the lab, so your real books are never opened. And because that
lab has no Mailgun, R2 or Stripe keys, **sending is not configured** — a
schedule that wants to send will write a draft and tell you it could not send,
rather than emailing anyone.

When you are done: `rm -rf "$LAB"`.

## 1. Duplication (TASK-7)

```bash
nigel invoice duplicate 1248 --issue 2026-03-01
nigel invoice show 1249
```

Expected:

```
Duplicated invoice #1248 as draft #1249 for 2490.00 USD
```

Check on the copy:

- **A new number**, and the source untouched.
- **Both line items**, with their quantities and unit amounts.
- **The issue-to-due term carried across, not the due date.** The source ran
  2026-01-15 to 2026-02-14 — thirty days — so a copy issued 2026-03-01 comes
  due 2026-03-31. A copy that inherited the literal due date would arrive
  already overdue, which is the bug this behaviour exists to avoid.
- Notes and terms copied; currency copied.
- Status `draft`, nothing published or paid.

Omit `--issue` and the copy is dated today.

## 2. A schedule, seeded from an invoice you already trust

```bash
nigel invoice schedule add --client 1 --cadence monthly \
  --start 2026-06-15 --net-days 30 --from 1248
nigel invoice schedule list
```

`--from 1248` takes the items, currency, notes and terms off that invoice, so
you are not retyping a bill you already got right. Supply `--item` instead to
type them out; the two are mutually exclusive.

`--net-days 30` is what gives each generated invoice a due date. Leave it off
when you typed the items with `--item` and generated invoices carry none, and
never go overdue. With `--from`, though, the source invoice's own issue-to-due
term comes across with the rest of its shape — #1248 ran thirty days, so a
schedule seeded from it bills Net 30 whether or not you say so.

## 3. Catch-up — the one that surprises people

A schedule does **not** start from today. It starts from `--start` and bills
every period between then and now, in order, each invoice dated its own period
rather than the day you ran it.

```bash
nigel invoice schedule run
```

With a start of 2026-06-15 and today in August, that is three invoices at once:

```
Generated 3 invoice(s).
  #1250  Cedar Systems  $2,490.00  2026-06-15  draft
  #1251  Cedar Systems  $2,490.00  2026-07-15  draft
  #1252  Cedar Systems  $2,490.00  2026-08-15  draft
```

This is correct — those periods really were owed — but it means **a
back-dated `--start` on a real client bills them for every month since**. Set
`--start` to the next period you actually want billed unless you mean to catch
up.

Then run it again:

```bash
nigel invoice schedule run
```

```
Generated 0 invoice(s).
```

Nothing is billed twice. A unique index on the schedule and period is what
enforces that, so it holds even if two runs overlap — which is what makes the
command safe to put in cron.

## 4. The anchor day survives short months

The billing day is remembered, not recomputed from the last invoice. Test it
across February:

```bash
nigel invoice schedule add --client 1 --cadence monthly \
  --start 2026-01-31 --net-days 15 --item "Support:1:500"
nigel invoice schedule run
```

A back-dated start bills every period since, so how many rows this prints
depends on the day you run it. The opening four are the point:

```
  #1253  Cedar Systems  $500.00  2026-01-31  draft
  #1254  Cedar Systems  $500.00  2026-02-28  draft
  #1255  Cedar Systems  $500.00  2026-03-31  draft
  #1256  Cedar Systems  $500.00  2026-04-30  draft
```

February clamps to the 28th and **March returns to the 31st**. A schedule that
walked forward from the last invoice would have stuck on the 28th for the rest
of the year, quietly moving a client's billing date. Use `--anchor-day` to set
a day different from the start date's.

## 5. A generated invoice cannot be deleted

```bash
nigel invoice delete 1250 --yes
```

```
Error: Cannot delete: invoice was generated by schedule 1, which keeps it as
the record of what that period billed — void it instead
Run `nigel invoice void 1250` to cancel it instead.
```

A hand-made draft still deletes normally — try `nigel invoice delete 1249 --yes`
to see the contrast. Confirm the same refusal appears in the TUI
(`nigel` → Invoices), since the point is that no screen offers an action the
database will reject. The web UI has no schedule surface yet, so its invoice
screen is the only place to check there.

## 6. Pause, resume, end

```bash
nigel invoice schedule pause 1     # nothing generates; row and history kept
nigel invoice schedule list        # paused schedules are hidden
nigel invoice schedule resume 1
nigel invoice schedule end 1       # terminal
nigel invoice schedule list --all  # ended and paused ones appear here
```

`end` is final and keeps the history. It will not decide what happens to
periods the schedule already owed — on a schedule that is behind it refuses and
makes you say which you meant:

```
Error: Schedule 1 has 5 unbilled periods on or before 2026-05-15: 2026-01-01,
2026-02-01, 2026-03-01, 2026-04-01, 2026-05-01. Bill them or forgive them —
ending cannot decide that for you.

  --bill     generate them as drafts, then end
  --forgive  end without billing them
```

`--bill` generates exactly what a run on the end date would have produced, as
drafts, even on an autosend schedule. `--forgive` ends without billing them.
Ending a schedule with nothing owed still takes neither flag.

## 7. Editing applies forward only

```bash
nigel invoice schedule edit 1 --item "Retainer:1:2600"
```

```
Updated schedule 1. Future invoices use the new figures.
```

Invoices already generated keep the old amount; the next period gets the new
one. On the schedule from step 2, which billed its periods at $2,490, `nigel invoice
show` on any of them still reads $2,490 while `nigel invoice schedule list` now
shows $2,600 against the next period — `edit --item` replaces every line rather
than adding to them. That is the intended rule — a rate change should not silently rewrite
bills a client has already seen. Confirm it on `nigel invoice schedule show 1`,
where past periods are listed with the invoices they produced.

## 8. Autosend, and what a run does when sending is not set up

```bash
nigel invoice schedule add --client 1 --cadence quarterly \
  --start 2026-05-01 --item "Audit:1:1500" --autosend
nigel invoice schedule run
echo "exit: $?"
```

In the lab, with no keys configured:

Numbers here follow on from whatever the earlier steps generated, so yours will
differ:

```
  #1261  Cedar Systems  $1,500.00  2026-05-01  draft — not sent: sending is not configured on this installation
  #1262  Cedar Systems  $1,500.00  2026-08-01  draft — not sent: sending is not configured on this installation
Error: Some invoices were not sent. See the lines above.
exit: 1
```

Both facts matter for automation: the invoice **is still generated** and waits
as a draft, and the run **exits non-zero** so cron reports it rather than
failing silently. A run with nothing due exits 0.

## 9. Wiring it up

`nigel invoice schedule run` never prompts, which is what makes it safe under
cron or launchd. Daily is enough — the period check is by date, not by how
often you run it, and a second run the same day generates nothing.

```
0 9 * * *  /usr/local/bin/nigel invoice schedule run
```

Point it at real books only after you have watched a catch-up in the lab and
agree with what it billed.

## 10. Before you trust it with a real client

- Create the schedule with `--start` set to the **next** period, not a past one,
  unless you have decided you want the back bills.
- Leave `--autosend` off for the first cycle. Let it draft, read the draft, send
  it by hand, and turn autosend on once the shape is right.
- Check `nigel invoice schedule show <id>` after the first real run and confirm
  the period, the amount and the due date are what you would have typed.
