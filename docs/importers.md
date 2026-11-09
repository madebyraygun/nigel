# Adding Importers to Nigel

All importer code lives in `crates/nigel-core/src/importer.rs`. Nigel uses enum dispatch — each bank format is a variant of `ImporterKind`, with match arms for detection and parsing. No trait objects, no plugin registry.

## Architecture

```
CSV/XLSX file
  → ImporterKind::detect() inspects headers/structure
  → ImporterKind::parse() extracts ParseOutcome { rows, rejects }
  → import_file() resolves the importer, parses, and hands a ParsedImport to
  → import_parsed(), which deduplicates and inserts into SQLite
```

A source that delivers transactions rather than a file builds its own `ParsedImport` and calls `import_parsed()` directly, so it gets the same `imports` row, rejects record and dedup as a file.

Key types:

- **`ImporterKind`** — enum with one variant per bank format
- **`ParsedRow`** — intermediate representation: `{ date: String, description: String, amount: f64, external_id: Option<String> }`
- **`ParsedImport`** — rows ready to become one import: the account, the name import history shows, the checksum, the format, and the `ParseOutcome`
- **`ParseOutcome`** — what one parse produced: `{ rows: Vec<ParsedRow>, rejects: Vec<RejectedRow> }`
- **`RejectedRow`** — a row that could not be read: `{ row_number: u64, content: String, reason: String }` — the 1-based line in the file, its fields rejoined with commas, and why. Stored in `import_rejects` and readable back per import
- **`ImportResult`** — returned by `import_file()`: counts of imported/skipped/malformed rows

## Step-by-Step: Adding an Importer

This walks through adding a fictional "Chase Checking" importer. The CSV looks like:

```csv
Details,Posting Date,Description,Amount,Type,Balance,Check or Slip #
DEBIT,01/15/2025,AMAZON PURCHASE,-42.50,ACH_DEBIT,1957.50,
CREDIT,01/16/2025,DIRECT DEPOSIT,3200.00,ACH_CREDIT,5157.50,
```

### 1. Add a variant to `ImporterKind`

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImporterKind {
    BofaChecking,
    BofaCreditCard,
    BofaLineOfCredit,
    #[cfg(feature = "gusto")]
    GustoPayroll,
    ChaseChecking,  // ← new
}
```

### 2. Add match arms to all methods

Add the new variant to each `match` in the `impl ImporterKind` block:

```rust
// key() — snake_case identifier, used by --format CLI flag
Self::ChaseChecking => "chase_checking",

// name() — human-readable display name
Self::ChaseChecking => "Chase Checking",

// account_types() — which account types this format applies to
Self::ChaseChecking => &["checking"],

// detect() — call your detection function
Self::ChaseChecking => detect_chase_checking(file_path),

// parse() — call your parse function
Self::ChaseChecking => parse_chase_checking(file_path),
```

If your importer doesn't need post-import processing, the `has_post_import()` and `post_import()` match arms already handle the `_ => false` / `_ => Ok(())` fallback.

### 3. Add to `ALL_IMPORTERS`

```rust
const ALL_IMPORTERS: &[ImporterKind] = &[
    ImporterKind::BofaChecking,
    ImporterKind::BofaCreditCard,
    ImporterKind::BofaLineOfCredit,
    #[cfg(feature = "gusto")]
    ImporterKind::GustoPayroll,
    ImporterKind::ChaseChecking,  // ← new
];
```

### 4. Write the detect function

Inspect the file's headers or structure to identify the format. Return `true` if the file matches, `false` otherwise.

```rust
fn detect_chase_checking(file_path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(file_path) else {
        return false;
    };
    // Chase checking CSVs have this specific header row
    content.starts_with("Details,Posting Date,Description,Amount,Type,Balance,Check or Slip #")
}
```

Tips:
- Check for unique header signatures — column names, column count, or file preamble
- Use `csv::ReaderBuilder` for more robust header inspection (see `detect_bofa_checking`)
- Return `false` on any I/O error

### 5. Write the parse function

Extract rows into a `ParseOutcome`, following the parse contract (see below).

```rust
fn parse_chase_checking(file_path: &Path) -> Result<ParseOutcome> {
    const MIN_COLS: usize = 4;
    let mut rdr = create_csv_reader(file_path)?;
    let mut out = ParseOutcome::default();
    let mut found_header = false;

    for result in rdr.records() {
        let record = match result {
            Ok(record) => record,
            Err(err) => {
                out.reject_unreadable(&err);
                continue;
            }
        };
        if !found_header {
            if record.get(0).is_some_and(|f| f.trim() == "Details") {
                found_header = true;
            }
            continue;
        }
        if record.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if record.len() < MIN_COLS {
            let reason = format!(
                "expected at least {MIN_COLS} columns, found {}",
                record.len()
            );
            out.reject(&record, reason);
            continue;
        }
        let Some(date) = parse_date_mdy(&record[1]) else {
            let reason = format!("date {:?} is not MM/DD/YYYY", record[1].trim());
            out.reject(&record, reason);
            continue;
        };
        let description = record[2].trim().to_string();
        let Some(amount) = parse_amount(&record[3]) else {
            let reason = format!("amount {:?} is not a number", record[3].trim());
            out.reject(&record, reason);
            continue;
        };
        out.rows.push(ParsedRow {
            date,
            description,
            amount,
            external_id: None,
        });
    }
    Ok(out)
}
```

### 6. Post-import hook (optional)

If your importer needs to auto-categorize transactions after import (like Gusto does for payroll), add logic to `has_post_import()` and `post_import()`:

```rust
fn has_post_import(&self) -> bool {
    match self {
        Self::ChaseChecking => true,  // only if needed
        // ...
    }
}
```

Most bank importers don't need this.

### 7. Add a test

```rust
#[test]
fn test_chase_checking_parse() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chase.csv");
    let content = "Details,Posting Date,Description,Amount,Type,Balance,Check or Slip #\n\
                   DEBIT,01/15/2025,AMAZON PURCHASE,-42.50,ACH_DEBIT,1957.50,\n\
                   CREDIT,01/16/2025,DIRECT DEPOSIT,3200.00,ACH_CREDIT,5157.50,\n";
    std::fs::write(&path, content).unwrap();

    let out = ImporterKind::ChaseChecking.parse(&path).unwrap();
    assert_eq!(out.rows.len(), 2);
    assert!(out.rejects.is_empty());
    assert_eq!(out.rows[0].date, "2025-01-15");
    assert_eq!(out.rows[0].description, "AMAZON PURCHASE");
    assert_eq!(out.rows[0].amount, -42.5);
    assert_eq!(out.rows[1].amount, 3200.0);
}

#[test]
fn test_chase_checking_detect() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chase.csv");
    std::fs::write(&path, "Details,Posting Date,Description,Amount,Type,Balance,Check or Slip #\n").unwrap();
    assert!(ImporterKind::ChaseChecking.detect(&path));

    let other = dir.path().join("other.csv");
    std::fs::write(&other, "Date,Description,Amount\n").unwrap();
    assert!(!ImporterKind::ChaseChecking.detect(&other));
}
```

## Shared Helpers

These are available in `crates/nigel-core/src/importer.rs` for use in any parser:

| Function | Purpose |
|----------|---------|
| `create_csv_reader(path)` | A flexible-length `csv::Reader` over the file, headers off |
| `parse_amount(raw)` | Strips commas, quotes, whitespace → `Option<f64>`; `None` is a reject, not a zero |
| `parse_date_mdy(raw)` | Converts `MM/DD/YYYY` → `Option<String>` of `YYYY-MM-DD` |
| `excel_serial_to_date(serial)` | Converts Excel serial number → `YYYY-MM-DD` string |

## Parse Contract

Every parser must produce a `ParseOutcome`. Each `ParsedRow` in `rows` carries:

- **`date`** — `YYYY-MM-DD` format (use `parse_date_mdy` for MM/DD/YYYY sources, `excel_serial_to_date` for XLSX)
- **`description`** — as-is from the source, trimmed
- **`amount`** — negative = expense/debit, positive = income/credit. If the source uses separate debit/credit columns or a type indicator, normalize to this convention in the parser.
- **`external_id`** — the source's own id for the transaction, when it has one (an OFX `FITID`, a feed's transaction id), prefixed with the source so ids from different feeds into one account cannot collide: `ofx:<FITID>`, `simplefin:<account id>:<id>`. `None` for formats without one.

Duplicate detection follows from `external_id`. A row with one is a duplicate exactly when the account already holds that id or an earlier row of the same import carried it; the description is not consulted, so a bank that rewrites a description does not re-import the row. A row without one is matched on (date, amount, description) by multiplicity: if the account holds `m` transactions with that key, the first `m` such rows in the file are skipped and the rest import, so identical rows within one statement all land. The database enforces one transaction per (account, `external_id`).

The match runs one way: a row without an id counts transactions that have one, but a row with an id never matches a transaction imported without one. A source with ids that starts on an account already fed by CSV must therefore start after that account's last imported date, not overlap it.

A row the parser cannot read goes to `rejects` — `out.reject(&record, reason)`, or
`out.reject_unreadable(&err)` when the CSV reader itself could not produce a record. The
reject keeps the line number in the file, the raw row, and a reason in the parser's own
words (`date "13/40/2025" is not MM/DD/YYYY`), because that reason is what the reader
sees in `nigel imports rejects <id>` and has to fix the file by.

A silent `continue` is only for lines that are not transactions at all: wholly blank
rows, the header, a "Beginning balance" summary. Test every field, not the first one —
a row with an empty date column and a real description and amount is a transaction the
parser cannot place, and it belongs in `rejects`. Anything that was meant to be a transaction and
is not usable is a reject — dropping it silently is how a statement imports short and
nobody finds out. A file whose parse yields no rows at all is refused by the import,
which reports the format, the malformed count and the first reasons rather than
recording an empty import.

## Feature Gating

To make an importer optional (like Gusto), gate it behind a Cargo feature:

1. Add the feature to `Cargo.toml`:
   ```toml
   [features]
   default = ["gusto"]
   gusto = ["dep:calamine"]
   ```

2. Gate enum variant, match arms, `ALL_IMPORTERS` entry, and functions with `#[cfg(feature = "...")]`:
   ```rust
   #[cfg(feature = "gusto")]
   GustoPayroll,
   ```

3. Test both configurations:
   ```bash
   cargo test                        # with feature
   cargo test --no-default-features  # without feature
   ```

Feature gating is only needed for importers that pull in heavy or optional dependencies. Standard CSV importers don't need gating.

## Account Type Matching

When `--format` is not specified, `get_for_file()` narrows candidates by `account_types()` then runs `detect()`. If your importer shares an account type with an existing one (e.g., two checking importers), make sure `detect()` returns `true` only for files that actually match your format. The first importer whose `detect()` returns `true` wins.

## SimpleFIN Bridge

[SimpleFIN Bridge](https://www.simplefin.org/protocol-v1.html) is a read-only aggregator: a one-time setup token is claimed for an access URL, and that URL returns accounts, balances and transactions as JSON. It is not an `ImporterKind` — there is no file to detect — but a feed that builds a `ParsedImport` per account and hands it to `import_parsed()`. Code lives in `crates/nigel-core/src/simplefin/`.

```bash
nigel sync setup              # paste the setup token; the access URL goes to settings.json
nigel sync accounts           # list the bridge's accounts
nigel sync accounts --map harborvale.example/chk1 "Operating"
nigel sync --dry-run          # what a sync would import
nigel sync                    # snapshot, import, categorize
```

- **Rows.** `posted` becomes the local calendar day, the `amount` string is rounded to cents (positive is money in, matching Nigel's sign), and `description` is kept as sent. Pending transactions and those with no posted time are skipped until they post. A transaction whose amount or time cannot be read is a reject: recorded on the account's import when the sync adds something, and reported in the summary either way.
- **Dedup.** Each row's `external_id` is `simplefin:<org>/<account id>:<transaction id>` — the protocol guarantees ids are unique within an account, not across them. A transaction already imported by id is never imported again, whatever its description says now.
- **Window.** A first sync of an account starts the day after its latest transaction. Later syncs re-read a week before the last sync, and the transaction id absorbs that overlap. Neither ever reaches back to the latest transaction imported from a file, whose rows carry no id a feed's row could match. Mapping or unmapping an account resets it to a first sync. `--since` overrides all of this, so a `--since` that reaches into days a CSV already covered imports those days twice. One request covers every mapped account; each account trims its own rows to its start day.
- **History.** Each account with something new becomes one `imports` row named `SimpleFIN: <bank> — <account>`, so `nigel imports list` and `nigel undo` treat it like a file. An account with nothing new writes none.
- **Balances.** Each account's `balance` and `balance-date` are stored in `account_balances`, and `nigel report balance` shows the latest beside the computed figure.
- **Errors.** A 403 means the access was revoked or expired — run `nigel sync setup` again; a 402 means the bridge subscription lapsed. Messages the bridge sends in `errors` are stripped of control characters and printed as warnings.

The web app does the same from **Settings → Bank sync**, over the [`/api/sync` routes](api.md#bank-sync).

Running it on a schedule is a matter of the host's scheduler; [the runbook](runbooks/schedule-simplefin-sync.md) has a systemd timer.

