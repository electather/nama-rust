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

#[test]
fn a_server_address_is_an_http_or_https_url() {
    assert!(is_address("https://nama.example.com"));
    assert!(is_address("https://nama.example.com/nama"));
    assert!(is_address("http://192.168.1.10:7980"));
    assert!(is_address("http://[fd00::10]:7980"));
    assert!(!is_address("javascript:alert(1)"));
    assert!(!is_address("data:text/html,hi"));
    assert!(!is_address("192.168.1.10:7980"));
    assert!(!is_address("/relative"));
    assert!(!is_address(""));
    assert!(!is_address(&format!(
        "https://{}.example.com",
        "a".repeat(ADDRESS_LENGTH)
    )));
}
