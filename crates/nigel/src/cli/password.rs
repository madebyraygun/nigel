pub use crate::password::*;

use crate::db::{is_encrypted, set_db_password};
use crate::error::Result;
use crate::settings::get_data_dir;

fn prompt(msg: &str) -> Result<String> {
    rpassword::prompt_password(msg).map_err(|e| crate::error::NigelError::Other(e.to_string()))
}

fn prompt_and_confirm(msg: &str) -> Result<String> {
    let pw1 = prompt(msg)?;
    let pw2 = prompt("Confirm password: ")?;
    if pw1.trim() != pw2.trim() {
        return Err(crate::error::NigelError::Other(
            "Passwords do not match.".into(),
        ));
    }
    let trimmed = pw1.trim();
    if trimmed.len() != pw1.len() {
        eprintln!("Note: leading/trailing spaces were removed from password.");
    }
    Ok(trimmed.to_string())
}

pub fn run_set() -> Result<()> {
    let db_path = get_data_dir().join("nigel.db");
    if is_encrypted(&db_path)? {
        eprintln!("Database is already encrypted. Use `nigel password change` instead.");
        return Ok(());
    }
    let new_pw = prompt_and_confirm("New password: ")?;
    if new_pw.is_empty() {
        eprintln!("Password cannot be empty.");
        return Ok(());
    }
    encrypt_database(&db_path, &new_pw)?;
    set_db_password(Some(new_pw));
    println!("Database encrypted successfully.");
    Ok(())
}

pub fn run_change() -> Result<()> {
    let db_path = get_data_dir().join("nigel.db");
    if !is_encrypted(&db_path)? {
        eprintln!("Database is not encrypted. Use `nigel password set` instead.");
        return Ok(());
    }
    let current_pw = prompt("Current password: ")?;
    let new_pw = prompt_and_confirm("New password: ")?;
    if new_pw.is_empty() {
        eprintln!("New password cannot be empty. Use `nigel password remove` to decrypt.");
        return Ok(());
    }
    rekey_database(&db_path, current_pw.trim(), &new_pw)?;
    set_db_password(Some(new_pw));
    println!("Password changed successfully.");
    Ok(())
}

pub fn run_remove() -> Result<()> {
    let db_path = get_data_dir().join("nigel.db");
    if !is_encrypted(&db_path)? {
        eprintln!("Database is not encrypted.");
        return Ok(());
    }
    let current_pw = prompt("Current password: ")?;
    decrypt_database(&db_path, current_pw.trim())?;
    set_db_password(None);
    println!("Database decrypted successfully. Password removed.");
    Ok(())
}
