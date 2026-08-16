use nigel_core::categorizer::categorize_transactions;
use nigel_core::db::get_connection;
use nigel_core::error::Result;
use nigel_core::settings::get_data_dir;

pub fn run() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let result = categorize_transactions(&conn)?;
    println!(
        "{} categorized, {} still flagged",
        result.categorized, result.still_flagged
    );
    Ok(())
}
