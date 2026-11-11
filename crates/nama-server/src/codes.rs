//! The Setup Code, and the random codes and secrets the Server hands out: Sign-in Codes and their
//! handles, Plugin Connection codes and session tokens.

use std::fmt;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

/// Crockford's base32 alphabet, which leaves out I, L, O and U so a code read off a log or a screen
/// can't be mistyped as another.
const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// How many characters a Setup Code has: 60 random bits.
const SETUP_CODE_LENGTH: usize = 12;

/// How many random bytes a secret carries, such as a session token.
const SECRET_BYTES: usize = 32;

/// The one-time code a fresh Server prints to its log, which whoever creates the first Admin must
/// enter. It is shown as `XXXX-XXXX-XXXX` and matched ignoring case, dashes and spaces.
#[derive(Debug, Clone)]
pub struct SetupCode(String);

impl SetupCode {
    /// A new random Setup Code.
    ///
    /// # Errors
    ///
    /// If the operating system's random number generator fails.
    pub fn generate() -> Result<Self, getrandom::Error> {
        random_code(SETUP_CODE_LENGTH).map(Self)
    }

    /// Whether `entered` is this code, ignoring case, dashes and spaces.
    pub(crate) fn matches(&self, entered: &str) -> bool {
        // Comparing hashes, so how long a wrong guess takes says nothing about the code.
        Sha256::digest(normalized_code(entered)) == Sha256::digest(&self.0)
    }
}

impl fmt::Display for SetupCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&code_in_groups(&self.0))
    }
}

/// `length` random characters of [`CODE_ALPHABET`], for a code a person reads and types.
pub(crate) fn random_code(length: usize) -> Result<String, getrandom::Error> {
    let mut bytes = vec![0; length];
    getrandom::fill(&mut bytes)?;
    // 256 is a multiple of 32, so every character is equally likely.
    Ok(bytes
        .iter()
        .filter_map(|byte| CODE_ALPHABET.get(usize::from(byte % 32)))
        .map(|&c| char::from(c))
        .collect())
}

/// A code as a person typed it, in the form [`random_code`] makes: without dashes or spaces, and
/// in upper case.
pub(crate) fn normalized_code(entered: &str) -> String {
    entered
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// A code [`random_code`] made, as it is shown: in groups of four joined by dashes.
pub(crate) fn code_in_groups(code: &str) -> String {
    let mut shown = String::with_capacity(code.len() + code.len() / 4);
    for (i, c) in code.chars().enumerate() {
        if i > 0 && i % 4 == 0 {
            shown.push('-');
        }
        shown.push(c);
    }
    shown
}

/// [`SECRET_BYTES`] random bytes, base64url-encoded: too many to guess.
pub(crate) fn random_secret() -> Result<String, getrandom::Error> {
    let mut bytes = [0; SECRET_BYTES];
    getrandom::fill(&mut bytes)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setup_code_matches_ignoring_case_dashes_and_spaces() {
        let code = SetupCode("ABCD0123WXYZ".to_owned());

        assert!(code.matches(" abcd-0123 wxyz\n"));
        assert!(!code.matches("ABCD-0123-WXY"));
    }

    #[test]
    fn a_setup_code_is_shown_in_groups_of_four() {
        assert_eq!(
            SetupCode("ABCD0123WXYZ".to_owned()).to_string(),
            "ABCD-0123-WXYZ"
        );
    }
}
