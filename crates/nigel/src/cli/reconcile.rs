use nigel_core::db::get_connection;
use nigel_core::error::Result;
use nigel_core::fmt::money;
use nigel_core::reconciler;
use nigel_core::settings::get_data_dir;

pub fn run(account: &str, month: &str, balance: f64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let result = reconciler::reconcile(&conn, account, month, balance)?;

    if result.is_reconciled {
        println!(
            "Reconciled! Calculated: {}",
            money(result.calculated_balance)
        );
    } else {
        println!(
            "DISCREPANCY: {}\n  Statement:  {}\n  Calculated: {}",
            money(result.discrepancy),
            money(result.statement_balance),
            money(result.calculated_balance)
        );
    }
    Ok(())
}
