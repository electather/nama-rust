use super::sort_title;
use crate::library::contents::{
    Decided,
    fixture::tmdb_movie,
    testing::{ALIEN, Fixture, movie},
};

#[test]
fn a_leading_english_article_is_dropped() {
    let titles = [
        "The Matrix",
        "A Quiet Place",
        "An American Werewolf",
        "Them!",
        "A",
    ];
    assert_eq!(
        titles.map(sort_title),
        ["matrix", "quiet place", "american werewolf", "them!", "a"]
    );
}

#[test]
fn case_and_accents_are_folded() {
    assert_eq!(sort_title("éclair"), sort_title("Eclair"));
    assert_eq!(sort_title("ÉCLAIR"), "eclair");
}

#[tokio::test]
async fn a_match_stored_before_sort_titles_gets_one_at_startup() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("The Thing", 1982)).await;
    let item = fx.item(ALIEN).await;
    let thing = Decided::Matched(Box::new(tmdb_movie(1091, "The Thing")));
    fx.contents.record(vec![(item, thing)]).await.unwrap();
    let sort_title = || {
        sqlx::query_scalar::<_, Option<String>>("SELECT sort_title FROM item_matches")
            .fetch_one(fx.contents.db.reader())
    };
    let stored = sort_title().await.unwrap();
    (fx.contents.db)
        .write(|tx| {
            Box::pin(async move {
                sqlx::query("UPDATE item_matches SET sort_title = NULL")
                    .execute(&mut **tx)
                    .await
            })
        })
        .await
        .unwrap();

    fx.contents.fill_sort_titles().await.unwrap();

    assert_eq!(stored.as_deref(), Some("thing"));
    assert_eq!(sort_title().await.unwrap(), stored);
}
