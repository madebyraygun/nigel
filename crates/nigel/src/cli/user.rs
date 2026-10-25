//! `nigel user` — a printing wrapper over `nigel_core::identity::users`.
//!
//! The CLI has full authority over the books it opens: it is how the first
//! admin is created and how a locked-out admin recovers.

use std::path::PathBuf;

use comfy_table::{Cell, Table};

use nigel_core::clock::now_utc;
use nigel_core::db::{get_connection, is_encrypted};
use nigel_core::error::Result;
use nigel_core::identity::users::{self, NewUser, User, UserListing};
use nigel_core::identity::Role;
use nigel_core::settings::get_data_dir;

pub const NOT_ENCRYPTED_WARNING: &str = "These books are not encrypted. Anyone who can read the \
     file can read the books. Encrypt them with `nigel password set` before you give anyone \
     else access.";

/// The role as typed on the command line. `read-only` is accepted beside the
/// stored spelling `read_only`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum RoleArg {
    Admin,
    Bookkeeper,
    #[value(name = "read_only", alias = "read-only")]
    ReadOnly,
}

impl From<RoleArg> for Role {
    fn from(arg: RoleArg) -> Self {
        match arg {
            RoleArg::Admin => Role::Admin,
            RoleArg::Bookkeeper => Role::Bookkeeper,
            RoleArg::ReadOnly => Role::ReadOnly,
        }
    }
}

fn db_path() -> PathBuf {
    get_data_dir().join("nigel.db")
}

pub(crate) fn temporary_password_lines(user: &User, temporary: &str) -> String {
    format!(
        "Temporary password: {temporary}\n\
         Give this to {}. They will choose their own password when they first sign in.\n",
        user.display_name
    )
}

pub(crate) fn added_message(user: &User, temporary: &str) -> String {
    format!(
        "Added {} <{}> ({}).\n{}",
        user.display_name,
        user.email,
        user.role.label(),
        temporary_password_lines(user, temporary)
    )
}

fn status(user: &User) -> &'static str {
    if !user.is_enabled() {
        "disabled"
    } else if user.must_change_password {
        "must change password"
    } else {
        "active"
    }
}

pub(crate) fn list_table(rows: &[UserListing]) -> String {
    if rows.is_empty() {
        return "No users. Add the first with `nigel user add <email> --name <name> --admin`."
            .to_string();
    }
    let mut table = Table::new();
    table.set_header(vec![
        "Email",
        "Name",
        "Role",
        "Status",
        "Two-step",
        "Last seen",
    ]);
    for row in rows {
        let user = &row.user;
        table.add_row(vec![
            Cell::new(&user.email),
            Cell::new(&user.display_name),
            Cell::new(user.role.label()),
            Cell::new(status(user)),
            Cell::new(if user.totp_enabled { "on" } else { "off" }),
            Cell::new(row.last_seen_at.as_deref().unwrap_or("never")),
        ]);
    }
    table.to_string()
}

pub fn add(email: &str, name: &str, role: Role) -> Result<()> {
    let path = db_path();
    let conn = get_connection(&path)?;
    let created = users::create_user(
        &conn,
        &NewUser {
            email,
            display_name: name,
            role,
        },
        now_utc(),
    )?;
    print!(
        "{}",
        added_message(&created.user, &created.temporary_password)
    );
    if created.first_user && !is_encrypted(&path)? {
        eprintln!("{NOT_ENCRYPTED_WARNING}");
    }
    Ok(())
}

pub fn list() -> Result<()> {
    let conn = get_connection(&db_path())?;
    println!("{}", list_table(&users::list_users(&conn, now_utc())?));
    Ok(())
}

pub fn set_role(email: &str, role: Role) -> Result<()> {
    let conn = get_connection(&db_path())?;
    let user = users::require_by_email(&conn, email)?;
    let user = users::set_role(&conn, user.id, role, now_utc())?;
    println!("{} is now {}.", user.email, user.role.label());
    Ok(())
}

pub fn disable(email: &str) -> Result<()> {
    let conn = get_connection(&db_path())?;
    let user = users::require_by_email(&conn, email)?;
    if !user.is_enabled() {
        println!("{} was already disabled.", user.email);
        return Ok(());
    }
    let user = users::disable_user(&conn, user.id, now_utc())?;
    println!("Disabled {} and ended their sessions.", user.email);
    Ok(())
}

pub fn enable(email: &str) -> Result<()> {
    let conn = get_connection(&db_path())?;
    let user = users::require_by_email(&conn, email)?;
    if user.is_enabled() {
        println!("{} is already enabled.", user.email);
        return Ok(());
    }
    let user = users::enable_user(&conn, user.id, now_utc())?;
    println!("Enabled {}.", user.email);
    Ok(())
}

pub fn reset_password(email: &str, clear_totp: bool) -> Result<()> {
    let conn = get_connection(&db_path())?;
    let user = users::require_by_email(&conn, email)?;
    let temporary = users::reset_password(&conn, user.id, clear_totp, now_utc())?;
    print!("{}", temporary_password_lines(&user, &temporary));
    if clear_totp {
        println!("Two-step verification is off for {}.", user.email);
    }
    println!("Every session {} had has ended.", user.email);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: Role, must_change: bool, disabled: bool, totp: bool) -> User {
        User {
            id: 1,
            uid: "0".repeat(32),
            email: "pat@example.com".into(),
            display_name: "Pat Example".into(),
            role,
            totp_enabled: totp,
            must_change_password: must_change,
            password_changed_at: "2026-01-01T00:00:00Z".into(),
            disabled_at: disabled.then(|| "2026-01-02T00:00:00Z".into()),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn the_added_message_names_the_user_and_says_the_password_once() {
        let message = added_message(
            &user(Role::Admin, true, false, false),
            "K7QXM-2RTPA-W9HCN-4FJDE",
        );
        assert_eq!(
            message,
            "Added Pat Example <pat@example.com> (Admin).\n\
             Temporary password: K7QXM-2RTPA-W9HCN-4FJDE\n\
             Give this to Pat Example. They will choose their own password when they first sign in.\n"
        );
    }

    #[test]
    fn the_list_shows_status_two_step_and_last_seen() {
        let rows = vec![
            UserListing {
                user: user(Role::Admin, false, false, true),
                last_seen_at: Some("2026-01-05T10:00:00Z".into()),
                active_sessions: 1,
            },
            UserListing {
                user: User {
                    email: "sam@example.com".into(),
                    ..user(Role::ReadOnly, true, false, false)
                },
                last_seen_at: None,
                active_sessions: 0,
            },
            UserListing {
                user: User {
                    email: "lee@example.com".into(),
                    ..user(Role::Bookkeeper, false, true, false)
                },
                last_seen_at: None,
                active_sessions: 0,
            },
        ];
        let table = list_table(&rows);
        for expected in [
            "pat@example.com",
            "Admin",
            "active",
            "on",
            "2026-01-05T10:00:00Z",
            "Read-only",
            "must change password",
            "never",
            "disabled",
        ] {
            assert!(table.contains(expected), "{expected}\n{table}");
        }
    }

    #[test]
    fn an_empty_list_says_how_to_add_the_first_admin() {
        assert!(list_table(&[]).contains("--admin"));
    }

    #[test]
    fn both_spellings_of_read_only_parse() {
        use clap::ValueEnum;
        assert_eq!(RoleArg::from_str("read_only", false), Ok(RoleArg::ReadOnly));
        assert_eq!(RoleArg::from_str("read-only", false), Ok(RoleArg::ReadOnly));
        assert_eq!(Role::from(RoleArg::ReadOnly), Role::ReadOnly);
    }
}
