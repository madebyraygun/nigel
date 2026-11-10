use std::io::BufRead;

use chrono::NaiveDate;

use crate::cli::backup;
use nigel_core::categorizer::categorize_transactions;
use nigel_core::db::get_connection;
use nigel_core::error::{NigelError, Result};
use nigel_core::fmt::money;
use nigel_core::settings::{get_data_dir, save_settings, try_load_settings};
use nigel_core::simplefin::{self, AccessUrl, AccountsQuery, SyncReport};

/// The stored access URL, or the instruction to make one.
pub fn stored_access() -> Result<AccessUrl> {
    let settings = try_load_settings()?;
    let raw = settings.simplefin_access_url.ok_or_else(|| {
        NigelError::Invalid(
            "SimpleFIN is not set up. Run `nigel sync setup` with a setup token from your bridge."
                .into(),
        )
    })?;
    AccessUrl::parse(&raw)
}

pub fn setup(token: Option<&str>) -> Result<()> {
    let token = match token {
        Some(t) => t.to_string(),
        None => {
            eprintln!("Paste the SimpleFIN setup token, then press Enter:");
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line)?;
            line
        }
    };
    let access = simplefin::claim(&token)?;
    let mut settings = try_load_settings()?;
    let replaced = settings.simplefin_access_url.is_some();
    settings.simplefin_access_url = Some(access.expose().to_string());
    save_settings(&settings)?;
    println!(
        "SimpleFIN is set up{}. Next: `nigel sync accounts` to link accounts.",
        if replaced {
            " (the previous link was replaced)"
        } else {
            ""
        }
    );
    Ok(())
}

pub fn accounts(map: Option<&[String]>, unmap: Option<&str>, format: Option<&str>) -> Result<()> {
    match format {
        None | Some("table") | Some("json") => {}
        Some(other) => {
            return Err(NigelError::Other(format!(
                "Unknown --format '{other}'. Expected 'table' or 'json'."
            )));
        }
    }
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    if format == Some("json") {
        let json = serde_json::to_string_pretty(&simplefin::list_accounts(&conn)?)
            .map_err(|e| NigelError::Other(e.to_string()))?;
        println!("{json}");
        return Ok(());
    }
    match (map, unmap) {
        (Some([key, account]), _) => {
            simplefin::map_account(&conn, key, account)?;
            println!("{key} now syncs into {account}.");
        }
        (_, Some(key)) => {
            simplefin::unmap_account(&conn, key)?;
            println!("{key} is no longer synced.");
        }
        _ => {
            let set = simplefin::fetch_accounts(
                &stored_access()?,
                &AccountsQuery {
                    balances_only: true,
                    ..AccountsQuery::default()
                },
            )?;
            simplefin::remember_accounts(&conn, &set)?;
            for warning in set
                .errors
                .iter()
                .map(|m| simplefin::client::sanitize_message(m))
            {
                eprintln!("SimpleFIN: {warning}");
            }
        }
    }
    let linked = simplefin::list_accounts(&conn)?;
    if linked.is_empty() {
        println!("The bridge returned no accounts.");
        return Ok(());
    }
    for a in &linked {
        let target = a.account.as_deref().unwrap_or("(not synced)");
        println!(
            "{:<40} {} — {} [{}] → {target}",
            a.key, a.org_name, a.name, a.currency
        );
    }
    if linked.iter().any(|a| a.account.is_none()) {
        println!("\nMap one with: nigel sync accounts --map <KEY> <ACCOUNT>");
    }
    Ok(())
}

pub fn run(since: Option<&str>, dry_run: bool) -> Result<()> {
    let since = since
        .map(|d| {
            NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .map_err(|_| NigelError::Invalid(format!("--since {d:?} is not YYYY-MM-DD")))
        })
        .transpose()?;
    let access = stored_access()?;
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;

    let Some(query) = simplefin::query_for(&conn, since)? else {
        println!("No SimpleFIN accounts are linked. Run `nigel sync accounts` to map them.");
        return Ok(());
    };
    let set = simplefin::fetch_accounts(&access, &query)?;

    let preview = simplefin::apply(&conn, &set, since, &crate::cli::today(), true)?;
    if dry_run {
        println!("Dry run \u{2014} no changes made");
        print_report(&preview, true);
        return Ok(());
    }

    // A sync with nothing to import changes only the reported balances and
    // the last-sync day, so an unattended daily run on a quiet day does not
    // leave a snapshot behind.
    if preview.imported() > 0 {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let snap_path = data_dir.join(format!("snapshots/pre-sync-{stamp}.db"));
        backup::snapshot(&conn, &snap_path)?;
        println!("Pre-sync snapshot saved to {}", snap_path.display());
    }

    let tx = conn.unchecked_transaction()?;
    let report = simplefin::apply(&tx, &set, since, &crate::cli::today(), false)?;
    let categorized = if report.imported() > 0 {
        Some(categorize_transactions(&tx)?)
    } else {
        None
    };
    tx.commit()?;

    print_report(&report, false);
    if let Some(counts) = categorized {
        println!(
            "Categorized {}, {} still flagged for review",
            counts.categorized, counts.still_flagged
        );
    }
    Ok(())
}

fn print_report(report: &SyncReport, dry_run: bool) {
    let verb = if dry_run { "would import" } else { "imported" };
    for a in &report.accounts {
        let mut line = format!(
            "{} → {}: {verb} {}, {} already held",
            a.label, a.account, a.imported, a.skipped
        );
        if a.malformed > 0 {
            line.push_str(&format!(", {} unreadable", a.malformed));
        }
        if let Some(b) = &a.balance {
            line.push_str(&format!(
                "; bank balance {} as of {}",
                money(b.balance),
                b.as_of
            ));
        }
        println!("{line}");
    }
    for label in &report.unmapped {
        println!("{label}: not linked, skipped");
    }
    for warning in &report.warnings {
        eprintln!("SimpleFIN: {warning}");
    }
}
