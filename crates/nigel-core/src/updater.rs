use crate::error::{NigelError, Result};
use crate::settings::{load_settings, save_settings};

const GITHUB_API_URL: &str = "https://api.github.com/repos/madebyraygun/nigel/releases/latest";

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const TIMESTAMP_FMT: &str = "%Y-%m-%dT%H:%M:%S";

/// Information about an available update.
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

/// Build an HTTP client with the given timeout.
pub fn http_client(timeout_secs: u64) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .user_agent(format!("nigel/{CURRENT_VERSION}"))
        .build()
        .map_err(|e| NigelError::Other(format!("HTTP client error: {e}")))
}

/// Returns the expected release asset name for the current platform.
pub fn asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => Some("nigel-universal-apple-darwin"),
        ("linux", "x86_64") => Some("nigel-x86_64-unknown-linux-gnu"),
        ("windows", "x86_64") => Some("nigel-x86_64-pc-windows-msvc.exe"),
        _ => None,
    }
}

/// Check the GitHub Releases API for a newer version.
/// Returns `Some(UpdateInfo)` if a newer version is available, `None` otherwise.
pub fn check_for_update() -> Result<Option<UpdateInfo>> {
    let client = http_client(5)?;

    let resp: serde_json::Value = client
        .get(GITHUB_API_URL)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| NigelError::Other(format!("Update check failed: {e}")))?
        .json()
        .map_err(|e| NigelError::Other(format!("Invalid response: {e}")))?;

    let tag = resp["tag_name"]
        .as_str()
        .ok_or_else(|| NigelError::Other("Missing tag_name in release".into()))?;

    let remote_version = tag.strip_prefix('v').unwrap_or(tag);

    if !is_newer(remote_version, CURRENT_VERSION) {
        return Ok(None);
    }

    let asset = asset_name()
        .ok_or_else(|| NigelError::Other("Unsupported platform for auto-update".into()))?;

    let download_url = resp["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|a| a["name"].as_str() == Some(asset))
                .and_then(|a| a["browser_download_url"].as_str())
        })
        .ok_or_else(|| NigelError::Other(format!("Release asset '{asset}' not found")))?
        .to_string();

    Ok(Some(UpdateInfo {
        version: remote_version.to_string(),
        download_url,
    }))
}

/// Non-blocking check on launch: respects cooldown and opt-out setting.
/// Returns the available update, or None.
///
/// The data-only half of `check_and_notify`. `nigel serve` needs the version
/// number rather than a sentence — it reports it as a JSON field and lets the
/// web UI write its own words — and must not bypass the cooldown to get it.
pub fn check_with_cooldown() -> Option<UpdateInfo> {
    let mut settings = load_settings();

    if !settings.update_check {
        return None;
    }

    let now = chrono::Local::now().naive_local();

    // Check cooldown (24 hours)
    if let Some(ref last_check) = settings.last_update_check {
        if let Ok(last) = chrono::NaiveDateTime::parse_from_str(last_check, TIMESTAMP_FMT) {
            if now.signed_duration_since(last) < chrono::Duration::hours(24) {
                return None;
            }
        }
    }

    // Update the timestamp regardless of check result.
    // If we can't persist, skip the check to avoid hammering the API on every launch.
    settings.last_update_check = Some(now.format(TIMESTAMP_FMT).to_string());
    if save_settings(&settings).is_err() {
        return None;
    }

    // Attempt the check, silently returning None on any error
    check_for_update().ok()?
}

/// How the terminal announces an available update.
pub fn update_notice(version: &str) -> String {
    format!("A new version of Nigel is available: v{version}. Run `nigel update` to install.")
}

/// Non-blocking check on launch: respects cooldown and opt-out setting.
/// Returns a notification message if an update is available, or None.
pub fn check_and_notify() -> Option<String> {
    check_with_cooldown().map(|info| update_notice(&info.version))
}

/// Compare two semver strings. Returns true if `remote` is newer than `current`.
pub fn is_newer(remote: &str, current: &str) -> bool {
    let remote_ver = semver::Version::parse(remote).ok();
    let current_ver = semver::Version::parse(current).ok();
    match (remote_ver, current_ver) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_name_returns_some() {
        // On any supported CI/dev platform this should return Some
        let name = asset_name();
        assert!(
            name.is_some(),
            "asset_name() returned None on this platform"
        );
        let name = name.unwrap();
        assert!(name.starts_with("nigel-"));
    }

    #[test]
    fn test_update_notice_names_the_version_and_the_command() {
        let notice = update_notice("1.2.3");
        assert!(notice.contains("v1.2.3"), "got {notice}");
        assert!(notice.contains("nigel update"), "got {notice}");
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("1.1.0", "1.0.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn test_is_newer_with_prerelease() {
        // Pre-release versions are lower than their release counterparts
        assert!(!is_newer("1.0.1-beta.1", "1.0.1"));
        assert!(is_newer("1.0.1", "1.0.1-beta.1"));
    }

    #[test]
    fn test_is_newer_invalid_version() {
        assert!(!is_newer("not-a-version", "1.0.0"));
        assert!(!is_newer("1.0.0", "not-a-version"));
    }

    #[test]
    fn test_current_version_is_valid_semver() {
        assert!(
            semver::Version::parse(CURRENT_VERSION).is_ok(),
            "CARGO_PKG_VERSION is not valid semver: {CURRENT_VERSION}"
        );
    }

    #[test]
    fn test_cooldown_within_24h() {
        // Simulate a recent check timestamp
        let now = chrono::Local::now().naive_local();
        let recent = now - chrono::Duration::hours(1);
        let timestamp = recent.format(TIMESTAMP_FMT).to_string();

        // Parse and check the cooldown logic
        let last = chrono::NaiveDateTime::parse_from_str(&timestamp, TIMESTAMP_FMT).unwrap();
        let duration = now.signed_duration_since(last);
        assert!(duration < chrono::Duration::hours(24));
    }

    #[test]
    fn test_cooldown_expired() {
        let now = chrono::Local::now().naive_local();
        let old = now - chrono::Duration::hours(25);
        let timestamp = old.format(TIMESTAMP_FMT).to_string();

        let last = chrono::NaiveDateTime::parse_from_str(&timestamp, TIMESTAMP_FMT).unwrap();
        let duration = now.signed_duration_since(last);
        assert!(duration >= chrono::Duration::hours(24));
    }
}
