use clap::{CommandFactory, Parser};

use nigel::cli::{
    self, AccountsCommands, BrowseCommands, CategoriesCommands, Cli, ClientCommands, Commands,
    DocumentCommands, ImportsCommands, InvoiceCommands, InvoiceScheduleCommands,
    InvoiceTemplateCommands, PasswordCommand, RulesCommands,
};
use nigel_core::error;

/// Reconcile Stripe payments before a data-bearing command runs. Best-effort:
/// with no Stripe key configured it does nothing, and any failure prints a
/// notice instead of failing the command the user actually asked for.
fn sync_invoice_payments() {
    let cfg = nigel_core::settings::invoicing_config();
    let Some(secret_key) = cfg.stripe_secret_key.clone() else {
        return;
    };
    let gateway = nigel_core::invoicing::stripe::StripeClient { secret_key };
    let data_dir = nigel_core::settings::get_data_dir();
    let result = nigel_core::db::get_connection(&data_dir.join("nigel.db")).and_then(|conn| {
        let report = nigel_core::invoicing::sync::sync_all_report(&conn, &cli::today(), &gateway)?;
        // A payment found at launch corrects the page it was found for, so a
        // client following their bookmark does not see a balance they settled.
        let warnings =
            nigel::cli::invoice::republish_all(&conn, &report.recorded_invoices, &cfg, &data_dir);
        Ok((report, warnings))
    });

    match result {
        Ok((report, warnings)) => {
            for failure in &report.failures {
                eprintln!(
                    "notice: invoice sync failed for #{}: {}",
                    failure.number, failure.message
                );
            }
            if report.recorded > 0 {
                eprintln!(
                    "notice: recorded {} new invoice payment(s)",
                    report.recorded
                );
            }
            for warning in warnings {
                eprintln!("notice: {warning}");
            }
        }
        Err(e) => eprintln!("notice: invoice sync skipped: {e}"),
    }
}

/// Pull online document responses before a data-bearing command runs.
/// Best-effort and silent without the private store configured; a failure
/// prints a notice instead of failing the command the user asked for.
fn sync_document_responses() {
    let config = nigel_core::settings::documents_config();
    let Some(source) = nigel_core::invoicing::wiring::optional_response_source(&config) else {
        return;
    };
    let publisher = nigel_core::invoicing::wiring::optional_document_publisher(&config);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let result =
        nigel_core::db::get_connection(&nigel_core::settings::get_data_dir().join("nigel.db"))
            .and_then(|conn| {
                let company = nigel_core::invoicing::wiring::company_name(&conn);
                nigel_core::documents::sync::sync_documents(
                    &conn,
                    &company,
                    &source,
                    publisher.as_ref(),
                    Some(deadline),
                )
            });
    match result {
        Ok(report) => {
            for failure in &report.failures {
                eprintln!(
                    "notice: document sync failed for #{}: {}",
                    failure.document_id,
                    cli::document::printable(&failure.message)
                );
            }
            for line in &report.lines {
                for warning in &line.warnings {
                    eprintln!("notice: {}", cli::document::printable(warning));
                }
            }
            if report.recorded > 0 {
                eprintln!(
                    "notice: recorded {} new document response(s)",
                    report.recorded
                );
            }
        }
        Err(e) => eprintln!(
            "notice: document sync skipped: {}",
            cli::document::printable(&e.to_string())
        ),
    }
}

fn main() {
    // Install ratatui panic hook once — restores terminal on panic for all TUI screens
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        hook(info);
    }));

    let cli = Cli::parse();

    let result = match cli.command {
        // Dashboard handles missing init via its own onboarding flow
        None => cli::dashboard::run(),
        Some(command) => {
            // Non-blocking update check for CLI subcommands (dashboard does its own).
            // Skip when running `nigel update` since it does its own check.
            if !matches!(command, Commands::Update) {
                if let Some(msg) = cli::update::check_and_notify() {
                    eprintln!("notice: {msg}");
                }
            }
            dispatch(command)
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

/// Commands built for cron and launchd, which must never reach a prompt.
fn is_unattended(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Invoice {
            command: InvoiceCommands::Schedule {
                command: InvoiceScheduleCommands::Run
            }
        } | Commands::Document {
            command: DocumentCommands::Sync
        }
    )
}

fn dispatch(command: Commands) -> error::Result<()> {
    // Commands that need an already-initialized database (skip for init/demo which create
    // new DBs, load which switches directories, update which needs no DB, and
    // `invoice template`, which only reads and writes a file in the data directory)
    let needs_existing_db = !matches!(
        command,
        Commands::Init { .. }
            | Commands::Demo
            | Commands::Load { .. }
            | Commands::Update
            | Commands::Invoice {
                command: InvoiceCommands::Template { .. }
            }
    );

    // Commands that need the encryption password up front. `password` does its own
    // prompting as part of set/change/remove, `completions` never touches the DB, and
    // `serve` has no stdin to prompt on — its clients unlock over HTTP instead, and
    // `invoice template` never opens the database.
    let needs_password = !matches!(
        command,
        Commands::Init { .. }
            | Commands::Demo
            | Commands::Password { .. }
            | Commands::Completions { .. }
            | Commands::Serve { .. }
            | Commands::Update
            | Commands::Invoice {
                command: InvoiceCommands::Template { .. }
            }
    );

    let db_path = nigel_core::settings::get_data_dir().join("nigel.db");

    if needs_existing_db && !db_path.exists() {
        return Err(error::NigelError::NotInitialized);
    }

    if needs_password && db_path.exists() {
        if is_unattended(&command) {
            nigel::cli::password::unlock_without_prompting(&db_path)?;
        } else {
            nigel::cli::password::prompt_password_if_needed(&db_path)?;
        }
    }

    // `restore` overwrites the database file and then migrates the restored copy itself,
    // so migrating the outgoing one first is wasted work that could abort the very
    // recovery meant to repair it.
    let replaces_db = matches!(command, Commands::Restore { .. });

    // Bring the schema up to date before any command reads or writes data. The
    // intersection of the two guards above is exactly the set of commands that open the
    // existing database with a usable password; init/demo/restore migrate via their own
    // init_db() call, the dashboard migrates in its own pre-flight, and serve migrates
    // whatever it can reach without a password.
    if needs_existing_db && needs_password && !replaces_db {
        let conn = nigel_core::db::get_connection(&db_path)?;
        nigel_core::db::init_db(&conn)?;
    }

    // Reconcile Stripe payments and document responses; `cli::launch_sync_allowed`
    // says which commands skip it and why.
    if cli::launch_sync_allowed(&command) {
        sync_invoice_payments();
        sync_document_responses();
    }

    match command {
        Commands::Init { data_dir, profile } => cli::init::run(data_dir, &profile),
        Commands::Accounts { command } => match command {
            AccountsCommands::Add {
                name,
                account_type,
                class,
                institution,
                last_four,
            } => cli::accounts::add(
                &name,
                &account_type,
                class.as_deref(),
                institution.as_deref(),
                last_four.as_deref(),
            ),
            AccountsCommands::List => cli::accounts::list(),
            AccountsCommands::Rename { id, name } => cli::accounts::rename(id, &name),
            AccountsCommands::Edit { id, name, class } => {
                cli::accounts::edit(id, name.as_deref(), class.as_deref())
            }
            AccountsCommands::Delete { id } => cli::accounts::delete(id),
        },
        Commands::Categories { command } => match command {
            CategoriesCommands::List => cli::categories::list(),
            CategoriesCommands::Add {
                name,
                category_type,
                class,
                tax_line,
                form_line,
            } => cli::categories::add(
                &name,
                &category_type,
                class.as_deref(),
                tax_line.as_deref(),
                form_line.as_deref(),
            ),
            CategoriesCommands::Rename { id, name } => cli::categories::rename(id, &name),
            CategoriesCommands::Update {
                id,
                name,
                category_type,
                class,
                tax_line,
                form_line,
            } => cli::categories::update(
                id,
                &name,
                &category_type,
                class.as_deref(),
                tax_line.as_deref(),
                form_line.as_deref(),
            ),
            CategoriesCommands::Delete { id } => cli::categories::delete(id),
        },
        Commands::Client { command } => match command {
            ClientCommands::Add {
                name,
                email,
                address,
                contacts,
            } => cli::client::add(&name, email.as_deref(), address.as_deref(), &contacts),
            ClientCommands::Show { id } => cli::client::show(id),
            ClientCommands::Edit {
                id,
                name,
                email,
                address,
                notes,
                contacts,
            } => cli::client::edit(id, name, email, address, notes, &contacts),
            ClientCommands::Delete { id, yes } => cli::client::delete(id, yes),
            ClientCommands::Archive { id } => cli::client::archive(id, &cli::today()),
            ClientCommands::Unarchive { id } => cli::client::unarchive(id),
            ClientCommands::List { all } => cli::client::list(all),
        },
        Commands::Document { command } => match command {
            DocumentCommands::Kinds { command } => cli::document::kinds(command),
            DocumentCommands::Add {
                client,
                kind,
                title,
                file,
            } => cli::document::add(client, &kind, &title, &file, &cli::today()),
            DocumentCommands::List {
                client,
                status,
                kind,
            } => cli::document::list(client, status.as_deref(), kind.as_deref()),
            DocumentCommands::Show { id } => cli::document::show(id),
            DocumentCommands::Preview { id, output_dir } => {
                cli::document::preview(id, output_dir.as_deref())
            }
            DocumentCommands::Send {
                id,
                signer,
                collaborators,
                yes,
            } => cli::document::send(id, signer.as_deref(), &collaborators, yes, &cli::today()),
            DocumentCommands::Sync => cli::document::sync(),
            DocumentCommands::Revise { id, file } => {
                cli::document::revise(id, &file, &cli::today())
            }
            DocumentCommands::Withdraw { id, yes } => {
                cli::document::withdraw(id, yes, &cli::today())
            }
            DocumentCommands::Accept { id, name, date } => {
                cli::document::accept(id, &name, &date.unwrap_or_else(cli::today))
            }
            DocumentCommands::RequestChanges {
                id,
                name,
                note,
                date,
            } => cli::document::request_changes(id, &name, &note, &date.unwrap_or_else(cli::today)),
            DocumentCommands::Decline { id, note, date } => {
                cli::document::decline(id, note.as_deref(), &date.unwrap_or_else(cli::today))
            }
            DocumentCommands::Countersign { id, name, date } => {
                cli::document::countersign(id, &name, &date.unwrap_or_else(cli::today))
            }
        },
        Commands::Invoice { command } => match command {
            InvoiceCommands::New {
                client,
                issue_date,
                due_date,
                currency,
                items,
                notes,
                terms,
            } => cli::invoice::new(
                client,
                &issue_date,
                due_date.as_deref(),
                &currency,
                &items,
                notes.as_deref(),
                terms.as_deref(),
            ),
            InvoiceCommands::Duplicate { number, issue_date } => {
                cli::invoice::duplicate(number, issue_date.as_deref().unwrap_or(&cli::today()))
            }
            InvoiceCommands::Edit {
                number,
                issue_date,
                due_date,
                clear_due,
                currency,
                notes,
                terms,
                items,
            } => cli::invoice::edit(
                number,
                issue_date,
                due_date,
                clear_due,
                currency,
                notes,
                terms,
                &items,
                &cli::today(),
            ),
            InvoiceCommands::Void { number, yes } => cli::invoice::void(number, yes, &cli::today()),
            InvoiceCommands::Delete { number, yes } => cli::invoice::delete(number, yes),
            InvoiceCommands::List => cli::invoice::list(&cli::today()),
            InvoiceCommands::Show { number } => cli::invoice::show(number, &cli::today()),
            InvoiceCommands::Preview { number, output_dir } => {
                cli::invoice::preview(number, output_dir)
            }
            InvoiceCommands::Send { number, yes } => cli::invoice::send(number, &cli::today(), yes),
            InvoiceCommands::Sync => cli::invoice::sync(&cli::today()),
            InvoiceCommands::Pay {
                number,
                amount,
                date,
                method,
            } => cli::invoice::pay(number, amount, &date, &method, &cli::today()),
            InvoiceCommands::Aging => cli::invoice::aging(&cli::today()),
            InvoiceCommands::Import { db } => cli::invoice::import(&db),
            InvoiceCommands::Template { command } => match command {
                InvoiceTemplateCommands::Export { output, force } => {
                    cli::invoice::template_export(output.as_deref(), force)
                }
                InvoiceTemplateCommands::Path => cli::invoice::template_show_path(),
            },
            InvoiceCommands::Schedule { command } => match command {
                InvoiceScheduleCommands::Add {
                    client,
                    cadence,
                    start,
                    anchor_day,
                    net_days,
                    currency,
                    items,
                    from,
                    notes,
                    terms,
                    autosend,
                } => cli::invoice_schedule::add(
                    client, &cadence, &start, anchor_day, net_days, currency, &items, from, notes,
                    terms, autosend,
                ),
                InvoiceScheduleCommands::List { all } => cli::invoice_schedule::list(all),
                InvoiceScheduleCommands::Show { id } => cli::invoice_schedule::show(id),
                InvoiceScheduleCommands::Edit {
                    id,
                    anchor_day,
                    net_days,
                    clear_net_days,
                    currency,
                    notes,
                    terms,
                    items,
                    autosend,
                    no_autosend,
                } => cli::invoice_schedule::edit(
                    id,
                    anchor_day,
                    net_days,
                    clear_net_days,
                    currency,
                    notes,
                    terms,
                    &items,
                    autosend,
                    no_autosend,
                ),
                InvoiceScheduleCommands::Pause { id } => {
                    cli::invoice_schedule::pause(id, &cli::today())
                }
                InvoiceScheduleCommands::Resume { id } => {
                    cli::invoice_schedule::resume(id, &cli::today())
                }
                InvoiceScheduleCommands::End { id, bill, forgive } => {
                    cli::invoice_schedule::end(id, &cli::today(), bill, forgive)
                }
                InvoiceScheduleCommands::Run => cli::invoice_schedule::run(&cli::today()),
            },
        },
        Commands::Imports { command } => match command {
            ImportsCommands::List => cli::imports::list(),
            ImportsCommands::Rejects { id } => cli::imports::rejects(id),
        },
        Commands::Import {
            file,
            account,
            format,
            dry_run,
            date_col,
            desc_col,
            amount_col,
            date_format,
            save_profile,
        } => cli::import::run(
            &file,
            &account,
            cli::import::ImportOpts {
                format: format.as_deref(),
                dry_run,
                date_col,
                desc_col,
                amount_col,
                date_format: date_format.as_deref(),
                save_profile: save_profile.as_deref(),
            },
        ),
        Commands::Categorize => cli::categorize::run(),
        Commands::Recategorize { args } => cli::recategorize::run(args),
        Commands::Demo => cli::demo::run(),
        Commands::Rules { command } => match command {
            RulesCommands::Add {
                pattern,
                category,
                vendor,
                match_type,
                priority,
            } => cli::rules::add(
                &pattern,
                &category,
                vendor.as_deref(),
                &match_type,
                priority,
            ),
            RulesCommands::List => cli::rules::list(),
            RulesCommands::Update {
                id,
                pattern,
                category,
                vendor,
                match_type,
                priority,
            } => cli::rules::update(id, pattern, category, vendor, match_type, priority),
            RulesCommands::Delete { id } => cli::rules::delete(id),
            RulesCommands::Test {
                pattern,
                match_type,
            } => cli::rules::test(&pattern, &match_type),
        },
        Commands::Review { id } => cli::review::run(id),
        Commands::Report { command } => cli::report::dispatch(command),
        Commands::Browse { command } => match command {
            BrowseCommands::Register {
                month,
                year,
                from_date,
                to_date,
                filters,
            } => cli::browse::register(month, year, from_date, to_date, &filters),
        },
        Commands::Reconcile {
            account,
            month,
            balance,
        } => cli::reconcile::run(&account, &month, balance),
        Commands::Load { path } => cli::load::run(&path),
        Commands::Backup { output } => cli::backup::run(output),
        Commands::Restore { path } => cli::restore::run(&path),
        Commands::Serve { port, no_open } => cli::serve::run(port, no_open),
        Commands::Undo => cli::undo::run(),
        Commands::Update => cli::update::run(),
        Commands::Status => cli::status::run(),
        Commands::Password { command } => match command {
            PasswordCommand::Set => cli::password::run_set(),
            PasswordCommand::Change => cli::password::run_change(),
            PasswordCommand::Remove => cli::password::run_remove(),
        },
        Commands::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut cli::Cli::command(),
                "nigel",
                &mut std::io::stdout(),
            );
            Ok(())
        }
    }
}
