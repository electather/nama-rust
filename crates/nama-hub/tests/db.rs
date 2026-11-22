//! Connecting to the Hub's PostgreSQL database.

use nama_hub::db;

#[tokio::test]
async fn a_bad_database_url_says_it_could_not_connect_to_postgres() {
    let err = db::connect("not a url").await.unwrap_err();

    assert!(
        err.to_string()
            .starts_with("could not connect to Postgres: "),
        "{err}"
    );
}
