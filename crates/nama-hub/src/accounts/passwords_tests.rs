use std::{
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use argon2::password_hash::phc::PasswordHash;
use futures_util::future::join_all;

use super::{CONCURRENT_HASHES, UNKNOWN_EMAIL_HASH, hash, on_hashing_thread, verify};

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

#[tokio::test]
async fn twenty_hashes_at_once_run_no_more_than_the_bound_together_and_all_finish() {
    static RUNNING: AtomicUsize = AtomicUsize::new(0);
    static MOST_AT_ONCE: AtomicUsize = AtomicUsize::new(0);
    let counted_hash = || {
        on_hashing_thread(|| {
            let running = RUNNING.fetch_add(1, Ordering::SeqCst) + 1;
            MOST_AT_ONCE.fetch_max(running, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(20));
            RUNNING.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        })
    };

    let hashed = join_all((0..20).map(|_| counted_hash())).await;

    assert!(hashed.iter().all(Result::is_ok));
    assert!(MOST_AT_ONCE.load(Ordering::SeqCst) <= CONCURRENT_HASHES);
}
