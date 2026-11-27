use super::*;

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
