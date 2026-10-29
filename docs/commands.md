# Command reference

Every `nigel` invocation this project documents, plus the web workspace's own commands. CLAUDE.md keeps the handful an agent needs constantly; this file is the full list, and `nigel --help` is always current.

## CLI

```bash
cargo build                                       # Debug build
cargo build --release                             # Release build
cargo test -- --test-threads=1                    # Run all tests (serial — the DB password is a process global)
cargo test --no-default-features -- --test-threads=1   # Test without gusto/pdf features
nigel                                             # Interactive dashboard (default)
nigel --help                                      # CLI help
nigel init                                        # Initialize (prompts for data dir on first run)
nigel init --data-dir ~/my-books                  # Initialize with custom data dir
nigel init --profile personal                     # Seed the personal chart of accounts (default: business)
nigel demo                                        # Load sample data to explore
nigel import <file> --account <name>              # Import CSV/XLSX (auto-detects format)
nigel import <file> --account <name> --format bofa_checking  # Import with explicit format
nigel import <file> --account <name> --dry-run           # Preview without importing
nigel import <file> --account <name> --date-col 0 --desc-col 1 --amount-col 3  # Generic CSV
nigel import <file> --account <name> --date-col 0 --desc-col 1 --amount-col 3 --save-profile chase  # Save profile
nigel import <file> --account <name> --format chase      # Use saved profile
nigel undo                                        # Undo the last import (with confirmation)
nigel imports list                                # Every import, newest first, with its row and dropped counts
nigel imports rejects 7                           # The rows import 7 could not parse: line, reason, raw row
nigel accounts add "BofA Checking" --type checking # Add an account (--class defaults from the type)
nigel accounts add "Globex Card" --type credit_card --class liability  # Set the class explicitly
nigel accounts rename 1 "New Name"                # Rename account by ID
nigel accounts edit 1 --name "New Name" --class liability  # Change the name, the class, or both
nigel accounts delete 3                           # Delete account by ID (blocked if has transactions)
nigel categories list                             # List all categories
nigel categories add "Consulting" --type income   # Add a category (--class defaults from the type)
nigel categories add "Owner Draw" --type expense --class equity  # Set the class explicitly
nigel categories rename 5 "Professional Fees"     # Rename a category
nigel categories update 5 "Fees" --type income --tax-line "Gross receipts"  # Update all fields
nigel categories update 5 "Owner Draw" --type expense --class equity  # Update the accounting class
nigel categories delete 30                        # Soft-delete a category
nigel rules test "ADOBE" --match-type contains    # Test pattern against transactions (dry run)
nigel rules update 1 --priority 10                # Update a rule field
nigel rules update 5 --category "Rent / Lease"    # Reassign rule category
nigel rules delete 3                              # Deactivate a rule (soft-delete)
nigel categorize                                  # Re-run rules on uncategorized
nigel recategorize 185 212 --category "Travel"    # Bulk move by IDs (applies immediately)
nigel recategorize --from-category "Cost of Goods Sold" --year 2025 --category "Supplies" --yes
                                                  # Bulk move by filters (--dry-run to preview; --yes to apply)
nigel review                                      # Interactive review
nigel review --id 185                             # Re-review a specific transaction by ID
nigel report pnl --year 2025                      # Interactive view (ratatui)
nigel report expenses --month 2025-03             # Expense breakdown
nigel report tax --year 2025                      # Tax summary
nigel report cashflow                             # Cash flow
nigel report balance                              # Cash position
nigel report register --year 2025                 # Interactive register browser
nigel report register --account "BofA Checking"   # Filter by account
nigel report register --category "Taxes & Licenses"  # Filter by category
nigel report register --uncategorized             # Only transactions with no category
nigel report flagged                              # Flagged transactions
nigel report k1 --year 2025                       # K-1 prep worksheet (1120-S)
nigel report aging                                # A/R aging buckets and open invoices
nigel report pnl --year 2025 --mode export        # Export as PDF
nigel report pnl --year 2025 --mode export --format text  # Export as text file
nigel report pnl --year 2025 --output ~/report.pdf  # --output implies export
nigel report all --year 2025                      # Bulk export all reports (PDF)
nigel report all --year 2025 --format text        # Bulk export as text files
nigel report all --year 2025 --output-dir ~/exports/  # Custom output directory
nigel browse register                            # All transactions, starts at today
nigel browse register --year 2025                 # Filter to a specific year
nigel browse register --account "BofA Checking"   # Browse filtered by account
nigel browse register --category "Taxes & Licenses"  # Browse filtered by category
nigel browse register --uncategorized             # Browse transactions with no category
nigel client add "Acme Co" --email ap@acme.test        # Add an invoicing client
nigel client list                                 # List clients with their IDs
nigel client show 1                               # One client: details plus invoice history
nigel client edit 1 --email ap@acme.test          # Update a client's name/email/address/notes
nigel client edit 1 --contact "ap@acme.test:Ada:AP" --contact "dana@acme.test"
                                                  # Replace the contact list (first = billed, rest cc'd)
nigel client delete 7 --yes                       # Delete (refused while any invoice bills them)
nigel client archive 7                            # Hide a finished client; touches no invoice
nigel client unarchive 7                          # Bring it back to the working list
nigel client list --all                           # Include archived clients, with the date
nigel invoice new --client 1 --issue 2026-08-04 --item "Consulting:10:150"  # Draft (--item repeatable)
nigel invoice new … --notes "Thanks" --terms "Net 30"  # Rendered on the invoice page and the PDF
nigel invoice edit 1248 --due 2026-09-30          # Edit a draft (published invoices refuse)
nigel invoice edit 1248 --clear-due               # Drop the due date, so it never goes overdue
nigel invoice duplicate 1248                      # Copy into a fresh draft, issued today
nigel invoice duplicate 1248 --issue 2026-09-01   # Copy with a given issue date
nigel invoice void 1248                           # Cancel an invoice (confirms; --yes to skip)
nigel invoice delete 1252                         # Delete an unsent draft (confirms; --yes to skip)
nigel invoice list                                # Number, status, client, total, due date
nigel invoice show 1248                           # Line items, paid amount, payment link
nigel invoice preview 1248                        # Render HTML/PDF locally, no network (<data_dir>/previews)
nigel invoice preview 1248 --output-dir /tmp      # Write the preview somewhere else
nigel invoice send 1248                                 # Render, write the preview files, confirm, then publish and email
nigel invoice send 1248 --yes                           # Skip the confirmation and the files
nigel invoice sync                                # Pull Stripe payments and record them
nigel invoice pay 1248 --date 2026-08-20          # Record a manual payment (default: full balance)
nigel invoice pay 1248 --date 2026-08-20 --amount 500 --method ach  # Partial/other method
nigel invoice aging                               # A/R aging buckets
nigel invoice import --from-invoiceshelf ~/is.sqlite  # One-time InvoiceShelf import
nigel invoice template export                     # Write the built-in page to <data_dir>/templates/invoice.html
nigel invoice template export --output ~/mine.html --force  # Somewhere else / overwrite
nigel invoice template path                       # Where Nigel looks, and whether an override is in effect
nigel invoice schedule add --client 1 --cadence monthly --start 2026-01-01 --item "Hosting:1:450"
nigel invoice schedule add --client 1 --cadence quarterly --start 2026-01-01 --from 1248  # Seed from an invoice
nigel invoice schedule list                       # Active schedules (--all includes paused and ended)
nigel invoice schedule show 1                     # Items, and every invoice it has generated
nigel invoice schedule edit 1 --item "Hosting:1:495"   # Applies to future invoices only
nigel invoice schedule pause 1                    # Stop generating without ending it
nigel invoice schedule resume 1                   # Start generating again
nigel invoice schedule end 1                      # End it for good; history is kept
nigel invoice schedule end 1 --bill               # Bill the periods it still owes, then end
nigel invoice schedule end 1 --forgive            # Write those periods off, then end
nigel invoice schedule run                        # Generate everything due (cron/launchd; never prompts)
nigel document kinds                              # List document kinds, including inactive ones
nigel document kinds add "Statement of Work"      # Add a kind
nigel document kinds rename 2 "Master Agreement"  # Rename a kind
nigel document kinds deactivate 2                 # Stop offering a kind for new documents
nigel document kinds reactivate 2                 # Offer a deactivated kind again
nigel document add --client 1 --kind Proposal --title "Website rebuild" --file ~/proposal.pdf  # File a PDF as a draft
nigel document list                               # Every document (--client, --status, --kind narrow it)
nigel document show 3                             # Versions, recipients, signatures and change requests; a version whose PDF is missing is flagged
nigel document preview 3                          # Write the recipient pages and PDF to <data_dir>/previews/document-3/, no network
nigel document preview 3 --output-dir /tmp        # Write it to /tmp/document-3/ instead
nigel document send 3                             # Confirm, then publish and email the signer a link of their own
nigel document send 3 --signer "Pat Example <pat@cedar.example.test>" --collaborator "Sam Example <sam@cedar.example.test>"  # Name the recipients
nigel document send 3 --yes                       # Skip the confirmation (required when stdin is not a terminal)
nigel document revise 3 --file ~/proposal-v2.pdf  # New draft version; live pages read "being revised" until it is sent, then "superseded"
nigel document withdraw 3                         # Replace its pages with a withdrawn notice (--yes skips the prompt); terminal
nigel document accept 3 --name "Pat Example"      # Record an acceptance received outside the response page (--date)
nigel document request-changes 3 --name "Pat Example" --note "Move the start date"  # Record a change request (--date)
nigel document decline 3 --note "Going another way"  # Record a decline (--note and --date optional); terminal
nigel document sync                               # Pull online responses and record them (cron; never prompts)
nigel document countersign 3 --name "Sam Example" # Record your countersignature on an accepted document (--date), then publish its signing record and email the signed copy
nigel document signing-record 3                   # Regenerate an executed document's signing record and republish it; non-zero exit if R2 is set and the upload failed
nigel reconcile "BofA Checking" --month 2025-03 --balance 12345.67
nigel serve                                       # Web UI + JSON API on 127.0.0.1:5731 (opens a browser)
nigel serve --port 8080                           # Bind a different port (0 = ephemeral)
nigel serve --no-open                             # Print the tokenized URL instead of opening a browser
nigel status                                      # Show active DB and summary stats, including a Dropped rows line naming the accounts
nigel load ~/other-books                          # Switch to a different data directory
nigel backup                                      # Back up DB and filed document PDFs to <data_dir>/backups/
nigel backup --output /tmp/nigel-backup.db        # Back up to custom path (PDFs in /tmp/nigel-backup.documents/)
nigel restore ~/backups/nigel-20250301-120000.db  # Restore the DB and, when nigel-20250301-120000.documents/ sits beside it, the filed PDFs
nigel password set                                # Encrypt an unencrypted database
nigel password change                             # Change password on encrypted database
nigel password remove                             # Decrypt database (remove password)
nigel user add pat@example.com --name "Pat Example" --admin  # Add a user with a temporary password; the first must be --admin
nigel user list                                   # Email, name, role, status, two-step, last active (— when signed out)
nigel user role sam@example.com read_only         # Change a role: admin, bookkeeper, read_only
nigel user disable sam@example.com                # Disable a user and end their sessions (users are never deleted)
nigel user enable sam@example.com                 # Enable a disabled user
nigel user reset-password sam@example.com         # New temporary password; ends their sessions (--clear-totp also turns off two-step)
nigel update                                      # Check for and install the latest version
nigel completions bash                            # Generate shell completions (bash, zsh, fish, powershell)
```

A renamed distributions category needs its class set by hand. The v10 backfill
recognizes `Owner Draw / Distribution` and `Owner Contribution` by name; a
category called something else was classified from its type and landed on
`expense`, where the K-1 worksheet counts it as a deduction. Give it the class
it actually is — `nigel categories update <id> "<name>" --type expense --class
equity` — and its `K-16d` money is reported as distributions.

### Users

Users are the people who sign in to a shared `nigel serve`, identified by email. These
commands open the books in the configured data directory like every other command, so an
encrypted database is unlocked by the prompt, `NIGEL_DB_PASSWORD`, or
`NIGEL_DB_PASSWORD_FILE`. Whoever can open the file has full authority over its users;
this is also how a locked-out admin recovers. Once this table has a user, `nigel serve` admits only signed-in users;
with none it stays in local mode with its token link. See "Modes" in
[api.md](api.md#modes).

```bash
nigel user add pat@example.com --name "Pat Example" --admin            # The first user must be --admin
nigel user add sam@example.com --name "Sam Example" --role bookkeeper  # --role admin|bookkeeper|read_only
nigel user list                                                        # Email, name, role, status, two-step, last active (— when signed out)
nigel user role sam@example.com read_only                              # Change a role
nigel user disable sam@example.com                                     # Disable and end their sessions
nigel user enable sam@example.com
nigel user reset-password sam@example.com                              # New temporary password; ends their sessions
nigel user reset-password pat@example.com --clear-totp                 # Also turn off two-step (lost device)
```

- Exactly one of `--admin` or `--role` is required; no role is granted by default.
- `add` and `reset-password` print a temporary password once, in the form
  `K7QXM-2RTPA-W9HCN-4FJDE`. It is stored only as an Argon2id hash and cannot be shown
  again. The user must choose their own password when they first sign in.
- `reset-password` prints `Ended every session for <email>.`.
- A chosen password is 12 to 256 characters after Unicode (NFC) normalization, must not
  equal (ignoring case) the user's email, the part of it before the `@`, or their name, and
  must differ from the current one. There is no common-password list.
- Emails are trimmed, lower-cased and NFC-normalized, and refused if they contain
  whitespace or control characters; display names are trimmed and NFC-normalized. Emails are therefore matched
  without regard to case or surrounding spaces. An unknown email is an error naming it.
- The first `add` on unencrypted books warns that anyone who can read the file can read the books,
  and recommends `nigel password set` first.
- Users are never deleted. The last enabled admin cannot be disabled or demoted.
- `list` never prints a password hash or a two-step secret.
- Two-step secrets are sealed with `server.key`, which sits beside `settings.json` in
  `~/.config/nigel/`, not in the data directory, so it is not in any backup. If it is lost,
  each user with two-step on needs `nigel user reset-password <email> --clear-totp`.

### Unattended password

Any command that opens encrypted books reads the password from `NIGEL_DB_PASSWORD_FILE` (a
path to a file holding it, one trailing newline stripped) or from `NIGEL_DB_PASSWORD`
before it would prompt. Setting both is an error. A value that is empty, unreadable or
wrong stops the command rather than falling back to a prompt.

### Web UI

Requires Node 20.19+ (22 recommended). All commands run from `web/`.

```bash
npm ci                                            # Install (committed lockfile)
npm run build                                     # theme -> ui -> app, output to web/dist
npm test                                          # vitest across all three packages
npm run lint                                      # eslint across all three packages
npm run typecheck                                 # tsc --noEmit across all three packages
npm run dev                                       # Vite dev server on :5173 (proxies to :5731)
npm run preview                                   # Component preview harness on :9090
```

Dev loop — run the backend and the dev server side by side, then open the
token URL **on the vite origin** so the session cookie lands there:

```bash
cargo run -- serve --no-open                      # terminal 1, prints /auth?token=<hex>
cd web && npm run dev                             # terminal 2
# browser: http://localhost:5173/auth?token=<hex>
```

`cargo build` works without node — `build.rs` seeds `web/dist` from
`web/placeholder/index.html` and the binary serves a "SPA not built" page. Run
`npm run build` in `web/` before `cargo build --release` to embed the real app.
