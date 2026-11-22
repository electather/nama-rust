use argon2::password_hash::phc::PasswordHash;

use super::{UNKNOWN_EMAIL_HASH, hash, verify};

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
