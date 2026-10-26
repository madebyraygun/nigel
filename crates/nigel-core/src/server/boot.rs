//! What `nigel serve` does before it binds: unlock the books from the
//! environment, read what the startup notices need, and say how it is
//! serving.

use std::ffi::OsString;
use std::path::Path;

use chrono::{DateTime, Utc};

use super::config::ServerConfig;
use super::mode::Mode;
use crate::error::Result;

/// Unlock encrypted books with a password from `NIGEL_DB_PASSWORD_FILE` or
/// `NIGEL_DB_PASSWORD` (`file` and `raw`), then leave it in the process-wide
/// slot. `Ok(false)` when the books are not encrypted or neither variable is
/// set; a wrong or unreadable password is an error, and fatal at startup.
pub fn unlock_with(db_path: &Path, raw: Option<OsString>, file: Option<OsString>) -> Result<bool> {
    if !crate::db::is_encrypted(db_path)? {
        return Ok(false);
    }
    match crate::db::env_password_from(db_path, raw, file)? {
        Some(password) => {
            crate::db::set_db_password(Some(zeroize::Zeroizing::new(password)));
            Ok(true)
        }
        None => Ok(false),
    }
}

/// [`unlock_with`] from the process environment.
pub fn unlock_from_env(db_path: &Path) -> Result<bool> {
    unlock_with(
        db_path,
        std::env::var_os("NIGEL_DB_PASSWORD"),
        std::env::var_os("NIGEL_DB_PASSWORD_FILE"),
    )
}

/// What the banner and notices need from the books.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BootFacts {
    pub encrypted: bool,
    pub company: Option<String>,
    /// Users whose two-step this machine cannot check (shared mode only).
    pub totp_unopenable: Vec<String>,
    /// Why server.key could not be read, with its path (shared mode only).
    pub key_unreadable: Option<String>,
}

/// Read the facts, and in shared mode sweep expired sessions.
pub fn read_facts(db_path: &Path, mode: Mode, now: DateTime<Utc>) -> Result<BootFacts> {
    if !super::routes::status::initialized(db_path) {
        return Ok(BootFacts::default());
    }
    let encrypted = crate::db::is_encrypted(db_path)?;
    if encrypted && crate::db::get_db_password().is_none() {
        return Ok(BootFacts {
            encrypted,
            ..BootFacts::default()
        });
    }
    let conn = crate::db::get_connection(db_path)?;
    let company = crate::db::get_metadata(&conn, "company_name").filter(|c| !c.trim().is_empty());
    let mut totp_unopenable = Vec::new();
    let mut key_unreadable = None;
    if mode == Mode::Shared {
        crate::identity::sessions::sweep_expired(&conn, now)?;
        let path = crate::identity::seal::key_path();
        match crate::identity::seal::ServerKey::load(&path) {
            Ok(key) => {
                totp_unopenable = crate::identity::users::totp_unopenable(&conn, key.as_ref())?;
            }
            Err(e) => key_unreadable = Some(format!("{}: {e}", path.display())),
        }
    }
    Ok(BootFacts {
        encrypted,
        company,
        totp_unopenable,
        key_unreadable,
    })
}

/// The notices `nigel serve` prints to stderr before it binds.
pub fn startup_notices(
    mode: Mode,
    facts: &BootFacts,
    config: &ServerConfig,
    key_warning: Option<String>,
) -> Vec<String> {
    let mut notices = Vec::new();
    match mode {
        Mode::Local => {
            if config.public_url.is_some() {
                notices.push(
                    "notice: server_public_url is set, but these books have no users (or are \
                     still locked), so Nigel accepts loopback requests only. Add the first user \
                     with `nigel user add` to share them."
                        .to_string(),
                );
            }
        }
        Mode::Shared => {
            if !facts.encrypted {
                notices.push(crate::identity::NOT_ENCRYPTED_WARNING.to_string());
            }
            if config.public_url.is_some() && config.client_ip_header.is_none() {
                notices.push(
                    "notice: server_public_url is set and client_ip_header is not, so Nigel \
                     sees every request through your proxy or tunnel as coming from one \
                     address, and all of them share one sign-in rate limit. Set \
                     client_ip_header to \"CF-Connecting-IP\" or \"X-Forwarded-For\"."
                        .to_string(),
                );
            }
            if let Some(reason) = &facts.key_unreadable {
                notices.push(format!(
                    "warning: two-step verification can't be checked until Nigel can read its \
                     server key ({reason}). Fix the file's ownership or permissions and restart."
                ));
            }
            if !facts.totp_unopenable.is_empty() {
                notices.push(format!(
                    "warning: two-step verification can't be checked for {} — this machine's \
                     server.key is missing or is not the one their secrets were sealed with. \
                     Recover each with `nigel user reset-password <email> --clear-totp`.",
                    facts.totp_unopenable.join(", ")
                ));
            }
        }
    }
    notices.extend(key_warning);
    notices
}

/// What `nigel serve` prints to stdout once it is listening.
pub fn serving_banner(
    mode: Mode,
    company: Option<&str>,
    config: &ServerConfig,
    port: u16,
    token: Option<&str>,
) -> String {
    let loopback = format!("http://127.0.0.1:{port}");
    match (mode, token) {
        (Mode::Local, Some(token)) => format!(
            "Nigel is serving on {loopback}\nOpen this URL to start a session:\n  {loopback}/auth?token={token}\nPress Ctrl-C to stop."
        ),
        _ => {
            let books = company.unwrap_or("these books");
            let first = match &config.public_url {
                Some(public) => format!(
                    "Nigel is serving {books} to signed-in users at {} (and {loopback}).",
                    public.origin()
                ),
                None => format!("Nigel is serving {books} to signed-in users at {loopback}."),
            };
            format!("{first}\nPress Ctrl-C to stop.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::config::{ClientIpHeader, PublicUrl, ServerConfig};
    use crate::server::testutil::{encrypt, temp_db, TempConfig, PASSWORD};

    /// Clears the process-global password when the test ends, failed
    /// assertion or not.
    struct ClearPassword;

    impl Drop for ClearPassword {
        fn drop(&mut self) {
            crate::db::set_db_password_for_tests(None);
        }
    }

    fn public() -> ServerConfig {
        ServerConfig {
            public_url: Some(PublicUrl::parse("https://books.example.com").unwrap()),
            client_ip_header: None,
        }
    }

    #[test]
    fn a_password_in_the_environment_unlocks_encrypted_books_at_boot() {
        let _clear = ClearPassword;
        let (_dir, db_path) = temp_db();
        encrypt(&db_path);
        crate::db::set_db_password_for_tests(None);
        assert!(
            !unlock_with(&db_path, None, None).unwrap(),
            "nothing set: stays locked"
        );
        assert!(crate::db::get_db_password().is_none());

        assert!(unlock_with(&db_path, Some(PASSWORD.into()), None).unwrap());
        assert!(crate::db::get_db_password().is_some());
        crate::db::set_db_password_for_tests(None);

        let err = unlock_with(&db_path, Some("pass-x7q-not-it".into()), None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("did not unlock"), "{err}");
        assert!(err.contains("NIGEL_DB_PASSWORD"), "{err}");
        assert!(!err.contains("x7q"), "the password leaked: {err}");
        assert!(crate::db::get_db_password().is_none());
    }

    #[test]
    fn a_password_in_the_environment_leaves_plain_books_alone() {
        let _clear = ClearPassword;
        let (_dir, db_path) = temp_db();
        assert!(!unlock_with(&db_path, Some("anything".into()), None).unwrap());
        assert!(crate::db::get_db_password().is_none());
    }

    #[test]
    fn local_mode_with_a_public_url_says_why_it_is_ignored() {
        let notices = startup_notices(Mode::Local, &BootFacts::default(), &public(), None);
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(notices[0].contains("server_public_url"), "{notices:?}");
        assert!(notices[0].contains("nigel user add"), "{notices:?}");
        assert!(startup_notices(
            Mode::Local,
            &BootFacts::default(),
            &ServerConfig::default(),
            None
        )
        .is_empty());
    }

    #[test]
    fn shared_mode_warns_about_plain_books_a_missing_ip_header_and_unopenable_two_step() {
        let facts = BootFacts {
            encrypted: false,
            company: Some("Acme".into()),
            totp_unopenable: vec!["sam@example.com".into()],
            key_unreadable: None,
        };
        let notices = startup_notices(Mode::Shared, &facts, &public(), Some("warning: key".into()));
        let all = notices.join("\n");
        assert!(
            all.contains(crate::identity::NOT_ENCRYPTED_WARNING),
            "{all}"
        );
        assert!(all.contains("client_ip_header"), "{all}");
        assert!(all.contains("one sign-in rate limit"), "{all}");
        assert!(
            all.contains("sam@example.com") && all.contains("--clear-totp"),
            "{all}"
        );
        assert!(all.contains("warning: key"), "{all}");

        let quiet = startup_notices(
            Mode::Shared,
            &BootFacts {
                encrypted: true,
                ..BootFacts::default()
            },
            &ServerConfig {
                client_ip_header: Some(ClientIpHeader::CfConnectingIp),
                ..public()
            },
            None,
        );
        assert!(quiet.is_empty(), "{quiet:?}");
    }

    #[test]
    fn the_banner_prints_a_token_only_in_local_mode() {
        let local = serving_banner(
            Mode::Local,
            None,
            &ServerConfig::default(),
            5731,
            Some("ab12"),
        );
        assert!(
            local.contains("http://127.0.0.1:5731/auth?token=ab12"),
            "{local}"
        );

        let shared = serving_banner(Mode::Shared, Some("Acme"), &public(), 5731, None);
        assert_eq!(
            shared.lines().next().unwrap(),
            "Nigel is serving Acme to signed-in users at https://books.example.com (and http://127.0.0.1:5731)."
        );
        assert!(!shared.contains("token"), "{shared}");

        let loopback_only =
            serving_banner(Mode::Shared, None, &ServerConfig::default(), 5731, None);
        assert_eq!(
            loopback_only.lines().next().unwrap(),
            "Nigel is serving these books to signed-in users at http://127.0.0.1:5731."
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_server_key_is_named_and_two_step_users_are_not_told_to_clear_it() {
        use std::os::unix::fs::PermissionsExt;
        let _config = TempConfig::new();
        let (_dir, db_path) = temp_db();
        let user = crate::server::testutil::add_ready_user(
            &db_path,
            "pat@example.com",
            crate::identity::Role::Admin,
        );
        let path = crate::identity::seal::key_path();
        let key = crate::identity::seal::ServerKey::load_or_create(&path).unwrap();
        let now = crate::clock::now_utc();
        let conn = crate::db::get_connection(&db_path).unwrap();
        crate::identity::users::set_totp_secret(&conn, user.id, &key, &[7u8; 20], 0, now).unwrap();

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::File::open(&path).is_ok() {
            eprintln!("skipped: running as a user that can read a mode-000 file");
            return;
        }
        let facts = read_facts(&db_path, Mode::Shared, now).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        assert!(facts.totp_unopenable.is_empty(), "{facts:?}");
        let notices = startup_notices(
            Mode::Shared,
            &BootFacts {
                encrypted: true,
                ..facts
            },
            &ServerConfig::default(),
            None,
        );
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(
            notices[0].contains(&path.display().to_string()),
            "{notices:?}"
        );
        assert!(!notices[0].contains("--clear-totp"), "{notices:?}");
    }

    #[test]
    fn boot_facts_sweep_expired_sessions_and_name_the_company() {
        let _config = TempConfig::new();
        let (_dir, db_path) = temp_db();
        let conn = crate::db::get_connection(&db_path).unwrap();
        crate::db::set_metadata(&conn, "company_name", "Acme").unwrap();
        let user = crate::server::testutil::add_ready_user(
            &db_path,
            "pat@example.com",
            crate::identity::Role::Admin,
        );
        let long_ago = chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        crate::server::testutil::session_for(
            &db_path,
            user.id,
            crate::identity::sessions::SessionKind::Cookie,
            long_ago,
        );

        let facts = read_facts(&db_path, Mode::Shared, crate::clock::now_utc()).unwrap();
        assert_eq!(facts.company.as_deref(), Some("Acme"));
        assert!(!facts.encrypted);
        assert!(facts.totp_unopenable.is_empty(), "{facts:?}");
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0, "the expired session was swept");
    }
}
