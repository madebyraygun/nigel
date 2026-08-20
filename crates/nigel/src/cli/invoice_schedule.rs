use comfy_table::{Cell, Table};

use nigel_core::db::get_connection;
use nigel_core::error::{NigelError, Result};
use nigel_core::fmt::money;
use nigel_core::invoicing::clients::get_client;
use nigel_core::invoicing::invoices::{get_invoice_by_number, invoice_shape};
use nigel_core::invoicing::render_html::load_template;
use nigel_core::invoicing::schedules::{
    add_schedule, draft_due_schedules, end_schedule, get_schedule, list_schedules, pause_schedule,
    resume_schedule, run_due_schedules, schedule_items, schedule_runs, update_schedule, Cadence,
    NewSchedule, Schedule, ScheduleRunReport, ScheduleScope, ScheduleUpdate, Senders,
};
use nigel_core::invoicing::wiring::{build_clients, company_profile, contact_email_for_preview};
use nigel_core::settings::{get_data_dir, invoicing_config, invoicing_status};

use crate::cli::invoice::parse_items;

/// The word a listing prints for a schedule's state.
fn state(schedule: &Schedule) -> &'static str {
    if schedule.ended_at.is_some() {
        "ended"
    } else if schedule.paused {
        "paused"
    } else if schedule.autosend {
        "autosend"
    } else {
        "draft"
    }
}

#[allow(clippy::too_many_arguments)]
pub fn add(
    client_id: i64,
    cadence: &str,
    start: &str,
    anchor_day: Option<u32>,
    net_days: Option<i64>,
    currency: Option<String>,
    items: &[String],
    from: Option<i64>,
    notes: Option<String>,
    terms: Option<String>,
    autosend: bool,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;

    // Either explicit items or an invoice's shape, never both — clap refuses the
    // combination, and this is the same duplication core reading the same shape.
    // The shape only fills in what was not typed: `currency` is an `Option` for
    // the reason `net_days` is, since a default here would be indistinguishable
    // from a choice and would quietly overrule one.
    let (items, currency, notes, terms, net_days) = match from {
        Some(number) => {
            let invoice = get_invoice_by_number(&conn, number)?;
            let shape = invoice_shape(&conn, invoice.id)?;
            (
                shape.items,
                currency.unwrap_or(shape.currency),
                notes.or(shape.notes),
                terms.or(shape.terms),
                net_days.or(shape.net_days),
            )
        }
        None => (
            parse_items(items)?,
            currency.unwrap_or_else(|| "USD".to_string()),
            notes,
            terms,
            net_days,
        ),
    };

    let anchor_day = match anchor_day {
        Some(day) => day,
        None => day_of(start)?,
    };

    let id = add_schedule(
        &conn,
        &NewSchedule {
            client_id,
            cadence: Cadence::parse(cadence)?,
            anchor_day,
            start_period: start.to_string(),
            net_days,
            currency,
            notes,
            terms,
            autosend,
            items,
        },
    )?;
    let client = get_client(&conn, client_id)?;
    println!(
        "Created schedule {id} for {}: {cadence} from {start}, {}",
        client.name,
        if autosend { "autosend" } else { "draft" }
    );
    Ok(())
}

/// The day-of-month a `YYYY-MM-DD` names, for the default anchor.
fn day_of(date: &str) -> Result<u32> {
    let day = nigel_core::invoicing::invoices::parse_date(date, "start")?;
    Ok(chrono::Datelike::day(&day))
}

pub fn list(all: bool) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let scope = if all {
        ScheduleScope::All
    } else {
        ScheduleScope::Active
    };
    let mut table = Table::new();
    table.set_header(vec!["ID", "Client", "Cadence", "Next", "Amount", "State"]);
    for schedule in list_schedules(&conn, scope)? {
        let total: f64 = schedule_items(&conn, schedule.id)?
            .iter()
            .map(|i| i.quantity * i.unit_amount)
            .sum();
        let client = get_client(&conn, schedule.client_id)
            .map(|c| c.name)
            .unwrap_or_else(|_| "\u{2014}".to_string());
        table.add_row(vec![
            Cell::new(schedule.id),
            Cell::new(client),
            Cell::new(&schedule.cadence),
            Cell::new(&schedule.next_period),
            Cell::new(money(total)),
            Cell::new(state(&schedule)),
        ]);
    }
    println!("Invoice schedules\n{table}");
    Ok(())
}

pub fn show(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let schedule = get_schedule(&conn, id)?;
    let client = get_client(&conn, schedule.client_id)?;

    println!(
        "Schedule {id}  [{}]  {}",
        state(&schedule),
        schedule.cadence
    );
    println!("Client:    {}", client.name);
    println!("Next:      {}", schedule.next_period);
    println!("Anchor:    day {}", schedule.anchor_day);
    println!(
        "Net days:  {}",
        schedule
            .net_days
            .map(|d| d.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!("Currency:  {}", schedule.currency);
    if let Some(ended) = &schedule.ended_at {
        println!("Ended:     {ended}");
    }

    let items = schedule_items(&conn, id)?;
    let mut table = Table::new();
    table.set_header(vec!["Description", "Qty", "Unit", "Amount"]);
    for item in &items {
        table.add_row(vec![
            Cell::new(&item.description),
            Cell::new(format!("{:.2}", item.quantity)),
            Cell::new(money(item.unit_amount)),
            Cell::new(money(item.quantity * item.unit_amount)),
        ]);
    }
    println!("{table}");

    let runs = schedule_runs(&conn, id)?;
    if runs.is_empty() {
        println!("Nothing generated yet.");
        return Ok(());
    }
    let mut history = Table::new();
    history.set_header(vec!["Period", "Invoice", "Generated"]);
    for run in runs {
        history.add_row(vec![
            Cell::new(run.period),
            Cell::new(format!("#{}", run.number)),
            Cell::new(run.generated_at),
        ]);
    }
    println!("Generated\n{history}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn edit(
    id: i64,
    anchor_day: Option<u32>,
    net_days: Option<i64>,
    clear_net_days: bool,
    currency: Option<String>,
    notes: Option<String>,
    terms: Option<String>,
    items: &[String],
    autosend: bool,
    no_autosend: bool,
) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let net_days = if clear_net_days {
        Some(None)
    } else {
        net_days.map(Some)
    };
    let autosend = match (autosend, no_autosend) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    };
    let items = if items.is_empty() {
        None
    } else {
        Some(parse_items(items)?)
    };
    update_schedule(
        &conn,
        id,
        &ScheduleUpdate {
            anchor_day,
            net_days,
            currency,
            notes: notes.map(Some),
            terms: terms.map(Some),
            autosend,
            items,
        },
    )?;
    println!("Updated schedule {id}. Future invoices use the new figures.");
    Ok(())
}

pub fn pause(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    pause_schedule(&conn, id)?;
    println!("Paused schedule {id}. Nothing is generated until it is resumed.");
    Ok(())
}

pub fn resume(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    resume_schedule(&conn, id)?;
    let schedule = get_schedule(&conn, id)?;
    println!(
        "Resumed schedule {id}. The next run generates from {}.",
        schedule.next_period
    );
    Ok(())
}

pub fn end(id: i64, today: &str) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    end_schedule(&conn, id, today)?;
    println!("Ended schedule {id}. Its invoices and its history are kept.");
    Ok(())
}

/// Generate everything due. The command a cron job or a launchd agent runs.
///
/// Sending is per schedule and only reachable when the full nine-key set is
/// configured; an installation that cannot send still generates every draft and
/// says so. The exit status is non-zero when anything a schedule asked for did
/// not happen, so a scheduled job surfaces it.
pub fn run(today: &str) -> Result<()> {
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;
    let cfg = invoicing_config();

    let report = if invoicing_status(&cfg).send_configured {
        let template = load_template(&data_dir)?;
        let profile = company_profile(&conn);
        let contact_email = contact_email_for_preview(&cfg).0;
        let branding = profile.branding(&template, &contact_email);
        let clients = build_clients(cfg, profile.name())?;
        for warning in clients.warnings() {
            eprintln!("notice: {warning}");
        }
        run_due_schedules(
            &conn,
            today,
            Some(&Senders {
                branding: &branding,
                gateway: clients.stripe(),
                publisher: clients.r2(),
                mailer: clients.mail(),
            }),
        )?
    } else {
        draft_due_schedules(&conn, today)?
    };

    print_report(&report);
    if report.has_failures() {
        return Err(NigelError::Other(
            "Some invoices were not sent. See the lines above.".to_string(),
        ));
    }
    Ok(())
}

fn print_report(report: &ScheduleRunReport) {
    println!("Generated {} invoice(s).", report.generated.len());
    for generated in &report.generated {
        let state = match (&generated.not_sent, generated.sent) {
            (Some(reason), _) => format!("draft — not sent: {reason}"),
            (None, true) => "sent".to_string(),
            (None, false) => "draft".to_string(),
        };
        println!(
            "  #{}  {}  {}  {}  {state}",
            generated.number,
            generated.client_name,
            money(generated.total),
            generated.period
        );
    }
    for failure in &report.failures {
        eprintln!(
            "notice: schedule {} stopped at {}: {}",
            failure.schedule_id, failure.period, failure.message
        );
    }
}
