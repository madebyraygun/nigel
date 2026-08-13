---
name: csv-rule-reviewer
description: Analyze a CSV before import and suggest categorization rules for unmatched transactions
---

# CSV Rule Reviewer

Analyze a bank CSV file before importing and suggest `nigel rules add` commands for transactions that would otherwise be flagged as uncategorized.

## Inputs

The user provides a bank CSV file path. They may also specify the importer format or account name.

## Workflow

### 1. Read the CSV

Read the provided file and identify:
- **Header row** — column names, position in file (may have preamble rows)
- **Date column** — which column contains the transaction date
- **Description column** — which column contains the description/memo
- **Amount column** — single signed column or separate debit/credit columns
- Skip preamble rows, summary rows, and blank lines

### 2. Extract unique descriptions

- Parse all data rows and collect the description values
- Normalize: uppercase, trim whitespace, collapse multiple spaces
- Deduplicate and count occurrences of each unique description

### 3. Read existing rules

Run `nigel rules list` to get current rules, or read the rules from the database directly:
```bash
nigel rules list
```
Note the pattern, match type, category, and vendor for each rule.

### 4. Simulate matching

For each unique description from the CSV:
- Check if any existing rule matches using the rule's match type:
  - `contains` — description contains the pattern (case-insensitive)
  - `starts_with` — description starts with the pattern (case-insensitive)
  - `regex` — description matches the regex pattern
- Track which descriptions are **matched** and which are **unmatched**

### 5. Group unmatched descriptions

Cluster unmatched descriptions by common substrings:
- Extract likely vendor names (first 2-3 words, common prefixes)
- Group descriptions that share the same vendor/prefix
- Sort groups by total occurrence count (most frequent first)

### 6. Read existing categories

Query the database for valid category names:
```bash
sqlite3 ~/Documents/nigel/nigel.db "SELECT name, category_type, description FROM categories WHERE is_active = 1 ORDER BY category_type, name"
```
Use the settings data dir if it differs from the default.

### 7. Suggest rules

For each cluster of unmatched descriptions, suggest a `nigel rules add` command:
- **Pattern**: the common substring that matches all descriptions in the cluster
- **Category**: best-fit from existing categories based on the description context
- **Vendor**: normalized vendor name (clean, title-cased)
- **Match type**: `contains` for most patterns, `starts_with` if the pattern is a prefix
- **Priority**: 0 (default) unless a more specific rule is needed

Format each suggestion as a ready-to-run command:
```bash
nigel rules add "PATTERN" --category "Category Name" --vendor "Vendor Name" --match-type contains
```

### 8. Output

Present a summary:
1. **Coverage stats** — X of Y unique descriptions already covered by rules
2. **Suggested rules** — grouped by category, each with the command to run
3. **Remaining unmatched** — descriptions that couldn't be confidently categorized (suggest manual review)

The user can copy/paste commands directly or approve them interactively.

## Tax-sensitive descriptions

Some transactions cost real money at filing time if they land in the wrong category. When a description looks like one of these, say so in the suggestion rather than quietly picking the nearest expense category — the right answer often depends on facts that are not in the CSV.

| Looks like | Why it matters | What to do |
| --- | --- | --- |
| Owner transfers to a personal account | Distributions reduce equity; they are not deductible and they belong on Schedule K line 16d | Suggest the owner distribution category, and note it is not an expense |
| Payments to a taxing authority (IRS, FTB, EDD, state web pay) | Estimated tax payments are credits against tax owed, not deductions; the $800 CA minimum franchise tax is deductible; penalties are neither | Do not lump these into `Taxes & Licenses` — ask which kind it is |
| Anything with PENALTY, LATE FEE or INTEREST from a tax authority | Tax and interest are deductible; penalties are not, and go on Schedule K line 16c | Flag for a split; one payment may need to be recorded as two or three transactions |
| Hardware and equipment over roughly $2,500 | Tangible property may need a §179 election and a placed-in-service date, which is often not the payment date | Suggest the equipment category and flag it for the asset register |
| Contractor and freelancer payments | Payees at or above $600/year may need a Form 1099-NEC; foreign contractors need a W-8BEN on file | Suggest `Contract Labor` and note the payee needs documentation checked |
| Health insurance premiums | For a 2% S-corp shareholder these are wages, not a benefits expense | Ask whether the payee is a shareholder-employee before categorizing |
| Payroll runs | Officer compensation (1120-S line 7) and employee wages (line 8) are separate lines | Check whether the payer has an officer wage category before defaulting to `Payroll — Wages` |

When the deciding fact is not in the file, ask. A confidently wrong category here is more expensive than an uncategorized transaction, which at least gets reviewed.

## Tips

- Prefer broader patterns that catch multiple descriptions over narrow exact matches
- When unsure about a category, suggest the most likely option with a comment noting uncertainty
- For transfers between accounts (e.g., "TRANSFER TO...", "Online Banking Transfer"), suggest the "Transfer" category
- For payroll-related descriptions, check if they match Gusto patterns before suggesting
- Watch for common bank-specific prefixes (e.g., "DEBIT CARD PURCHASE", "ACH DEBIT") that can be stripped to find the actual vendor name
- Vendor names are what the 1099 roster groups by — normalize them consistently (`Adobe`, not `Adobe` in one rule and `Adobe Inc` in another) so annual per-payee totals are trustworthy
