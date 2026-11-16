use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use super::{
    super::{
        Change, Decided, ImageRef, Item, ItemKind,
        fixture::tmdb_movie,
        testing::{ALIEN, Fixture, movie},
    },
    library_with_movie, matched_episodes, show_poster,
};
use crate::library::versions::METADATA_VERSION;

#[test]
fn an_unrefreshed_shows_poster_is_its_first_seasons_after_the_specials_that_has_one() {
    let specials_and_first = [(0, Some("/specials.jpg")), (1, Some("/1.jpg"))];
    let first_without = [(2, Some("/2.jpg")), (1, None)];
    let specials_alone = [(0, Some("/specials.jpg"))];
    let none: [(i64, Option<&str>); 2] = [(0, None), (1, None)];

    assert_eq!(show_poster(None, 0, specials_and_first), Some("/1.jpg"));
    assert_eq!(show_poster(None, 0, first_without), Some("/2.jpg"));
    assert_eq!(show_poster(None, 0, specials_alone), Some("/specials.jpg"));
    assert_eq!(show_poster(None, 0, none), None);
}

#[test]
fn a_refreshed_show_without_its_own_poster_has_none_not_a_seasons() {
    let seasons = [(1, Some("/1.jpg"))];

    assert_eq!(show_poster(None, METADATA_VERSION, seasons), None);
    assert_eq!(
        show_poster(Some("/own.jpg"), METADATA_VERSION, seasons),
        Some("/own.jpg")
    );
}

fn imdb(value: &str) -> ExternalId {
    ExternalId::new("imdb", value).unwrap()
}

/// Matches the one Item stored at `path` to a Movie with `ids`.
async fn match_movie(fx: &Fixture, path: &str, ids: ExternalIds) {
    let item = fx.item(path).await;
    let metadata = super::super::Metadata {
        ids,
        ..tmdb_movie(0, "Alien")
    };
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

async fn library_of(fx: &Fixture, id: &ExternalId) -> Option<i64> {
    fx.contents.library_with_movie(id).await.unwrap()
}

async fn id_rows(fx: &Fixture) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM item_match_ids")
        .fetch_one(fx.contents.db.reader())
        .await
        .unwrap()
}

#[tokio::test]
async fn a_movie_is_found_by_any_of_its_ids() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(
        &fx,
        ALIEN,
        ExternalIds::from_iter([ExternalId::tmdb(348), imdb("tt0078748")]),
    )
    .await;

    assert_eq!(
        library_of(&fx, &ExternalId::tmdb(348)).await,
        Some(fx.library)
    );
    assert_eq!(library_of(&fx, &imdb("tt0078748")).await, Some(fx.library));
    assert_eq!(library_of(&fx, &ExternalId::tmdb(679)).await, None);
    assert_eq!(library_of(&fx, &imdb("tt0090605")).await, None);
    // A TVDB id with the same value is another title's.
    assert_eq!(library_of(&fx, &ExternalId::tvdb(348)).await, None);
}

#[tokio::test]
async fn a_seasons_episodes_are_found_by_any_of_their_shows_ids() {
    let fx = Fixture::new().await;
    let path = "/media/Firefly/Season 01/Firefly - S01E02.mkv";
    let episode = Item::Episode {
        show: "Firefly".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: 2,
    };
    fx.store(path, 10, episode).await;
    let item = fx.item(path).await;
    let metadata = super::super::Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(1001)]),
        show_ids: ExternalIds::from_iter([
            ExternalId::tmdb(1437),
            imdb("tt0303461"),
            ExternalId::tvdb(78874),
        ]),
        ..tmdb_movie(0, "The Train Job")
    };
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();

    let mut conn = fx.contents.db.reader().acquire().await.unwrap();
    for show in [
        ExternalId::tmdb(1437),
        imdb("tt0303461"),
        ExternalId::tvdb(78874),
    ] {
        let found = matched_episodes(&mut conn, &show, 1).await.unwrap();
        assert_eq!(found, [2].into(), "{show}");
    }
    let other_season = matched_episodes(&mut conn, &ExternalId::tmdb(1437), 2).await;
    assert!(other_season.unwrap().is_empty());
    // The Episode's own id isn't its Show's, nor is it a Movie.
    let own = matched_episodes(&mut conn, &ExternalId::tmdb(1001), 1).await;
    assert!(own.unwrap().is_empty());
    let as_movie = library_with_movie(&mut conn, &ExternalId::tmdb(1001)).await;
    assert_eq!(as_movie.unwrap(), None);
}

#[tokio::test]
async fn matching_an_item_again_replaces_its_ids() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(
        &fx,
        ALIEN,
        ExternalIds::from_iter([ExternalId::tmdb(348), imdb("tt0078748")]),
    )
    .await;

    match_movie(&fx, ALIEN, ExternalIds::from_iter([ExternalId::tmdb(679)])).await;

    assert_eq!(
        library_of(&fx, &ExternalId::tmdb(679)).await,
        Some(fx.library)
    );
    assert_eq!(library_of(&fx, &ExternalId::tmdb(348)).await, None);
    assert_eq!(library_of(&fx, &imdb("tt0078748")).await, None);
    assert_eq!(id_rows(&fx).await, 1);

    let item = fx.item(ALIEN).await;
    fx.contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();
    assert_eq!(id_rows(&fx).await, 0);
}

#[tokio::test]
async fn removing_a_file_removes_its_items_ids() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(
        &fx,
        ALIEN,
        ExternalIds::from_iter([ExternalId::tmdb(348), imdb("tt0078748")]),
    )
    .await;
    assert_eq!(id_rows(&fx).await, 2);

    let file = fx.contents.stored(fx.library).await.unwrap()[ALIEN].id;
    fx.contents
        .apply(fx.library, vec![Change::Remove(file)])
        .await
        .unwrap();

    assert_eq!(id_rows(&fx).await, 0);
}

fn made_up(namespace: &str, value: &str) -> ExternalId {
    ExternalId::new(namespace, value).unwrap()
}

#[tokio::test]
async fn a_movie_matched_only_to_a_made_up_id_is_listed_and_found_by_it() {
    let fx = Fixture::new().await;
    let anidb = made_up("anidb", "m-1");
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(&fx, ALIEN, ExternalIds::from_iter([anidb.clone()])).await;

    let movies = fx.contents.movies(fx.library).await.unwrap();
    assert_eq!(movies.len(), 1);
    assert_eq!(movies[0].id, anidb);
    assert_eq!(movies[0].ids.get("anidb"), Some(anidb.clone()));
    assert_eq!(library_of(&fx, &anidb).await, Some(fx.library));
}

#[tokio::test]
async fn two_files_matched_to_one_made_up_id_are_one_movie_with_two_versions() {
    let fx = Fixture::new().await;
    let anidb = made_up("anidb", "m-1");
    let other = "/media/Alien (1979)/Alien (1979) - Director's Cut.mkv";
    for path in [ALIEN, other] {
        fx.store(path, 10, movie("Alien", 1979)).await;
        match_movie(&fx, path, ExternalIds::from_iter([anidb.clone()])).await;
    }

    let movies = fx.contents.movies(fx.library).await.unwrap();
    assert_eq!(movies.len(), 1);
    assert_eq!(movies[0].id, anidb);
    assert_eq!(movies[0].versions.len(), 2);
}

#[tokio::test]
async fn a_movie_with_a_made_up_and_a_tmdb_id_is_identified_by_tmdb_and_found_by_either() {
    let fx = Fixture::new().await;
    let mal = made_up("mal", "42");
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(
        &fx,
        ALIEN,
        ExternalIds::from_iter([mal.clone(), ExternalId::tmdb(348)]),
    )
    .await;

    let movies = fx.contents.movies(fx.library).await.unwrap();
    assert_eq!(movies.len(), 1);
    assert_eq!(movies[0].id, ExternalId::tmdb(348));
    assert_eq!(library_of(&fx, &mal).await, Some(fx.library));
    assert_eq!(
        library_of(&fx, &ExternalId::tmdb(348)).await,
        Some(fx.library)
    );
}

#[tokio::test]
async fn an_episode_of_a_show_with_only_a_made_up_id_is_grouped_and_found_under_it() {
    let fx = Fixture::new().await;
    let mal = made_up("mal", "1");
    let path = "/media/Cowboy Bebop/Season 01/Cowboy Bebop - S01E01.mkv";
    let episode = Item::Episode {
        show: "Cowboy Bebop".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: 1,
    };
    fx.store(path, 10, episode).await;
    let item = fx.item(path).await;
    let metadata = super::super::Metadata {
        ids: ExternalIds::from_iter([made_up("mal", "e-1")]),
        show_ids: ExternalIds::from_iter([mal.clone()]),
        show_title: Some("Cowboy Bebop".to_owned()),
        ..tmdb_movie(0, "Asteroid Blues")
    };
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();

    let shows = fx.contents.shows(fx.library).await.unwrap();
    assert_eq!(shows.len(), 1);
    assert_eq!(shows[0].id, mal);
    let numbers: Vec<_> = shows[0].episodes().map(|found| found.number).collect();
    assert_eq!(numbers, [1]);
    let mut conn = fx.contents.db.reader().acquire().await.unwrap();
    let found = matched_episodes(&mut conn, &mal, 1).await.unwrap();
    assert_eq!(found, [1].into());
}

/// Inserts a `title` id for `item_id` in `namespace` straight into SQLite.
async fn insert_id(fx: &Fixture, item_id: i64, namespace: &'static str) -> sqlx::Result<()> {
    fx.contents
        .db
        .write(move |tx| {
            Box::pin(async move {
                sqlx::query(
                    "INSERT INTO item_match_ids (item_id, scope, namespace, value) \
                     VALUES (?, 'title', ?, '1')",
                )
                .bind(item_id)
                .bind(namespace)
                .execute(&mut **tx)
                .await?;
                Ok(())
            })
        })
        .await
}

#[tokio::test]
async fn sqlite_refuses_an_id_whose_namespace_is_not_lowercase_letters_and_digits() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(&fx, ALIEN, ExternalIds::from_iter([ExternalId::tmdb(348)])).await;
    let item_id: i64 = sqlx::query_scalar("SELECT item_id FROM item_match_ids")
        .fetch_one(fx.contents.db.reader())
        .await
        .unwrap();

    for namespace in ["TMDB", "tm-db"] {
        assert!(
            insert_id(&fx, item_id, namespace).await.is_err(),
            "{namespace}"
        );
    }
}

#[tokio::test]
async fn a_library_lists_its_titles_under_the_id_of_its_first_title_source() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(
        &fx,
        ALIEN,
        ExternalIds::from_iter([ExternalId::tmdb(348), ExternalId::tvdb(1)]),
    )
    .await;
    let path = "/media/Firefly/Season 01/Firefly - S01E02.mkv";
    let episode = Item::Episode {
        show: "Firefly".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: 2,
    };
    fx.store(path, 10, episode).await;
    let metadata = super::super::Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(1001), ExternalId::tvdb(297_989)]),
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(1437), ExternalId::tvdb(78874)]),
        ..tmdb_movie(0, "The Train Job")
    };
    let item = fx.item(path).await;
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
    let listed = async || {
        let movies = fx.contents.movies(fx.library).await.unwrap();
        let shows = fx.contents.shows(fx.library).await.unwrap();
        let episode_id = shows[0].seasons[&1].episodes[0].id.clone();
        (movies[0].id.clone(), shows[0].id.clone(), episode_id)
    };

    let listed_by_tmdb = (
        ExternalId::tmdb(348),
        ExternalId::tmdb(1437),
        ExternalId::tmdb(1001),
    );
    assert_eq!(listed().await, listed_by_tmdb);

    let order = vec!["tvdb".to_owned(), "tmdb".to_owned()];
    fx.libraries
        .set_title_sources(fx.library, order)
        .await
        .unwrap();
    let after_reorder = (
        ExternalId::tvdb(1),
        ExternalId::tvdb(78874),
        ExternalId::tvdb(297_989),
    );
    assert_eq!(listed().await, after_reorder);
}

#[tokio::test]
async fn an_image_is_stored_and_served_under_its_own_plugin() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let item = fx.item(ALIEN).await;
    let fanart = PluginId::from_static("fanart");
    let poster = ImageRef {
        plugin: fanart.clone(),
        path: "/poster.jpg".to_owned(),
    };
    let metadata = super::super::Metadata {
        poster: Some(poster.clone()),
        ..tmdb_movie(348, "Alien")
    };
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();

    let movies = fx.contents.movies(fx.library).await.unwrap();
    assert_eq!(movies[0].plugin, "tmdb");
    assert_eq!(movies[0].poster, Some(poster));
    let images = fx.contents.images(Some(fx.library)).await.unwrap();
    assert_eq!(images, [("fanart".to_owned(), "/poster.jpg".to_owned())]);
    assert!(
        fx.contents
            .stores_image("fanart", "/poster.jpg")
            .await
            .unwrap()
    );
    assert!(
        !fx.contents
            .stores_image("tmdb", "/poster.jpg")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn a_title_logo_is_stored_read_back_and_kept() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let item = fx.item(ALIEN).await;
    let logo = ImageRef {
        plugin: PluginId::from_static("fanart"),
        path: "/logo.png".to_owned(),
    };
    let metadata = super::super::Metadata {
        logo: Some(logo.clone()),
        ..tmdb_movie(348, "Alien")
    };
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();

    let movies = fx.contents.movies(fx.library).await.unwrap();
    assert_eq!(movies[0].logo, Some(logo));
    let images = fx.contents.images(Some(fx.library)).await.unwrap();
    assert_eq!(images, [("fanart".to_owned(), "/logo.png".to_owned())]);
    assert!(
        fx.contents
            .stores_image("fanart", "/logo.png")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn a_seasons_poster_keeps_the_plugin_it_was_stored_under() {
    let fx = Fixture::new().await;
    let path = "/media/Firefly/Season 01/Firefly - S01E02.mkv";
    let episode = Item::Episode {
        show: "Firefly".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: 2,
    };
    fx.store(path, 10, episode).await;
    let item = fx.item(path).await;
    let poster = ImageRef {
        plugin: PluginId::from_static("fanart"),
        path: "/season-1.jpg".to_owned(),
    };
    let metadata = super::super::Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(1001)]),
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(1437)]),
        poster: Some(poster.clone()),
        ..tmdb_movie(0, "The Train Job")
    };
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();

    let shows = fx.contents.shows(fx.library).await.unwrap();
    assert_eq!(shows[0].seasons[&1].episodes[0].plugin, "tmdb");
    assert_eq!(shows[0].seasons[&1].poster, Some(poster));
}

#[tokio::test]
async fn a_title_named_by_any_of_its_ids_gets_every_id_in_the_asked_order() {
    let fx = Fixture::new().await;
    let alien_ids = ExternalIds::from_iter([ExternalId::tmdb(348), imdb("tt0078748")]);
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    match_movie(&fx, ALIEN, alien_ids.clone()).await;
    let path = "/media/Firefly/Season 01/Firefly - S01E02.mkv";
    let episode = Item::Episode {
        show: "Firefly".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: 2,
    };
    fx.store(path, 10, episode).await;
    let train_job_ids = ExternalIds::from_iter([ExternalId::tmdb(1001), ExternalId::tvdb(297_989)]);
    let metadata = super::super::Metadata {
        ids: train_job_ids.clone(),
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(1437)]),
        ..tmdb_movie(0, "The Train Job")
    };
    let item = fx.item(path).await;
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
    let every_id = async |kind, named: &[ExternalId]| {
        fx.contents.every_id(fx.library, kind, named).await.unwrap()
    };

    let not_here = ExternalId::tmdb(1);
    let movies = every_id(
        ItemKind::Movie,
        &[not_here.clone(), imdb("tt0078748"), ExternalId::tmdb(1001)],
    )
    .await;
    let episodes = every_id(ItemKind::Episode, &[ExternalId::tvdb(297_989)]).await;

    let only = |id: &ExternalId| ExternalIds::from_iter([id.clone()]);
    let an_episode_isnt_a_movie = only(&ExternalId::tmdb(1001));
    assert_eq!(
        movies,
        [only(&not_here), alien_ids, an_episode_isnt_a_movie]
    );
    assert_eq!(episodes, [train_job_ids]);
}

#[tokio::test]
async fn an_id_two_movies_share_gives_the_ids_of_the_first_by_title() {
    let fx = Fixture::new().await;
    let shared = imdb("tt0078748");
    let aliens = "/media/Aliens (1986)/Aliens (1986).mkv";
    for (path, title, tmdb) in [(aliens, "Aliens", 679), (ALIEN, "Alien", 348)] {
        fx.store(path, 10, movie(title, 1979)).await;
        let item = fx.item(path).await;
        let metadata = super::super::Metadata {
            ids: ExternalIds::from_iter([ExternalId::tmdb(tmdb), shared.clone()]),
            ..tmdb_movie(tmdb, title)
        };
        fx.contents
            .record(vec![(item, Decided::Matched(Box::new(metadata)))])
            .await
            .unwrap();
    }

    let found = (fx
        .contents
        .every_id(fx.library, ItemKind::Movie, std::slice::from_ref(&shared)))
    .await
    .unwrap();

    assert_eq!(
        found,
        [ExternalIds::from_iter([ExternalId::tmdb(348), shared])]
    );
}
