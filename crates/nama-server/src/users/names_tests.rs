use super::*;

#[test]
fn a_name_is_kept_composed() {
    let name = checked_name("name", "a User's name", " Re\u{301}my ");

    assert_eq!(name.unwrap(), "R\u{E9}my");
}

#[test]
fn a_name_is_measured_once_composed() {
    let name = checked_name("name", "a User's name", &"e\u{301}".repeat(64));

    assert_eq!(name.unwrap(), "\u{E9}".repeat(64));
}

#[test]
fn a_name_holding_an_invisible_character_is_refused_naming_it() {
    for character in [
        '\t', '\n', '\u{200B}', '\u{200D}', '\u{202E}', '\u{A0}', '\u{3164}', '\u{FE0F}',
        '\u{E000}',
    ] {
        let refused = checked_name("name", "a User's name", &format!("Ada{character}x"));

        assert!(
            matches!(
                refused,
                Err(Error::NameCharacter { field: "name", character: c, .. }) if c == character
            ),
            "{character:?}: {refused:?}"
        );
    }
}

#[test]
fn names_in_any_script_with_inner_spaces_are_accepted() {
    for name in [
        "Zoe\u{308}",
        "\u{141}ukasz",
        "Jos\u{E9}",
        "\u{674E}\u{5C0F}\u{9F99}",
        "Ada Lovelace",
    ] {
        let checked = checked_name("name", "a User's name", name);

        assert!(checked.is_ok(), "{name}: {checked:?}");
    }
}
