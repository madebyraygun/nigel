pub use nigel_core::password::*;

use std::path::Path;

use nigel_core::db::{env_password_if_set, get_connection, is_encrypted, set_db_password};
use nigel_core::error::Result;
use nigel_core::settings::get_data_dir;

fn prompt(msg: &str) -> Result<String> {
    rpassword::prompt_password(msg).map_err(|e| nigel_core::error::NigelError::Other(e.to_string()))
}

/// If the database is encrypted, unlock it from `NIGEL_DB_PASSWORD`, falling
/// back to prompting on stdin (up to 3 attempts). Sets the global password on
/// success. Returns an error if the environment holds an unusable password, or
/// after 3 failed prompts.
pub fn prompt_password_if_needed(db_path: &Path) -> Result<()> {
    if !is_encrypted(db_path)? {
        return Ok(());
    }

    if let Some(pw) = env_password_if_set(db_path)? {
        set_db_password(Some(pw));
        return Ok(());
    }

    for attempt in 1..=3 {
        let pw = prompt("Database password: ")?;
        set_db_password(Some(pw));
        // get_connection runs PRAGMAs that read the DB header, so it will fail
        // on a wrong password. Match on the result instead of using ? to avoid
        // short-circuiting the retry loop.
        match get_connection(db_path) {
            Ok(_) => return Ok(()),
            Err(_) => {
                set_db_password(None);
                if attempt < 3 {
                    eprintln!("Wrong password. Try again ({attempt}/3).");
                }
            }
        }
    }
    Err(nigel_core::error::NigelError::Other(
        "Failed to unlock database after 3 attempts.".into(),
    ))
}

fn prompt_and_confirm(msg: &str) -> Result<String> {
    let pw1 = prompt(msg)?;
    let pw2 = prompt("Confirm password: ")?;
    if pw1.trim() != pw2.trim() {
        return Err(nigel_core::error::NigelError::Other(
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

#[cfg(test)]
mod tests {
    use super::*;
    use nigel_core::db::{init_db, open_connection};

    /// The companion `backup_ignores_env_password_on_plain_database` covers the
    /// case this cannot: setting the variable requires a child process, because
    /// the environment is shared across cargo's parallel test threads.
    #[test]
    fn test_plain_db_needs_no_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("plain.db");
        let conn = open_connection(&db_path, None).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        assert!(prompt_password_if_needed(&db_path).is_ok());
    }
}
