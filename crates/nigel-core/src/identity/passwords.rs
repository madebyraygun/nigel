//! User passwords: the rules a new one must meet, Argon2id hashing, and the
//! temporary passwords an admin hands out.
//!
//! This is sign-in identity. The books' encryption password is `crate::password`
//! and has nothing to do with it.

use std::fmt;
use std::sync::OnceLock;

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::Rng;
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use crate::error::{NigelError, Result};

pub const MIN_LENGTH: usize = 12;
pub const MAX_LENGTH: usize = 256;

/// Argon2id memory cost in KiB (19 MiB), passes and lanes. Pinned rather than
/// taken from the crate's defaults, so a dependency bump cannot change them.
const M_COST_KIB: u32 = 19 * 1024;
const T_COST: u32 = 2;
const P_COST: u32 = 1;
const OUTPUT_LEN: usize = 32;

/// Crockford's base32 alphabet: no I, L, O or U, so nothing on paper reads as
/// two different characters.
const TEMPORARY_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Why a new password was refused. `reason_code` is the API's `details.reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordRule {
    TooShort,
    TooLong,
    MatchesIdentity,
    Unchanged,
}

impl PasswordRule {
    pub fn reason_code(self) -> &'static str {
        match self {
            PasswordRule::TooShort => "too_short",
            PasswordRule::TooLong => "too_long",
            PasswordRule::MatchesIdentity => "matches_identity",
            PasswordRule::Unchanged => "unchanged",
        }
    }
}

impl fmt::Display for PasswordRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PasswordRule::TooShort => {
                write!(f, "Use at least {MIN_LENGTH} characters.")
            }
            PasswordRule::TooLong => write!(f, "Use at most {MAX_LENGTH} characters."),
            PasswordRule::MatchesIdentity => write!(
                f,
                "Don't use your email address or your name as your password."
            ),
            PasswordRule::Unchanged => {
                write!(f, "Choose a password different from your current one.")
            }
        }
    }
}

impl std::error::Error for PasswordRule {}

impl From<PasswordRule> for NigelError {
    fn from(rule: PasswordRule) -> Self {
        NigelError::Invalid(rule.to_string())
    }
}

/// Who the password belongs to, for the identity rule, and the hash it would
/// replace, for the unchanged rule.
pub struct PasswordOwner<'a> {
    pub email: &'a str,
    pub display_name: &'a str,
    pub current_hash: Option<&'a str>,
}

/// NFC, so the same password typed on two keyboards hashes the same.
pub fn normalize(password: &str) -> Zeroizing<String> {
    Zeroizing::new(password.nfc().collect())
}

/// The one check every new password passes, from the CLI and the API alike.
/// Checked in this order: length, identity, unchanged.
pub fn check_new_password(
    password: &str,
    owner: &PasswordOwner<'_>,
) -> std::result::Result<(), PasswordRule> {
    let normalized = normalize(password);
    let length = normalized.chars().count();
    if length < MIN_LENGTH {
        return Err(PasswordRule::TooShort);
    }
    if length > MAX_LENGTH {
        return Err(PasswordRule::TooLong);
    }
    let folded = Zeroizing::new(normalized.to_lowercase());
    let email = owner.email.trim().to_lowercase();
    let local_part = email.split('@').next().unwrap_or("").to_string();
    let name = owner.display_name.trim().to_lowercase();
    if [email, local_part, name]
        .iter()
        .any(|identity| !identity.is_empty() && *identity == *folded)
    {
        return Err(PasswordRule::MatchesIdentity);
    }
    if let Some(current) = owner.current_hash {
        if verify_password(password, current) {
            return Err(PasswordRule::Unchanged);
        }
    }
    Ok(())
}

fn hasher() -> Argon2<'static> {
    // unwrap: the constants above are within Argon2's documented bounds
    let params = Params::new(M_COST_KIB, T_COST, P_COST, Some(OUTPUT_LEN)).unwrap();
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// An Argon2id PHC string for `password`, NFC-normalized, with a fresh
/// 16-byte salt.
pub fn hash_password(password: &str) -> Result<String> {
    let normalized = normalize(password);
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    hasher()
        .hash_password(normalized.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| NigelError::Other("Could not hash the password.".into()))
}

/// Whether `password` matches the PHC string. A string that does not parse is
/// a mismatch, never a panic.
pub fn verify_password(password: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return false;
    };
    let normalized = normalize(password);
    hasher()
        .verify_password(normalized.as_bytes(), &parsed)
        .is_ok()
}

/// One verification against a fixed hash, for an email that matches no user,
/// so an unknown address costs the same time as a known one.
pub fn verify_dummy(password: &str) {
    static DUMMY: OnceLock<String> = OnceLock::new();
    let phc = DUMMY.get_or_init(|| {
        // unwrap: hashing a fixed string with valid parameters cannot fail
        hash_password("nigel-dummy-password-never-matches").unwrap()
    });
    let _ = verify_password(password, phc);
}

/// Twenty characters from [`TEMPORARY_ALPHABET`] in four groups of five, about
/// 100 bits: `K7QXM-2RTPA-W9HCN-4FJDE`.
pub fn temporary_password() -> Zeroizing<String> {
    let mut rng = rand::rngs::OsRng;
    let mut out = Zeroizing::new(String::with_capacity(23));
    for i in 0..20 {
        if i > 0 && i % 5 == 0 {
            out.push('-');
        }
        out.push(TEMPORARY_ALPHABET[rng.gen_range(0..32)] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> PasswordOwner<'static> {
        PasswordOwner {
            email: "pat@example.com",
            display_name: "Pat Example",
            current_hash: None,
        }
    }

    #[test]
    fn hashes_are_argon2id_with_the_pinned_parameters() {
        let phc = hash_password("correct horse battery").unwrap();
        assert!(phc.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"), "{phc}");
        let salt_b64 = phc.split('$').nth(4).unwrap();
        assert_eq!(salt_b64.len(), 22, "a 16-byte salt is 22 base64 chars");
    }

    #[test]
    fn the_same_password_hashes_differently_each_time() {
        let a = hash_password("correct horse battery").unwrap();
        let b = hash_password("correct horse battery").unwrap();
        assert_ne!(a, b);
        assert!(verify_password("correct horse battery", &a));
        assert!(verify_password("correct horse battery", &b));
    }

    #[test]
    fn verify_refuses_a_wrong_password_and_garbage() {
        let phc = hash_password("correct horse battery").unwrap();
        assert!(!verify_password("correct horse batter", &phc));
        assert!(!verify_password("correct horse battery", "not a hash"));
        assert!(!verify_password("correct horse battery", ""));
    }

    #[test]
    fn nfc_and_nfd_spellings_verify_against_each_other() {
        let nfc = "caf\u{e9} au lait daily";
        let nfd = "cafe\u{301} au lait daily";
        let phc = hash_password(nfc).unwrap();
        assert!(verify_password(nfd, &phc));
    }

    #[test]
    fn the_dummy_path_runs_and_matches_nothing() {
        verify_dummy("anything at all here");
        verify_dummy("");
    }

    #[test]
    fn the_dummy_path_costs_about_as_much_as_a_real_verify() {
        use std::time::Instant;
        let phc = hash_password("correct horse battery").unwrap();
        verify_dummy("warm the lazy hash");
        let real = {
            let start = Instant::now();
            verify_password("wrong password here", &phc);
            start.elapsed()
        };
        let dummy = {
            let start = Instant::now();
            verify_dummy("wrong password here");
            start.elapsed()
        };
        assert!(dummy * 4 > real, "dummy {dummy:?} vs real {real:?}");
        assert!(real * 4 > dummy, "dummy {dummy:?} vs real {real:?}");
    }

    #[test]
    fn length_counts_characters_after_normalization() {
        assert_eq!(
            check_new_password("elevenchars", &owner()),
            Err(PasswordRule::TooShort)
        );
        assert_eq!(check_new_password("twelve chars", &owner()), Ok(()));
        // Eleven characters, twelve code points before NFC.
        let nfd = "cafe\u{301}-au-lai";
        assert_eq!(nfd.chars().count(), 12);
        assert_eq!(
            check_new_password(nfd, &owner()),
            Err(PasswordRule::TooShort)
        );
        assert_eq!(
            check_new_password(&"x".repeat(257), &owner()),
            Err(PasswordRule::TooLong)
        );
        assert_eq!(check_new_password(&"x".repeat(256), &owner()), Ok(()));
    }

    #[test]
    fn the_email_its_local_part_and_the_name_are_refused() {
        let owner = PasswordOwner {
            email: "patexample12@example.com",
            display_name: "Pat Example Jones",
            current_hash: None,
        };
        for candidate in [
            "PatExample12@example.com",
            "patexample12",
            "pat example jones",
        ] {
            assert_eq!(
                check_new_password(candidate, &owner),
                Err(PasswordRule::MatchesIdentity),
                "{candidate}"
            );
        }
    }

    #[test]
    fn the_current_password_is_refused_as_unchanged() {
        let current = hash_password("correct horse battery").unwrap();
        let owner = PasswordOwner {
            current_hash: Some(&current),
            ..owner()
        };
        assert_eq!(
            check_new_password("correct horse battery", &owner),
            Err(PasswordRule::Unchanged)
        );
        assert_eq!(check_new_password("a different one", &owner), Ok(()));
    }

    #[test]
    fn each_rule_has_its_wire_code() {
        let codes: Vec<_> = [
            PasswordRule::TooShort,
            PasswordRule::TooLong,
            PasswordRule::MatchesIdentity,
            PasswordRule::Unchanged,
        ]
        .iter()
        .map(|r| r.reason_code())
        .collect();
        assert_eq!(
            codes,
            ["too_short", "too_long", "matches_identity", "unchanged"]
        );
    }

    #[test]
    fn temporary_passwords_are_four_groups_of_five_unambiguous_characters() {
        let pw = temporary_password();
        assert_eq!(pw.len(), 23, "{}", pw.as_str());
        let groups: Vec<&str> = pw.split('-').collect();
        assert_eq!(groups.len(), 4);
        for group in groups {
            assert_eq!(group.len(), 5);
            assert!(group.bytes().all(|b| TEMPORARY_ALPHABET.contains(&b)));
        }
        assert_ne!(temporary_password().as_str(), pw.as_str());
    }

    #[test]
    fn a_temporary_password_passes_the_rules_it_never_has_to_meet() {
        let pw = temporary_password();
        assert_eq!(check_new_password(&pw, &owner()), Ok(()));
    }
}
