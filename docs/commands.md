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
nigel accounts rename 1 "New Name"                # Rename account by ID
nigel accounts delete 3                           # Delete account by ID (blocked if has transactions)
nigel categories list                             # List all categories
nigel categories add "Consulting" --type income   # Add a category
nigel categories rename 5 "Professional Fees"     # Rename a category
nigel categories update 5 "Fees" --type income --tax-line "Gross receipts"  # Update all fields
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
nigel reconcile "BofA Checking" --month 2025-03 --balance 12345.67
nigel serve                                       # Web UI + JSON API on 127.0.0.1:5731 (opens a browser)
nigel serve --port 8080                           # Bind a different port (0 = ephemeral)
nigel serve --no-open                             # Print the tokenized URL instead of opening a browser
nigel status                                      # Show active DB and summary stats
nigel load ~/other-books                          # Switch to a different data directory
nigel backup                                      # Back up DB to <data_dir>/backups/
nigel backup --output /tmp/nigel-backup.db        # Back up to custom path
nigel restore ~/backups/nigel-20250301-120000.db  # Restore from a backup file
nigel password set                                # Encrypt an unencrypted database
nigel password change                             # Change password on encrypted database
nigel password remove                             # Decrypt database (remove password)
nigel update                                      # Check for and install the latest version
nigel completions bash                            # Generate shell completions (bash, zsh, fish, powershell)
```

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
