use std::collections::BTreeMap;

use super::Recipient;

#[test]
fn a_user_recipients_debug_text_holds_its_keys_and_none_of_its_values() {
    let recipient = Recipient::User(BTreeMap::from([
        ("topic".to_owned(), "alerts-7f3k".to_owned()),
        ("token".to_owned(), "tk_s3cret".to_owned()),
    ]));

    let text = format!("{recipient:?}");

    assert!(text.contains("topic") && text.contains("token"), "{text}");
    assert!(
        !text.contains("alerts-7f3k") && !text.contains("tk_s3cret"),
        "{text}"
    );
}
