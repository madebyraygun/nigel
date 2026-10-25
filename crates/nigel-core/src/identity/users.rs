//! Users: created, listed, re-roled, disabled and enabled, never deleted.
//!
//! Invariant: books that have users always have at least one enabled admin.
//! The first user must be an admin, and the last enabled admin cannot be
//! disabled or demoted. Every write that could break it checks inside one
//! `BEGIN IMMEDIATE` transaction, and the CLI and the API both come here.

use std::fmt;

use chrono::{DateTime, Utc};
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use super::passwords::{
    check_new_password, hash_password, temporary_password, verify_dummy, verify_password,
    PasswordOwner, PasswordRule,
};
use super::seal::ServerKey;
use super::{sessions, stamp, totp, Role};
use crate::error::{NigelError, Result};

pub const DISPLAY_NAME_MAX_CHARS: usize = 100;

/// A user as every caller sees one. No hash and no secret: those never leave
/// this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub uid: String,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub totp_enabled: bool,
    pub must_change_password: bool,
    pub password_changed_at: String,
    pub disabled_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl User {
    pub fn is_enabled(&self) -> bool {
        self.disabled_at.is_none()
    }
}

/// A row of `nigel user list` and `GET /api/users`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserListing {
    pub user: User,
    pub last_seen_at: Option<String>,
    pub active_sessions: i64,
}

pub struct NewUser<'a> {
    pub email: &'a str,
    pub display_name: &'a str,
    pub role: Role,
}

/// A created user and the temporary password to hand them, once.
pub struct CreatedUser {
    pub user: User,
    pub temporary_password: Zeroizing<String>,
    /// Whether this was the books' first user.
    pub first_user: bool,
}

/// What a sign-in attempt proved. A wrong email, a wrong password and a
/// disabled user are all `Invalid`, and all cost one Argon2id verification.
#[derive(Debug)]
pub enum Credentials {
    Valid(User),
    Invalid,
}

/// Why a password change was refused.
#[derive(Debug)]
pub enum PasswordChangeError {
    WrongCurrentPassword,
    Rule(PasswordRule),
    Failed(NigelError),
}

impl fmt::Display for PasswordChangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PasswordChangeError::WrongCurrentPassword => {
                write!(f, "The current password is not right.")
            }
            PasswordChangeError::Rule(rule) => write!(f, "{rule}"),
            PasswordChangeError::Failed(e) => write!(f, "{e}"),
        }
    }
}

impl From<NigelError> for PasswordChangeError {
    fn from(e: NigelError) -> Self {
        PasswordChangeError::Failed(e)
    }
}

impl From<rusqlite::Error> for PasswordChangeError {
    fn from(e: rusqlite::Error) -> Self {
        PasswordChangeError::Failed(e.into())
    }
}

impl From<PasswordChangeError> for NigelError {
    fn from(e: PasswordChangeError) -> Self {
        match e {
            PasswordChangeError::Failed(inner) => inner,
            other => NigelError::Invalid(other.to_string()),
        }
    }
}

const USER_COLUMNS: &str = "id, uid, email, display_name, role, totp_secret IS NOT NULL, \
     must_change_password, password_changed_at, disabled_at, created_at, updated_at";

fn user_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    let role: String = r.get(4)?;
    let role = Role::parse(&role).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            format!("unknown role {role:?}").into(),
        )
    })?;
    Ok(User {
        id: r.get(0)?,
        uid: r.get(1)?,
        email: r.get(2)?,
        display_name: r.get(3)?,
        role,
        totp_enabled: r.get(5)?,
        must_change_password: r.get(6)?,
        password_changed_at: r.get(7)?,
        disabled_at: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
    })
}

/// Trimmed, lower-cased and NFC-normalized, with one `@`, a non-empty local
/// part and domain, and no whitespace or control characters.
pub fn normalize_email(raw: &str) -> Result<String> {
    let email: String = raw.trim().to_lowercase().nfc().collect();
    let mut parts = email.split('@');
    let (local, domain) = (parts.next().unwrap_or(""), parts.next());
    let well_formed = matches!(domain, Some(d) if !d.is_empty())
        && parts.next().is_none()
        && !local.is_empty()
        && !email.chars().any(|c| c.is_whitespace() || c.is_control());
    if well_formed {
        Ok(email)
    } else {
        Err(NigelError::Invalid(format!(
            "{:?} is not an email address.",
            raw.trim()
        )))
    }
}

/// Trimmed and NFC-normalized, so the stored name compares equal to a
/// password typed from either composition.
fn clean_display_name(raw: &str) -> Result<String> {
    let name: String = raw.trim().nfc().collect();
    if name.is_empty() {
        return Err(NigelError::Invalid("A user needs a name.".into()));
    }
    if name.chars().count() > DISPLAY_NAME_MAX_CHARS {
        return Err(NigelError::Invalid(format!(
            "Use at most {DISPLAY_NAME_MAX_CHARS} characters for a name."
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(NigelError::Invalid(
            "A name cannot contain control characters.".into(),
        ));
    }
    Ok(name)
}

fn new_uid() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn immediate(conn: &Connection) -> Result<Transaction<'_>> {
    Ok(Transaction::new_unchecked(
        conn,
        TransactionBehavior::Immediate,
    )?)
}

fn last_admin(message: &str) -> NigelError {
    NigelError::Conflict {
        code: "last_admin",
        message: message.to_string(),
    }
}

fn not_found(id: i64) -> NigelError {
    NigelError::NotFound(format!("User not found: id {id}"))
}

fn enabled_admins_other_than(conn: &Connection, id: i64) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM users
          WHERE role = 'admin' AND disabled_at IS NULL AND id != ?1",
        [id],
        |r| r.get(0),
    )?)
}

/// Whether the books have any user at all — what decides local or shared mode.
pub fn has_users(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM users)", [], |r| r.get(0))?)
}

pub fn get_user(conn: &Connection, id: i64) -> Result<Option<User>> {
    Ok(conn
        .query_row(
            &format!("SELECT {USER_COLUMNS} FROM users WHERE id = ?1"),
            [id],
            user_from_row,
        )
        .optional()?)
}

pub fn find_by_email(conn: &Connection, email: &str) -> Result<Option<User>> {
    let email = normalize_email(email)?;
    Ok(conn
        .query_row(
            &format!("SELECT {USER_COLUMNS} FROM users WHERE email = ?1"),
            [email],
            user_from_row,
        )
        .optional()?)
}

/// [`find_by_email`], with an unknown address as an error naming it.
pub fn require_by_email(conn: &Connection, email: &str) -> Result<User> {
    find_by_email(conn, email)?.ok_or_else(|| {
        NigelError::NotFound(format!(
            "No user has the email {:?}. Run `nigel user list` to see who does.",
            email.trim()
        ))
    })
}

/// Every user, oldest first, with when each was last seen and how many
/// unexpired sessions each holds at `now`.
pub fn list_users(conn: &Connection, now: DateTime<Utc>) -> Result<Vec<UserListing>> {
    let sql = format!(
        "SELECT {USER_COLUMNS},
                (SELECT MAX(last_seen_at) FROM sessions s WHERE s.user_id = users.id),
                (SELECT COUNT(*) FROM sessions s
                  WHERE s.user_id = users.id AND s.expires_at > ?1)
           FROM users ORDER BY id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([stamp(now)], |r| {
        Ok(UserListing {
            user: user_from_row(r)?,
            last_seen_at: r.get(11)?,
            active_sessions: r.get(12)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Create a user with a temporary password they must change at first sign-in.
pub fn create_user(
    conn: &Connection,
    new: &NewUser<'_>,
    now: DateTime<Utc>,
) -> Result<CreatedUser> {
    let email = normalize_email(new.email)?;
    let display_name = clean_display_name(new.display_name)?;
    let temporary = temporary_password();
    let hash = hash_password(&temporary)?;

    let tx = immediate(conn)?;
    let first_user = !has_users(&tx)?;
    if first_user && new.role != Role::Admin {
        return Err(last_admin(
            "The first user must be an admin. Add them with --admin.",
        ));
    }
    let taken: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = ?1)",
        [&email],
        |r| r.get(0),
    )?;
    if taken {
        return Err(NigelError::Conflict {
            code: "duplicate_email",
            message: format!("A user with the email {email} already exists."),
        });
    }
    let ts = stamp(now);
    tx.execute(
        "INSERT INTO users (uid, email, display_name, password_hash, role,
                            must_change_password, password_changed_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6, ?6)",
        params![new_uid(), email, display_name, hash, new.role.as_str(), ts],
    )?;
    let id = tx.last_insert_rowid();
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    tx.commit()?;
    Ok(CreatedUser {
        user,
        temporary_password: temporary,
        first_user,
    })
}

pub fn set_role(conn: &Connection, id: i64, role: Role, now: DateTime<Utc>) -> Result<User> {
    let tx = immediate(conn)?;
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    if user.role == Role::Admin
        && role != Role::Admin
        && user.is_enabled()
        && enabled_admins_other_than(&tx, id)? == 0
    {
        return Err(last_admin(&format!(
            "{} is the only enabled admin. Make someone else an admin first.",
            user.email
        )));
    }
    tx.execute(
        "UPDATE users SET role = ?1, updated_at = ?2 WHERE id = ?3",
        params![role.as_str(), stamp(now), id],
    )?;
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    tx.commit()?;
    Ok(user)
}

/// Disable a user and end every session they hold. Disabling a disabled user
/// changes nothing and keeps the original `disabled_at`.
pub fn disable_user(conn: &Connection, id: i64, now: DateTime<Utc>) -> Result<User> {
    let tx = immediate(conn)?;
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    if !user.is_enabled() {
        return Ok(user);
    }
    if user.role == Role::Admin && enabled_admins_other_than(&tx, id)? == 0 {
        return Err(last_admin(&format!(
            "{} is the only enabled admin and cannot be disabled.",
            user.email
        )));
    }
    let ts = stamp(now);
    tx.execute(
        "UPDATE users SET disabled_at = ?1, updated_at = ?1 WHERE id = ?2",
        params![ts, id],
    )?;
    sessions::end_user_sessions(&tx, id, None)?;
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    tx.commit()?;
    Ok(user)
}

pub fn enable_user(conn: &Connection, id: i64, now: DateTime<Utc>) -> Result<User> {
    let tx = immediate(conn)?;
    get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    tx.execute(
        "UPDATE users SET disabled_at = NULL, updated_at = ?1
          WHERE id = ?2 AND disabled_at IS NOT NULL",
        params![stamp(now), id],
    )?;
    let user = get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    tx.commit()?;
    Ok(user)
}

/// A new temporary password: `must_change_password` set, every session ended,
/// and with `clear_totp` the second factor removed too (a lost device).
pub fn reset_password(
    conn: &Connection,
    id: i64,
    clear_totp: bool,
    now: DateTime<Utc>,
) -> Result<Zeroizing<String>> {
    let temporary = temporary_password();
    let hash = hash_password(&temporary)?;
    let tx = immediate(conn)?;
    get_user(&tx, id)?.ok_or_else(|| not_found(id))?;
    let ts = stamp(now);
    tx.execute(
        "UPDATE users SET password_hash = ?1, must_change_password = 1,
                          password_changed_at = ?2, updated_at = ?2
          WHERE id = ?3",
        params![hash, ts, id],
    )?;
    if clear_totp {
        tx.execute(
            "UPDATE users SET totp_secret = NULL, totp_last_step = NULL WHERE id = ?1",
            [id],
        )?;
    }
    sessions::end_user_sessions(&tx, id, None)?;
    tx.commit()?;
    Ok(temporary)
}

/// Check an email and password. An address that matches no user, or is not an
/// address at all, still runs one Argon2id verification against a fixed hash.
pub fn verify_credentials(conn: &Connection, email: &str, password: &str) -> Result<Credentials> {
    let Ok(email) = normalize_email(email) else {
        verify_dummy(password);
        return Ok(Credentials::Invalid);
    };
    let row = conn
        .query_row(
            &format!("SELECT {USER_COLUMNS}, password_hash FROM users WHERE email = ?1"),
            [email],
            |r| Ok((user_from_row(r)?, r.get::<_, String>(11)?)),
        )
        .optional()?;
    let Some((user, hash)) = row else {
        verify_dummy(password);
        return Ok(Credentials::Invalid);
    };
    if verify_password(password, &hash) && user.is_enabled() {
        Ok(Credentials::Valid(user))
    } else {
        Ok(Credentials::Invalid)
    }
}

/// Change a user's own password. The current one must verify, the new one must
/// pass the rules; on success `must_change_password` is cleared and every
/// session but `keep_session` ends.
pub fn change_password(
    conn: &Connection,
    id: i64,
    current: &str,
    new: &str,
    keep_session: Option<i64>,
    now: DateTime<Utc>,
) -> std::result::Result<(), PasswordChangeError> {
    let (user, hash) = conn
        .query_row(
            &format!("SELECT {USER_COLUMNS}, password_hash FROM users WHERE id = ?1"),
            [id],
            |r| Ok((user_from_row(r)?, r.get::<_, String>(11)?)),
        )
        .optional()?
        .ok_or_else(|| not_found(id))?;
    if !verify_password(current, &hash) {
        return Err(PasswordChangeError::WrongCurrentPassword);
    }
    check_new_password(
        new,
        &PasswordOwner {
            email: &user.email,
            display_name: &user.display_name,
            current_hash: Some(&hash),
        },
    )
    .map_err(PasswordChangeError::Rule)?;
    let new_hash = hash_password(new)?;
    commit_password_change(conn, id, &hash, &new_hash, keep_session, now)
}

/// Write `new_hash` only if the stored hash is still the one the current
/// password was verified against and the user is enabled. A reset or a
/// disable in between leaves the row and the sessions untouched.
fn commit_password_change(
    conn: &Connection,
    id: i64,
    expected_hash: &str,
    new_hash: &str,
    keep_session: Option<i64>,
    now: DateTime<Utc>,
) -> std::result::Result<(), PasswordChangeError> {
    let tx = immediate(conn)?;
    let ts = stamp(now);
    let changed = tx.execute(
        "UPDATE users SET password_hash = ?1, must_change_password = 0,
                          password_changed_at = ?2, updated_at = ?2
          WHERE id = ?3 AND password_hash = ?4 AND disabled_at IS NULL",
        params![new_hash, ts, id, expected_hash],
    )?;
    if changed == 0 {
        return Err(PasswordChangeError::WrongCurrentPassword);
    }
    sessions::end_user_sessions(&tx, id, keep_session)?;
    tx.commit()?;
    Ok(())
}

/// What a code proved against a user's second factor.
#[derive(Debug, PartialEq, Eq)]
pub enum TotpCheck {
    Accepted,
    Rejected,
    NotEnrolled,
}

/// Store a confirmed second factor, sealed for this user. `confirmed_step` is
/// the step of the code that confirmed it, so that code cannot sign in again.
pub fn set_totp_secret(
    conn: &Connection,
    id: i64,
    key: &ServerKey,
    secret: &[u8],
    confirmed_step: i64,
    now: DateTime<Utc>,
) -> Result<()> {
    let user = get_user(conn, id)?.ok_or_else(|| not_found(id))?;
    let sealed = key.seal(&user.uid, secret)?;
    let changed = conn.execute(
        "UPDATE users SET totp_secret = ?1, totp_last_step = ?2, updated_at = ?3 WHERE id = ?4",
        params![sealed, confirmed_step, stamp(now), id],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

pub fn clear_totp(conn: &Connection, id: i64, now: DateTime<Utc>) -> Result<()> {
    let changed = conn.execute(
        "UPDATE users SET totp_secret = NULL, totp_last_step = NULL, updated_at = ?1 WHERE id = ?2",
        params![stamp(now), id],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// Check a code against a user's sealed secret. A code is accepted only when
/// its step is later than the last accepted one, and the write that records it
/// is guarded on that, so two sign-ins racing with one code cannot both pass.
/// A disabled user is rejected.
/// A secret the server key cannot open is an error, not a rejection: the
/// caller logs which user.
pub fn verify_totp(
    conn: &Connection,
    id: i64,
    key: &ServerKey,
    code: &str,
    now: DateTime<Utc>,
) -> Result<TotpCheck> {
    let tx = immediate(conn)?;
    let row: Option<(String, Option<Vec<u8>>)> = tx
        .query_row(
            "SELECT uid, totp_secret FROM users WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (uid, sealed) = row.ok_or_else(|| not_found(id))?;
    let Some(sealed) = sealed else {
        return Ok(TotpCheck::NotEnrolled);
    };
    let secret = key.open(&uid, &sealed)?;
    let Some(step) = totp::matching_step(&secret, code, now) else {
        return Ok(TotpCheck::Rejected);
    };
    let changed = tx.execute(
        "UPDATE users SET totp_last_step = ?1
          WHERE id = ?2 AND disabled_at IS NULL
            AND (totp_last_step IS NULL OR totp_last_step < ?1)",
        params![step, id],
    )?;
    if changed == 0 {
        return Ok(TotpCheck::Rejected);
    }
    tx.commit()?;
    Ok(TotpCheck::Accepted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::sessions::{create_session, SessionKind};
    use crate::identity::testing::{at, db};

    const T0: &str = "2026-01-01T00:00:00Z";

    fn add(conn: &Connection, email: &str, role: Role) -> CreatedUser {
        create_user(
            conn,
            &NewUser {
                email,
                display_name: "Pat Example",
                role,
            },
            at(T0),
        )
        .unwrap()
    }

    fn conflict_code(err: NigelError) -> &'static str {
        match err {
            NigelError::Conflict { code, .. } => code,
            other => panic!("expected a conflict, got {other:?}"),
        }
    }

    #[test]
    fn emails_are_trimmed_lowercased_and_shape_checked() {
        assert_eq!(
            normalize_email("  Pat@Example.COM ").unwrap(),
            "pat@example.com"
        );
        for bad in [
            "",
            "pat",
            "@example.com",
            "pat@",
            "pat@@example.com",
            "pat x@example.com",
        ] {
            assert!(normalize_email(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn emails_with_control_characters_are_refused_without_echoing_them() {
        for bad in [
            "pat\u{1b}]0;x\u{7}@example.com",
            "pat\u{7}@example.com",
            "pat\u{0}@example.com",
        ] {
            let err = normalize_email(bad).unwrap_err();
            assert!(matches!(err, NigelError::Invalid(_)), "{bad:?}");
            let message = err.to_string();
            assert!(!message.chars().any(char::is_control), "{message:?}");
        }
    }

    #[test]
    fn emails_and_names_are_stored_nfc_normalized() {
        let (_dir, conn) = db();
        let created = create_user(
            &conn,
            &NewUser {
                email: "jose\u{301}@example.com",
                display_name: "Jose\u{301} Example",
                role: Role::Admin,
            },
            at(T0),
        )
        .unwrap();
        assert_eq!(created.user.email, "jos\u{e9}@example.com");
        assert_eq!(created.user.display_name, "Jos\u{e9} Example");
        assert!(find_by_email(&conn, "jos\u{e9}@example.com")
            .unwrap()
            .is_some());
        assert!(matches!(
            change_password(
                &conn,
                created.user.id,
                &created.temporary_password,
                "jos\u{e9} example",
                None,
                at(T0)
            ),
            Err(PasswordChangeError::Rule(PasswordRule::MatchesIdentity))
        ));
    }

    #[test]
    fn the_first_user_must_be_an_admin() {
        let (_dir, conn) = db();
        let err = create_user(
            &conn,
            &NewUser {
                email: "sam@example.com",
                display_name: "Sam Example",
                role: Role::Bookkeeper,
            },
            at(T0),
        )
        .err()
        .unwrap();
        assert_eq!(conflict_code(err), "last_admin");
        assert!(!has_users(&conn).unwrap());
    }

    #[test]
    fn a_created_user_has_a_temporary_password_and_must_change_it() {
        let (_dir, conn) = db();
        let created = add(&conn, " Pat@Example.com ", Role::Admin);
        assert!(created.first_user);
        let user = &created.user;
        assert_eq!(user.email, "pat@example.com");
        assert_eq!(user.uid.len(), 32);
        assert!(user.must_change_password);
        assert!(!user.totp_enabled);
        assert_eq!(user.created_at, T0);
        assert!(matches!(
            verify_credentials(&conn, "PAT@example.com", &created.temporary_password).unwrap(),
            Credentials::Valid(_)
        ));
        let stored: String = conn
            .query_row("SELECT password_hash FROM users", [], |r| r.get(0))
            .unwrap();
        assert!(stored.starts_with("$argon2id$"));
        assert!(!stored.contains(created.temporary_password.as_str()));
        assert!(!add(&conn, "sam@example.com", Role::Bookkeeper).first_user);
    }

    #[test]
    fn a_second_spelling_of_an_email_is_a_duplicate() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        let err = create_user(
            &conn,
            &NewUser {
                email: "PAT@example.com",
                display_name: "Pat Again",
                role: Role::Admin,
            },
            at(T0),
        )
        .err()
        .unwrap();
        assert_eq!(conflict_code(err), "duplicate_email");
    }

    #[test]
    fn a_blank_or_overlong_name_is_refused() {
        let (_dir, conn) = db();
        for name in ["   ".to_string(), "x".repeat(101), "Pat\u{7}".to_string()] {
            let result = create_user(
                &conn,
                &NewUser {
                    email: "pat@example.com",
                    display_name: &name,
                    role: Role::Admin,
                },
                at(T0),
            );
            assert!(matches!(result, Err(NigelError::Invalid(_))), "{name:?}");
        }
    }

    #[test]
    fn the_last_enabled_admin_cannot_be_demoted_or_disabled() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper).user.id;
        assert_eq!(
            conflict_code(set_role(&conn, pat, Role::Bookkeeper, at(T0)).unwrap_err()),
            "last_admin"
        );
        assert_eq!(
            conflict_code(disable_user(&conn, pat, at(T0)).unwrap_err()),
            "last_admin"
        );
        set_role(&conn, sam, Role::Admin, at(T0)).unwrap();
        disable_user(&conn, pat, at(T0)).unwrap();
        assert_eq!(
            conflict_code(set_role(&conn, sam, Role::ReadOnly, at(T0)).unwrap_err()),
            "last_admin",
            "a disabled admin does not count"
        );
        set_role(&conn, pat, Role::ReadOnly, at(T0)).unwrap();
    }

    #[test]
    fn disable_and_enable_are_idempotent() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper).user.id;
        let first = disable_user(&conn, sam, at(T0)).unwrap();
        let again = disable_user(&conn, sam, at("2026-02-01T00:00:00Z")).unwrap();
        assert_eq!(first.disabled_at.as_deref(), Some(T0));
        assert_eq!(again.disabled_at.as_deref(), Some(T0));
        assert!(enable_user(&conn, sam, at(T0)).unwrap().is_enabled());
        assert!(enable_user(&conn, sam, at(T0)).unwrap().is_enabled());
    }

    #[test]
    fn a_disabled_user_cannot_sign_in() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper);
        disable_user(&conn, sam.user.id, at(T0)).unwrap();
        assert!(matches!(
            verify_credentials(&conn, "sam@example.com", &sam.temporary_password).unwrap(),
            Credentials::Invalid
        ));
    }

    #[test]
    fn unknown_wrong_and_malformed_credentials_are_all_invalid() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        for (email, password) in [
            ("nobody@example.com", "whatever password"),
            ("pat@example.com", "wrong password here"),
            ("not an email", "whatever password"),
        ] {
            assert!(
                matches!(
                    verify_credentials(&conn, email, password).unwrap(),
                    Credentials::Invalid
                ),
                "{email}"
            );
        }
    }

    #[test]
    fn reset_issues_a_new_password_and_ends_every_session() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin);
        create_session(&conn, pat.user.id, SessionKind::Cookie, None, None, at(T0)).unwrap();
        let fresh = reset_password(&conn, pat.user.id, false, at("2026-02-01T00:00:00Z")).unwrap();
        assert_ne!(fresh.as_str(), pat.temporary_password.as_str());
        assert!(matches!(
            verify_credentials(&conn, "pat@example.com", &pat.temporary_password).unwrap(),
            Credentials::Invalid
        ));
        assert!(matches!(
            verify_credentials(&conn, "pat@example.com", &fresh).unwrap(),
            Credentials::Valid(u) if u.must_change_password
                && u.password_changed_at == "2026-02-01T00:00:00Z"
        ));
        let sessions: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sessions, 0);
    }

    #[test]
    fn reset_with_clear_totp_removes_the_second_factor() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        conn.execute(
            "UPDATE users SET totp_secret = x'0102', totp_last_step = 5 WHERE id = ?1",
            [pat],
        )
        .unwrap();
        reset_password(&conn, pat, false, at(T0)).unwrap();
        assert!(get_user(&conn, pat).unwrap().unwrap().totp_enabled);
        reset_password(&conn, pat, true, at(T0)).unwrap();
        let (secret, step): (Option<Vec<u8>>, Option<i64>) = conn
            .query_row(
                "SELECT totp_secret, totp_last_step FROM users WHERE id = ?1",
                [pat],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((secret, step), (None, None));
    }

    #[test]
    fn changing_a_password_checks_the_current_one_and_the_rules() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin);
        let id = pat.user.id;
        let temp = pat.temporary_password.as_str();
        assert!(matches!(
            change_password(
                &conn,
                id,
                "not it at all",
                "a new long password",
                None,
                at(T0)
            ),
            Err(PasswordChangeError::WrongCurrentPassword)
        ));
        assert!(matches!(
            change_password(&conn, id, temp, "short", None, at(T0)),
            Err(PasswordChangeError::Rule(PasswordRule::TooShort))
        ));
        assert!(matches!(
            change_password(&conn, id, temp, temp, None, at(T0)),
            Err(PasswordChangeError::Rule(PasswordRule::Unchanged))
        ));
        let keep = create_session(&conn, id, SessionKind::Cookie, None, None, at(T0)).unwrap();
        create_session(&conn, id, SessionKind::Bearer, None, None, at(T0)).unwrap();
        change_password(
            &conn,
            id,
            temp,
            "a new long password",
            Some(keep.id),
            at(T0),
        )
        .unwrap();
        let user = get_user(&conn, id).unwrap().unwrap();
        assert!(!user.must_change_password);
        let left: Vec<i64> = conn
            .prepare("SELECT id FROM sessions")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(left, [keep.id]);
        assert!(matches!(
            verify_credentials(&conn, "pat@example.com", "a new long password").unwrap(),
            Credentials::Valid(_)
        ));
    }

    fn stored_hash(conn: &Connection, id: i64) -> String {
        conn.query_row("SELECT password_hash FROM users WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
    }

    fn session_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn a_change_verified_before_a_reset_does_not_undo_the_reset() {
        let (_dir, conn) = db();
        let id = add(&conn, "pat@example.com", Role::Admin).user.id;
        let before = stored_hash(&conn, id);
        let fresh = reset_password(&conn, id, false, at(T0)).unwrap();
        let after = stored_hash(&conn, id);
        create_session(&conn, id, SessionKind::Cookie, None, None, at(T0)).unwrap();
        let new_hash = hash_password("a new long password").unwrap();
        assert!(matches!(
            commit_password_change(&conn, id, &before, &new_hash, None, at(T0)),
            Err(PasswordChangeError::WrongCurrentPassword)
        ));
        assert_eq!(stored_hash(&conn, id), after);
        assert!(get_user(&conn, id).unwrap().unwrap().must_change_password);
        assert_eq!(session_count(&conn), 1);
        assert!(matches!(
            verify_credentials(&conn, "pat@example.com", &fresh).unwrap(),
            Credentials::Valid(_)
        ));
    }

    #[test]
    fn a_disabled_user_cannot_change_their_password() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper);
        disable_user(&conn, sam.user.id, at(T0)).unwrap();
        let before = stored_hash(&conn, sam.user.id);
        assert!(matches!(
            change_password(
                &conn,
                sam.user.id,
                &sam.temporary_password,
                "a new long password",
                None,
                at(T0)
            ),
            Err(PasswordChangeError::WrongCurrentPassword)
        ));
        assert_eq!(stored_hash(&conn, sam.user.id), before);
        assert!(
            get_user(&conn, sam.user.id)
                .unwrap()
                .unwrap()
                .must_change_password
        );
    }

    #[test]
    fn the_listing_reports_last_seen_and_live_sessions_and_no_secrets() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        add(&conn, "sam@example.com", Role::Bookkeeper);
        create_session(&conn, pat, SessionKind::Cookie, None, None, at(T0)).unwrap();
        create_session(
            &conn,
            pat,
            SessionKind::Cookie,
            None,
            None,
            at("2026-01-10T00:00:00Z"),
        )
        .unwrap();
        let rows = list_users(&conn, at("2026-02-05T00:00:00Z")).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].user.email, "pat@example.com");
        assert_eq!(
            rows[0].last_seen_at.as_deref(),
            Some("2026-01-10T00:00:00Z")
        );
        assert_eq!(
            rows[0].active_sessions, 1,
            "the first expired on 2026-01-31"
        );
        assert_eq!(rows[1].last_seen_at, None);
        assert!(!format!("{rows:?}").contains("argon2"));
    }

    #[test]
    fn unknown_ids_and_emails_are_not_found() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        assert!(matches!(
            set_role(&conn, 999, Role::Admin, at(T0)),
            Err(NigelError::NotFound(_))
        ));
        assert!(matches!(
            disable_user(&conn, 999, at(T0)),
            Err(NigelError::NotFound(_))
        ));
        assert!(matches!(
            enable_user(&conn, 999, at(T0)),
            Err(NigelError::NotFound(_))
        ));
        assert!(matches!(
            reset_password(&conn, 999, false, at(T0)),
            Err(NigelError::NotFound(_))
        ));
        let err = require_by_email(&conn, "nobody@example.com").unwrap_err();
        assert!(err.to_string().contains("\"nobody@example.com\""), "{err}");
    }

    #[test]
    fn disabling_a_user_ends_their_sessions_and_no_one_elses() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        let sam = add(&conn, "sam@example.com", Role::Admin).user.id;
        create_session(&conn, sam, SessionKind::Cookie, None, None, at(T0)).unwrap();
        create_session(&conn, pat, SessionKind::Cookie, None, None, at(T0)).unwrap();
        disable_user(&conn, sam, at(T0)).unwrap();
        let count = |id: i64| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE user_id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(count(sam), 0);
        assert_eq!(count(pat), 1);
    }

    #[test]
    fn a_totp_code_is_accepted_once_and_never_replayed() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        let key = ServerKey::from_bytes([1; 32]);
        let secret = [3u8; 20];
        let now = at("2026-03-01T12:00:00Z");
        let step = totp::step_at(now);
        assert_eq!(
            verify_totp(&conn, pat, &key, "123456", now).unwrap(),
            TotpCheck::NotEnrolled
        );
        set_totp_secret(&conn, pat, &key, &secret, step - 1, now).unwrap();
        let stored: Vec<u8> = conn
            .query_row("SELECT totp_secret FROM users WHERE id = ?1", [pat], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(!stored.windows(20).any(|w| w == secret), "stored sealed");

        let confirmed = totp::code_for_step(&secret, step - 1);
        assert_eq!(
            verify_totp(&conn, pat, &key, &confirmed, now).unwrap(),
            TotpCheck::Rejected,
            "the confirming code cannot sign in"
        );
        let code = totp::code_for_step(&secret, step);
        assert_eq!(
            verify_totp(&conn, pat, &key, &code, now).unwrap(),
            TotpCheck::Accepted
        );
        assert_eq!(
            verify_totp(&conn, pat, &key, &code, now).unwrap(),
            TotpCheck::Rejected,
            "replay"
        );
        let next = totp::code_for_step(&secret, step + 1);
        assert_eq!(
            verify_totp(&conn, pat, &key, &next, now).unwrap(),
            TotpCheck::Accepted
        );
    }

    #[test]
    fn a_code_for_an_earlier_step_is_refused_after_a_later_one() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        let key = ServerKey::from_bytes([1; 32]);
        let secret = [3u8; 20];
        let now = at("2026-03-01T12:00:00Z");
        let step = totp::step_at(now);
        set_totp_secret(&conn, pat, &key, &secret, step - 5, now).unwrap();
        let later = totp::code_for_step(&secret, step + 1);
        assert_eq!(
            verify_totp(&conn, pat, &key, &later, now).unwrap(),
            TotpCheck::Accepted
        );
        let earlier = totp::code_for_step(&secret, step);
        assert_eq!(
            verify_totp(&conn, pat, &key, &earlier, now).unwrap(),
            TotpCheck::Rejected
        );
        let last: i64 = conn
            .query_row(
                "SELECT totp_last_step FROM users WHERE id = ?1",
                [pat],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(last, step + 1, "a refused code does not move the guard");
    }

    #[test]
    fn a_secret_sealed_under_another_key_is_an_error_not_a_rejection() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        set_totp_secret(
            &conn,
            pat,
            &ServerKey::from_bytes([1; 32]),
            &[3; 20],
            0,
            at(T0),
        )
        .unwrap();
        let err = verify_totp(
            &conn,
            pat,
            &ServerKey::from_bytes([2; 32]),
            "123456",
            at(T0),
        );
        assert!(err.is_err());
    }

    #[test]
    fn clearing_totp_turns_it_off() {
        let (_dir, conn) = db();
        let pat = add(&conn, "pat@example.com", Role::Admin).user.id;
        set_totp_secret(
            &conn,
            pat,
            &ServerKey::from_bytes([1; 32]),
            &[3; 20],
            0,
            at(T0),
        )
        .unwrap();
        clear_totp(&conn, pat, at(T0)).unwrap();
        assert!(!get_user(&conn, pat).unwrap().unwrap().totp_enabled);
        assert!(clear_totp(&conn, 999, at(T0)).is_err());
    }

    #[test]
    fn a_disabled_user_cannot_pass_the_second_factor() {
        let (_dir, conn) = db();
        add(&conn, "pat@example.com", Role::Admin);
        let sam = add(&conn, "sam@example.com", Role::Bookkeeper).user.id;
        let key = ServerKey::from_bytes([1; 32]);
        let secret = [3u8; 20];
        let now = at("2026-03-01T12:00:00Z");
        let step = totp::step_at(now);
        set_totp_secret(&conn, sam, &key, &secret, step - 5, now).unwrap();
        disable_user(&conn, sam, now).unwrap();
        let code = totp::code_for_step(&secret, step);
        assert_eq!(
            verify_totp(&conn, sam, &key, &code, now).unwrap(),
            TotpCheck::Rejected
        );
        let last: i64 = conn
            .query_row(
                "SELECT totp_last_step FROM users WHERE id = ?1",
                [sam],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(last, step - 5);
    }
}
