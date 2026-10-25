//! Who may sign in to a shared `nigel serve`: users, their sessions, their
//! passwords and their second factor.
//!
//! Not under `server/`, because the CLI creates and manages users too. The
//! books' own encryption password is `crate::password`, a different thing.
//!
//! Every function that writes or judges a time takes `now` as a parameter;
//! `clock::now_utc()` is the one read, at the caller.

pub mod passwords;
pub mod seal;
pub mod sessions;
pub mod totp;
pub mod users;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

/// Ordered `ReadOnly < Bookkeeper < Admin`. Serialized as the stored word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    ReadOnly,
    Bookkeeper,
    Admin,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::ReadOnly, Role::Bookkeeper, Role::Admin];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::ReadOnly => "read_only",
            Role::Bookkeeper => "bookkeeper",
            Role::Admin => "admin",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read_only" => Some(Role::ReadOnly),
            "bookkeeper" => Some(Role::Bookkeeper),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }

    /// The word a person reads: Admin, Bookkeeper, Read-only.
    pub fn label(self) -> &'static str {
        match self {
            Role::ReadOnly => "Read-only",
            Role::Bookkeeper => "Bookkeeper",
            Role::Admin => "Admin",
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Printed by `nigel user add` for the first user and by `nigel serve` at
/// startup in shared mode, when the books are not encrypted.
pub const NOT_ENCRYPTED_WARNING: &str = "These books are not encrypted. Anyone who can read the \
     file can read the books. Encrypt them with `nigel password set` before you give anyone \
     else access.";

/// The stored form of an instant: UTC RFC 3339 to the second, `Z`-suffixed,
/// so stored values compare correctly as strings.
pub(crate) fn stamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
pub(crate) mod testing {
    use chrono::{DateTime, Utc};
    use rusqlite::Connection;

    pub fn db() -> (tempfile::TempDir, Connection) {
        crate::db::set_db_password_for_tests(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = crate::db::open_connection(&dir.path().join("nigel.db"), None).expect("open");
        crate::db::init_db(&conn).expect("init");
        (dir, conn)
    }

    pub fn at(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .expect("timestamp")
            .with_timezone(&Utc)
    }

    /// A user row written directly, for tests of the layers below `users.rs`.
    /// `must_change_password` is set, as it is for every new user.
    pub fn insert_user(conn: &Connection, email: &str, role: &str) -> i64 {
        conn.execute(
            "INSERT INTO users (uid, email, display_name, password_hash, role,
                                must_change_password, password_changed_at, created_at, updated_at)
             VALUES (lower(hex(randomblob(16))), ?1, 'Pat Example', 'unused', ?2, 1,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![email, role],
        )
        .expect("insert user");
        conn.last_insert_rowid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_order_and_round_trip() {
        assert!(Role::ReadOnly < Role::Bookkeeper && Role::Bookkeeper < Role::Admin);
        for role in Role::ALL {
            assert_eq!(Role::parse(role.as_str()), Some(role));
        }
        assert_eq!(Role::parse("owner"), None);
        assert_eq!(Role::ReadOnly.label(), "Read-only");
    }

    #[test]
    fn a_role_serializes_as_its_stored_word_and_displays_it() {
        assert_eq!(
            serde_json::to_string(&Role::ReadOnly).unwrap(),
            "\"read_only\""
        );
        assert_eq!(serde_json::to_string(&Role::Admin).unwrap(), "\"admin\"");
        let parsed: Role = serde_json::from_str("\"bookkeeper\"").unwrap();
        assert_eq!(parsed, Role::Bookkeeper);
        assert!(serde_json::from_str::<Role>("\"owner\"").is_err());
        assert_eq!(Role::ReadOnly.to_string(), "read_only");
    }

    #[test]
    fn stamps_are_utc_seconds_with_a_z() {
        assert_eq!(
            stamp(testing::at("2026-10-24T09:30:15.987+02:00")),
            "2026-10-24T07:30:15Z"
        );
    }
}
