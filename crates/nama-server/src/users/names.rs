//! What a User's or Device's name may hold, and the key a User's is unique under.

use icu_normalizer::ComposingNormalizerBorrowed;
use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{DefaultIgnorableCodePoint, GeneralCategory, GeneralCategoryGroup},
};

use super::{Error, NAME_LENGTH};

/// `name`, given as the request field `field`, trimmed and in NFC, if it then has an allowed
/// length and no invisible character. `what` names it in the error.
pub(super) fn checked_name(
    field: &'static str,
    what: &'static str,
    name: &str,
) -> Result<String, Error> {
    let name = normalised(name);
    if let Some(character) = name.chars().find(|&c| is_invisible(c)) {
        Err(Error::NameCharacter {
            field,
            what,
            character,
        })
    } else if NAME_LENGTH.contains(&name.chars().count()) {
        Ok(name)
    } else {
        Err(Error::NameLength { field, what })
    }
}

/// Whether `c` can't be seen in a name, or looks like a plain space without being one: a name
/// holding one could pass for another User's.
fn is_invisible(c: char) -> bool {
    let category = CodePointMapData::<GeneralCategory>::new().get(c);
    c != ' '
        && (GeneralCategoryGroup::Other.contains(category)
            || GeneralCategoryGroup::Separator.contains(category)
            || CodePointSetData::new::<DefaultIgnorableCodePoint>().contains(c))
}

/// `name` trimmed and in NFC, where it becomes the same name typed with a letter and its accent
/// apart or as one character.
pub(super) fn normalised(name: &str) -> String {
    nfc(name.trim())
}

fn nfc(text: &str) -> String {
    ComposingNormalizerBorrowed::new_nfc()
        .normalize(text)
        .into_owned()
}

/// The `username` a User's `name` is stored and looked up under, so names are unique and found
/// ignoring case and how accents were typed. Sign-in passes raw input, so it normalises itself,
/// again after lowercasing, which can leave a string out of NFC.
// ponytail: a `username` stored before names were normalised isn't re-keyed. It differs only for
// a name typed decomposed, and such a User no longer signs in by name; delete and recreate it.
pub(super) fn name_key(name: &str) -> String {
    nfc(&normalised(name).to_lowercase())
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
