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
