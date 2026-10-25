//! RFC 6238 time-based one-time passwords: HMAC-SHA1, six digits, a 30-second
//! step, a 160-bit secret, and one step of skew either side.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

pub const SECRET_LEN: usize = 20;
pub const STEP_SECONDS: i64 = 30;
pub const DIGITS: u32 = 6;
/// Steps accepted either side of the current one.
pub const SKEW_STEPS: i64 = 1;

/// A fresh 160-bit secret from the operating system's CSPRNG.
pub fn generate_secret() -> Zeroizing<[u8; SECRET_LEN]> {
    let mut secret = Zeroizing::new([0u8; SECRET_LEN]);
    rand::rngs::OsRng.fill_bytes(secret.as_mut());
    secret
}

/// The RFC 6238 time step `now` falls in.
pub fn step_at(now: DateTime<Utc>) -> i64 {
    now.timestamp().div_euclid(STEP_SECONDS)
}

/// RFC 4226 HOTP: the `digits`-digit code for `counter`.
pub fn hotp(secret: &[u8], counter: u64, digits: u32) -> u32 {
    // unwrap: HMAC accepts a key of any length
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).unwrap();
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    binary % 10u32.pow(digits)
}

/// The six-digit code for a step, zero-padded.
pub fn code_for_step(secret: &[u8], step: i64) -> String {
    format!(
        "{:0width$}",
        hotp(secret, step as u64, DIGITS),
        width = DIGITS as usize
    )
}

/// The code as typed, reduced to its digits: spaces are dropped (authenticator
/// apps show `123 456`), and anything else that is not exactly six ASCII
/// digits is `None`.
fn clean_code(code: &str) -> Option<String> {
    let digits: String = code.chars().filter(|c| *c != ' ').collect();
    (digits.len() == DIGITS as usize && digits.bytes().all(|b| b.is_ascii_digit()))
        .then_some(digits)
}

/// The step a code matches within the skew window around `now`, newest first,
/// or `None`. Every candidate is compared in constant time, and all three are
/// always computed.
pub fn matching_step(secret: &[u8], code: &str, now: DateTime<Utc>) -> Option<i64> {
    let code = clean_code(code)?;
    let current = step_at(now);
    let mut found = None;
    for step in (current - SKEW_STEPS..=current + SKEW_STEPS).rev() {
        let expected = code_for_step(secret, step);
        if bool::from(expected.as_bytes().ct_eq(code.as_bytes())) && found.is_none() {
            found = Some(step);
        }
    }
    found
}

/// The secret as unpadded RFC 4648 base32, for typing in by hand.
pub fn secret_base32(secret: &[u8]) -> Zeroizing<String> {
    Zeroizing::new(data_encoding::BASE32_NOPAD.encode(secret))
}

/// `otpauth://totp/Nigel:<email>?secret=…&issuer=Nigel`, the URI a QR code
/// carries. The email is percent-encoded as a path segment.
pub fn otpauth_uri(email: &str, secret: &[u8]) -> Zeroizing<String> {
    let mut label = String::new();
    for byte in email.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'@' => {
                label.push(byte as char)
            }
            _ => label.push_str(&format!("%{byte:02X}")),
        }
    }
    Zeroizing::new(format!(
        "otpauth://totp/Nigel:{label}?secret={}&issuer=Nigel",
        secret_base32(secret).as_str()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RFC_SECRET: &[u8] = b"12345678901234567890";

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).unwrap()
    }

    /// RFC 6238 appendix B, SHA-1 column, eight digits.
    #[test]
    fn matches_the_rfc_6238_sha1_vectors() {
        let vectors = [
            (59, 94287082),
            (1111111109, 7081804),
            (1111111111, 14050471),
            (1234567890, 89005924),
            (2000000000, 69279037),
            (20000000000, 65353130),
        ];
        for (seconds, expected) in vectors {
            let step = step_at(at(seconds)) as u64;
            assert_eq!(hotp(RFC_SECRET, step, 8), expected, "T={seconds}");
        }
    }

    #[test]
    fn six_digit_codes_are_the_low_digits_zero_padded() {
        assert_eq!(code_for_step(RFC_SECRET, step_at(at(1111111109))), "081804");
        assert_eq!(code_for_step(RFC_SECRET, step_at(at(1234567890))), "005924");
    }

    #[test]
    fn one_step_either_side_is_accepted_and_two_is_not() {
        let now = at(1_800_000_000);
        let current = step_at(now);
        for offset in [-1, 0, 1] {
            let code = code_for_step(RFC_SECRET, current + offset);
            assert_eq!(
                matching_step(RFC_SECRET, &code, now),
                Some(current + offset),
                "offset {offset}"
            );
        }
        for offset in [-2, 2] {
            let code = code_for_step(RFC_SECRET, current + offset);
            assert_eq!(
                matching_step(RFC_SECRET, &code, now),
                None,
                "offset {offset}"
            );
        }
    }

    #[test]
    fn a_code_typed_with_a_space_still_matches_and_junk_never_does() {
        let now = at(1_800_000_000);
        let code = code_for_step(RFC_SECRET, step_at(now));
        let spaced = format!("{} {}", &code[..3], &code[3..]);
        assert!(matching_step(RFC_SECRET, &spaced, now).is_some());
        for junk in [
            "",
            "12345",
            "1234567",
            "abcdef",
            "１２３４５６",
            "12345\n",
            "123456\n",
        ] {
            assert_eq!(matching_step(RFC_SECRET, junk, now), None, "{junk:?}");
        }
    }

    #[test]
    fn a_generated_secret_is_160_bits_and_not_repeated() {
        let a = generate_secret();
        let b = generate_secret();
        assert_eq!(a.len(), 20);
        assert_ne!(*a, *b);
    }

    #[test]
    fn the_uri_carries_the_base32_secret_and_the_issuer() {
        let uri = otpauth_uri("pat+books@example.com", RFC_SECRET);
        assert_eq!(
            uri.as_str(),
            "otpauth://totp/Nigel:pat%2Bbooks@example.com?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Nigel"
        );
    }
}
