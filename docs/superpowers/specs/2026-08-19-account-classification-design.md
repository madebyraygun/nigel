# Account Classification (TASK-9.1) — Design

**Goal:** One accounting-class vocabulary — asset, liability, equity, revenue,
expense — on both accounts and categories, with every report classifying from
it instead of from account-type strings or category-name checks. This is the
scheduled first step of the tax-package epic (TASK-102): Schedule L and the
equity work cannot exist without it.

The task file (`backlog task 9.1 --plain`) carries the full problem statement,
the migration mapping, the surfaces to audit, and the rejected alternative
(full table unification — deferred to TASK-9.2). This spec settles the
decisions the task leaves open. Where this spec and the task text disagree,
this spec wins; where this spec is silent, the task text is binding.

## Decisions

### 1. The class is a Rust enum first, a column second

`nigel_core::db::AccountClass { Asset, Liability, Equity, Revenue, Expense }`
with `as_str()`/`from_str()` in the same shape as `db::Profile` — the repo's
existing pattern for a closed set stored as text (and the direction TASK-60
names). The columns are `accounts.class TEXT NOT NULL` and
`categories.class TEXT NOT NULL`, each with a `CHECK (class IN ('asset',
'liability', 'equity', 'revenue', 'expense'))` so the closed set holds even
against hand-edited databases, not just against Rust callers.

Everything that reads a class goes through the enum. **No `_ =>` arm anywhere
a class is matched**: the trap the task names — a new class silently absorbed
into expenses — is prevented by exhaustive matching, and the compiler enforces
it only when no catch-all exists. The one permitted default is parsing a
corrupt string out of the database, which is an error, not a class.

### 2. Migration backfill (exactly the task's mapping)

Via `migrations.rs`, in one migration: checking/savings/payroll → `asset`;
credit_card/line_of_credit → `liability`; `category_type = 'income'` →
`revenue`; `category_type = 'expense'` → `expense`; then the seeded
`Owner Draw / Distribution` category → `equity` by name, after the general
rule, so the order is what makes it correct. A new `Owner Contribution`
equity category is seeded for the business profile (the task names its
absence as a defect), added idempotently the way category seeding already
works. `category_type` stays — it is the user-facing income/expense split the
UI organizes by; `class` is accounting structure underneath it.

### 3. Sign convention lives in one function (AC #4)

Transactions keep their bank-statement signs; nothing in the register or
importers changes. The convention is owned by one place in the reports module
— `fn natural_balance(class, raw_sum) -> i64` (or equivalent) — that says what
"the balance" of an account means per class: a liability with money owed
reports positive, an asset with money in it reports positive. Balance,
Schedule L (later), and the TUI/web balance surfaces all call it; none of them
re-derive sign from account-type strings. One implementation, one test table.

### 4. Reports classify from class, nothing else

The audit list from the task (`reports.rs` — P&L, expenses, tax summary, K-1,
cash flow, balance; `category_manager.rs`; `accounts.rs`; categories routes;
TUI screens; web equivalents) is worked through by replacing every
account-type-string branch and category-name check with a class match. Equity
is excluded from deductions everywhere — the distributions-counted-as-expenses
defect dies here, including the special-cases-by-name in existing reports.
AC #7's guard test: a synthetic category with each class in turn, asserting
expense totals move only for `expense`.

### 5. API and UI surface

- API: accounts and categories responses gain `class`; create/edit accept it
  (validated against the closed set). Defaults on create: accounts default by
  their `account_type` mapping; categories default by their `category_type`
  (`income` → revenue, `expense` → expense) — so existing clients keep working
  unchanged and the field is additive.
- CLI/TUI: class shown and settable where accounts and categories are edited,
  using the plain words (asset, liability, equity, revenue, expense).
- Web: the categories and accounts screens show and edit class through
  existing form primitives; any new visual element ships component-first. No
  debit/credit vocabulary anywhere user-facing (AC #6) — the five class words
  are the entire vocabulary.

### 6. What this does not do

No table merger, no journal lines (TASK-9.2). No Schedule L (TASK-102.1 — it
consumes this). No `--as-of` balances (TASK-46). No renaming of user-visible
account types or category groups.

## Testing

- Enum round-trip and CHECK-constraint tests; a corrupt class string surfaces
  as an error, not a default.
- Migration test on a seeded pre-migration database: every account and
  category lands on the task's mapping, distributions land on equity, and a
  second run is a no-op.
- The `natural_balance` table test across all five classes.
- The AC #7 absorption guard.
- Report-level regressions: P&L and tax summary totals unchanged on the
  fixture books after migration (the classification is structure, not new
  math), except where distributions were being miscounted — those assert the
  corrected number.
- CLI/TUI/API/web edit paths set and read class; fixtures from the fictional
  cast.

## Delivery note

Per the epic and the task (AC #10): the PR from this branch is opened as a
**draft** and stays a draft until the operator reviews it.
