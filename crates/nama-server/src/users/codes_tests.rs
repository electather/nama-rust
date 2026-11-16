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
