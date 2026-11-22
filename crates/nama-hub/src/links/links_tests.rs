use super::*;

#[test]
fn a_code_is_eight_letters_from_the_alphabet() {
    let code = random_code().unwrap();

    assert_eq!(code.len(), CODE_LENGTH);
    assert!(
        code.bytes().all(|letter| CODE_LETTERS.contains(&letter)),
        "{code}"
    );
}

#[test]
fn a_code_typed_any_way_is_stored_as_shown_without_its_hyphen() {
    assert_eq!(typed_code("bdfh-jklm "), "BDFHJKLM");
    assert_eq!(shown_code("BDFHJKLM"), "BDFH-JKLM");
}
