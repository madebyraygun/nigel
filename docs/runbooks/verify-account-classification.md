# Verifying your books after the classification migration

Migration v10 gives every account and every category an **accounting class** —
`asset`, `liability`, `equity`, `revenue`, or `expense` — and teaches the
reports to read it. It backfills a class for the chart you already have, using
your account types and category types, plus two categories matched by name.

The backfill is a good guess, not a reading of your intent. This runbook walks
the places where the guess can be wrong on a real chart of accounts, and what
each one costs if you leave it.

Budget about fifteen minutes. Steps 1–3 are the ones that matter; 4–6 confirm
the reports agree with what you set.

## What the migration decided, in one table

| What it looked at | What it set |
|---|---|
| Account type `credit_card` or `line_of_credit` | `liability` |
| Every other account type | `asset` |
| Category type `income` | `revenue` |
| Category type `expense` | `expense` |
| Category named exactly `Owner Draw / Distribution` | `equity` |
| Category named exactly `Owner Contribution` | `equity` |

**That last pair is the sharp edge.** The match is on the exact name. If you
renamed your distributions category to anything else — "Distributions",
"Owner's Draw", "Shareholder Distribution", "Member Draw" — it landed on
`expense`, and nothing told you. Step 3 is where you catch that.

## 0. Take a backup first

The migration runs the first time any Nigel build opens the database, and it
runs whether or not you meant it to. Before you start:

```bash
nigel backup
```

That writes `<data_dir>/backups/nigel-YYYYMMDD-HHMMSS.db` and prints the path.
Keep it. If anything in this runbook goes sideways:

```bash
nigel restore /path/to/nigel-YYYYMMDD-HHMMSS.db
```

`nigel status` will tell you which database you are actually pointed at, which
is worth confirming if you keep more than one set of books.

> **A note on reading reports.** Run in a terminal, `nigel report …` opens the
> interactive viewer. Pipe it (`| cat`) and you get plain text you can scroll,
> diff, or paste. Every report command below is written to be piped.

## 1. Confirm the migration actually ran

You will have seen it go by once:

```
Applying migration v10: classify accounts and categories as asset/liability/equity/revenue/expense
```

If you missed it, the proof is a column:

```bash
nigel accounts list
```

A **Class** column between Type and Institution means v10 has run. No Class
column means you are on an older build.

## 2. Check your accounts

```bash
nigel accounts list
```

Read down the Class column. What you want to see:

- Credit cards and lines of credit → **liability**
- Checking, savings, payroll, everything else → **asset**

Anything with an unusual account type fell through to `asset` by default, so a
loan or a card recorded under a custom type is the thing to look for.

Fix one:

```bash
nigel accounts edit 3 --class liability
```

`accounts edit` is a partial update — it changes only what you pass, and leaves
the name, institution, and last four alone.

**What it costs if wrong:** a liability sitting as an asset inflates your cash
position by its balance. It is visible on `nigel report balance` immediately.

## 3. Check your categories — this is the important one

```bash
nigel categories list
```

Read down the Class column, and look hard at anything that is owner money
rather than business activity. Names worth searching for:

> draw, distribution, dividend, owner, member, shareholder, contribution, capital

Every one of those should read **equity**. If any of them reads `expense` or
`revenue`, the backfill missed it because the name did not match the two
literals in the table above.

### Fixing a category

**The safe way — the dashboard.** Run `nigel`, choose **Categories** from the
menu, select the category, and edit it. The form comes pre-filled with the
values already on the row, so setting the class leaves the tax line and form
line untouched.

**The command-line way — read this before you use it.** `categories update` is
a full replace, not a partial one. Every field you omit is written as empty,
including `--tax-line` and `--form-line`. Copy those two values out of
`categories list` first and restate them:

```bash
nigel categories update 42 "Owner Distributions" \
  --type expense \
  --class equity \
  --tax-line "Not deductible" \
  --form-line "K-16d"
```

If the category has no tax line or form line to begin with, omitting them is
harmless.

**What it costs if wrong:** this is the bug the whole change exists to kill. A
distributions category classed as `expense` is money you paid yourself being
deducted from business income — it understates your profit, overstates your
deductions, and reports `Distributions: 0` on the K-1 worksheet. Which brings
us to the next step.

## 4. Confirm the K-1 worksheet sees your distributions

```bash
nigel report k1 --year 2026 | cat
```

Two things to check:

1. **Distributions is not zero** — assuming you took any this year. If it reads
   zero and you know better, go back to step 3: an equity category is still
   sitting on `expense`.
2. **Your equity categories appear under Schedule K items**, not under
   deductions.

The class is now read first and it is final. A category classed `equity` is a
Schedule K item no matter what form line it carries — an equity row pointing at
a deduction line is treated as a chart-of-accounts mistake rather than as a
deduction. Money out to the owner counts as a distribution; money in is a
contribution and reduces nothing.

## 5. Confirm the P&L no longer deducts owner money

```bash
nigel report pnl --year 2026 | cat
```

Equity categories are out of both columns — not revenue, not expense. If your
net income moved **up** by roughly your distributions total compared to what
you remember, that is exactly the fix landing, and it is the number to sanity
check against last year's return.

The expense breakdown follows the same rule:

```bash
nigel report expenses --year 2026 | cat
```

Owner draws should no longer appear as an expense line.

## 6. Uncategorized money is disclosed, not hidden

Uncategorized transactions still count toward year-to-date net income — leaving
them out would quietly change your headline number — but they are now called
out where the figure appears:

```bash
nigel report balance | cat
```

Under **YTD Net Income** you will see, when there is anything to say:

```
Includes $1,234.56 across 7 uncategorized transactions — run `nigel review` to sort them.
```

To see what they are:

```bash
nigel report register --uncategorized | cat
```

Two things to know about that footnote:

- **It is year-to-date only.** Uncategorized transactions dated in a prior year
  are not in the figure and not in the note.
- **`No transactions found (uncategorized)` means there is nothing to
  disclose**, and the absent footnote is correct rather than broken.

## 7. Write down what you checked

Worth recording, because these are the numbers you will compare against next
time:

| Check | Where | Your result |
|---|---|---|
| Accounts with the wrong class | `nigel accounts list` | |
| Equity categories the backfill missed | `nigel categories list` | |
| Distributions total | `nigel report k1 --year <yr>` | |
| Net income, after any fixes | `nigel report pnl --year <yr>` | |
| Uncategorized count and total | `nigel report balance` | |

## If something is wrong

Restore the backup from step 0, then report:

- the category or account name,
- the class it was given,
- the class it should have,
- and what you saw on the report that gave it away.

A backfill rule that is wrong for your chart is worth fixing in the migration
itself, not just in your database — anyone with the same naming convention hits
the same thing.
