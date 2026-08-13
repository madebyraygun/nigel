---
name: importer-engineer
description: Analyze a bank CSV/XLSX data file and generate a complete Nigel importer. Use when the user provides a bank statement file and wants to create an importer for it.
---

# Importer Engineer

Generate a working Nigel importer from a bank data file.

## Inputs

The user provides a bank statement file (CSV or XLSX). They may also specify:
- An importer name (e.g., "Chase Checking")
- The account type (checking, credit_card, line_of_credit, payroll)

## Workflow

### 1. Analyze the data file

Read the provided file and identify:
- **File format** — CSV or XLSX
- **Header row** — column names, position in file (may have preamble rows)
- **Header signature** — unique string or pattern to use for detection
- **Column mapping** — which columns contain: date, description, amount (and optionally: type/direction indicator, balance, check number)
- **Date format** — MM/DD/YYYY, YYYY-MM-DD, Excel serial, or other
- **Amount convention** — single signed column, separate debit/credit columns, or unsigned amount with type indicator (D/C)
- **Rows to skip** — preamble, summary rows, "Beginning balance" entries

### 2. Read the existing pattern

Read `src/importer.rs` to understand:
- Current `ImporterKind` variants and match arms
- How existing detect/parse functions work
- Available helpers: `parse_amount`, `parse_date_mdy`, `excel_serial_to_date`, `parse_csv_line`

Also read `docs/importers.md` for the full walkthrough.

### 3. Determine the importer specification

From the analysis, determine:
- **Variant name** — PascalCase (e.g., `ChaseChecking`)
- **Key** — snake_case (e.g., `chase_checking`)
- **Display name** — human-readable (e.g., `"Chase Checking"`)
- **Account types** — which account types this format applies to
- **Detection logic** — what makes this file uniquely identifiable
- **Parse logic** — how to extract date, description, amount from each row
- **Amount normalization** — how to convert to the negative=expense, positive=income convention
- **Feature gating** — only if the format requires an optional dependency (e.g., calamine for XLSX)

### 4. Generate the code

Make the following changes to `src/importer.rs`:

1. Add variant to `ImporterKind` enum
2. Add match arms to `key()`, `name()`, `account_types()`, `detect()`, `parse()`
3. Add entry to `ALL_IMPORTERS`
4. Write `detect_<key>()` function
5. Write `parse_<key>()` function
6. If XLSX format: add `#[cfg(feature = "...")]` gating and update `Cargo.toml`

### 5. Generate a test

Add a test in the `mod tests` block that:
- Creates a temp file with sample data copied from the real file (3-5 representative rows)
- Verifies `detect()` returns true for matching files and false for non-matching
- Verifies `parse()` returns correct row count, dates, descriptions, and amounts
- Sanitize any personal data in sample rows (replace names, account numbers, etc.)

### 6. Verify

Run `cargo test` to confirm the new importer passes all tests and doesn't break existing ones. Run `cargo test --no-default-features` if feature-gated code was modified.

## Don't discard tax-relevant detail

Statements and payroll exports usually carry more than date, description and amount, and the extra columns are exactly what the year-end reports need. Discarding them at parse time means going back to PDFs at filing time. When the source file offers them, note them in the importer spec even if the current schema has nowhere to put them yet — and say so in the summary rather than dropping them silently.

- **Opening and closing balances.** Most statements state a beginning balance in a preamble or summary row that the parser is told to skip. Those rows are the source for per-account opening balances, without which no year-end balance sheet is correct.
- **Per-employee payroll detail.** Payroll exports break gross pay down by person and pay item. Aggregating a run into one wage transaction throws away the officer/employee split (1120-S line 7 vs line 8) and any separately reported pay item, such as 2% shareholder health insurance. Prefer preserving the breakdown, or flag clearly that it is being collapsed.
- **Payment method and card indicators.** Whether a contractor was paid by card or by ACH determines whether a Form 1099-NEC is the payer's responsibility at all.
- **Reference and confirmation numbers.** The way a duplicated tax payment gets identified as duplicated.
- **Posting date vs. transaction date.** They differ across year end, which is precisely when it matters on cash-basis books.

If a format contains one of these and the parser cannot yet use it, say which column it is and what it would be good for. That is a better outcome than a clean-looking importer that quietly loses the figure.

## Code Conventions

- Detection functions: `fn detect_<key>(file_path: &Path) -> bool`
- Parse functions: `fn parse_<key>(file_path: &Path) -> Result<Vec<ParsedRow>>`
- Use existing helpers (`parse_amount`, `parse_date_mdy`, etc.) where possible
- Dates must be `YYYY-MM-DD` in the output `ParsedRow`
- Amounts: negative = expense, positive = income
- Skip empty rows, preamble, and summary/balance rows
- Place new importer sections between the last bank parser and the `parse_csv_line` helper at the bottom
