use argon2::password_hash::phc::PasswordHash;

use super::{UNKNOWN_EMAIL_HASH, hash, verify};
use crate::accounts::Error;

#[tokio::test]
async fn a_hashed_password_verifies_and_another_doesnt() {
    let hashed = hash("correct horse".to_owned()).await.unwrap();

    assert!(hashed.starts_with("$argon2id$"));
    assert!(
        verify("correct horse".to_owned(), hashed.clone())
            .await
            .unwrap()
    );
    assert!(!verify("wrong horse".to_owned(), hashed).await.unwrap());
}

#[tokio::test]
async fn a_password_too_short_or_too_long_isnt_hashed() {
    for password in ["seven c".to_owned(), "x".repeat(1025)] {
        assert!(matches!(hash(password).await, Err(Error::PasswordLength)));
    }
}

#[tokio::test]
async fn the_unknown_email_hash_costs_what_a_new_one_does() {
    let new = hash("any password".to_owned()).await.unwrap();
    let params = |hash: &str| PasswordHash::new(hash).unwrap().params.to_string();

    assert_eq!(params(UNKNOWN_EMAIL_HASH), params(&new));
    assert!(
        verify(
            "a password no Account has".to_owned(),
            UNKNOWN_EMAIL_HASH.to_owned()
        )
        .await
        .unwrap()
    );
}
