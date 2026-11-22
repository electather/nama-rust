use super::*;

const ADDRESSES: [&str; 2] = ["https://nama.example", "http://192.168.1.2:7980"];

#[test]
fn a_return_address_is_on_a_server_address_by_scheme_host_and_port() {
    assert!(is_on_server("https://nama.example/sign-in?a=b", ADDRESSES));
    assert!(is_on_server("https://NAMA.example:443/sign-in", ADDRESSES));
    assert!(is_on_server("HTTP://192.168.1.2:7980/sign-in", ADDRESSES));
    assert!(!is_on_server("https://evil.example/sign-in", ADDRESSES));
    assert!(!is_on_server("http://nama.example/sign-in", ADDRESSES));
    assert!(!is_on_server("http://192.168.1.2/sign-in", ADDRESSES));
    assert!(!is_on_server(
        "https://nama.example.evil.example/",
        ADDRESSES
    ));
    assert!(!is_on_server(
        "https://nama.example@evil.example/",
        ADDRESSES
    ));
    assert!(!is_on_server("blob:https://nama.example/1", ADDRESSES));
    assert!(!is_on_server("javascript://%0aalert(1)", ADDRESSES));
    assert!(!is_on_server("/sign-in", ADDRESSES));
    assert!(!is_on_server("https://nama.example/sign-in", []));
    assert!(!is_on_server(
        &format!("https://nama.example/{}", "a".repeat(URL_LENGTH)),
        ADDRESSES
    ));
}

#[test]
fn the_return_address_carries_the_approval_code() {
    assert_eq!(
        with_code(
            "https://nama.example/sign-in/account?redirect=%2Fmovies",
            "c0de"
        )
        .unwrap(),
        "https://nama.example/sign-in/account?redirect=%2Fmovies&code=c0de"
    );
    assert_eq!(
        with_code("https://nama.example/sign-in#top", "c0de").unwrap(),
        "https://nama.example/sign-in?code=c0de#top"
    );
}

#[test]
fn codes_are_equal_only_byte_for_byte() {
    assert!(constant_time_eq(b"c0de", b"c0de"));
    assert!(!constant_time_eq(b"c0de", b"c0df"));
    assert!(!constant_time_eq(b"c0de", b"c0d"));
}
