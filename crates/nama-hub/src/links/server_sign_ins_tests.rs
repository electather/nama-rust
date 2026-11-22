use super::*;

#[test]
fn only_an_http_or_https_address_is_a_return_address() {
    assert!(is_web_address("https://nama.example/sign-in"));
    assert!(is_web_address("HTTP://192.168.1.2:7980/sign-in"));
    assert!(!is_web_address("javascript://%0aalert(1)"));
    assert!(!is_web_address("/sign-in"));
    assert!(!is_web_address(&format!(
        "https://{}",
        "a".repeat(URL_LENGTH)
    )));
}
