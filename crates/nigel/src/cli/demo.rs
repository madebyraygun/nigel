//! `nigel demo` — the CLI's front door to `nigel_core::demo`.

use std::path::PathBuf;

use nigel_core::db::{get_connection, init_db};
use nigel_core::demo::{demo_summary, seed_demo, ACCOUNT_NAME};
use nigel_core::error::Result;
use nigel_core::settings::load_settings;

pub fn run() -> Result<()> {
    let settings = load_settings();
    let db_path = PathBuf::from(&settings.data_dir).join("nigel.db");

    if !db_path.exists() {
        eprintln!("No database found. Run `nigel init` first.");
        std::process::exit(1);
    }

    let conn = get_connection(&db_path)?;
    init_db(&conn)?;

    // Demo data is business books: its rules name business categories, so on
    // a personal chart the inserts would fail partway through the category
    // lookups, leaving transactions with no import row for `nigel undo`.
    if nigel_core::db::get_profile(&conn) == nigel_core::db::Profile::Personal {
        eprintln!("These books are personal, and the demo data is a business (its rules");
        eprintln!("name business categories). Try it in its own directory instead:");
        eprintln!("  nigel init --data-dir ~/nigel-demo && nigel demo");
        std::process::exit(1);
    }

    if !seed_demo(&conn)? {
        println!(
            "Demo data already loaded (account '{}' exists).",
            ACCOUNT_NAME
        );
        return Ok(());
    }
    let summary = demo_summary(&conn)?;

    println!("Demo data loaded!");
    println!("  Account:      {ACCOUNT_NAME}");
    println!("  Transactions: {}", summary.transactions);
    println!("  Rules:        {}", summary.rules);
    println!("  Categorized:  {}", summary.categorized);
    println!("  Flagged:      {}", summary.flagged);
    println!("  Clients:      {}", summary.clients);
    println!("  Invoices:     {}", summary.invoices);
    println!();
    println!("Try these next:");
    println!("  nigel accounts list");
    println!("  nigel rules list");
    println!("  nigel report pnl");
    println!("  nigel report flagged");
    println!("  nigel review");
    println!("  nigel invoice list");

    Ok(())
}

/// Create a demo data directory and switch settings to point at it, so the
/// user's real books stay clean.
pub fn setup_demo() -> Result<()> {
    nigel_core::demo::setup_demo_dir()?;
    Ok(())
}
