use super::{EMAIL_LENGTH, is_email};

#[test]
fn an_email_is_something_at_something_without_spaces() {
    let longest = format!("{}@example.com", "a".repeat(EMAIL_LENGTH - 12));
    for email in [
        "ada@example.com",
        "a@b",
        "\"a@b\"@example.com",
        longest.as_str(),
    ] {
        assert!(is_email(email), "{email}");
    }
    let too_long = format!("a{longest}");
    for email in [
        "",
        "ada",
        "@example.com",
        "ada@",
        "ada @example.com",
        too_long.as_str(),
    ] {
        assert!(!is_email(email), "{email}");
    }
}
