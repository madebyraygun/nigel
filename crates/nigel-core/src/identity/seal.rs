//! Sealing TOTP secrets under the per-machine server key.
//!
//! The key is 32 random bytes in `<config_dir>/server.key`, never in the data
//! directory, so a backup or a copied database carries no usable second
//! factor. A sealed value is `0x01 || 24-byte nonce || ciphertext+tag`,
//! XChaCha20-Poly1305 with the user's `uid` as associated data, so it cannot
//! be moved to another user's row.

use std::fmt;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use zeroize::Zeroizing;

use crate::error::{NigelError, Result};

pub const KEY_FILE: &str = "server.key";
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const SEAL_VERSION: u8 = 1;

/// The server key. `Debug` is redacted and the bytes are wiped on drop.
pub struct ServerKey(Zeroizing<[u8; KEY_LEN]>);

impl fmt::Debug for ServerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ServerKey(<redacted>)")
    }
}

/// Where the key lives: beside `settings.json`.
pub fn key_path() -> PathBuf {
    crate::settings::config_dir().join(KEY_FILE)
}

impl ServerKey {
    #[cfg(any(test, feature = "testutil"))]
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// The key at `path`, or `None` when there is no file. A file of the wrong
    /// length is an error and is left exactly as it is.
    pub fn load(path: &Path) -> Result<Option<Self>> {
        let mut file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut bytes = Zeroizing::new(Vec::with_capacity(KEY_LEN));
        file.read_to_end(&mut bytes)?;
        if bytes.len() != KEY_LEN {
            return Err(NigelError::Other(format!(
                "{} is not a Nigel server key ({} bytes, expected {KEY_LEN}). \
                 It has been left as it is. Move it aside to create a new one; \
                 every user with two-step verification will then need \
                 `nigel user reset-password <email> --clear-totp`.",
                path.display(),
                bytes.len()
            )));
        }
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        key.copy_from_slice(&bytes);
        Ok(Some(Self(key)))
    }

    /// The key at `path`, created first when there is none: mode 0600 in a
    /// 0700 directory. The bytes are written to a private temporary file in
    /// the same directory and hard-linked into place, which never replaces an
    /// existing file, so no reader sees a partial key and two processes racing
    /// to create it agree on one: the loser reads the winner's.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if let Some(key) = Self::load(path)? {
            return Ok(key);
        }
        let dir = match path.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => dir,
            _ => Path::new("."),
        };
        std::fs::create_dir_all(dir)?;
        crate::settings::restrict_dir_permissions(dir)?;

        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        rand::rngs::OsRng.fill_bytes(key.as_mut());
        match install_new_key(dir, path, key.as_ref()) {
            Ok(()) => Ok(Self(key)),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Self::load(path)?
                .ok_or_else(|| NigelError::Other(format!("{} vanished", path.display()))),
            Err(e) => Err(key_install_error(path, e)),
        }
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(Key::from_slice(self.0.as_ref()))
    }

    /// Seal `plaintext` for the user `uid`.
    pub fn seal(&self, uid: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher()
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: uid.as_bytes(),
                },
            )
            .map_err(|_| NigelError::Other("Could not seal the secret.".into()))?;
        let mut sealed = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
        sealed.push(SEAL_VERSION);
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(&ciphertext);
        Ok(sealed)
    }

    /// Open a value sealed for `uid`. The wrong key, the wrong `uid`, a
    /// truncated value and an unknown version are all the same error, which
    /// names none of the bytes.
    pub fn open(&self, uid: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let refused = || {
            NigelError::Other(
                "A two-step secret could not be opened with this machine's server key.".into(),
            )
        };
        if sealed.len() < 1 + NONCE_LEN || sealed[0] != SEAL_VERSION {
            return Err(refused());
        }
        let (nonce, ciphertext) = sealed[1..].split_at(NONCE_LEN);
        self.cipher()
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: uid.as_bytes(),
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| refused())
    }
}

/// Write `key` to a private temporary file in `dir` and hard-link it to
/// `path`. A link never replaces an existing file, so a second process racing
/// to create the key gets `AlreadyExists` and reads the winner's. The
/// temporary file is removed on every path, and after a successful link the
/// directory is synced so the new name survives a crash.
fn install_new_key(dir: &Path, path: &Path, key: &[u8]) -> std::io::Result<()> {
    let mut suffix = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut suffix);
    let temp = dir.join(format!(".{KEY_FILE}.{}.tmp", hex::encode(suffix)));

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let linked = options
        .open(&temp)
        .and_then(|mut file| {
            file.write_all(key)?;
            file.sync_all()
        })
        .and_then(|()| std::fs::hard_link(&temp, path));
    let _ = std::fs::remove_file(&temp);
    linked?;
    sync_dir(dir)
}

#[cfg(unix)]
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}

fn key_install_error(path: &Path, e: std::io::Error) -> NigelError {
    NigelError::Other(format!(
        "Could not create the server key at {}: {e}",
        path.display()
    ))
}

/// A sentence for `nigel serve` to print when the key, or the directory it
/// lives in, is open to anyone but its owner. `None` when both are private or
/// there is no key yet.
pub fn permission_warning(path: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file_mode = std::fs::metadata(path).ok()?.permissions().mode() & 0o777;
        if file_mode & 0o077 != 0 {
            return Some(format!(
                "warning: {} can be read by other users (mode {file_mode:o}). Run `chmod 600 {}`.",
                path.display(),
                path.display()
            ));
        }
        let dir = path.parent()?;
        let dir_mode = std::fs::metadata(dir).ok()?.permissions().mode() & 0o777;
        if dir_mode & 0o077 != 0 {
            return Some(format!(
                "warning: {} is open to other users (mode {dir_mode:o}). Run `chmod 700 {}`.",
                dir.display(),
                dir.display()
            ));
        }
        None
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UID: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn a_sealed_secret_round_trips() {
        let key = ServerKey::from_bytes([7; 32]);
        let sealed = key.seal(UID, b"twenty byte secret!!").unwrap();
        assert_eq!(sealed[0], 1);
        assert_eq!(sealed.len(), 1 + 24 + 20 + 16);
        assert_eq!(
            key.open(UID, &sealed).unwrap().as_slice(),
            b"twenty byte secret!!"
        );
    }

    #[test]
    fn sealing_twice_gives_different_bytes() {
        let key = ServerKey::from_bytes([7; 32]);
        assert_ne!(key.seal(UID, b"x").unwrap(), key.seal(UID, b"x").unwrap());
    }

    #[test]
    fn another_users_uid_cannot_open_it() {
        let key = ServerKey::from_bytes([7; 32]);
        let sealed = key.seal(UID, b"secret").unwrap();
        assert!(key
            .open("ffffffffffffffffffffffffffffffff", &sealed)
            .is_err());
    }

    #[test]
    fn another_key_cannot_open_it() {
        let sealed = ServerKey::from_bytes([7; 32]).seal(UID, b"secret").unwrap();
        assert!(ServerKey::from_bytes([8; 32]).open(UID, &sealed).is_err());
    }

    #[test]
    fn truncated_tampered_and_unknown_versions_are_refused() {
        let key = ServerKey::from_bytes([7; 32]);
        let sealed = key.seal(UID, b"secret").unwrap();
        assert!(key.open(UID, &sealed[..10]).is_err());
        assert!(key.open(UID, &[]).is_err());
        let mut flipped = sealed.clone();
        *flipped.last_mut().unwrap() ^= 1;
        assert!(key.open(UID, &flipped).is_err());
        let mut versioned = sealed;
        versioned[0] = 2;
        assert!(key.open(UID, &versioned).is_err());
    }

    #[test]
    fn debug_never_prints_the_key() {
        let key = ServerKey::from_bytes([0xAB; 32]);
        let printed = format!("{key:?}");
        assert_eq!(printed, "ServerKey(<redacted>)");
    }

    #[test]
    fn the_key_file_is_created_once_owner_only_and_reread() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config").join(KEY_FILE);
        assert!(ServerKey::load(&path).unwrap().is_none());
        let created = ServerKey::load_or_create(&path).unwrap();
        let sealed = created.seal(UID, b"secret").unwrap();
        let reread = ServerKey::load_or_create(&path).unwrap();
        assert_eq!(reread.open(UID, &sealed).unwrap().as_slice(), b"secret");
        assert_eq!(std::fs::read(&path).unwrap().len(), 32);
        let entries: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, [KEY_FILE]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            let dir_mode = std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600);
            assert_eq!(dir_mode, 0o700);
        }
    }

    #[test]
    fn a_damaged_key_file_is_an_error_and_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEY_FILE);
        std::fs::write(&path, b"short").unwrap();
        let err = ServerKey::load_or_create(&path).unwrap_err().to_string();
        assert!(err.contains("5 bytes"), "{err}");
        assert!(err.contains("--clear-totp"), "{err}");
        assert_eq!(std::fs::read(&path).unwrap(), b"short");
    }

    #[test]
    fn the_key_lives_in_the_config_directory_not_the_data_directory() {
        let _config = crate::settings::TempConfigDir::new();
        assert_eq!(key_path(), crate::settings::config_dir().join("server.key"));
        assert!(!key_path().starts_with(crate::settings::get_data_dir()));
    }

    fn entries(dir: &Path) -> Vec<std::ffi::OsString> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn losing_the_race_keeps_the_winners_key_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEY_FILE);
        std::fs::write(&path, [9u8; 32]).unwrap();

        let err = install_new_key(dir.path(), &path, &[1u8; 32]).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&path).unwrap(), [9u8; 32]);
        assert_eq!(entries(dir.path()), [KEY_FILE]);
        let key = ServerKey::load_or_create(&path).unwrap();
        let sealed = ServerKey::from_bytes([9; 32]).seal(UID, b"x").unwrap();
        assert_eq!(key.open(UID, &sealed).unwrap().as_slice(), b"x");
    }

    #[test]
    fn a_failed_link_leaves_no_temp_file_and_the_error_names_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join(KEY_FILE);

        let err = install_new_key(dir.path(), &path, &[1u8; 32]).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(entries(dir.path()).is_empty(), "{:?}", entries(dir.path()));
        let message = key_install_error(&path, err).to_string();
        assert!(message.contains(&path.display().to_string()), "{message}");
    }

    #[cfg(unix)]
    #[test]
    fn loose_permissions_on_the_key_or_its_directory_are_reported() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let path = config.join(KEY_FILE);
        assert_eq!(permission_warning(&path), None, "no key yet");

        ServerKey::load_or_create(&path).unwrap();
        assert_eq!(permission_warning(&path), None);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let warning = permission_warning(&path).unwrap();
        assert!(warning.contains("chmod 600"), "{warning}");
        assert!(warning.contains(&path.display().to_string()), "{warning}");

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o755)).unwrap();
        let warning = permission_warning(&path).unwrap();
        assert!(warning.contains("chmod 700"), "{warning}");
    }
}
