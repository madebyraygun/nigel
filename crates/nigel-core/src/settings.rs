use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{NigelError, Result};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub data_dir: String,
    #[serde(default)]
    pub user_name: String,
    #[serde(default = "default_true")]
    pub update_check: bool,
    #[serde(default)]
    pub last_update_check: Option<String>,
    #[serde(default)]
    pub stripe_secret_key: Option<String>,
    #[serde(default)]
    pub mailgun_api_key: Option<String>,
    #[serde(default)]
    pub mailgun_domain: Option<String>,
    #[serde(default)]
    pub from_email: Option<String>,
    #[serde(default)]
    pub from_name: Option<String>,
    #[serde(default)]
    pub reply_to_email: Option<String>,
    #[serde(default)]
    pub contact_email: Option<String>,
    #[serde(default)]
    pub r2_account_id: Option<String>,
    #[serde(default)]
    pub r2_access_key: Option<String>,
    #[serde(default)]
    pub r2_secret_key: Option<String>,
    #[serde(default)]
    pub r2_bucket: Option<String>,
    #[serde(default)]
    pub public_base_url: Option<String>,
    #[serde(default)]
    pub r2_private_bucket: Option<String>,
    #[serde(default)]
    pub documents_base_url: Option<String>,
    #[serde(default)]
    pub document_response_url: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir().to_string_lossy().to_string(),
            user_name: String::new(),
            update_check: true,
            last_update_check: None,
            stripe_secret_key: None,
            mailgun_api_key: None,
            mailgun_domain: None,
            from_email: None,
            from_name: None,
            reply_to_email: None,
            contact_email: None,
            r2_account_id: None,
            r2_access_key: None,
            r2_secret_key: None,
            r2_bucket: None,
            public_base_url: None,
            r2_private_bucket: None,
            documents_base_url: None,
            document_response_url: None,
        }
    }
}

/// Test-only redirection of the config directory.
///
/// Without it, any test that calls [`save_settings`] rewrites the developer's
/// real `~/.config/nigel/settings.json` and repoints their data directory. An
/// in-crate override is used rather than `$HOME`: `dirs::home_dir` does not
/// consult the environment on every platform, and mutating the environment is
/// process-global and unsafe in newer editions.
#[cfg(any(test, feature = "testutil"))]
static CONFIG_DIR_OVERRIDE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Point the config directory somewhere else and hand back what it was.
///
/// Returning the previous value is what lets a guard put it back rather than
/// clearing it: clearing exposes the real `~/.config/nigel/settings.json` to
/// whatever is still running, and the value it writes there is a temporary
/// directory that is about to be deleted.
#[cfg(any(test, feature = "testutil"))]
pub fn set_config_dir_for_tests(dir: Option<PathBuf>) -> Option<PathBuf> {
    // unwrap: poisoned mutex means a thread panicked — unrecoverable
    std::mem::replace(&mut *CONFIG_DIR_OVERRIDE.lock().unwrap(), dir)
}

/// Test-only suppression of the `NIGEL_*` layer of [`invoicing_config`].
///
/// The environment wins over settings.json, which is a documented production
/// feature — and it is how this repository's own operator configures invoicing,
/// so on their machine "nothing is configured" would resolve nine real
/// credentials and a test that expects a refusal would instead send an invoice
/// through Stripe, R2 and Mailgun. The suite is offline unconditionally, not
/// offline-if-your-shell-happens-to-be-empty, so the guard below takes the
/// environment out of the resolution entirely rather than trusting it to be
/// unset. An in-crate override is used rather than unsetting the variables:
/// mutating the process environment is global, unsafe in newer editions, and
/// would have to be undone even on a panicking test.
#[cfg(any(test, feature = "testutil"))]
static SUPPRESS_INVOICING_ENV: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

/// Turn the environment layer off (or back on) and hand back what it was, so a
/// guard restores rather than clears — the [`set_config_dir_for_tests`] rule.
#[cfg(any(test, feature = "testutil"))]
pub fn suppress_invoicing_env_for_tests(suppress: bool) -> bool {
    // unwrap: poisoned mutex means a thread panicked — unrecoverable
    std::mem::replace(&mut *SUPPRESS_INVOICING_ENV.lock().unwrap(), suppress)
}

#[cfg(any(test, feature = "testutil"))]
fn invoicing_env_is_suppressed() -> bool {
    // unwrap: poisoned mutex means a thread panicked — unrecoverable
    *SUPPRESS_INVOICING_ENV.lock().unwrap()
}

/// Resolve settings from a temporary directory alone, for the life of the
/// guard: `~/.config/nigel` is redirected and the `NIGEL_*` environment layer
/// is switched off, so everything this module answers comes from the temporary
/// file and nothing else.
///
/// Lives here rather than in the server's test helpers because the config is
/// this module's, and tests outside the `serve` feature need it too: anything
/// that reads [`load_settings`] otherwise answers from whatever the developer's
/// own settings.json — or shell — happens to say.
#[cfg(any(test, feature = "testutil"))]
pub struct TempConfigDir {
    _dir: tempfile::TempDir,
    previous: Option<PathBuf>,
    previously_suppressed: bool,
}

#[cfg(any(test, feature = "testutil"))]
impl TempConfigDir {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let previous = set_config_dir_for_tests(Some(dir.path().to_path_buf()));
        let previously_suppressed = suppress_invoicing_env_for_tests(true);
        Self {
            _dir: dir,
            previous,
            previously_suppressed,
        }
    }
}

#[cfg(any(test, feature = "testutil"))]
impl Default for TempConfigDir {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "testutil"))]
impl Drop for TempConfigDir {
    /// Restores what was there rather than clearing, so a guard dropping while
    /// another is alive cannot hand the real config directory back to a test
    /// that is still writing.
    fn drop(&mut self) {
        set_config_dir_for_tests(self.previous.take());
        suppress_invoicing_env_for_tests(self.previously_suppressed);
    }
}

/// The config directory an embedding host chose for this process.
///
/// A host whose home directory is not the user's — the sandboxed Swift shell,
/// whose `$HOME` is its app container — names the directory itself through
/// [`set_config_dir`] before it serves anything. The CLI and `nigel serve`
/// never set it and resolve from the home directory.
static HOST_CONFIG_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Fix this process's config directory, once.
///
/// Every path derived from [`config_dir`] — `settings.json`, the invoicing
/// and documents config, window state — then resolves under `dir`. Setting
/// the same directory again is a no-op; a different one is refused with the
/// directory already in force, because settings read from one place and
/// written to another is how a process loses its configuration.
pub fn set_config_dir(dir: PathBuf) -> std::result::Result<(), PathBuf> {
    claim_config_dir(&HOST_CONFIG_DIR, dir)
}

fn claim_config_dir(
    cell: &std::sync::OnceLock<PathBuf>,
    dir: PathBuf,
) -> std::result::Result<(), PathBuf> {
    let held = cell.get_or_init(|| dir.clone());
    if *held == dir {
        Ok(())
    } else {
        Err(held.clone())
    }
}

/// The directory nigel's machine-local configuration lives in: the one a
/// host fixed with [`set_config_dir`], else `~/.config/nigel`. Honors
/// [`set_config_dir_for_tests`] ahead of both, like every path derived from it.
pub fn config_dir() -> PathBuf {
    #[cfg(any(test, feature = "testutil"))]
    // unwrap: poisoned mutex means a thread panicked — unrecoverable
    if let Some(dir) = CONFIG_DIR_OVERRIDE.lock().unwrap().clone() {
        return dir;
    }

    if let Some(dir) = HOST_CONFIG_DIR.get() {
        return dir.clone();
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("nigel")
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Documents")
        .join("nigel")
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Settings::default()
    }
}

pub fn save_settings(settings: &Settings) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    restrict_dir_permissions(&dir)?;
    let json =
        serde_json::to_string_pretty(settings).map_err(|e| NigelError::Settings(e.to_string()))?;
    write_settings_file(&settings_path(), &format!("{json}\n"))
}

/// Replace settings.json whole: the contents go to a new owner-only file
/// beside it, are flushed to disk, and are renamed over the old one. A reader
/// sees the previous file or the new one, never a partly written or truncated
/// one, and a crash mid-write leaves the previous file intact.
fn write_settings_file(path: &std::path::Path, contents: &str) -> Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    // A symlinked settings.json (a dotfiles checkout, say) is written through:
    // the temporary file goes beside the link's target and replaces that, so
    // the link survives.
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = path.as_path();
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "settings.json".to_string());
    let tmp_prefix = format!(".{file_name}.");
    let tmp = path.with_file_name(format!(
        "{tmp_prefix}{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));

    let written = (|| -> Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)?;
        #[cfg(unix)]
        if let Some(dir) = path.parent() {
            std::fs::File::open(dir)?.sync_all()?;
        }
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return written;
    }
    sweep_stale_temp_files(path, &tmp_prefix);
    Ok(())
}

/// How old a leftover temporary settings file has to be before a save removes
/// it — far longer than any save takes, so a concurrent writer's file is safe.
const STALE_TEMP_AGE: std::time::Duration = std::time::Duration::from_secs(60);

/// Remove temporary settings files a killed process left beside `path`. They
/// can hold API keys, so they are not left to accumulate. Only files older than
/// [`STALE_TEMP_AGE`] and, on Unix, owned by the user who owns the file just
/// written are touched. Best-effort: a failure here never fails the save.
fn sweep_stale_temp_files(path: &std::path::Path, tmp_prefix: &str) {
    let Some(dir) = path.parent() else { return };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    #[cfg(unix)]
    let owner = {
        use std::os::unix::fs::MetadataExt;
        match std::fs::metadata(path) {
            Ok(meta) => meta.uid(),
            Err(_) => return,
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(tmp_prefix) || !name.ends_with(".tmp") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.uid() != owner {
                continue;
            }
        }
        let stale = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|age| age >= STALE_TEMP_AGE);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

pub fn settings_file_exists() -> bool {
    settings_path().exists()
}

/// Read and remove legacy `company_name` from settings.json if present.
/// Returns the value so it can be migrated to the DB metadata table.
pub fn migrate_company_name() -> Option<String> {
    let path = settings_path();
    let content = std::fs::read_to_string(&path).ok()?;
    let mut raw: serde_json::Value = serde_json::from_str(&content).ok()?;
    let company = raw.as_object_mut()?.remove("company_name")?;
    let name = company.as_str()?.to_string();
    if name.is_empty() {
        return None;
    }
    // Rewrite settings without company_name
    if let Ok(json) = serde_json::to_string_pretty(&raw) {
        let _ = write_settings_file(&path, &format!("{json}\n"));
    }
    Some(name)
}

/// Resolved invoicing credentials and endpoints.
///
/// `Default` is an installation with nothing configured, which is a real state
/// rather than an omission: void and republish both work on one, and a test that
/// wants to reach nothing says so by handing this over.
#[derive(Default)]
pub struct InvoicingConfig {
    pub stripe_secret_key: Option<String>,
    pub mailgun_api_key: Option<String>,
    pub mailgun_domain: Option<String>,
    pub from_email: Option<String>,
    pub from_name: Option<String>,
    pub reply_to_email: Option<String>,
    pub contact_email: Option<String>,
    pub r2_account_id: Option<String>,
    pub r2_access_key: Option<String>,
    pub r2_secret_key: Option<String>,
    pub r2_bucket: Option<String>,
    pub public_base_url: Option<String>,
}

pub fn invoicing_config_from(s: &Settings) -> InvoicingConfig {
    invoicing_config_with(s, process_env)
}

/// The process environment, or nothing at all under a `TempConfigDir` — see
/// `SUPPRESS_INVOICING_ENV`, which exists so that a machine with the `NIGEL_*`
/// variables exported cannot turn an offline test into a real send.
fn process_env(name: &str) -> Option<String> {
    #[cfg(any(test, feature = "testutil"))]
    if invoicing_env_is_suppressed() {
        return None;
    }
    std::env::var(name).ok()
}

/// The env lookup is injected so tests can exercise the env-wins precedence
/// without mutating the process environment.
fn invoicing_config_with(s: &Settings, env: impl Fn(&str) -> Option<String>) -> InvoicingConfig {
    let env_or = |name: &str, file_val: &Option<String>| env(name).or_else(|| file_val.clone());
    InvoicingConfig {
        stripe_secret_key: env_or("NIGEL_STRIPE_SECRET_KEY", &s.stripe_secret_key),
        mailgun_api_key: env_or("NIGEL_MAILGUN_API_KEY", &s.mailgun_api_key),
        mailgun_domain: env_or("NIGEL_MAILGUN_DOMAIN", &s.mailgun_domain),
        from_email: env_or("NIGEL_FROM_EMAIL", &s.from_email),
        from_name: env_or("NIGEL_FROM_NAME", &s.from_name),
        reply_to_email: env_or("NIGEL_REPLY_TO_EMAIL", &s.reply_to_email),
        contact_email: env_or("NIGEL_CONTACT_EMAIL", &s.contact_email),
        r2_account_id: env_or("NIGEL_R2_ACCOUNT_ID", &s.r2_account_id),
        r2_access_key: env_or("NIGEL_R2_ACCESS_KEY", &s.r2_access_key),
        r2_secret_key: env_or("NIGEL_R2_SECRET_KEY", &s.r2_secret_key),
        r2_bucket: env_or("NIGEL_R2_BUCKET", &s.r2_bucket),
        public_base_url: url_setting(&env, "NIGEL_PUBLIC_BASE_URL", &s.public_base_url),
    }
}

/// A URL-valued setting as it is composed into links: the environment first,
/// then the file, each with leading and trailing whitespace trimmed and a value
/// that is nothing but whitespace treated as unset — so a blank variable does
/// not hide the file's value. Control characters are not whitespace and survive
/// this, which is what lets `r2::validate_url_setting` refuse them by name.
fn url_setting(
    env: impl Fn(&str) -> Option<String>,
    name: &str,
    file_val: &Option<String>,
) -> Option<String> {
    let trimmed = |v: Option<String>| v.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    trimmed(env(name)).or_else(|| trimmed(file_val.clone()))
}

pub fn invoicing_config() -> InvoicingConfig {
    invoicing_config_from(&load_settings())
}

/// Everything the documents feature reads: the invoicing keys it shares with
/// invoices plus the three it adds. Stripe is carried but never required.
#[derive(Default)]
pub struct DocumentsConfig {
    pub invoicing: InvoicingConfig,
    pub r2_private_bucket: Option<String>,
    pub documents_base_url: Option<String>,
    pub document_response_url: Option<String>,
}

pub fn documents_config_from(s: &Settings) -> DocumentsConfig {
    documents_config_with(s, process_env)
}

fn documents_config_with(s: &Settings, env: impl Fn(&str) -> Option<String>) -> DocumentsConfig {
    let env_or = |name: &str, file_val: &Option<String>| env(name).or_else(|| file_val.clone());
    DocumentsConfig {
        invoicing: invoicing_config_with(s, &env),
        r2_private_bucket: env_or("NIGEL_R2_PRIVATE_BUCKET", &s.r2_private_bucket),
        documents_base_url: url_setting(&env, "NIGEL_DOCUMENTS_BASE_URL", &s.documents_base_url),
        document_response_url: url_setting(
            &env,
            "NIGEL_DOCUMENT_RESPONSE_URL",
            &s.document_response_url,
        ),
    }
}

pub fn documents_config() -> DocumentsConfig {
    documents_config_from(&load_settings())
}

/// The base URL documents publish under: the explicit override, or the invoice
/// base with its trailing `/i` replaced by `/d`. `None` when neither gives one.
pub fn derive_documents_base(
    documents_base_url: Option<&str>,
    public_base_url: Option<&str>,
) -> Option<String> {
    if let Some(explicit) = documents_base_url {
        return Some(explicit.to_string());
    }
    let invoice_base = public_base_url?.trim_end_matches('/');
    invoice_base
        .strip_suffix("/i")
        .map(|root| format!("{root}/d"))
}

/// Which document features are usable, by key name only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentsStatus {
    pub send_configured: bool,
    pub sync_configured: bool,
    pub response_form: bool,
    pub missing: Vec<&'static str>,
}

pub fn documents_status(cfg: &DocumentsConfig) -> DocumentsStatus {
    let inv = &cfg.invoicing;
    let documents_base = derive_documents_base(
        cfg.documents_base_url.as_deref(),
        inv.public_base_url.as_deref(),
    );
    let keys: [(&'static str, bool); 9] = [
        ("mailgun_api_key", inv.mailgun_api_key.is_some()),
        ("mailgun_domain", inv.mailgun_domain.is_some()),
        ("from_email", inv.from_email.is_some()),
        ("r2_account_id", inv.r2_account_id.is_some()),
        ("r2_access_key", inv.r2_access_key.is_some()),
        ("r2_secret_key", inv.r2_secret_key.is_some()),
        ("r2_bucket", inv.r2_bucket.is_some()),
        ("r2_private_bucket", cfg.r2_private_bucket.is_some()),
        ("documents_base_url", documents_base.is_some()),
    ];
    let missing: Vec<&'static str> = keys
        .iter()
        .filter(|(_, set)| !set)
        .map(|(name, _)| *name)
        .collect();
    let sync_configured = inv.r2_account_id.is_some()
        && inv.r2_access_key.is_some()
        && inv.r2_secret_key.is_some()
        && cfg.r2_private_bucket.is_some();
    DocumentsStatus {
        send_configured: missing.is_empty(),
        sync_configured,
        response_form: cfg.document_response_url.is_some(),
        missing,
    }
}

/// Which invoicing keys are set, by name.
///
/// Key names only, never values: three of the nine are plaintext secrets, and
/// the names are already published in `docs/invoicing.md`. It is what greys out
/// a Send button and what an empty state points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoicingStatus {
    pub send_configured: bool,
    pub sync_configured: bool,
    pub missing: Vec<&'static str>,
    /// A configured `public_base_url` that is probably pointing at the wrong
    /// prefix. Absent when it is unset (that is `missing`'s job) or fine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_base_url_warning: Option<&'static str>,
}

pub fn invoicing_status(cfg: &InvoicingConfig) -> InvoicingStatus {
    // `docs/invoicing.md`'s order, which is also `build_clients`' requirement
    // list: every one of the nine is needed to send.
    let keys: [(&'static str, &Option<String>); 9] = [
        ("stripe_secret_key", &cfg.stripe_secret_key),
        ("mailgun_api_key", &cfg.mailgun_api_key),
        ("mailgun_domain", &cfg.mailgun_domain),
        ("from_email", &cfg.from_email),
        ("r2_account_id", &cfg.r2_account_id),
        ("r2_access_key", &cfg.r2_access_key),
        ("r2_secret_key", &cfg.r2_secret_key),
        ("r2_bucket", &cfg.r2_bucket),
        ("public_base_url", &cfg.public_base_url),
    ];
    let missing: Vec<&'static str> = keys
        .iter()
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| *name)
        .collect();

    InvoicingStatus {
        send_configured: missing.is_empty(),
        sync_configured: cfg.stripe_secret_key.is_some(),
        missing,
        public_base_url_warning: cfg
            .public_base_url
            .as_deref()
            .and_then(crate::invoicing::r2::public_base_url_warning),
    }
}

pub fn get_data_dir() -> PathBuf {
    PathBuf::from(&load_settings().data_dir)
}

pub fn shellexpand_path(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return path.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .to_string()
}

/// Restrict a file to owner-only read/write (0o600) on Unix.
/// No-op on non-Unix platforms.
pub fn restrict_file_permissions(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Restrict a directory to owner-only access (0o700) on Unix.
/// No-op on non-Unix platforms.
pub fn restrict_dir_permissions(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_config_dir_is_fixed_once_and_a_different_one_is_refused() {
        let cell = std::sync::OnceLock::new();
        let first = PathBuf::from("/books/config");

        assert_eq!(claim_config_dir(&cell, first.clone()), Ok(()));
        assert_eq!(claim_config_dir(&cell, first.clone()), Ok(()));
        assert_eq!(
            claim_config_dir(&cell, PathBuf::from("/elsewhere")),
            Err(first.clone())
        );
        assert_eq!(cell.get(), Some(&first));
    }

    fn fully_configured() -> InvoicingConfig {
        InvoicingConfig {
            stripe_secret_key: Some("sk_live_secret".into()),
            mailgun_api_key: Some("key-secret".into()),
            mailgun_domain: Some("mg.example.com".into()),
            from_email: Some("billing@example.com".into()),
            from_name: Some("Bluepeak".into()),
            reply_to_email: Some("sam@example.com".into()),
            contact_email: Some("accounts@example.com".into()),
            r2_account_id: Some("acct-secret".into()),
            r2_access_key: Some("access-secret".into()),
            r2_secret_key: Some("secret-secret".into()),
            r2_bucket: Some("billing".into()),
            public_base_url: Some("https://billing.example.com/i".into()),
        }
    }

    #[test]
    fn the_invoicing_status_never_carries_a_value() {
        let status = invoicing_status(&fully_configured());
        assert!(status.send_configured);
        assert!(status.sync_configured);
        assert!(status.missing.is_empty());

        // The warning field is only on a status that earns one, so the
        // invariant is checked against both shapes.
        let mut warned = fully_configured();
        warned.public_base_url = Some("https://billing.example.com".into());
        let warning_status = invoicing_status(&warned);
        assert!(warning_status.public_base_url_warning.is_some());

        for status in [&status, &warning_status] {
            let rendered = serde_json::to_string(status).expect("serializes");
            for value in [
                "sk_live_secret",
                "key-secret",
                "mg.example.com",
                "billing@example.com",
                "Bluepeak",
                "sam@example.com",
                "accounts@example.com",
                "acct-secret",
                "access-secret",
                "secret-secret",
                "billing",
                "https://billing.example.com/i",
                "https://billing.example.com",
            ] {
                assert!(!rendered.contains(value), "{value} leaked into {rendered}");
            }
        }
    }

    #[test]
    fn the_optional_envelope_keys_never_appear_in_the_missing_list() {
        let mut cfg = fully_configured();
        cfg.from_name = None;
        cfg.reply_to_email = None;
        cfg.contact_email = None;
        let status = invoicing_status(&cfg);
        assert!(
            status.send_configured,
            "an optional key cannot block a send"
        );
        assert!(status.missing.is_empty(), "got: {:?}", status.missing);
    }

    #[test]
    fn the_new_envelope_keys_resolve_from_the_environment_first() {
        let file_settings = Settings {
            from_name: Some("File Co".into()),
            reply_to_email: Some("file@example.test".into()),
            contact_email: Some("file-contact@example.test".into()),
            ..Settings::default()
        };
        let cfg = invoicing_config_with(&file_settings, |name| match name {
            "NIGEL_FROM_NAME" => Some("Env Co".into()),
            "NIGEL_REPLY_TO_EMAIL" => Some("env@example.test".into()),
            "NIGEL_CONTACT_EMAIL" => Some("env-contact@example.test".into()),
            _ => None,
        });
        assert_eq!(cfg.from_name.as_deref(), Some("Env Co"));
        assert_eq!(cfg.reply_to_email.as_deref(), Some("env@example.test"));
        assert_eq!(
            cfg.contact_email.as_deref(),
            Some("env-contact@example.test")
        );

        let from_file = invoicing_config_with(&file_settings, |_| None);
        assert_eq!(from_file.from_name.as_deref(), Some("File Co"));
        assert_eq!(
            from_file.contact_email.as_deref(),
            Some("file-contact@example.test")
        );
    }

    #[test]
    fn a_settings_file_written_before_these_keys_existed_still_loads() {
        let json = r#"{"data_dir": "/tmp/t", "from_email": "billing@mg.example.com"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.from_email.as_deref(), Some("billing@mg.example.com"));
        assert!(s.from_name.is_none() && s.reply_to_email.is_none() && s.contact_email.is_none());
    }

    #[test]
    fn the_status_warns_about_a_base_url_that_misses_the_i_prefix() {
        let mut cfg = fully_configured();
        cfg.public_base_url = Some("https://billing.example.com".into());
        let status = invoicing_status(&cfg);
        assert!(status.send_configured, "a warning is not a refusal");
        assert!(status.public_base_url_warning.is_some());
    }

    #[test]
    fn an_unset_base_url_is_missing_rather_than_warned_about() {
        let mut cfg = fully_configured();
        cfg.public_base_url = None;
        let status = invoicing_status(&cfg);
        assert!(status.public_base_url_warning.is_none());
        assert!(status.missing.contains(&"public_base_url"));
    }

    #[test]
    fn invoicing_status_names_the_missing_keys_in_the_documented_order() {
        let mut cfg = fully_configured();
        cfg.r2_bucket = None;
        cfg.stripe_secret_key = None;
        cfg.public_base_url = None;

        let status = invoicing_status(&cfg);
        assert!(!status.send_configured);
        assert!(!status.sync_configured, "sync is stripe_secret_key alone");
        assert_eq!(
            status.missing,
            vec!["stripe_secret_key", "r2_bucket", "public_base_url"]
        );
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = Settings {
            data_dir: "/tmp/test".to_string(),
            user_name: "Alice".to_string(),
            update_check: true,
            last_update_check: None,
            ..Settings::default()
        };
        let json = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&path, &json).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: Settings = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.user_name, "Alice");
        assert_eq!(loaded.data_dir, "/tmp/test");
    }

    #[test]
    fn test_load_returns_defaults_when_missing() {
        let s = Settings::default();
        assert!(s.user_name.is_empty());
        assert!(!s.data_dir.is_empty());
    }

    #[test]
    fn test_load_merges_with_defaults() {
        let json = r#"{"data_dir": "/tmp/test", "user_name": "Bob"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.user_name, "Bob");
    }

    #[test]
    fn test_ignores_unknown_fields_from_older_versions() {
        let json = r#"{"data_dir": "/tmp/test", "user_name": "Bob", "fiscal_year_start": "07"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.user_name, "Bob");
    }

    #[test]
    fn test_save_creates_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("deep").join("nested").join("dir");
        std::fs::create_dir_all(&nested).unwrap();
        let path = nested.join("settings.json");
        let settings = Settings::default();
        let json = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&path, format!("{json}\n")).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_update_check_defaults_true() {
        let s = Settings::default();
        assert!(s.update_check);
        assert!(s.last_update_check.is_none());
    }

    #[test]
    fn test_update_check_roundtrip() {
        let settings = Settings {
            data_dir: "/tmp/test".to_string(),
            user_name: "Alice".to_string(),
            update_check: false,
            last_update_check: Some("2025-06-15T10:30:00".to_string()),
            ..Settings::default()
        };
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert!(!loaded.update_check);
        assert_eq!(
            loaded.last_update_check.as_deref(),
            Some("2025-06-15T10:30:00")
        );
    }

    /// The suite must be offline whatever the developer's shell says, and the
    /// operator of this repository does export these variables.
    #[test]
    fn a_temp_config_dir_takes_the_environment_out_of_the_invoicing_config() {
        let _guard = TempConfigDir::new();
        // Safe here and only here: the suite runs on one thread (see `db.rs`),
        // this is the process's own variable, and it is removed below.
        std::env::set_var("NIGEL_STRIPE_SECRET_KEY", "sk_live_not_a_real_key");

        let cfg = invoicing_config();
        assert!(
            cfg.stripe_secret_key.is_none(),
            "the environment reached a test that expects nothing configured"
        );
        assert!(!invoicing_status(&cfg).sync_configured);

        std::env::remove_var("NIGEL_STRIPE_SECRET_KEY");
    }

    /// And the suppression is the guard's, not a permanent change: production
    /// resolution still reads the environment first.
    #[test]
    fn the_environment_layer_comes_back_when_the_guard_is_dropped() {
        {
            let _guard = TempConfigDir::new();
            assert!(invoicing_env_is_suppressed());
        }
        assert!(!invoicing_env_is_suppressed());
    }

    #[test]
    fn url_settings_are_trimmed_and_whitespace_alone_is_unset() {
        let file_settings = Settings {
            public_base_url: Some("  https://billing.example.com/i \t\n".into()),
            documents_base_url: Some("\thttps://docs.example.com/d  ".into()),
            document_response_url: Some("   ".into()),
            ..Settings::default()
        };
        let cfg = documents_config_with(&file_settings, |_| None);
        assert_eq!(
            cfg.invoicing.public_base_url.as_deref(),
            Some("https://billing.example.com/i")
        );
        assert_eq!(
            cfg.documents_base_url.as_deref(),
            Some("https://docs.example.com/d")
        );
        assert_eq!(cfg.document_response_url, None);

        let from_env = invoicing_config_with(&Settings::default(), |name| {
            (name == "NIGEL_PUBLIC_BASE_URL").then(|| " https://env.example.com/i ".into())
        });
        assert_eq!(
            from_env.public_base_url.as_deref(),
            Some("https://env.example.com/i")
        );

        let file_settings = Settings {
            public_base_url: Some("https://billing.example.com/i".into()),
            documents_base_url: Some("https://docs.example.com/d".into()),
            document_response_url: Some("https://docs.example.com/d/respond".into()),
            ..Settings::default()
        };
        let blank_env = documents_config_with(&file_settings, |_| Some("  \t".into()));
        assert_eq!(
            blank_env.invoicing.public_base_url.as_deref(),
            Some("https://billing.example.com/i"),
            "a blank variable is unset, not a value hiding the file's"
        );
        assert_eq!(
            blank_env.documents_base_url.as_deref(),
            Some("https://docs.example.com/d")
        );
        assert_eq!(
            blank_env.document_response_url.as_deref(),
            Some("https://docs.example.com/d/respond")
        );
    }

    /// Trimming stops at whitespace: a trailing run of NULs reaches the
    /// validator intact, which refuses it by name.
    #[test]
    fn trailing_nuls_survive_the_trim_for_the_validator_to_refuse() {
        let file_settings = Settings {
            public_base_url: Some("billing.example.com\0\0\0\0\0\0\0\0\0\0".into()),
            ..Settings::default()
        };
        let cfg = invoicing_config_with(&file_settings, |_| None);
        let base = cfg.public_base_url.expect("still set");
        assert!(base.ends_with('\0'));
        let err = crate::invoicing::r2::validate_public_base_url(&format!("https://{base}"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("public_base_url"), "got: {err}");
    }

    /// The file on disk after a save is exactly what the settings serialize to:
    /// a shorter value written over a longer one leaves no trailing bytes, no
    /// temporary file is left beside it, and a backup the operator keeps next
    /// to it is never touched.
    #[test]
    fn a_save_writes_settings_json_byte_for_byte_and_leaves_its_backup_alone() {
        let _config = TempConfigDir::new();
        let dir = config_dir();
        let path = settings_path();

        let long = Settings {
            data_dir: "/books/acme".into(),
            public_base_url: Some("https://billing.example.com/i".into()),
            ..Settings::default()
        };
        save_settings(&long).unwrap();
        let before = std::fs::read(&path).unwrap();

        let backup = dir.join("settings.json.bak");
        std::fs::write(&backup, &before).unwrap();

        let short = Settings {
            public_base_url: Some("https://a.test/i".into()),
            ..long.clone()
        };
        save_settings(&short).unwrap();

        let after = std::fs::read(&path).unwrap();
        let expected = format!("{}\n", serde_json::to_string_pretty(&short).unwrap());
        assert_eq!(after, expected.as_bytes());
        assert!(after.len() < before.len());
        assert!(!after.contains(&0), "no NUL byte in the file");
        assert_eq!(
            load_settings().public_base_url.as_deref(),
            Some("https://a.test/i")
        );

        assert_eq!(std::fs::read(&backup).unwrap(), before);

        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["settings.json", "settings.json.bak"]);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    /// Nigel never pads a value, but it does not launder one either: a value
    /// already carrying NULs is written back exactly as it was read, so the
    /// validator — not the writer — is where it is caught.
    #[test]
    fn a_value_round_trips_through_load_and_save_unchanged() {
        let _config = TempConfigDir::new();
        let stored = Settings {
            public_base_url: Some("billing.example.com\0\0\0".into()),
            documents_base_url: Some("https://docs.example.com/d".into()),
            ..Settings::default()
        };
        save_settings(&stored).unwrap();
        let first = std::fs::read(settings_path()).unwrap();

        save_settings(&load_settings()).unwrap();
        assert_eq!(std::fs::read(settings_path()).unwrap(), first);
        assert_eq!(load_settings().public_base_url, stored.public_base_url);
    }

    /// A settings.json that is a symlink stays one: the save replaces the
    /// link's target, and the temporary file is written beside the target.
    #[cfg(unix)]
    #[test]
    fn a_save_writes_through_a_symlinked_settings_file() {
        let _config = TempConfigDir::new();
        let elsewhere = tempfile::tempdir().unwrap();
        let target = elsewhere.path().join("nigel-settings.json");
        std::fs::write(&target, "{\"data_dir\": \"/books/old\"}\n").unwrap();
        std::fs::create_dir_all(config_dir()).unwrap();
        std::os::unix::fs::symlink(&target, settings_path()).unwrap();

        let saved = Settings {
            data_dir: "/books/acme".into(),
            ..Settings::default()
        };
        save_settings(&saved).unwrap();

        let link = std::fs::symlink_metadata(settings_path()).unwrap();
        assert!(link.file_type().is_symlink(), "the link was replaced");
        let expected = format!("{}\n", serde_json::to_string_pretty(&saved).unwrap());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), expected);
        assert_eq!(load_settings().data_dir, "/books/acme");

        let beside_target: Vec<_> = std::fs::read_dir(elsewhere.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(beside_target, ["nigel-settings.json"]);
    }

    /// A temporary file a killed save left behind can hold API keys. The next
    /// save removes it once it is old enough that no save can still own it, and
    /// leaves a fresh one alone.
    #[test]
    fn a_save_sweeps_stale_temporary_files_and_spares_fresh_ones() {
        let _config = TempConfigDir::new();
        save_settings(&Settings::default()).unwrap();
        let dir = config_dir();

        let stale = dir.join(".settings.json.99999.0.tmp");
        let fresh = dir.join(".settings.json.99999.1.tmp");
        let unrelated = dir.join(".other.json.99999.0.tmp");
        for path in [&stale, &fresh, &unrelated] {
            std::fs::write(path, "{\"stripe_secret_key\": \"sk_test_left_behind\"}").unwrap();
        }
        let old = std::time::SystemTime::now() - STALE_TEMP_AGE * 2;
        for path in [&stale, &unrelated] {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(old)
                .unwrap();
        }

        save_settings(&Settings::default()).unwrap();

        assert!(!stale.exists(), "the stale temporary file was swept");
        assert!(fresh.exists(), "a fresh one may belong to a save in flight");
        assert!(
            unrelated.exists(),
            "only settings.json's own files are swept"
        );
    }

    #[test]
    fn invoicing_config_prefers_env_over_settings() {
        let file_settings = Settings {
            data_dir: "/x".into(),
            user_name: String::new(),
            update_check: true,
            last_update_check: None,
            stripe_secret_key: Some("rk_file".into()),
            public_base_url: Some("https://file.example/i".into()),
            ..Settings::default()
        };

        let cfg = invoicing_config_with(&file_settings, |name| match name {
            "NIGEL_STRIPE_SECRET_KEY" => Some("rk_env".into()),
            "NIGEL_PUBLIC_BASE_URL" => Some("https://env.example/i".into()),
            _ => None,
        });
        assert_eq!(cfg.stripe_secret_key.as_deref(), Some("rk_env"));
        assert_eq!(
            cfg.public_base_url.as_deref(),
            Some("https://env.example/i")
        );

        let cfg2 = invoicing_config_with(&file_settings, |_| None);
        assert_eq!(cfg2.stripe_secret_key.as_deref(), Some("rk_file"));
        assert_eq!(
            cfg2.public_base_url.as_deref(),
            Some("https://file.example/i")
        );

        let cfg3 = invoicing_config_with(&Settings::default(), |_| None);
        assert_eq!(cfg3.public_base_url, None);
        assert_eq!(cfg3.mailgun_domain, None);
        assert_eq!(cfg3.from_email, None);
    }

    #[test]
    fn the_documents_base_is_the_override_or_the_invoice_base_with_d() {
        assert_eq!(
            derive_documents_base(None, Some("https://billing.example.com/i")),
            Some("https://billing.example.com/d".into())
        );
        assert_eq!(
            derive_documents_base(None, Some("https://billing.example.com/i/")),
            Some("https://billing.example.com/d".into())
        );
        assert_eq!(
            derive_documents_base(
                Some("https://docs.example.com/d"),
                Some("https://billing.example.com/i")
            ),
            Some("https://docs.example.com/d".into())
        );
        assert_eq!(
            derive_documents_base(None, Some("https://billing.example.com")),
            None
        );
        assert_eq!(derive_documents_base(None, None), None);
    }

    #[test]
    fn documents_need_no_stripe_key_and_name_what_is_missing() {
        let cfg = DocumentsConfig {
            invoicing: InvoicingConfig {
                stripe_secret_key: None,
                ..fully_configured()
            },
            r2_private_bucket: None,
            ..Default::default()
        };
        let status = documents_status(&cfg);
        assert!(!status.send_configured);
        assert_eq!(status.missing, vec!["r2_private_bucket"]);
        assert!(!status.response_form);
    }

    #[test]
    fn a_fully_configured_documents_setup_sends_syncs_and_serves_the_form() {
        let cfg = DocumentsConfig {
            invoicing: fully_configured(),
            r2_private_bucket: Some("private".into()),
            document_response_url: Some("https://docs.example.com/d/respond".into()),
            ..Default::default()
        };
        let status = documents_status(&cfg);
        assert!(status.send_configured && status.sync_configured && status.response_form);
        assert!(status.missing.is_empty());
    }

    #[test]
    fn the_document_keys_resolve_from_the_environment_first() {
        let file = Settings {
            r2_private_bucket: Some("file-private".into()),
            ..Settings::default()
        };
        let cfg = documents_config_with(&file, |name| match name {
            "NIGEL_R2_PRIVATE_BUCKET" => Some("env-private".into()),
            "NIGEL_DOCUMENT_RESPONSE_URL" => Some("https://docs.example.com/d/respond".into()),
            _ => None,
        });
        assert_eq!(cfg.r2_private_bucket.as_deref(), Some("env-private"));
        assert_eq!(
            cfg.document_response_url.as_deref(),
            Some("https://docs.example.com/d/respond")
        );
        let from_file = documents_config_with(&file, |_| None);
        assert_eq!(from_file.r2_private_bucket.as_deref(), Some("file-private"));
    }

    #[test]
    fn test_legacy_settings_get_update_check_default() {
        // Simulates loading settings.json that was created before update_check existed
        let json = r#"{"data_dir": "/tmp/test", "user_name": "Bob"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.update_check); // defaults to true
        assert!(s.last_update_check.is_none());
    }
}
