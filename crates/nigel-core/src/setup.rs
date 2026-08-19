//! Creating a set of books: the one implementation both front ends call.
//!
//! The terminal's onboarding and the web's `POST /api/setup` collect the same
//! four answers in very different ways and then have to do exactly the same
//! six things with them. Those six live here, in the order they have to happen
//! in: the password global is set *before* the database file is created, so
//! SQLCipher's `PRAGMA key` is in force for the first page written and the
//! books are never briefly in plaintext.

use std::path::PathBuf;

use crate::db;
use crate::error::Result;
use crate::settings;

/// The answers a set of books is created from.
pub struct SetupPlan {
    /// Who to greet. Empty leaves whatever `settings.json` already holds.
    pub user_name: String,
    /// The business or household name. Empty writes no metadata.
    pub company_name: String,
    /// Which chart of accounts. Only takes effect on a fresh database.
    pub profile: db::Profile,
    /// `Some` encrypts the database; `None` leaves the current key alone.
    pub password: Option<String>,
    /// Where the books go.
    ///
    /// The caller's value rather than `settings.json`'s, because a caller may
    /// have already decided which directory it is talking about: the web route
    /// checks the path it is serving for existing books, and has to write to
    /// that same value or the check and the write can disagree.
    pub data_dir: PathBuf,
}

/// Create the data directory tree and the database under `plan.data_dir`, and
/// answer where the database landed.
///
/// Safe to call against books that already exist: the directory creation is
/// idempotent, `init_db_with_profile` migrates rather than reseeds, and an
/// empty name or company writes nothing.
pub fn run(plan: &SetupPlan) -> Result<PathBuf> {
    let mut stored = settings::load_settings();
    if !plan.user_name.is_empty() {
        stored.user_name = plan.user_name.clone();
    }
    settings::save_settings(&stored)?;

    if let Some(password) = plan.password.as_ref() {
        db::set_db_password(Some(password.clone()));
    }

    let data_dir = &plan.data_dir;
    for dir in [
        data_dir.to_path_buf(),
        data_dir.join("exports"),
        data_dir.join("snapshots"),
        data_dir.join("backups"),
    ] {
        std::fs::create_dir_all(&dir)?;
        settings::restrict_dir_permissions(&dir)?;
    }

    let db_path = data_dir.join("nigel.db");
    let conn = db::get_connection(&db_path)?;
    db::init_db_with_profile(&conn, plan.profile)?;
    if !plan.company_name.is_empty() {
        db::set_metadata(&conn, "company_name", &plan.company_name)?;
    }

    Ok(db_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every test here writes settings.json and creates the data directory it
    /// names, so every one needs the redirect.
    fn fixture() -> (crate::settings::TempConfigDir, tempfile::TempDir) {
        crate::db::set_db_password(None);
        let config = crate::settings::TempConfigDir::new();
        let dir = tempfile::tempdir().expect("tempdir");
        let mut settings = crate::settings::load_settings();
        settings.data_dir = dir.path().join("books").to_string_lossy().to_string();
        crate::settings::save_settings(&settings).expect("save settings");
        (config, dir)
    }

    fn plan(password: Option<&str>) -> SetupPlan {
        SetupPlan {
            user_name: "Marta".to_string(),
            company_name: "Cedar Systems".to_string(),
            profile: crate::db::Profile::Business,
            password: password.map(str::to_string),
            data_dir: PathBuf::from(crate::settings::load_settings().data_dir),
        }
    }

    #[test]
    fn it_builds_the_whole_directory_tree() {
        let (_config, _dir) = fixture();

        let db_path = run(&plan(None)).expect("setup");

        let data_dir = db_path.parent().expect("parent");
        assert!(db_path.exists(), "no database at {}", db_path.display());
        for name in ["exports", "snapshots", "backups"] {
            assert!(data_dir.join(name).is_dir(), "missing {name}/");
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_directories_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let (_config, _dir) = fixture();

        let db_path = run(&plan(None)).expect("setup");
        let data_dir = db_path.parent().expect("parent");

        for dir in [
            data_dir.to_path_buf(),
            data_dir.join("exports"),
            data_dir.join("snapshots"),
            data_dir.join("backups"),
        ] {
            let mode = std::fs::metadata(&dir)
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "{} is not 0700", dir.display());
        }
    }

    #[test]
    fn it_saves_the_name_and_the_company_where_each_belongs() {
        let (_config, _dir) = fixture();

        let db_path = run(&plan(None)).expect("setup");

        assert_eq!(crate::settings::load_settings().user_name, "Marta");
        let conn = crate::db::get_connection(&db_path).expect("open");
        assert_eq!(
            crate::db::get_metadata(&conn, "company_name").as_deref(),
            Some("Cedar Systems")
        );
    }

    #[test]
    fn it_honours_the_chosen_profile() {
        let (_config, _dir) = fixture();
        let mut personal = plan(None);
        personal.profile = crate::db::Profile::Personal;

        let db_path = run(&personal).expect("setup");

        let conn = crate::db::get_connection(&db_path).expect("open");
        assert_eq!(crate::db::get_profile(&conn), crate::db::Profile::Personal);
    }

    #[test]
    fn a_password_encrypts_the_database_from_its_first_write() {
        // The point of setting the global *before* the file exists: SQLCipher's
        // PRAGMA key has to be in force for the very first page written, or the
        // books sit in plaintext with a password the user believes protects them.
        let (_config, _dir) = fixture();

        let db_path = run(&plan(Some("correct horse battery staple"))).expect("setup");

        assert!(
            crate::db::is_encrypted(&db_path).expect("probe"),
            "not encrypted"
        );
        crate::db::set_db_password(None);
        assert!(
            crate::db::open_connection(&db_path, None).is_err(),
            "opened without the key"
        );
    }

    #[test]
    fn no_password_leaves_the_file_readable_without_one() {
        let (_config, _dir) = fixture();

        let db_path = run(&plan(None)).expect("setup");

        assert!(!crate::db::is_encrypted(&db_path).expect("probe"));
        crate::db::open_connection(&db_path, None).expect("open plaintext");
    }

    #[test]
    fn it_writes_where_the_plan_says_rather_than_where_settings_points() {
        // The caller owns the directory. A caller that has already decided
        // which books it means — the web route, checking the path it serves —
        // must not have the write land somewhere else.
        let (_config, dir) = fixture();
        let mut elsewhere = plan(None);
        elsewhere.data_dir = dir.path().join("elsewhere");

        let db_path = run(&elsewhere).expect("setup");

        assert_eq!(db_path, dir.path().join("elsewhere").join("nigel.db"));
        assert!(db_path.exists(), "no database at {}", db_path.display());
        assert!(
            !dir.path().join("books").join("nigel.db").exists(),
            "it followed settings.json instead of the plan"
        );
    }

    #[test]
    fn an_empty_name_leaves_the_stored_one_alone() {
        // The dashboard calls this on every launch, not only the first, and a
        // returning user's plan carries whatever settings.json already holds.
        let (_config, _dir) = fixture();
        run(&plan(None)).expect("first setup");

        let mut anonymous = plan(None);
        anonymous.user_name = String::new();
        anonymous.company_name = String::new();
        run(&anonymous).expect("second setup");

        assert_eq!(crate::settings::load_settings().user_name, "Marta");
    }
}
