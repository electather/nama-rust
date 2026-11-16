use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use super::{
    super::{
        Decided, Item, ItemKind, Metadata,
        fixture::tmdb_movie,
        testing::{Fixture, movie},
    },
    Listed,
};

/// Stores a Movie file at `path` and Matches it to a Movie with `ids`, called `title`.
async fn matched_movie(fx: &Fixture, path: &str, title: &str, ids: ExternalIds) {
    fx.store(path, 10, movie(title, 2000)).await;
    let metadata = Metadata {
        ids,
        ..tmdb_movie(0, title)
    };
    let item = fx.item(path).await;
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

/// Stores an Episode file at `path`, Season 1 Episode `number` of Dark, and Matches it to the
/// Episode `tmdb-(1000 + number)` called `title`, of a Show with `show_ids`.
async fn matched_episode(
    fx: &Fixture,
    path: &str,
    number: i64,
    title: &str,
    show_ids: ExternalIds,
) {
    let episode = Item::Episode {
        show: "Dark".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: number,
    };
    fx.store(path, 10, episode).await;
    let metadata = Metadata {
        show_ids,
        show_title: Some("Dark".to_owned()),
        ..tmdb_movie(1000 + number, title)
    };
    let item = fx.item(path).await;
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

async fn page_ids(
    fx: &Fixture,
    after: Option<&ExternalId>,
    limit: Option<usize>,
) -> Vec<ExternalId> {
    let page = fx
        .contents
        .titles_page(fx.library, ItemKind::Movie, after, limit);
    (page.await.unwrap().into_iter())
        .map(|listed| listed.id)
        .collect()
}

#[tokio::test]
async fn a_page_lists_each_movie_once_by_id_after_the_one_named() {
    let fx = Fixture::new().await;
    for (tmdb, path) in [
        (10, "/media/10.mkv"),
        (9, "/media/9.mkv"),
        (2, "/media/2.mkv"),
    ] {
        let ids = ExternalIds::from_iter([ExternalId::tmdb(tmdb)]);
        matched_movie(&fx, path, &format!("Movie {tmdb}"), ids).await;
    }
    let cut = ExternalIds::from_iter([ExternalId::tmdb(9)]);
    matched_movie(&fx, "/media/9 - Cut.mkv", "Movie 9", cut).await;

    let all = page_ids(&fx, None, None).await;
    let first = page_ids(&fx, None, Some(2)).await;
    let rest = page_ids(&fx, Some(&ExternalId::tmdb(9)), Some(2)).await;

    let ids =
        |tmdb: &[i64]| -> Vec<ExternalId> { tmdb.iter().map(|&id| ExternalId::tmdb(id)).collect() };
    assert_eq!(all, ids(&[2, 9, 10]));
    assert_eq!(first, ids(&[2, 9]));
    assert_eq!(rest, ids(&[10]));
}

#[tokio::test]
async fn a_page_names_a_movie_by_its_librarys_first_title_source() {
    let fx = Fixture::new().await;
    let ids = ExternalIds::from_iter([ExternalId::tmdb(348), ExternalId::tvdb(1)]);
    matched_movie(&fx, "/media/Alien.mkv", "Alien", ids).await;

    let before = page_ids(&fx, None, None).await;
    let order = vec!["tvdb".to_owned(), "tmdb".to_owned()];
    fx.libraries
        .set_title_sources(fx.library, order)
        .await
        .unwrap();
    let after_reorder = page_ids(&fx, None, None).await;

    assert_eq!(before, [ExternalId::tmdb(348)]);
    assert_eq!(after_reorder, [ExternalId::tvdb(1)]);
}

#[tokio::test]
async fn a_page_of_episodes_gives_their_show_and_numbers_and_leaves_out_one_without_a_show_id() {
    let fx = Fixture::new().await;
    let dark = ExternalIds::from_iter([ExternalId::tmdb(70_523)]);
    matched_episode(&fx, "/media/Dark - S01E01.mkv", 1, "Secrets", dark).await;
    let showless = ExternalIds::default();
    matched_episode(&fx, "/media/Dark - S01E02.mkv", 2, "Lies", showless).await;

    let page = fx
        .contents
        .titles_page(fx.library, ItemKind::Episode, None, None);

    assert_eq!(
        page.await.unwrap(),
        [Listed {
            id: ExternalId::tmdb(1001),
            title: "Secrets".to_owned(),
            show_title: Some("Dark".to_owned()),
            season: Some(1),
            episode: Some(1),
        }]
    );
}

#[tokio::test]
async fn show_titles_names_each_asked_episodes_show_and_none_for_one_not_stored() {
    let fx = Fixture::new().await;
    let dark = ExternalIds::from_iter([ExternalId::tmdb(70_523)]);
    matched_episode(&fx, "/media/Dark - S01E02.mkv", 2, "Lies", dark).await;

    let named = [ExternalId::tmdb(9), ExternalId::tmdb(1002)];
    let titles = fx.contents.show_titles(fx.library, &named).await.unwrap();

    assert_eq!(titles, [None, Some("Dark".to_owned())]);
}
